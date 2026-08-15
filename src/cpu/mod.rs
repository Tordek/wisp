pub mod assembler;
mod encoding;

use std::fmt::Debug;

use crate::bus::{Address, Bus, Native, Offset};
use int_enum::IntEnum;

type WordSize = u64;

pub enum MemoryLayout {}
impl MemoryLayout {
    // Standard addresses.
    pub const CPU_END: Address = Address(0xffffffffffffffff);
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
    Macro = 5,
    Character = 6,
    String = 7,
    Vector = 8,
    Float = 9,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct LispWord(WordSize);

impl LispWord {
    pub const fn undefined() -> LispWord {
        Self::new(WordType::Undefined as u8, 0)
    }

    pub fn tag(&self) -> u8 {
        (self.0 & 0xff) as u8
    }

    pub const fn payload(&self) -> WordSize {
        self.0 >> 8
    }

    pub const fn new(tag: u8, payload: WordSize) -> Self {
        Self((payload << 8) | (tag as u64))
    }

    // Convenience methods
    pub const fn symbol(value: WordSize) -> Self {
        Self::new(WordType::Symbol as u8, value)
    }

    pub const fn fixnum(value: i64) -> Self {
        Self::new(WordType::Fixnum as u8, value as u64)
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

    fn as_fixnum(self) -> Result<i64, Trap> {
        self.ensure(WordType::Fixnum)?;
        Ok((self.0 as i64) >> 8)
    }
}

impl Debug for LispWord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Word::")?;
        match WordType::try_from(self.tag()) {
            Ok(tag) => write!(f, "{:?}", tag)?,
            Err(_) => write!(f, "[{}]", self.tag())?,
        };
        write!(f, "({})", self.payload())
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

#[derive(Debug, Default)]
pub struct Cpu {
    registers: [WordSize; 32],

    pending_interrupt: Option<u64>,

    pub halted: bool,
    interrupts_disabled: bool,
}

impl Cpu {
    pub const ENV: Register = Register(13);
    pub const NIL: Register = Register(14);
    pub const T: Register = Register(15);

    pub const CONS_FREE: MachineRegister = MachineRegister(9);
    pub const CONS_END: MachineRegister = MachineRegister(10);
    pub const GEN_FREE: MachineRegister = MachineRegister(11);
    pub const GEN_END: MachineRegister = MachineRegister(12);
    pub const SP: MachineRegister = MachineRegister(13);
    pub const PC: MachineRegister = MachineRegister(14);
    pub const VBR: MachineRegister = MachineRegister(15);
    pub const WORD_SIZE: u64 = 8;
    pub const INSTRUCTION_SIZE: u64 = 2 * Self::WORD_SIZE;

    pub fn interrupt(&mut self, int: u64) {
        if self.pending_interrupt.is_none() {
            self.pending_interrupt = Some(int);
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
impl Register {
    fn offset(&self) -> usize {
        self.0 as usize
    }

    fn from_offset(offset: usize) -> Self {
        Self(offset as u8)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MachineRegister(pub u8);
impl Debug for MachineRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "A{}", self.0)
    }
}
impl MachineRegister {
    fn offset(&self) -> usize {
        self.0 as usize + 16
    }

    fn from_offset(offset: usize) -> Self {
        Self(offset as u8 - 16)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RegAndOff {
    op1: Option<Register>,
    off: Option<LispWord>,
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
    Comparison {
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
    Req {
        dst: Register,
        prototype: Register,
        size: Register,
    },
    // Higher level
    Cons {
        dst: Register,
        car: Register,
        cdr: Register,
    },
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
        count: RegAndOff,
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
    pub fn reset(&mut self) {
        self.set_mregister(Cpu::PC, Native::from(MemoryLayout::RESET_VECTOR));
        self.set_register(Cpu::T, LispWord(0));
        self.set_register(Cpu::NIL, LispWord(1));
        self.interrupts_disabled = true;
        self.halted = false;
        self.pending_interrupt = None;
    }

    fn register(&self, r: Register) -> LispWord {
        LispWord(self.registers[r.offset()])
    }

    fn set_register(&mut self, r: Register, data: LispWord) {
        self.registers[r.offset()] = data.0
    }

    fn mregister(&self, r: MachineRegister) -> Native {
        Native(self.registers[r.offset()])
    }

    fn set_mregister(&mut self, r: MachineRegister, data: Native) {
        self.registers[r.offset()] = data.0
    }

    fn fetch(&self, memory: &Bus) -> (Native, Native) {
        let instruction_low = memory.read_word(Address::from(self.mregister(Cpu::PC)));
        let instruction_high = memory
            .read_word(Address::from(self.mregister(Cpu::PC)) + Offset(Self::WORD_SIZE as i64));
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
            self.register(Cpu::T)
        } else {
            self.register(Cpu::NIL)
        }
    }

    fn push(&mut self, memory: &mut Bus, val: Native) {
        self.set_mregister(
            Cpu::SP,
            (Address::from(self.mregister(Cpu::SP)) - Offset(Self::WORD_SIZE as i64)).into(),
        );
        memory.write_word(Address::from(self.mregister(Cpu::SP)), val);
    }

    fn pop(&mut self, memory: &mut Bus) -> Native {
        let res = memory.read_word(Address::from(self.mregister(Cpu::SP)));
        self.set_mregister(
            Cpu::SP,
            (Address::from(self.mregister(Cpu::SP)) + Offset(Self::WORD_SIZE as i64)).into(),
        );
        res
    }

    fn pop_word(&mut self, memory: &mut Bus) -> Result<LispWord, Trap> {
        let res = Self::read_word(memory, Address::from(self.mregister(Cpu::SP)));
        self.set_mregister(
            Cpu::SP,
            (Address::from(self.mregister(Cpu::SP)) + Offset(Self::WORD_SIZE as i64)).into(),
        );
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
            let op = self.register(reg);
            add += op.payload();
            tag = op.tag();
        }
        if let Some(off) = off {
            add += off.payload();
            tag = off.tag();
        }
        LispWord::new(tag, add)
    }

    fn get_offset_reg_val(
        &self,
        base: Option<Register>,
        off: Option<LispWord>,
    ) -> Result<i64, Trap> {
        let mut add = 0;
        if let Some(reg) = base {
            add += self.register(reg).as_fixnum()?
        }
        if let Some(off) = off {
            add += off.as_fixnum()?
        }
        Ok(add)
    }

    fn get_offset_addr_val(&self, base: Option<MachineRegister>, off: Option<Native>) -> Native {
        let mut add = 0;
        if let Some(reg) = base {
            add = self.mregister(reg).0
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
        // TODO: This should be one Status register
        self.push(memory, Native(self.interrupts_disabled as u64));
        self.push(memory, Native(interruption));
        self.push(memory, Native(next_pc.0));
        self.interrupts_disabled = true;
        let location = Address::from(memory.read_word(
            Address::from(self.mregister(Cpu::VBR))
                + Offset(interruption as i64 * Self::WORD_SIZE as i64),
        ));
        Ok(location)
    }

    fn get_jump_addr(&self, memory: &Bus, src: JumpTarget) -> Address {
        match src {
            JumpTarget::Absolute(v) => v,
            JumpTarget::Register(r) => Address(self.register(r).payload()),
            JumpTarget::Machine(pos, off) => Address::from(self.mregister(pos)) + off,
            JumpTarget::IndirectMachine(reg, off) => {
                let pos = Address::from(self.mregister(reg)) + off;
                memory.read_word(pos).into()
            }
            JumpTarget::IndirectRegister(r) => {
                let pos = Address(self.register(r).payload());
                memory.read_word(pos).into()
            }
        }
    }

    fn read_location(&self, memory: &Bus, src: Location) -> Native {
        match src {
            Location::Literal(v) => v,
            Location::Absolute(a) => memory.read_word(a),
            Location::Register(r) => self.register(r).into(),
            Location::Machine(pos) => self.mregister(pos),
            Location::IndirectMachine(reg, off) => {
                let pos = Address::from(self.mregister(reg)) + off;
                memory.read_word(pos)
            }
            Location::IndirectRegister(r) => {
                let pos = Address(self.register(r).payload());
                memory.read_word(pos)
            }
        }
    }

    fn read_location_byte(&self, memory: &Bus, src: Location) -> u8 {
        match src {
            Location::Literal(v) => v.0 as u8,
            Location::Absolute(a) => memory.read_byte(a),
            Location::Register(r) => self.register(r).payload() as u8,
            Location::Machine(pos) => self.mregister(pos).0 as u8,
            Location::IndirectMachine(reg, off) => {
                let pos = Address::from(self.mregister(reg)) + off;
                memory.read_byte(pos)
            }
            Location::IndirectRegister(r) => {
                let pos = Address(self.register(r).payload());
                memory.read_byte(pos)
            }
        }
    }

    fn execute(&mut self, instruction: Instruction, memory: &mut Bus) -> Result<Address, Trap> {
        let next_pc = Address(self.mregister(Cpu::PC).0) + Offset(Cpu::INSTRUCTION_SIZE as i64);
        match instruction {
            // Control flow
            Instruction::Halt => {
                self.halted = true;
            }
            Instruction::Nop => {}

            Instruction::Jump { condition, target } => {
                let nil = self.register(Cpu::NIL);

                let should_jump = match condition {
                    Condition::Always => true,
                    Condition::False(r) => self.register(r) == nil,
                    Condition::True(r) => self.register(r) != nil,
                };

                if should_jump {
                    return Ok(self.get_jump_addr(memory, target));
                }
            }

            Instruction::Call { target } => {
                let next_pc =
                    Address::from(self.mregister(Cpu::PC)) + Offset(Cpu::INSTRUCTION_SIZE as i64);
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
            Instruction::Comparison {
                op,
                dst,
                operands: EitherSource::Mach(MachSource { op1, op2, op3 }),
            } => {
                let op1_obj = self.mregister(op1);
                let op2_obj = self.get_offset_addr_val(op2, op3);

                let result = match op {
                    Comparison::Eq => op1_obj == op2_obj,
                    Comparison::Ne => op1_obj != op2_obj,
                    Comparison::Gt => op1_obj > op2_obj,
                    Comparison::Gte => op1_obj >= op2_obj,
                    Comparison::Lt => op1_obj < op2_obj,
                    Comparison::Lte => op1_obj <= op2_obj,
                };

                self.set_register(dst, self.to_machine_bool(result));
            }

            Instruction::Comparison {
                op,
                dst,
                operands: EitherSource::Reg(RegSource { op1, op2, op3 }),
            } => {
                let op1_obj = self.register(op1);
                let op2_obj = self.get_offset_reg_val_unchecked(op2, op3);

                let result = match op {
                    Comparison::Eq => op1_obj == op2_obj,
                    Comparison::Ne => op1_obj != op2_obj,
                    Comparison::Gt => op1_obj.as_fixnum()? > op2_obj.as_fixnum()?,
                    Comparison::Gte => op1_obj.as_fixnum()? >= op2_obj.as_fixnum()?,
                    Comparison::Lt => op1_obj.as_fixnum()? < op2_obj.as_fixnum()?,
                    Comparison::Lte => op1_obj.as_fixnum()? <= op2_obj.as_fixnum()?,
                };
                self.set_register(dst, self.to_machine_bool(result));
            }

            Instruction::Binary {
                op,
                dst,
                operands: RegSource { op1, op2, op3 },
            } => {
                let op1_obj = self.register(op1).as_fixnum()?;
                let op2_obj = self.get_offset_reg_val(op2, op3)?;

                let result = LispWord::fixnum(match op {
                    BinaryOp::Add => op1_obj + op2_obj,
                    BinaryOp::Sub => op1_obj - op2_obj,
                    BinaryOp::Mul => op1_obj * op2_obj,
                    BinaryOp::Shl => op1_obj << op2_obj,
                    BinaryOp::Shr => op1_obj >> op2_obj,
                });

                self.set_register(dst, result);
            }

            Instruction::Int(interruption) => {
                return self.run_interrupt(memory, interruption, next_pc);
            }

            Instruction::IReturn => {
                let return_address = self.pop(memory);
                let _interrupt = self.pop(memory);
                self.interrupts_disabled = self.pop(memory).0 != 0;
                return Ok(Address::from(return_address));
            }

            Instruction::Car(TwoRegs { dst, src }) => {
                let src_obj = self.register(src).ensure(WordType::Cons)?;

                self.set_register(
                    dst,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET)?,
                );
            }
            Instruction::Cdr(TwoRegs { dst, src }) => {
                let src_obj = self.register(src).ensure(WordType::Cons)?;

                self.set_register(
                    dst,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET)?,
                );
            }
            Instruction::SetCar(TwoRegs { dst, src }) => {
                let dst_obj = self.register(dst).ensure(WordType::Cons)?;
                let val_obj = self.register(src);

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CAR_OFFSET,
                    val_obj.into(),
                );
            }
            Instruction::SetCdr(TwoRegs { dst, src }) => {
                let dst_obj = self.register(dst).ensure(WordType::Cons)?;
                let val_obj = self.register(src);

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CDR_OFFSET,
                    val_obj.into(),
                );
            }
            Instruction::Cons { dst, car, cdr } => {
                let free = self.mregister(Self::CONS_FREE);
                if free.0 + 16 <= self.mregister(Self::CONS_END).0 {
                    memory.write_word(
                        Address::from(free) + ConsLayout::CAR_OFFSET,
                        self.register(car).into(),
                    );
                    memory.write_word(
                        Address::from(free) + ConsLayout::CDR_OFFSET,
                        self.register(cdr).into(),
                    );
                    self.set_register(dst, LispWord::cons(free.0));
                    self.set_mregister(Self::CONS_FREE, free + Native(16));
                } else {
                    return self.run_interrupt(
                        memory,
                        0x03,
                        Address::from(self.mregister(Self::PC)),
                    ); // Come back.
                }
            }

            Instruction::Req {
                dst,
                prototype,
                size,
            } => {
                let sizew = self.register(size).as_fixnum()? as u64;
                let sizeb = sizew * 8;
                let free = self.mregister(Self::GEN_FREE);
                if free.0 + sizeb <= self.mregister(Self::GEN_END).0 {
                    for i in 0..sizew {
                        memory.write_word(
                            Address::from(free) + Offset(i as i64 * 8),
                            self.register(Register(i as u8)).into(),
                        )
                    }
                    let prototype = self.register(prototype);
                    let result = LispWord::new(prototype.tag(), free.0);
                    self.set_register(dst, result);
                    self.set_mregister(Self::GEN_FREE, free + Native(sizeb));
                } else {
                    return self.run_interrupt(
                        memory,
                        0x03,
                        Address::from(self.mregister(Self::PC)),
                    ); // Come back.
                }
            }

            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.register(src).ensure(WordType::Cons)?;

                self.set_register(
                    car,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET)?,
                );
                self.set_register(
                    cdr,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET)?,
                );
            }

            Instruction::IDiv {
                div,
                rem,
                operands: RegSource { op1, op2, op3 },
            } => {
                let op1_obj = self.register(op1).as_fixnum()?;
                let op2_obj = self.get_offset_reg_val(op2, op3)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.set_register(div, LispWord::fixnum(op1_obj / op2_obj));
                self.set_register(rem, LispWord::fixnum(op1_obj % op2_obj));
            }

            Instruction::PopR { dst } => {
                let result = self.pop_word(memory)?;
                self.set_register(dst, result);
            }
            Instruction::PushR { src } => {
                self.push(memory, self.register(src).into());
            }
            Instruction::PopA { dst } => {
                let result = self.pop(memory);
                self.set_mregister(dst, result);
            }
            Instruction::PushA { src } => {
                self.push(memory, self.mregister(src));
            }

            Instruction::MBinary {
                op,
                dst,
                operands: MachSource { op1, op2, op3 },
            } => {
                let val1 = self.mregister(op1);
                let val2 = self.get_offset_addr_val(op2, op3);
                let result = match op {
                    MBinaryOp::Add => val1 + val2,
                    MBinaryOp::Sub => val1 - val2,
                };
                self.set_mregister(dst, result);
            }
            Instruction::GetPayload { dst, src } => {
                self.set_mregister(dst, Native(self.register(src).payload()));
            }
            Instruction::GetTag { src, dst } => {
                self.set_mregister(dst, Native(self.register(src).tag() as WordSize));
            }
            Instruction::SetPayload { dst, src } => {
                self.set_register(
                    dst,
                    LispWord::new(self.register(dst).tag(), self.mregister(src).0),
                );
            }
            Instruction::SetTag { src, dst } => {
                self.set_register(
                    dst,
                    LispWord::new(self.mregister(src).0 as u8, self.register(dst).payload()),
                );
            }
            Instruction::Mov8 { dst, src } => {
                let value = self.read_location_byte(memory, src);
                match dst {
                    Location::Literal(_) => return Err(Trap::InvalidInstruction), // Makes no sense to move into a literal.
                    Location::Absolute(a) => memory.write_byte(a, value),
                    Location::Machine(r) => self.set_mregister(r, Native(value as WordSize)),
                    Location::Register(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                    Location::IndirectMachine(r, off) => {
                        memory.write_byte(Address::from(self.mregister(r)) + off, value)
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
                    Location::Machine(r) => self.set_mregister(r, value),
                    Location::Register(r) => self
                        .set_register(r, LispWord::try_from(value).map_err(|_| Trap::TypeError)?),
                    Location::IndirectMachine(r, off) => {
                        memory.write_word(Address::from(self.mregister(r)) + off, value)
                    }
                    Location::IndirectRegister(r) => {
                        memory.write_word(Address(self.register(r).payload()), value)
                    }
                }
            }
            Instruction::MemCpy { dst, src, count } => {
                let srcadd = Address::from(self.mregister(src));
                let dstadd = Address::from(self.mregister(dst));
                let count = self.get_offset_reg_val(count.op1, count.off)?;
                for i in 0..count {
                    let value = memory.read_byte(srcadd + Offset(i));
                    memory.write_byte(dstadd + Offset(i), value)
                }
            }
            // Instruction::MemSet { dst, src, count } => {
            //     let srcadd = self.mregister(src);
            //     let dstadd = self.mregister(dst);
            //     for i in 0..count.0 {
            //         memory.write_byte(
            //             Address::from(dstadd) + Offset(i as i64),
            //             memory.read_byte(Address::from(srcadd)),
            //         )
            //     }
            //     Ok(Address(self.mregister(Cpu::PC).0)
            //         + Offset(Cpu::INSTRUCTION_SIZE as i64))
            // }
            Instruction::Typep { dst, src, compare } => {
                let src_obj = self.register(src);
                self.set_register(dst, self.to_machine_bool(src_obj.tag() as u64 == compare.0));
            }
            Instruction::DisableInterrupts => {
                self.interrupts_disabled = true;
            }
            Instruction::EnableInterrupts => {
                self.interrupts_disabled = false;
            }
        };

        Ok(next_pc)
    }

    pub fn step(&mut self, memory: &mut Bus) -> Result<(), Trap> {
        match self.pending_interrupt {
            Some(i) => {
                self.halted = false;
                let next_pc = Native::from(self.run_interrupt(
                    memory,
                    i,
                    Address::from(self.mregister(Cpu::PC)),
                )?);
                self.set_mregister(Cpu::PC, next_pc)
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
            self.mregister(Cpu::PC).0,
            self.mregister(Cpu::SP).0,
            instruction
        );
        let next_pc = self.execute(instruction, memory)?;
        self.set_mregister(Cpu::PC, Native::from(next_pc));
        Ok(())
    }

    pub fn full_step(&mut self, memory: &mut Bus) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.set_mregister(MachineRegister(0), self.mregister(Cpu::PC));
                self.set_register(Register(0), LispWord::fixnum(trap as i64));
                self.set_mregister(
                    Cpu::PC,
                    memory.read_word(
                        Address::from(self.mregister(Cpu::VBR)) + InterruptTableOffset::TRAP_VECTOR,
                    ),
                );
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
    use crate::ram::Ram;

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
        let memory = Ram::new(0x10000);
        let mut bus = Bus::new();
        bus.install(0x0..0x10000, Box::new(memory.ram_device))
            .unwrap();
        bus
    }

    #[test]
    fn test_halt() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.set_mregister(Cpu::PC, Native(0x1000));
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.mregister(Cpu::PC)),
            vec![Instruction::Halt],
        );
        cpu.full_step(&mut test_bus);
        assert_eq!(cpu.mregister(Cpu::PC), Native(0x1010));
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.set_mregister(Cpu::PC, Native(0x1000));
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.mregister(Cpu::PC)),
            vec![Instruction::Nop],
        );
        cpu.full_step(&mut test_bus);
        assert_eq!(cpu.mregister(Cpu::PC), Native(0x1010));
        assert!(!cpu.halted);
        Ok(())
    }

    #[test]
    fn test_multiple_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.set_mregister(Cpu::PC, Native(0x1000));
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.mregister(Cpu::PC)),
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
        assert_eq!(cpu.mregister(Cpu::PC), Native(0x10a0));
        Ok(())
    }

    // #[test]
    // fn test_jump_adr() -> Result<(), String> {
    //     let mut cpu = Cpu::default();
    //     let mut memory = TestMemory::new();
    //     cpu.set_mregister(Cpu::PC, Address(0x1000));
    //     cpu.machine_reg[0] = Address(0x2000);
    //     TestMemory::load_instructions(
    //         &mut memory,
    //         cpu.mregister(Cpu::PC),
    //         parse_asm! {
    //             JUMP [A 0];
    //         },
    //     );
    //     cpu.full_step(memory.as_mut_slice());
    //     assert_eq!(cpu.mregister(Cpu::PC), Address(0x2000));
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
    //         cpu.register[Register(0)],
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
