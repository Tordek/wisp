pub mod assembler;
mod encoding;

use std::ops::Add;

use crate::memory::{Address, Memory, Offset};
use int_enum::IntEnum;

type WordSize = u64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Count(u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Native(pub u64);

impl std::ops::Add<Native> for Native {
    type Output = Native;

    fn add(self, rhs: Native) -> Native {
        Native(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Native> for Native {
    type Output = Native;

    fn sub(self, rhs: Native) -> Native {
        Native(self.0.wrapping_sub_signed(rhs.0 as i64))
    }
}

impl From<u64> for Native {
    fn from(value: u64) -> Self {
        Native(value)
    }
}

impl From<Native> for u64 {
    fn from(value: Native) -> Self {
        value.0
    }
}

impl From<Address> for Native {
    fn from(value: Address) -> Self {
        Native(value.0)
    }
}

impl From<Native> for Address {
    fn from(value: Native) -> Self {
        Address(value.0)
    }
}

impl Memory {
    fn read_word(&self, addr: Address) -> Native {
        Native(u64::from_le_bytes(
            self.bytes[(addr.0 as usize)..][..8]
                .try_into()
                .expect("Out of bands memory access."),
        ))
    }
    fn write_word(&mut self, addr: Address, data: Native) {
        self.bytes[(addr.0 as usize)..][..8].copy_from_slice(&data.0.to_le_bytes())
    }
}

pub enum MemoryLayout {}
impl MemoryLayout {
    // Standard addresses.
    pub const NIL_ROOT: Address = Address(0x0000000000000000);
    pub const T_ROOT: Address = Address(0x0000000000000008);

    pub const RESET_VECTOR: Address = Address(0x000000000007ef0);
    pub const INTERRUPT_TABLE: Address = Address(0x000000000007f00);
    pub const CPU_RESERVED_END: Address = Address(0x000000000018000);
}

pub enum InterruptTableOffset {}
impl InterruptTableOffset {
    pub const TRAP_VECTOR: Offset = Offset(0x00);
    pub const ALLOC_VECTOR: Offset = Offset(0x10);
    pub const ALLOC_CONS_VECTOR: Offset = Offset(0x018);
    pub const END_RESERVED_INTERRUPTS: Offset = Offset(0xf0);
}

pub enum SymbolLayout {}
impl SymbolLayout {
    pub const NAME_OFFSET: Offset = Offset(0);
    pub const PLIST_OFFSET: Offset = Offset(8);
}

pub enum StringLayout {}
impl StringLayout {
    pub const LENGTH_OFFSET: Offset = Offset(0);
    pub const DATA_OFFSET: Offset = Offset(8);
}

pub enum ConsLayout {}
impl ConsLayout {
    pub const CAR_OFFSET: Offset = Offset(0);
    pub const CDR_OFFSET: Offset = Offset(8);
}

#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy, IntEnum, Debug)]
pub enum WordType {
    Undefined,
    Fixnum,
    Symbol,
    Cons,
    Function,
    Closure,
    Character,
    String,
    Vector,
    Float,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct LispWord(u64);

impl LispWord {
    pub fn undefined() -> LispWord {
        LispWord::new(WordType::Undefined, 0)
    }

    pub fn tag(&self) -> WordType {
        WordType::try_from((self.0 >> 56) as u8).unwrap()
    }

    pub const fn payload(&self) -> u64 {
        self.0 & 0x00ff_ffff_ffff_ffff
    }

    pub const fn new(tag: WordType, payload: WordSize) -> Self {
        Self(((tag as u8) as u64) << 56 | (payload & 0x00ff_ffff_ffff_ffff))
    }

    // Convenience methods
    pub const fn symbol(value: WordSize) -> Self {
        Self::new(WordType::Symbol, value)
    }

    pub const fn fixnum(value: WordSize) -> Self {
        Self::new(WordType::Fixnum, value)
    }

    pub const fn cons(address: WordSize) -> Self {
        Self::new(WordType::Cons, address)
    }

    pub const fn char(address: WordSize) -> Self {
        Self::new(WordType::Character, address & 0xff)
    }

    fn ensure(self, word_type: WordType) -> Result<Self, Trap> {
        if self.tag() == word_type {
            Ok(self)
        } else {
            Err(Trap::TypeError)
        }
    }

    fn as_fixnum(self) -> Result<u64, Trap> {
        Ok(self.ensure(WordType::Fixnum)?.payload())
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
    registers: [LispWord; 16],

    /// Machine registers
    machine_reg: [Native; 8],

    interrupt: Option<u64>,

    pub halted: bool,
}
impl Cpu {
    pub const SP: MachineRegister = MachineRegister(5);
    pub const PC: MachineRegister = MachineRegister(6);
    pub const ENV: MachineRegister = MachineRegister(7);
    pub const WORD_SIZE: u64 = 8;
    pub const INSTRUCTION_SIZE: u64 = 2 * Self::WORD_SIZE;

    pub fn interrupt(&mut self, int: u64) {
        self.interrupt = Some(int);
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            registers: [LispWord::undefined(); 16],
            machine_reg: [Native(0); 8],
            halted: false,
            interrupt: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Register(pub u64);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MachineRegister(pub u64);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ThreeRegs {
    pub dst: Register,
    pub op1: Register,
    pub op2: Option<Register>,
    pub op3: Option<LispWord>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ThreeMachs {
    pub dst: MachineRegister,
    pub op1: MachineRegister,
    pub op2: Option<MachineRegister>,
    pub op3: Option<Native>,
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
        operands: ThreeMachs,
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

    // Higher level
    // Cons { -- CONS does not exist - it is only an alias for INT 0x02
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
        op1: Register,
        op2: Option<Register>,
        op3: Option<LispWord>,
    },
    Binary {
        op: BinaryOp,
        operands: ThreeRegs,
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
    /// while (count--) *dst++ = *src++
    MemCpy {
        dst: MachineRegister,
        src: MachineRegister,
        count: Count,
    },
    /// while (count--) *dst++ = src
    MemSet {
        dst: MachineRegister,
        src: MachineRegister,
        count: Count,
    },

    /// PUSHes PC on the stack and jumps
    Call {
        target: JumpTarget,
    },

    /// POPs PC from the stack
    Return,
}

impl Cpu {
    pub fn reset(&mut self, memory: &mut Memory) {
        self.machine_reg[Cpu::PC.0 as usize] = memory.read_word(MemoryLayout::RESET_VECTOR);
    }

    fn fetch(&self, memory: &Memory) -> (Native, Native) {
        let instruction_low =
            memory.read_word(Address(Address(self.machine_reg[Cpu::PC.0 as usize].0).0));
        let instruction_high = memory.read_word(
            Address(Address(self.machine_reg[Cpu::PC.0 as usize].0).0)
                + Offset(Self::WORD_SIZE as i64),
        );
        (instruction_low, instruction_high)
    }

    fn read_word(memory: &Memory, address: Address) -> Result<LispWord, Trap> {
        memory
            .read_word(address)
            .try_into()
            .map_err(|_| Trap::TypeError)
    }

    fn read_register_as(
        &self,
        Register(r): Register,
        word_type: WordType,
    ) -> Result<LispWord, Trap> {
        let src_obj = self.registers[r as usize];

        if src_obj.tag() != word_type {
            return Err(Trap::TypeError);
        }

        Ok(src_obj)
    }

    fn nil(memory: &Memory) -> Result<LispWord, Trap> {
        Self::read_word(memory, MemoryLayout::NIL_ROOT)
    }
    fn t(memory: &Memory) -> Result<LispWord, Trap> {
        Self::read_word(memory, MemoryLayout::T_ROOT)
    }

    fn to_machine_bool(memory: &mut Memory, val: bool) -> Result<LispWord, Trap> {
        if val {
            Self::t(memory)
        } else {
            Self::nil(memory)
        }
    }

    fn push(&mut self, memory: &mut Memory, val: Native) {
        memory.write_word(Address::from(self.machine_reg[Cpu::SP.0 as usize]), val);
        self.machine_reg[Cpu::SP.0 as usize] =
            (Address::from(self.machine_reg[Cpu::SP.0 as usize]) - Offset(Self::WORD_SIZE as i64))
                .into();
    }

    fn pop(&mut self, memory: &mut Memory) -> Native {
        self.machine_reg[Cpu::SP.0 as usize] =
            (Address::from(self.machine_reg[Cpu::SP.0 as usize]) + Offset(Self::WORD_SIZE as i64))
                .into();
        memory.read_word(Address::from(self.machine_reg[Cpu::SP.0 as usize]))
    }

    fn pop_word(&mut self, memory: &mut Memory) -> Result<LispWord, Trap> {
        self.machine_reg[Cpu::SP.0 as usize] =
            (Address::from(self.machine_reg[Cpu::SP.0 as usize]) + Offset(Self::WORD_SIZE as i64))
                .into();
        Self::read_word(memory, Address::from(self.machine_reg[Cpu::SP.0 as usize]))
    }

    fn get_offset_reg_val_unchecked(
        &self,
        base: Option<Register>,
        off: Option<LispWord>,
    ) -> LispWord {
        let mut add = 0;
        if let Some(reg) = base {
            add += self.registers[reg.0 as usize].payload()
        }
        if let Some(off) = off {
            add += off.0
        }
        LispWord::fixnum(add)
    }
    fn get_offset_reg_val(
        &self,
        base: Option<Register>,
        off: Option<LispWord>,
    ) -> Result<LispWord, Trap> {
        let mut add = 0;
        if let Some(reg) = base {
            add += self.registers[reg.0 as usize].as_fixnum()?
        }
        if let Some(off) = off {
            add += LispWord::try_from(off)
                .map_err(|_| Trap::TypeError)?
                .as_fixnum()?
        }
        Ok(LispWord::fixnum(add))
    }

    fn get_offset_addr_val(&self, base: Option<MachineRegister>, off: Option<Native>) -> Native {
        let mut add = 0;
        if let Some(reg) = base {
            add += self.machine_reg[reg.0 as usize].0
        }
        if let Some(off) = off {
            add = add + off.0
        }
        Native(add)
    }

    fn run_interrupt(
        &mut self,
        memory: &mut Memory,
        interruption: u64,
        next_pc: Address,
    ) -> Result<Address, Trap> {
        self.interrupt = None;
        match interruption {
            0x03 => {
                self.push(memory, self.registers[0 as usize].into());
                self.push(memory, self.registers[1 as usize].into());
                // CONS takes its params as R0 and R1
                self.registers[0 as usize] = LispWord::new(WordType::Fixnum, 2); // Size: 2
                self.registers[1 as usize] = LispWord::new(WordType::Fixnum, 2);
                // Type: Int
            }
            _ => (),
        }

        self.push(memory, Native(interruption));
        self.push(memory, Native(next_pc.0));
        let location = Address::from(memory.read_word(
            MemoryLayout::INTERRUPT_TABLE + Offset(interruption as i64 * Self::WORD_SIZE as i64),
        ));
        Ok(location)
    }

    fn get_jump_addr(&self, memory: &Memory, src: JumpTarget) -> Address {
        match src {
            JumpTarget::Absolute(v) => v.into(),
            JumpTarget::Register(Register(r)) => Address(self.registers[r as usize].payload()),
            JumpTarget::Machine(MachineRegister(pos), off) => {
                (Address::from(self.machine_reg[pos as usize]) + off).into()
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

    fn from_location(&self, memory: &Memory, src: Location) -> Native {
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

    fn execute(&mut self, instruction: Instruction, memory: &mut Memory) -> Result<Address, Trap> {
        match instruction {
            // Control flow
            Instruction::Halt => {
                self.halted = true;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0))
            }
            Instruction::Nop => Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                + Offset(Cpu::INSTRUCTION_SIZE as i64)),

            Instruction::Jump { condition, target } => {
                let nil = Self::nil(memory)?;

                let should_jump = match condition {
                    Condition::Always => true,
                    Condition::False(Register(r)) => self.registers[r as usize] == nil,
                    Condition::True(Register(r)) => self.registers[r as usize] != nil,
                };

                if should_jump {
                    Ok(Address::from(self.get_jump_addr(memory, target)))
                } else {
                    Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                        + Offset(Cpu::INSTRUCTION_SIZE as i64))
                }
            }

            Instruction::Call { target } => {
                let next_pc = Address::from(self.machine_reg[Cpu::PC.0 as usize])
                    + Offset(Cpu::INSTRUCTION_SIZE as i64);
                let address = Address::from(self.get_jump_addr(memory, target));

                self.push(memory, Native::from(next_pc));
                Ok(address)
            }

            Instruction::Return => {
                let return_address = self.pop(memory);
                Ok(Address::from(return_address))
            }

            Instruction::MakeClosure { dst, code } => todo!(),

            // Comparison
            Instruction::Binary {
                op,
                operands: ThreeRegs { dst, op1, op2, op3 },
            } => {
                let op1_obj = self.registers[op1.0 as usize];
                let op2_obj = self.get_offset_reg_val_unchecked(op2, op3);

                let result = match op {
                    BinaryOp::Add => LispWord::fixnum(op1_obj.as_fixnum()? + op2_obj.as_fixnum()?),
                    BinaryOp::Sub => LispWord::fixnum(op1_obj.as_fixnum()? - op2_obj.as_fixnum()?),
                    BinaryOp::Mul => LispWord::fixnum(op1_obj.as_fixnum()? * op2_obj.as_fixnum()?),
                    BinaryOp::Eq => Self::to_machine_bool(memory, op1_obj == op2_obj)?,
                    BinaryOp::Ne => Self::to_machine_bool(memory, op1_obj != op2_obj)?,
                    BinaryOp::Gt => {
                        Self::to_machine_bool(memory, op1_obj.as_fixnum()? > op2_obj.as_fixnum()?)?
                    }
                    BinaryOp::Gte => {
                        Self::to_machine_bool(memory, op1_obj.as_fixnum()? >= op2_obj.as_fixnum()?)?
                    }
                    BinaryOp::Lt => {
                        Self::to_machine_bool(memory, op1_obj.as_fixnum()? < op2_obj.as_fixnum()?)?
                    }
                    BinaryOp::Lte => {
                        Self::to_machine_bool(memory, op1_obj.as_fixnum()? <= op2_obj.as_fixnum()?)?
                    }
                    BinaryOp::Shl => LispWord::fixnum(op1_obj.as_fixnum()? << op2_obj.as_fixnum()?),
                    BinaryOp::Shr => LispWord::fixnum(op1_obj.as_fixnum()? >> op2_obj.as_fixnum()?),
                };

                self.registers[dst.0 as usize] = result;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }

            // Cons
            Instruction::Int(interruption) => {
                let next_pc = Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64);
                self.run_interrupt(memory, interruption, next_pc)
            }

            Instruction::IReturn => {
                let return_address = self.pop(memory);

                let interrupt = self.pop(memory);
                match interrupt.0 {
                    3 => {
                        let cdr = self.pop(memory);
                        let car = self.pop(memory);
                        memory.write_word(
                            Address::from(self.machine_reg[0 as usize]) + ConsLayout::CAR_OFFSET,
                            car,
                        );
                        memory.write_word(
                            Address::from(self.machine_reg[0 as usize]) + ConsLayout::CDR_OFFSET,
                            cdr,
                        );
                        self.registers[0] =
                            LispWord::cons(self.machine_reg[0 as usize].0 as WordSize);
                    }
                    _ => {}
                }
                Ok(Address::from(return_address))
            }

            Instruction::Car(TwoRegs { dst, src }) => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET)?;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::Cdr(TwoRegs { dst, src }) => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET)?;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::SetCar(TwoRegs { dst, src }) => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[src.0 as usize];

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CAR_OFFSET,
                    val_obj.into(),
                );
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::SetCdr(TwoRegs { dst, src }) => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[src.0 as usize];

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CDR_OFFSET,
                    val_obj.into(),
                );
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[car.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET)?;
                self.registers[cdr.0 as usize] =
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET)?;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }

            Instruction::IDiv {
                div,
                rem,
                op1,
                op2,
                op3,
            } => {
                let op1_obj = self.registers[op1.0 as usize].ensure(WordType::Fixnum)?;
                let op2_obj = self.get_offset_reg_val(op2, op3)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[div.0 as usize] =
                    LispWord::new(WordType::Fixnum, op1_obj.payload() / op2_obj.payload());
                self.registers[rem.0 as usize] =
                    LispWord::new(WordType::Fixnum, op1_obj.payload() % op2_obj.payload());
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }

            Instruction::PopR { dst } => {
                let result = self.pop_word(memory)?;
                self.registers[dst.0 as usize] = result;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::PushR { src } => {
                self.push(memory, self.registers[src.0 as usize].into());
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::PopA { dst } => {
                let result = self.pop(memory);
                self.machine_reg[dst.0 as usize] = result;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::PushA { src } => {
                self.push(memory, self.machine_reg[src.0 as usize]);
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }

            Instruction::MBinary {
                op,
                operands: ThreeMachs { dst, op1, op2, op3 },
            } => {
                let val1 = self.machine_reg[op1.0 as usize];
                let val2 = self.get_offset_addr_val(op2, op3);
                let result = match op {
                    MBinaryOp::Add => val1 + val2,
                    MBinaryOp::Sub => val1 - val2,
                };
                self.machine_reg[dst.0 as usize] = result;
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::GetPayload { dst, src } => {
                self.machine_reg[dst.0 as usize] = Native(self.registers[src.0 as usize].payload());
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::GetTag { src, dst } => {
                self.machine_reg[dst.0 as usize] =
                    Native(self.registers[src.0 as usize].tag() as u64);
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::SetPayload { dst, src } => {
                self.registers[dst.0 as usize] = LispWord::new(
                    self.registers[dst.0 as usize].tag(),
                    self.machine_reg[src.0 as usize].0,
                );
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::SetTag { src, dst } => {
                self.registers[dst.0 as usize] = LispWord::new(
                    WordType::try_from(self.machine_reg[src.0 as usize].0 as u8)
                        .map_err(|_| Trap::TypeError)?,
                    self.registers[dst.0 as usize].payload(),
                );
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::Mov8 { dst, src } => {
                let value = self.from_location(memory, src).0 as u8;
                match dst {
                    Location::Literal(a) => return Err(Trap::InvalidInstruction), // Makes no sense to move into a literal.
                    Location::Absolute(a) => memory.bytes[a.0 as usize] = value,
                    Location::Machine(MachineRegister(r)) => {
                        self.machine_reg[(r as i64) as usize] = Native(value as u64)
                    }
                    Location::Register(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.bytes
                            [(Address::from(self.machine_reg[r as usize]) + off).0 as usize] = value
                    }
                    Location::IndirectRegister(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                }
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::Mov { dst, src } => {
                let value = self.from_location(memory, src);
                match dst {
                    Location::Literal(a) => return Err(Trap::InvalidInstruction), // Makes no sense to move into a literal.
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
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::MemCpy { dst, src, count } => {
                let srcadd = self.machine_reg[src.0 as usize];
                let dstadd = self.machine_reg[dst.0 as usize];
                for i in 0..count.0 {
                    memory.bytes[dstadd.0 as usize + i as usize] =
                        memory.bytes[srcadd.0 as usize + i as usize]
                }
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
            Instruction::MemSet { dst, src, count } => {
                let srcadd = self.machine_reg[src.0 as usize];
                let dstadd = self.machine_reg[dst.0 as usize];
                for i in 0..count.0 {
                    memory.bytes[dstadd.0 as usize + i as usize] = memory.bytes[srcadd.0 as usize]
                }
                Ok(Address(self.machine_reg[Cpu::PC.0 as usize].0)
                    + Offset(Cpu::INSTRUCTION_SIZE as i64))
            }
        }
    }

    pub fn step(&mut self, memory: &mut Memory) -> Result<(), Trap> {
        let (lo, hi) = self.fetch(memory);
        let instruction = Instruction::decode(lo.0, hi.0)?;

        if instruction != Instruction::Halt {
            println!(
                "0x{:x} {:?}",
                Address(self.machine_reg[Cpu::PC.0 as usize].0).0,
                instruction
            );
        }

        let next_pc = self.execute(instruction, memory)?;
        match self.interrupt {
            Some(i) => {
                Address(self.machine_reg[Cpu::PC.0 as usize].0) =
                    self.run_interrupt(memory, i, next_pc)?
            }
            None => {
                Address(self.machine_reg[Cpu::PC.0 as usize].0) = next_pc;
            }
        };
        Ok(())
    }

    pub fn full_step(&mut self, memory: &mut Memory) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.machine_reg[0 as usize] = self.machine_reg[Cpu::PC.0 as usize];
                self.registers[0 as usize] = LispWord::new(WordType::Fixnum, trap as WordSize);
                self.machine_reg[Cpu::PC.0 as usize] = memory
                    .read_word(MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR)
            }
        };
    }
}

#[derive(Debug)]
pub enum Trap {
    TypeError,
    InvalidInstruction,
    Unimplemented,
}

// #[cfg(test)]
// mod tests {
//     use crate::parse_asm;

//     use super::*;

//     struct TestMemory {}

//     impl TestMemory {
//         fn new() -> Vec<u8> {
//             let mut data = vec![0 as u8; 0x10000];
//             data.as_mut_slice().write_word(0, Word::symbol(0).into());
//             data.as_mut_slice().write_word(8, Word::symbol(1).into());
//             data
//         }

//         fn load_instructions(data: &mut Memory, start: Address, instructions: Vec<Instruction>) {
//             let mut pos = 0;
//             for instr in instructions {
//                 let (lo, hi) = instr.encode();
//                 data.write_word((start + Offset(pos * Cpu::Self::WORD_SIZE as i64)).0, lo);
//                 pos += 1;
//                 data.write_word((start + Offset(pos * Cpu::Self::WORD_SIZE as i64)).0, hi);
//                 pos += 1;
//             }
//         }
//     }

//     #[test]
//     fn test_halt() -> Result<(), String> {
//         let mut cpu = Cpu::default();
//         let mut memory = TestMemory::new();
//         cpu.address[Cpu::PC.0] = Address(0x1000);
//         TestMemory::load_instructions(&mut memory, cpu.address[Cpu::PC.0], vec![Instruction::Halt]);
//         cpu.full_step(memory.as_mut_slice());
//         assert_eq!(cpu.address[Cpu::PC.0], Address(0x1000));
//         assert!(cpu.halted);
//         Ok(())
//     }

//     #[test]
//     fn test_nop() -> Result<(), String> {
//         let mut cpu = Cpu::default();
//         let mut memory = TestMemory::new();
//         cpu.address[Cpu::PC.0] = Address(0x1000);
//         TestMemory::load_instructions(&mut memory, cpu.address[Cpu::PC.0], vec![Instruction::Nop]);
//         cpu.full_step(memory.as_mut_slice());
//         assert_eq!(cpu.address[Cpu::PC.0], Address(0x1010));
//         Ok(())
//     }

//     #[test]
//     fn test_multiple_nop() -> Result<(), String> {
//         let mut cpu = Cpu::default();
//         let mut memory = TestMemory::new();
//         cpu.address[Cpu::PC.0] = Address(0x1000);
//         TestMemory::load_instructions(
//             &mut memory,
//             cpu.address[Cpu::PC.0],
//             parse_asm! {
//                 NOP;
//                 NOP;
//                 NOP;
//                 NOP;
//                 NOP;
//                 NOP;
//                 NOP;
//                 NOP;
//                 HALT;
//             },
//         );

//         while !cpu.halted {
//             cpu.full_step(memory.as_mut_slice());
//         }
//         assert_eq!(cpu.address[Cpu::PC.0], Address(0x1080));
//         assert!(cpu.halted);
//         Ok(())
//     }

//     #[test]
//     fn test_jump_adr() -> Result<(), String> {
//         let mut cpu = Cpu::default();
//         let mut memory = TestMemory::new();
//         cpu.address[Cpu::PC.0] = Address(0x1000);
//         cpu.address[0] = Address(0x2000);
//         TestMemory::load_instructions(
//             &mut memory,
//             cpu.address[Cpu::PC.0],
//             parse_asm! {
//                 JUMP [A 0];
//             },
//         );
//         cpu.full_step(memory.as_mut_slice());
//         assert_eq!(cpu.address[Cpu::PC.0], Address(0x2000));
//         Ok(())
//     }

//     #[test]
//     fn test_cons_builds_cell() -> Result<(), String> {
//         let mut cpu = Cpu::default();
//         let mut memory = TestMemory::new();

//         let cons_hook = 0x2000;
//         let trap_hook = 0x2100;
//         let cons_cell = 0x3000;
//         let code_base = 0x4000;

//         Cpu::write_word(&mut memory, MemoryLayout::RESET_VECTOR, code_base);
//         Cpu::write_word(
//             &mut memory,
//             MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_VECTOR,
//             cons_hook,
//         );
//         Cpu::write_word(
//             &mut memory,
//             MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_CONS_VECTOR,
//             cons_hook,
//         );
//         Cpu::write_word(
//             &mut memory,
//             MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR,
//             trap_hook,
//         );
//         cpu.reset(&mut memory);

//         TestMemory::load_instructions(
//             &mut memory,
//             code_base as u64,
//             parse_asm! {
//                 MOV A Cpu::SP, 0x800;
//                 MOV R 0, Word::fixnum(42);
//                 MOV R 1, Word::fixnum(99);
//                 CONS;
//                 NOP;
//                 HALT;
//             },
//         );

//         // Fake CONS_HOOK
//         TestMemory::load_instructions(
//             &mut memory,
//             cons_hook,
//             parse_asm! {
//                 MOV A 0, cons_cell;
//                 IRETURN;
//             },
//         );

//         TestMemory::load_instructions(
//             &mut memory,
//             trap_hook,
//             parse_asm! {
//                 HALT;
//             },
//         );

//         cpu.reset(&mut memory);
//         while !cpu.halted {
//             cpu.full_step(memory.as_mut_slice());
//         }

//         assert_eq!(
//             cpu.registers[0 as usize],
//             Word::new(WordType::Cons, cons_cell as u64)
//         );

//         assert_eq!(
//             Word::try_from(memory.as_mut_slice().read_word(cons_cell))
//                 .expect("couldn't parse word"),
//             Word::new(WordType::Fixnum, 42)
//         );

//         assert_eq!(
//             Word::try_from(memory.as_mut_slice().read_word(cons_cell + Self::WORD_SIZE))
//                 .expect("couldn't parse word"),
//             Word::new(WordType::Fixnum, 99).into()
//         );

//         assert!(cpu.halted);

//         Ok(())
//     }
// }
