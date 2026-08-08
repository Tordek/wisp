pub mod assembler;
mod encoding;

use std::fmt::Debug;

use crate::bus::{Address, Bus, Native, Offset};
use int_enum::IntEnum;

type WordSize = u64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Count(u64);

pub enum MemoryLayout {}
impl MemoryLayout {
    // Standard addresses.
    pub const CPU_END: Address = Address(0xffffffffffffffff);
    pub const NIL_ROOT: Address = Address(Self::CPU_END.0 - 0x08 + 1);
    pub const T_ROOT: Address = Address(Self::CPU_END.0 - 0x10 + 1);
    pub const RESET_VECTOR: Address = Address(Self::CPU_END.0 - 0x20 + 1);
}

pub enum InterruptTableOffset {}
impl InterruptTableOffset {
    pub const TRAP_VECTOR: Offset = Offset(0x00);
    // pub const ALLOC_VECTOR: Offset = Offset(0x10);
    // pub const ALLOC_CONS_VECTOR: Offset = Offset(0x018);
    pub const END_RESERVED_INTERRUPTS: Offset = Offset(0xf0);
}

// pub enum SymbolLayout {}
// impl SymbolLayout {
//     pub const NAME_OFFSET: Offset = Offset(0);
//     pub const PLIST_OFFSET: Offset = Offset(8);
// }

// pub enum StringLayout {}
// impl StringLayout {
//     pub const LENGTH_OFFSET: Offset = Offset(0);
//     pub const DATA_OFFSET: Offset = Offset(8);
// }

pub enum ConsLayout {}
impl ConsLayout {
    pub const CAR_OFFSET: Offset = Offset(0);
    pub const CDR_OFFSET: Offset = Offset(8);
}

#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy, IntEnum, Debug)]
pub enum WordType {
    Undefined = 0,
    Fixnum = 1,
    Symbol = 2,
    Cons = 3,
    Function = 4,
    Closure = 5,
    Character = 6,
    String = 7,
    Vector = 8,
    Float = 9,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct LispWord(u64);

impl LispWord {
    pub const fn undefined() -> LispWord {
        LispWord::new(WordType::Undefined as u8, 0)
    }

    pub fn tag(&self) -> u8 {
        (self.0 >> 56) as u8
    }

    pub const fn payload(&self) -> u64 {
        self.0 & 0x00ff_ffff_ffff_ffff
    }

    pub const fn new(tag: u8, payload: WordSize) -> Self {
        Self((tag as u64) << 56 | (payload & 0x00ff_ffff_ffff_ffff))
    }

    // Convenience methods
    pub const fn symbol(value: WordSize) -> Self {
        Self::new(WordType::Symbol as u8, value)
    }

    pub const fn fixnum(value: WordSize) -> Self {
        Self::new(WordType::Fixnum as u8, value)
    }

    pub const fn cons(address: WordSize) -> Self {
        Self::new(WordType::Cons as u8, address)
    }

    pub const fn char(address: WordSize) -> Self {
        Self::new(WordType::Character as u8, address & 0xff)
    }

    pub const fn string(address: WordSize) -> Self {
        Self::new(WordType::String as u8, address)
    }

    fn ensure(self, word_type: WordType) -> Result<Self, Trap> {
        if self.tag() == word_type as u8 {
            Ok(self)
        } else {
            Err(Trap::TypeError)
        }
    }

    fn as_fixnum(self) -> Result<u64, Trap> {
        Ok(self.ensure(WordType::Fixnum)?.payload())
    }
}

impl Debug for LispWord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Word::{:?}({})", self.tag(), self.payload())
    }
}

#[derive(Debug)]
pub enum WordParseError {
    InvalidPrefix,
}
impl TryFrom<Native> for LispWord {
    fn try_from(value: Native) -> Result<Self, Self::Error> {
        Ok(LispWord(value.0))
    }
    type Error = WordParseError;
}
impl From<LispWord> for Native {
    fn from(value: LispWord) -> Native {
        Native(value.0)
    }
}

#[derive(Debug)]
pub struct Cpu {
    /// Common registers.
    registers: [LispWord; 24],

    /// Machine registers
    machine_reg: [Native; 8],

    pending_interrupt: Option<u64>,

    pub halted: bool,
    interrupts_disabled: bool,
}

impl Cpu {
    pub const SP: MachineRegister = MachineRegister(4);
    pub const PC: MachineRegister = MachineRegister(5);
    // pub const ENV: MachineRegister = MachineRegister(6);
    pub const VBR: MachineRegister = MachineRegister(7);
    pub const T: Register = Register(23);
    pub const NIL: Register = Register(22);
    pub const WORD_SIZE: u64 = 8;
    pub const INSTRUCTION_SIZE: u64 = 2 * Self::WORD_SIZE;

    pub fn interrupt(&mut self, int: u64) {
        if !self.interrupts_disabled {
            self.pending_interrupt = Some(int);
            self.interrupts_disabled = true;
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            registers: [LispWord::undefined(); 24],
            machine_reg: [Native(0); 8],
            halted: false,
            pending_interrupt: None,
            interrupts_disabled: false,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Register(pub u8);
impl Debug for Register {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}", self.0)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MachineRegister(pub u8);
impl Debug for MachineRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "A{}", self.0)
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct RegSource {
    pub op1: Register,
    pub op2: Option<Register>,
    pub op3: Option<LispWord>,
}
impl Debug for RegSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.op2, self.op3) {
            (Some(op2), Some(op3)) => write!(f, "{:?}, {:?} + {:?}", self.op1, op2, op3),
            (Some(op2), None) => write!(f, "{:?}, {:?}", self.op1, op2),
            (None, Some(op3)) => write!(f, "{:?}, {:?}", self.op1, op3),
            (None, None) => write!(f, "!!!Invalid {:?}, NONE, NONE", self.op1),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct MachSource {
    pub op1: MachineRegister,
    pub op2: Option<MachineRegister>,
    pub op3: Option<Native>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EitherSource {
    Reg(RegSource),
    Mach(MachSource),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TwoRegs {
    dst: Register,
    src: Register,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Location {
    Literal(Native),
    Register(Register),
    Machine(MachineRegister),
    Absolute(Address),
    IndirectRegister(Register),
    IndirectMachine(MachineRegister, Offset),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum JumpTarget {
    Absolute(Address),
    Register(Register),
    Machine(MachineRegister, Offset),
    IndirectRegister(Register),
    IndirectMachine(MachineRegister, Offset),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Condition {
    Always,
    True(Register),
    False(Register),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MBinaryOp {
    Add,
    Sub,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Shl,
    Shr,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Comparison {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Instruction {
    // Lower level
    /// Halts execution
    Halt,

    /// Do nothing.
    Nop,

    /// Jumps unconditionally
    Jump {
        condition: Condition,
        target: JumpTarget,
    },

    /// PUSHes all Registers and Machinees to the stack, then the PC, and jumps.
    /// For some internal INTs (<0x80, like CONS) it may perform additional work.
    Int(WordSize),

    /// POPs the PC and and all Registers lower than (count) are restored.
    /// All registers are popped, but the count indicates which ones are kept.
    /// For some internal INTs, it may perform additional work.
    IReturn,

    /// Push raw data onto the stack.
    PushA {
        src: MachineRegister,
    },
    /// Pop raw data from the stack.
    PopA {
        dst: MachineRegister,
    },
    Mov {
        dst: Location,
        src: Location,
    },
    Mov8 {
        dst: Location,
        src: Location,
    },
    MBinary {
        op: MBinaryOp,
        dst: MachineRegister,
        operands: MachSource,
    },
    MComparison {
        op: Comparison,
        dst: Register,
        operands: EitherSource,
    },
    SetTag {
        dst: Register,
        src: MachineRegister,
    },
    GetTag {
        dst: MachineRegister,
        src: Register,
    },
    SetPayload {
        dst: Register,
        src: MachineRegister,
    },
    GetPayload {
        dst: MachineRegister,
        src: Register,
    },
    EnableInterrupts,
    DisableInterrupts,

    // Higher level
    // Cons { -- CONS does not exist - it is only an alias for INT 0x03
    //     car: Register,
    //     cdr: Register,
    // },
    /// Optimization for destructuring both parts of a cons.
    Uncons {
        car: Register,
        cdr: Register,
        src: Register,
    },
    /// Gets the first part of a cons - typechecks.
    Car(TwoRegs),
    /// Gets the second part of a cons - typechecks.
    Cdr(TwoRegs),
    /// Sets the first part of a cons - typechecks.
    SetCar(TwoRegs),
    /// Sets the second part of a cons - typechecks.
    SetCdr(TwoRegs),
    /// Calculates div <- op1 / (op2 + imm), rem <- op1 % (op2 + imm)
    IDiv {
        div: Register,
        rem: Register,
        operands: RegSource,
    },
    Binary {
        op: BinaryOp,
        dst: Register,
        operands: RegSource,
    },
    MakeClosure {
        dst: Register,
        code: MachineRegister,
    },
    /// Push a word onto the stack
    PushR {
        src: Register,
    },
    /// Pop a word onto the stack - and validate it's a word.
    PopR {
        dst: Register,
    },
    Typep {
        dst: Register,
        src: Register,
        compare: Native,
    },
    /// while (count--) *dst++ = *src++
    MemCpy {
        dst: MachineRegister,
        src: MachineRegister,
        count: Count,
    },
    /// while (count--) *dst++ = src
    // MemSet {
    //     dst: MachineRegister,
    //     src: MachineRegister,
    //     count: Count,
    // },

    /// PUSHes PC on the stack and jumps
    Call {
        target: JumpTarget,
    },

    /// POPs PC from the stack
    Return,
}

impl Cpu {
    pub fn reset(&mut self, bus: &Bus) {
        self.machine_reg[Cpu::PC.0 as usize] = bus.read_word(MemoryLayout::RESET_VECTOR);
        self.registers[Cpu::T.0 as usize] = LispWord(bus.read_word(MemoryLayout::T_ROOT).0);
        self.registers[Cpu::NIL.0 as usize] = LispWord(bus.read_word(MemoryLayout::NIL_ROOT).0);
        self.interrupts_disabled = true;
        self.halted = false;
        self.pending_interrupt = None;
    }

    fn fetch(&self, memory: &Bus) -> (Native, Native) {
        let instruction_low = memory.read_word(Address::from(self.machine_reg[Cpu::PC.0 as usize]));
        let instruction_high = memory.read_word(
            Address::from(self.machine_reg[Cpu::PC.0 as usize]) + Offset(Self::WORD_SIZE as i64),
        );
        (instruction_low, instruction_high)
    }

    fn read_word(memory: &Bus, address: Address) -> Result<LispWord, Trap> {
        memory
            .read_word(address)
            .try_into()
            .map_err(|_| Trap::TypeError)
    }

    fn to_machine_bool(&self, val: bool) -> LispWord {
        if val {
            self.registers[Cpu::T.0 as usize]
        } else {
            self.registers[Cpu::NIL.0 as usize]
        }
    }

    fn push(&mut self, memory: &mut Bus, val: Native) {
        self.machine_reg[Cpu::SP.0 as usize] =
            (Address::from(self.machine_reg[Cpu::SP.0 as usize]) - Offset(Self::WORD_SIZE as i64))
                .into();
        memory.write_word(Address::from(self.machine_reg[Cpu::SP.0 as usize]), val);
    }

    fn pop(&mut self, memory: &mut Bus) -> Native {
        let res = memory.read_word(Address::from(self.machine_reg[Cpu::SP.0 as usize]));
        self.machine_reg[Cpu::SP.0 as usize] =
            (Address::from(self.machine_reg[Cpu::SP.0 as usize]) + Offset(Self::WORD_SIZE as i64))
                .into();
        res
    }

    fn pop_word(&mut self, memory: &mut Bus) -> Result<LispWord, Trap> {
        let res = Self::read_word(memory, Address::from(self.machine_reg[Cpu::SP.0 as usize]));
        self.machine_reg[Cpu::SP.0 as usize] =
            (Address::from(self.machine_reg[Cpu::SP.0 as usize]) + Offset(Self::WORD_SIZE as i64))
                .into();
        res
    }

    fn get_offset_reg_val_unchecked(
        &self,
        base: Option<Register>,
        off: Option<LispWord>,
    ) -> LispWord {
        let mut add = 0;
        let mut tag = 0;
        if let Some(reg) = base {
            let op = self.registers[reg.0 as usize];
            add += op.payload();
            tag = op.tag();
        }
        if let Some(off) = off {
            add += off.0;
            tag = off.tag();
        }
        LispWord::new(tag, add)
    }

    fn get_offset_reg_val(
        &self,
        base: Option<Register>,
        off: Option<LispWord>,
    ) -> Result<u64, Trap> {
        let mut add = 0;
        if let Some(reg) = base {
            add += self.registers[reg.0 as usize].as_fixnum()?
        }
        if let Some(off) = off {
            add += off.as_fixnum()?
        }
        Ok(add)
    }

    fn get_offset_addr_val(&self, base: Option<MachineRegister>, off: Option<Native>) -> Native {
        let mut add = 0;
        if let Some(reg) = base {
            add = self.machine_reg[reg.0 as usize].0
        }
        if let Some(off) = off {
            add += off.0
        }
        Native(add)
    }

    fn run_interrupt(
        &mut self,
        memory: &mut Bus,
        interruption: u64,
        next_pc: Address,
    ) -> Result<Address, Trap> {
        self.pending_interrupt = None;
        match interruption {
            0x03 => {
                self.push(memory, self.registers[0_usize].into());
                self.push(memory, self.registers[1_usize].into());
                // CONS takes its params as R0 and R1
                self.registers[0_usize] = LispWord::new(WordType::Fixnum.into(), 16); // Size: 2
                self.registers[1_usize] =
                    LispWord::new(WordType::Fixnum.into(), WordType::Cons as u8 as u64);
                // Type: Int
            }
            _ => (),
        }

        self.push(memory, Native(self.interrupts_disabled as u64));
        self.push(memory, Native(interruption));
        self.push(memory, Native(next_pc.0));
        let location = Address::from(memory.read_word(
            Address::from(self.machine_reg[Cpu::VBR.0 as usize])
                + Offset(interruption as i64 * Self::WORD_SIZE as i64),
        ));
        Ok(location)
    }

    fn get_jump_addr(&self, memory: &Bus, src: JumpTarget) -> Address {
        match src {
            JumpTarget::Absolute(v) => v,
            JumpTarget::Register(Register(r)) => Address(self.registers[r as usize].payload()),
            JumpTarget::Machine(MachineRegister(pos), off) => {
                Address::from(self.machine_reg[pos as usize]) + off
            }
            JumpTarget::IndirectMachine(MachineRegister(reg), off) => {
                let pos = Address::from(self.machine_reg[reg as usize]) + off;
                memory.read_word(pos).into()
            }
            JumpTarget::IndirectRegister(Register(r)) => {
                let pos = Address(self.registers[r as usize].payload());
                memory.read_word(pos).into()
            }
        }
    }

    fn read_location(&self, memory: &Bus, src: Location) -> Native {
        match src {
            Location::Literal(v) => v,
            Location::Absolute(a) => memory.read_word(a),
            Location::Register(Register(r)) => self.registers[r as usize].into(),
            Location::Machine(MachineRegister(pos)) => self.machine_reg[pos as usize],
            Location::IndirectMachine(MachineRegister(reg), off) => {
                let pos = Address::from(self.machine_reg[reg as usize]) + off;
                memory.read_word(pos)
            }
            Location::IndirectRegister(Register(r)) => {
                let pos = Address(self.registers[r as usize].payload());
                memory.read_word(pos)
            }
        }
    }

    fn execute(&mut self, instruction: Instruction, memory: &mut Bus) -> Result<Address, Trap> {
        match instruction {
            // Control flow
            Instruction::Halt => {
                self.halted = true;
            }
            Instruction::Nop => {}

            Instruction::Jump { condition, target } => {
                let nil = self.registers[Cpu::NIL.0 as usize];

                let should_jump = match condition {
                    Condition::Always => true,
                    Condition::False(Register(r)) => self.registers[r as usize] == nil,
                    Condition::True(Register(r)) => self.registers[r as usize] != nil,
                };

                if should_jump {
                    return Ok(self.get_jump_addr(memory, target));
                }
            }

            Instruction::Call { target } => {
                let next_pc = Address::from(self.machine_reg[Cpu::PC.0 as usize])
                    + Offset(Cpu::INSTRUCTION_SIZE as i64);
                let address = self.get_jump_addr(memory, target);

                self.push(memory, Native::from(next_pc));
                return Ok(address);
            }

            Instruction::Return => {
                let return_address = self.pop(memory);
                return Ok(Address::from(return_address));
            }

            Instruction::MakeClosure { dst: _, code: _ } => todo!(),

            // Comparison
            Instruction::MComparison {
                op,
                dst,
                operands: EitherSource::Mach(MachSource { op1, op2, op3 }),
            } => {
                let op1_obj = self.machine_reg[op1.0 as usize];
                let op2_obj = self.get_offset_addr_val(op2, op3);

                let result = self.to_machine_bool(match op {
                    Comparison::Eq => op1_obj == op2_obj,
                    Comparison::Ne => op1_obj != op2_obj,
                    Comparison::Gt => op1_obj > op2_obj,
                    Comparison::Gte => op1_obj >= op2_obj,
                    Comparison::Lt => op1_obj < op2_obj,
                    Comparison::Lte => op1_obj <= op2_obj,
                });
                self.registers[dst.0 as usize] = result;
            }

            Instruction::MComparison {
                op,
                dst,
                operands: EitherSource::Reg(RegSource { op1, op2, op3 }),
            } => {
                let op1_obj = self.registers[op1.0 as usize];
                let op2_obj = self.get_offset_reg_val_unchecked(op2, op3);

                let result = self.to_machine_bool(match op {
                    Comparison::Eq => op1_obj == op2_obj,
                    Comparison::Ne => op1_obj != op2_obj,
                    Comparison::Gt => op1_obj.as_fixnum()? > op2_obj.as_fixnum()?,
                    Comparison::Gte => op1_obj.as_fixnum()? >= op2_obj.as_fixnum()?,
                    Comparison::Lt => op1_obj.as_fixnum()? < op2_obj.as_fixnum()?,
                    Comparison::Lte => op1_obj.as_fixnum()? <= op2_obj.as_fixnum()?,
                });
                self.registers[dst.0 as usize] = result;
            }

            Instruction::Binary {
                op,
                dst,
                operands: RegSource { op1, op2, op3 },
            } => {
                let op1_obj = self.registers[op1.0 as usize].as_fixnum()?;
                let op2_obj = self.get_offset_reg_val(op2, op3)?;

                let result = LispWord::fixnum(match op {
                    BinaryOp::Add => op1_obj + op2_obj,
                    BinaryOp::Sub => op1_obj - op2_obj,
                    BinaryOp::Mul => op1_obj * op2_obj,
                    BinaryOp::Shl => op1_obj << op2_obj,
                    BinaryOp::Shr => op1_obj >> op2_obj,
                });

                self.registers[dst.0 as usize] = result;
            }

            // Cons
            Instruction::Int(interruption) => {
                let next_pc = Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64);
                return self.run_interrupt(memory, interruption, next_pc);
            }

            Instruction::IReturn => {
                let return_address = self.pop(memory);

                let interrupt = self.pop(memory);
                let interrupt_disabled = self.pop(memory);
                match interrupt.0 {
                    0x03 => {
                        let cdr = self.pop(memory);
                        let car = self.pop(memory);
                        memory.write_word(
                            Address::from(self.machine_reg[0_usize]) + ConsLayout::CAR_OFFSET,
                            car,
                        );
                        memory.write_word(
                            Address::from(self.machine_reg[0_usize]) + ConsLayout::CDR_OFFSET,
                            cdr,
                        );
                        self.registers[0] = LispWord::cons(self.machine_reg[0_usize].0 as WordSize);
                    }
                    _ => {}
                }
                self.interrupts_disabled = interrupt_disabled.0 != 0;
                return Ok(Address::from(return_address));
            }

            Instruction::Car(TwoRegs { dst, src }) => {
                let src_obj = self.registers[src.0 as usize].ensure(WordType::Cons)?;

                self.registers[dst.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET)?;
            }
            Instruction::Cdr(TwoRegs { dst, src }) => {
                let src_obj = self.registers[src.0 as usize].ensure(WordType::Cons)?;

                self.registers[dst.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET)?;
            }
            Instruction::SetCar(TwoRegs { dst, src }) => {
                let dst_obj = self.registers[dst.0 as usize].ensure(WordType::Cons)?;
                let val_obj = self.registers[src.0 as usize];

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CAR_OFFSET,
                    val_obj.into(),
                );
            }
            Instruction::SetCdr(TwoRegs { dst, src }) => {
                let dst_obj = self.registers[dst.0 as usize].ensure(WordType::Cons)?;
                let val_obj = self.registers[src.0 as usize];

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CDR_OFFSET,
                    val_obj.into(),
                );
            }
            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.registers[src.0 as usize].ensure(WordType::Cons)?;

                self.registers[car.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET)?;
                self.registers[cdr.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET)?;
            }

            Instruction::IDiv {
                div,
                rem,
                operands: RegSource { op1, op2, op3 },
            } => {
                let op1_obj = self.registers[op1.0 as usize].as_fixnum()?;
                let op2_obj = self.get_offset_reg_val(op2, op3)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[div.0 as usize] = LispWord::fixnum(op1_obj / op2_obj);
                self.registers[rem.0 as usize] = LispWord::fixnum(op1_obj % op2_obj);
            }

            Instruction::PopR { dst } => {
                let result = self.pop_word(memory)?;
                self.registers[dst.0 as usize] = result;
            }
            Instruction::PushR { src } => {
                self.push(memory, self.registers[src.0 as usize].into());
            }
            Instruction::PopA { dst } => {
                let result = self.pop(memory);
                self.machine_reg[dst.0 as usize] = result;
            }
            Instruction::PushA { src } => {
                self.push(memory, self.machine_reg[src.0 as usize]);
            }

            Instruction::MBinary {
                op,
                dst,
                operands: MachSource { op1, op2, op3 },
            } => {
                let val1 = self.machine_reg[op1.0 as usize];
                let val2 = self.get_offset_addr_val(op2, op3);
                let result = match op {
                    MBinaryOp::Add => val1 + val2,
                    MBinaryOp::Sub => val1 - val2,
                };
                self.machine_reg[dst.0 as usize] = result;
            }
            Instruction::GetPayload { dst, src } => {
                self.machine_reg[dst.0 as usize] = Native(self.registers[src.0 as usize].payload());
            }
            Instruction::GetTag { src, dst } => {
                self.machine_reg[dst.0 as usize] =
                    Native(self.registers[src.0 as usize].tag() as u64);
            }
            Instruction::SetPayload { dst, src } => {
                self.registers[dst.0 as usize] = LispWord::new(
                    self.registers[dst.0 as usize].tag(),
                    self.machine_reg[src.0 as usize].0,
                );
            }
            Instruction::SetTag { src, dst } => {
                self.registers[dst.0 as usize] = LispWord::new(
                    self.machine_reg[src.0 as usize].0 as u8,
                    self.registers[dst.0 as usize].payload(),
                );
            }
            Instruction::Mov8 { dst, src } => {
                let value = self.read_location(memory, src).0 as u8;
                match dst {
                    Location::Literal(_) => return Err(Trap::InvalidInstruction), // Makes no sense to move into a literal.
                    Location::Absolute(a) => memory.write_byte(a, value),
                    Location::Machine(MachineRegister(r)) => {
                        self.machine_reg[(r as i64) as usize] = Native(value as u64)
                    }
                    Location::Register(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.write_byte(Address::from(self.machine_reg[r as usize]) + off, value)
                    }
                    Location::IndirectRegister(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                }
            }
            Instruction::Mov { dst, src } => {
                let value = self.read_location(memory, src);
                match dst {
                    Location::Literal(_) => return Err(Trap::InvalidInstruction), // Makes no sense to move into a literal.
                    Location::Absolute(a) => memory.write_word(a, value),
                    Location::Machine(MachineRegister(r)) => self.machine_reg[r as usize] = value,
                    Location::Register(Register(r)) => {
                        self.registers[r as usize] =
                            LispWord::try_from(value).map_err(|_| Trap::TypeError)?
                    }
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.write_word(Address::from(self.machine_reg[r as usize]) + off, value)
                    }
                    Location::IndirectRegister(Register(r)) => {
                        memory.write_word(Address(self.registers[r as usize].payload()), value)
                    }
                }
            }
            Instruction::MemCpy { dst, src, count } => {
                let srcadd = self.machine_reg[src.0 as usize];
                let dstadd = self.machine_reg[dst.0 as usize];
                for i in 0..count.0 {
                    memory.write_byte(
                        Address::from(dstadd) + Offset(i as i64),
                        memory.read_byte(Address::from(srcadd) + Offset(i as i64)),
                    )
                }
            }
            // Instruction::MemSet { dst, src, count } => {
            //     let srcadd = self.machine_reg[src.0 as usize];
            //     let dstadd = self.machine_reg[dst.0 as usize];
            //     for i in 0..count.0 {
            //         memory.write_byte(
            //             Address::from(dstadd) + Offset(i as i64),
            //             memory.read_byte(Address::from(srcadd)),
            //         )
            //     }
            //     Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
            //         + Offset(Cpu::INSTRUCTION_SIZE as i64))
            // }
            Instruction::Typep { dst, src, compare } => {
                let src_obj = self.registers[src.0 as usize];
                self.registers[dst.0 as usize] =
                    self.to_machine_bool(src_obj.tag() as u64 == compare.0);
            }
            Instruction::DisableInterrupts => {
                self.interrupts_disabled = true;
            }
            Instruction::EnableInterrupts => {
                self.interrupts_disabled = true;
            }
        };

        Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0) + Offset(Cpu::INSTRUCTION_SIZE as i64))
    }

    pub fn step(&mut self, memory: &mut Bus) -> Result<(), Trap> {
        match self.pending_interrupt {
            Some(i) => {
                self.halted = false;
                self.machine_reg[Cpu::PC.0 as usize] = Native::from(self.run_interrupt(
                    memory,
                    i,
                    Address::from(self.machine_reg[Cpu::PC.0 as usize]),
                )?)
            }
            None => {
                if self.halted {
                    return Ok(());
                }
            }
        };

        let (lo, hi) = self.fetch(memory);
        let instruction = Instruction::decode(lo.0, hi.0).map_err(|_| Trap::InvalidInstruction)?;
        println!(
            "PC: 0x{:x} SP:{:x} {:?}",
            Address(self.machine_reg[Cpu::PC.0 as usize].0).0,
            Address(self.machine_reg[Cpu::SP.0 as usize].0).0,
            instruction
        );
        let next_pc = self.execute(instruction, memory)?;
        Address(self.machine_reg[Cpu::PC.0 as usize].0) = next_pc;
        Ok(())
    }

    pub fn full_step(&mut self, memory: &mut Bus) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.machine_reg[0_usize] = self.machine_reg[Cpu::PC.0 as usize];
                self.registers[0_usize] = LispWord::new(WordType::Fixnum.into(), trap as WordSize);
                self.machine_reg[Cpu::PC.0 as usize] = memory.read_word(
                    Address::from(self.machine_reg[Cpu::VBR.0 as usize])
                        + InterruptTableOffset::TRAP_VECTOR,
                )
            }
        };
    }
}

#[derive(Debug)]
pub enum Trap {
    TypeError,
    InvalidInstruction,
}

#[cfg(test)]
mod tests {

    use crate::ram::Memory;

    use super::*;

    struct TestMemory {}

    impl TestMemory {
        fn load_instructions(bus: &mut Bus, start: Address, instructions: Vec<Instruction>) {
            let mut pos = 0;
            for instr in instructions {
                let (lo, hi) = instr.encode().unwrap();
                bus.write_word(start + Offset(pos * Cpu::WORD_SIZE as i64), Native(lo));
                pos += 1;
                bus.write_word(start + Offset(pos * Cpu::WORD_SIZE as i64), Native(hi));
                pos += 1;
            }
        }
    }

    fn test_setup<'a>() -> Bus<'a> {
        let memory = Memory::new(0x10000);
        let mut bus = Bus::new();
        bus.install(0x0..0x10000, Box::new(memory)).unwrap();
        bus
    }

    #[test]
    fn test_halt() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.machine_reg[Cpu::PC.0 as usize] = Native(0x1000);
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.machine_reg[Cpu::PC.0 as usize]),
            vec![Instruction::Halt],
        );
        cpu.full_step(&mut test_bus);
        assert_eq!(cpu.machine_reg[Cpu::PC.0 as usize], Native(0x1010));
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.machine_reg[Cpu::PC.0 as usize] = Native(0x1000);
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.machine_reg[Cpu::PC.0 as usize]),
            vec![Instruction::Nop],
        );
        cpu.full_step(&mut test_bus);
        assert_eq!(cpu.machine_reg[Cpu::PC.0 as usize], Native(0x1010));
        assert!(!cpu.halted);
        Ok(())
    }

    #[test]
    fn test_multiple_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.machine_reg[Cpu::PC.0 as usize] = Native(0x1000);
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.machine_reg[Cpu::PC.0 as usize]),
            vec![
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Nop,
                Instruction::Halt,
            ],
        );
        while !cpu.halted {
            cpu.full_step(&mut test_bus);
        }
        assert_eq!(cpu.machine_reg[Cpu::PC.0 as usize], Native(0x10a0));
        Ok(())
    }

    // #[test]
    // fn test_jump_adr() -> Result<(), String> {
    //     let mut cpu = Cpu::default();
    //     let mut memory = TestMemory::new();
    //     cpu.machine_reg[Cpu::PC.0 as usize] = Address(0x1000);
    //     cpu.machine_reg[0] = Address(0x2000);
    //     TestMemory::load_instructions(
    //         &mut memory,
    //         cpu.machine_reg[Cpu::PC.0 as usize],
    //         parse_asm! {
    //             JUMP [A 0];
    //         },
    //     );
    //     cpu.full_step(memory.as_mut_slice());
    //     assert_eq!(cpu.machine_reg[Cpu::PC.0 as usize], Address(0x2000));
    //     Ok(())
    // }

    // #[test]
    // fn test_cons_builds_cell() -> Result<(), String> {
    //     let mut cpu = Cpu::default();
    //     let mut memory = TestMemory::new();

    //     let cons_hook = 0x2000;
    //     let trap_hook = 0x2100;
    //     let cons_cell = 0x3000;
    //     let code_base = 0x4000;

    //     Cpu::write_word(&mut memory, MemoryLayout::RESET_VECTOR, code_base);
    //     Cpu::write_word(
    //         &mut memory,
    //         MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_VECTOR,
    //         cons_hook,
    //     );
    //     Cpu::write_word(
    //         &mut memory,
    //         MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_CONS_VECTOR,
    //         cons_hook,
    //     );
    //     Cpu::write_word(
    //         &mut memory,
    //         MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR,
    //         trap_hook,
    //     );
    //     cpu.reset(&mut memory);

    //     TestMemory::load_instructions(
    //         &mut memory,
    //         code_base as u64,
    //         parse_asm! {
    //             MOV A Cpu::SP, 0x800;
    //             MOV R 0, Word::fixnum(42);
    //             MOV R 1, Word::fixnum(99);
    //             CONS;
    //             NOP;
    //             HALT;
    //         },
    //     );

    //     // Fake CONS_HOOK
    //     TestMemory::load_instructions(
    //         &mut memory,
    //         cons_hook,
    //         parse_asm! {
    //             MOV A 0, cons_cell;
    //             IRETURN;
    //         },
    //     );

    //     TestMemory::load_instructions(
    //         &mut memory,
    //         trap_hook,
    //         parse_asm! {
    //             HALT;
    //         },
    //     );

    //     cpu.reset(&mut memory);
    //     while !cpu.halted {
    //         cpu.full_step(memory.as_mut_slice());
    //     }

    //     assert_eq!(
    //         cpu.registers[0_usize],
    //         Word::new(WordType::Cons, cons_cell as u64)
    //     );

    //     assert_eq!(
    //         Word::try_from(memory.as_mut_slice().read_word(cons_cell))
    //             .expect("couldn't parse word"),
    //         Word::new(WordType::Fixnum, 42)
    //     );

    //     assert_eq!(
    //         Word::try_from(memory.as_mut_slice().read_word(cons_cell + Self::WORD_SIZE))
    //             .expect("couldn't parse word"),
    //         Word::new(WordType::Fixnum, 99).into()
    //     );

    //     assert!(cpu.halted);

    //     Ok(())
    // }
}
