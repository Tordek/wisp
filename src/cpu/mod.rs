mod assembler;
mod encoding;

use crate::memory::Memory;
use int_enum::IntEnum;

type Address = usize;
type WordSize = u64;
const WORD_SIZE: usize = 8;

pub enum MemoryLayout {}
impl MemoryLayout {
    // Standard addresses.
    pub const NIL_ROOT: Address = 0x0000000000000000;
    pub const T_ROOT: Address = 0x0000000000000008;

    pub const RESET_VECTOR: Address = 0x000000000007ef0;
    pub const INTERRUPT_TABLE: Address = 0x000000000007f00;
    pub const CPU_RESERVED_END: Address = 0x000000000018000;
}

pub enum InterruptTableOffset {}
impl InterruptTableOffset {
    pub const TRAP_VECTOR: Address = 0x00000000000000;
    pub const ALLOC_VECTOR: Address = 0x10;
    pub const ALLOC_CONS_VECTOR: Address = 0x018;
    pub const END_RESERVED_INTERRUPTS: Address = 0xf0;
}

pub enum SymbolLayout {}
impl SymbolLayout {
    pub const NAME_OFFSET: Address = 0;
    pub const PLIST_OFFSET: Address = 8;
}

pub enum StringLayout {}
impl StringLayout {
    pub const LENGTH_OFFSET: Address = 0;
    pub const DATA_OFFSET: Address = 8;
}

pub enum ConsLayout {}
impl ConsLayout {
    pub const CAR_OFFSET: Address = 0;
    pub const CDR_OFFSET: Address = 8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Root {
    NIL,
    T,
    CONS,
    FIXNUM,
    SYMBOL,
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
pub struct Word {
    tag: WordType,
    payload: WordSize,
}

impl Word {
    pub fn undefined() -> Word {
        Word {
            tag: WordType::Undefined,
            payload: 0,
        }
    }

    pub const fn new(tag: WordType, payload: WordSize) -> Self {
        Self {
            tag,
            payload: payload & 0x00ff_ffff_ffff_ffff,
        }
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
}

#[derive(Debug)]
pub enum WordParseError {
    InvalidPrefix,
}
impl TryFrom<WordSize> for Word {
    fn try_from(value: WordSize) -> Result<Self, Self::Error> {
        const DATA_MASK: WordSize = 0x00FF_FFFF_FFFF_FFFF;
        let raw_tag = value >> 56;
        let payload = value & DATA_MASK;
        Ok(Word {
            tag: WordType::try_from(raw_tag as u8).map_err(|_| WordParseError::InvalidPrefix)?,
            payload,
        })
    }
    type Error = WordParseError;
}
impl From<Word> for WordSize {
    fn from(value: Word) -> WordSize {
        let tag = value.tag;
        let payload = value.payload;

        (tag as u8 as WordSize) << 56 | payload
    }
}

#[derive(Debug)]
pub struct Cpu {
    /// Common registers.
    registers: [Word; 16],

    /// Machine registers
    address: [Address; 8],

    pub halted: bool,
}
impl Cpu {
    pub const SP: Address = 5;
    pub const PC: Address = 6;
    pub const ENV: Address = 7;
    pub const WORD_SIZE: Address = 8;
    pub const INSTRUCTION_SIZE: Address = 2 * Self::WORD_SIZE;
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            registers: [Word::undefined(); 16],
            address: [0; 8],
            halted: false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Register(pub Address);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct MachineRegister(pub Address);

#[derive(Debug, PartialEq, Eq)]
pub enum JumpAddressing {
    /// Jumping relative to current PC is done by JUMPing to [adr=PC=0x06]+offset
    MachineRegister {
        adr: Option<MachineRegister>,
        offset: i64,
    },
    Register {
        adr: Register,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct ThreeRegs {
    pub dst: Register,
    pub op1: Register,
    pub op2: Option<Register>,
    pub imm: Word,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ThreeMachs {
    pub dst: MachineRegister,
    pub op1: MachineRegister,
    // TODO: Convert to OffsetAddress.
    pub op2: Option<MachineRegister>,
    pub imm: Address,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OffsetAddress {
    base: Option<MachineRegister>,
    off: Address,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TwoRegs {
    dst: Register,
    src: Register,
}

#[derive(Debug)]
pub struct TwoMachs {
    src: MachineRegister,
    dst: MachineRegister,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Location {
    Register(Register),
    Machine(MachineRegister),
    IndirectRegister(Register),
    IndirectMachine(MachineRegister, i64),
    Absolute(Address),
}

pub struct ByteAddressing {
    base: MachineRegister,
    offset: MachineRegister,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Instruction {
    // Lower level
    /// Halts execution
    Halt,

    /// Do nothing.
    Nop,

    /// Jumps unconditionally
    Jump {
        target: JumpAddressing,
    },

    /// Jump if its Register is 't
    JumpIf {
        condition: Register,
        target: JumpAddressing,
    },

    /// Jump if its Register is 'nil
    JumpIfNot {
        condition: Register,
        target: JumpAddressing,
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
    /// Load a literal WORD onto a register. Validate it.
    LoadLiteral {
        dst: Register,
        val: Word,
    },
    /// Load a raw Word
    LoadMachine {
        dst: MachineRegister,
        val: Address,
    },
    Mov {
        dst: Location,
        src: Location,
    },
    Mov8 {
        dst: Location,
        src: Location,
    },
    AAdd(ThreeMachs),
    ASub(ThreeMachs),
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
    /// Calculates dst <- op1 + (op2 + imm)
    Add(ThreeRegs),
    /// Calculates dst <- op1 * (op2 + imm)
    Mul(ThreeRegs),
    /// Calculates div <- op1 / (op2 + imm), rem <- op1 % (op2 + imm)
    IDiv {
        div: Register,
        rem: Register,
        op1: Register,
        op2: Option<Register>,
        imm: Word,
    },
    /// Calculates dst <- op1 - (op2 + imm) - typechecks, only fixnums.
    Sub(ThreeRegs),
    /// Calculates dst <- op1 == (op2 + imm)
    ///! It traps if imm is not 0 when the operands are not fixnum.
    Eq(ThreeRegs),
    /// Calculates dst <- op1 !- (op2 + imm)
    ///! It traps if imm is not 0 when the operands are not fixnum.
    Ne(ThreeRegs),
    /// Calculates dst <- op1 > (op2 + imm) - typechecks, only fixnums.
    Gt(ThreeRegs),
    /// Calculates dst <- op1 < (op2 + imm) - typechecks, only fixnums.
    Gte(ThreeRegs),
    /// Calculates dst <- op1 >= (op2 + imm) - typechecks, only fixnums.
    Lt(ThreeRegs),
    /// Calculates dst <- op1 <= (op2 + imm) - typechecks, only fixnums.
    Lte(ThreeRegs),
    // Mov8 {
    //     dst: ByteAddressing,
    //     src: ByteAddressing,
    // },
    /// I have no idea what this does yet.
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
        count: u64,
    },
    /// while (count--) *dst++ = src
    MemSet {
        dst: MachineRegister,
        src: MachineRegister,
        count: u64,
    },

    /// PUSHes PC on the stack and jumps
    Call {
        target: JumpAddressing,
    },

    /// POPs PC from the stack
    Return,
}

impl Cpu {
    pub fn reset(&mut self, memory: &mut [u8]) {
        self.address[Cpu::PC] = memory.read_word(MemoryLayout::RESET_VECTOR) as Address;
    }

    fn fetch(&self, memory: &[u8]) -> (WordSize, WordSize) {
        let instruction_low = memory.read_word(self.address[Cpu::PC]);
        let instruction_high = memory.read_word(self.address[Cpu::PC] + WORD_SIZE);
        (instruction_low, instruction_high)
    }

    fn read_word(memory: &[u8], address: Address) -> Result<Word, Trap> {
        memory
            .read_word(address as Address)
            .try_into()
            .map_err(|_| Trap::TypeError)
    }

    fn read_register_as(&self, Register(r): Register, word_type: WordType) -> Result<Word, Trap> {
        let src_obj = self.registers[r];

        if src_obj.tag != word_type {
            return Err(Trap::TypeError);
        }

        Ok(src_obj)
    }

    fn nil(memory: &[u8]) -> Result<Word, Trap> {
        Self::read_word(memory, MemoryLayout::NIL_ROOT)
    }
    fn t(memory: &[u8]) -> Result<Word, Trap> {
        Self::read_word(memory, MemoryLayout::T_ROOT)
    }

    fn to_machine_bool(memory: &mut [u8], val: bool) -> Result<Word, Trap> {
        if val {
            Self::t(memory)
        } else {
            Self::nil(memory)
        }
    }

    fn push(&mut self, memory: &mut [u8], val: WordSize) {
        memory.write_word(self.address[Cpu::SP], val);
        self.address[Cpu::SP] -= WORD_SIZE;
    }

    fn pop(&mut self, memory: &mut [u8]) -> WordSize {
        self.address[Cpu::SP] += WORD_SIZE;
        memory.read_word(self.address[Cpu::SP])
    }
    fn pop_word(&mut self, memory: &mut [u8]) -> Result<Word, Trap> {
        self.address[Cpu::SP] += WORD_SIZE;
        Self::read_word(memory, self.address[Cpu::SP])
    }

    fn reg_with_offset(&self, op: Option<Register>, off: Word) -> Result<Word, Trap> {
        match (op, off) {
            (None, off) => Ok(off),
            (
                Some(r),
                Word {
                    tag: WordType::Fixnum,
                    payload: 0,
                },
            ) => Ok(self.registers[r.0]),
            (
                Some(r),
                Word {
                    tag: WordType::Fixnum,
                    payload: offset,
                },
            ) => match self.registers[r.0] {
                Word {
                    tag: WordType::Fixnum,
                    payload,
                } => Ok(Word::new(WordType::Fixnum, payload + offset)),
                _ => Err(Trap::TypeError),
            },
            _ => Err(Trap::TypeError),
        }
    }

    fn addr_with_offset(&self, op: Option<MachineRegister>, off: i64) -> Address {
        match op {
            Some(MachineRegister(r)) => (self.address[r] as i64 + off) as Address,
            None => off as Address,
        }
    }

    fn ensure(w: Word, word_type: WordType) -> Result<Word, Trap> {
        if w.tag == word_type {
            Ok(w)
        } else {
            Err(Trap::TypeError)
        }
    }

    fn execute(&mut self, instruction: Instruction, memory: &mut [u8]) -> Result<Address, Trap> {
        match instruction {
            // Control flow
            Instruction::Halt => {
                self.halted = true;
                Ok(self.address[Cpu::PC])
            }
            Instruction::Nop => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),

            Instruction::Jump {
                target: JumpAddressing::MachineRegister { adr, offset },
            } => Ok(self.addr_with_offset(adr, offset)),
            Instruction::Jump {
                target: JumpAddressing::Register { adr },
            } => {
                todo!()
            }

            Instruction::JumpIf {
                condition,
                target: JumpAddressing::MachineRegister { adr, offset },
            } => {
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value == nil {
                    Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
                } else {
                    Ok(self.addr_with_offset(adr, offset))
                }
            }
            Instruction::JumpIf {
                condition,
                target: JumpAddressing::Register { adr },
            } => {
                todo!()
            }

            Instruction::JumpIfNot {
                condition,
                target: JumpAddressing::MachineRegister { adr, offset },
            } => {
                println!("===============");
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value != nil {
                    println!("No jump");
                    Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
                } else {
                    println!("jump");
                    Ok(self.addr_with_offset(adr, offset))
                }
            }
            Instruction::JumpIfNot {
                condition,
                target: JumpAddressing::Register { adr },
            } => {
                todo!()
            }

            Instruction::Call { target } => todo!(),

            Instruction::Return => {
                let return_address = self.pop(memory);
                Ok(return_address as Address)
            }
            Instruction::MakeClosure { dst, code } => todo!(),

            // Comparison
            Instruction::Eq(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = self.registers[op1.0];
                let op2_obj = match op2 {
                    Some(op) => match self.registers[op.0] {
                        Word {
                            tag: WordType::Fixnum,
                            payload,
                        } => Ok(Word::new(
                            WordType::Fixnum,
                            payload + Word::try_from(imm).map_err(|_| Trap::TypeError)?.payload,
                        )),
                        word => {
                            if imm == Word::new(WordType::Fixnum, 0) {
                                Ok(word)
                            } else {
                                Err(Trap::TypeError)
                            }
                        }
                    },
                    None => Word::try_from(imm).map_err(|_| Trap::TypeError),
                }?;

                self.registers[dst.0] = Self::to_machine_bool(memory, op1_obj == op2_obj)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Ne(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = self.registers[op1.0];
                let op2_obj = self.reg_with_offset(op2, imm)?;
                self.registers[dst.0] = Self::to_machine_bool(memory, op1_obj != op2_obj)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Gt(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload > op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Gte(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload >= op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Lt(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload < op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Lte(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload >= op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            // Cons
            Instruction::Int(interruption) => {
                let next_pc = self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE;
                match interruption {
                    0x03 => {
                        self.push(memory, self.registers[0].into());
                        self.push(memory, self.registers[1].into());
                        // CONS takes its params as R0 and R1
                        self.registers[0] = Word::new(WordType::Fixnum, 2).into(); // Size: 2
                        self.registers[1] = Word::new(WordType::Fixnum, 2).into();
                        // Type: Int
                    }
                    _ => (),
                }

                self.push(memory, interruption as WordSize);
                self.push(memory, next_pc as WordSize);
                let location = memory
                    .read_word(MemoryLayout::INTERRUPT_TABLE + interruption as Address * WORD_SIZE);
                Ok(location as Address)
            }
            Instruction::IReturn => {
                let return_address = self.pop(memory);

                let interrupt = self.pop(memory);
                if interrupt == 3 {
                    let cdr = self.pop(memory);
                    let car = self.pop(memory);
                    memory.write_word(self.address[0] + ConsLayout::CAR_OFFSET, car);
                    memory.write_word(self.address[0] + ConsLayout::CDR_OFFSET, cdr);
                    self.registers[0] = Word::cons(self.address[0] as WordSize);
                }
                Ok(return_address as Address)
            }
            Instruction::Car(TwoRegs { dst, src }) => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload as Address + ConsLayout::CAR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Cdr(TwoRegs { dst, src }) => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload as Address + ConsLayout::CDR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetCar(TwoRegs { dst, src }) => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[src.0];

                memory.write_word(
                    dst_obj.payload as Address + ConsLayout::CAR_OFFSET,
                    val_obj.into(),
                );
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetCdr(TwoRegs { dst, src }) => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[src.0];

                memory.write_word(
                    dst_obj.payload as Address + ConsLayout::CDR_OFFSET,
                    val_obj.into(),
                );
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[car.0] =
                    Self::read_word(memory, src_obj.payload as Address + ConsLayout::CAR_OFFSET)?;
                self.registers[cdr.0] =
                    Self::read_word(memory, src_obj.payload as Address + ConsLayout::CDR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            // Arithmetic
            Instruction::Add(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] =
                    Word::new(WordType::Fixnum, op1_obj.payload + op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Sub(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] =
                    Word::new(WordType::Fixnum, op1_obj.payload - op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Mul(ThreeRegs { dst, op1, op2, imm }) => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] =
                    Word::new(WordType::Fixnum, op1_obj.payload * op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::IDiv {
                div,
                rem,
                op1,
                op2,
                imm,
            } => {
                let op1_obj = Self::ensure(self.registers[op1.0], WordType::Fixnum)?;
                let op2_obj = self.reg_with_offset(op2, imm)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[div.0] =
                    Word::new(WordType::Fixnum, op1_obj.payload / op2_obj.payload);
                self.registers[rem.0] =
                    Word::new(WordType::Fixnum, op1_obj.payload % op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            Instruction::PopR { dst } => {
                let result = self.pop_word(memory)?;
                self.registers[dst.0] = result;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::PushR { src } => {
                self.push(memory, self.registers[src.0].into());
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::PopA { dst } => {
                let result = self.pop(memory);
                self.address[dst.0] = result as Address;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::PushA { src } => {
                self.push(memory, self.address[src.0] as WordSize);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            Instruction::LoadMachine { dst, val: address } => {
                self.address[dst.0] = address;

                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::AAdd(ThreeMachs { dst, op1, op2, imm }) => {
                let val1 = self.address[op1.0];
                let val2 = self.addr_with_offset(op2, imm as i64);
                self.address[dst.0] = val1 + val2;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::ASub(ThreeMachs { dst, op1, op2, imm }) => todo!(),
            Instruction::GetPayload { dst, src } => {
                self.address[dst.0] = u64::from(self.registers[src.0].payload) as usize;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::GetTag { src, dst } => todo!(),
            Instruction::LoadLiteral { dst, val } => {
                self.registers[dst.0] = val;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetPayload { dst, src } => todo!(),
            Instruction::SetTag { src, dst } => todo!(),
            Instruction::Mov8 { dst, src } => {
                let value = match src {
                    Location::Absolute(a) => memory.read_word(a as Address),
                    Location::Machine(MachineRegister(r)) => self.address[r] as WordSize,
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.read_word((self.address[r] as i64 + off) as Address)
                    }
                    Location::IndirectRegister(Register(r)) => {
                        return Err(Trap::InvalidInstruction);
                    }
                    Location::Register(Register(r)) => {
                        return Err(Trap::InvalidInstruction);
                    }
                };
                println!("src {:?}", src);
                println!("dst {:?}", dst);
                match dst {
                    Location::Absolute(a) => memory.write_word(a as Address, value),
                    Location::Machine(MachineRegister(r)) => self.address[r] = value as Address,
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.write_word((self.address[r] as i64 + off) as Address, value)
                    }
                    Location::IndirectRegister(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                    Location::Register(_) => {
                        return Err(Trap::InvalidInstruction);
                    }
                }
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Mov { dst, src } => {
                let value = match src {
                    Location::Absolute(a) => memory.read_word(a as Address),
                    Location::Machine(MachineRegister(r)) => self.address[r] as WordSize,
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.read_word((self.address[r] as i64 + off) as Address)
                    }
                    Location::IndirectRegister(Register(r)) => {
                        memory.read_word(self.registers[r].payload as Address)
                    }
                    Location::Register(Register(r)) => self.registers[r].into(),
                };
                match dst {
                    Location::Absolute(a) => memory.write_word(a as Address, value),
                    Location::Machine(MachineRegister(r)) => self.address[r] = value as Address,
                    Location::IndirectMachine(MachineRegister(r), off) => {
                        memory.write_word((self.address[r] as i64 + off) as Address, value)
                    }
                    Location::IndirectRegister(Register(r)) => {
                        memory.write_word(self.registers[r].payload as Address, value)
                    }
                    Location::Register(Register(r)) => {
                        self.registers[r] = Word::try_from(value).map_err(|_| Trap::TypeError)?
                    }
                }
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::MemCpy { dst, src, count } => {
                let srcadd = self.address[src.0];
                let dstadd = self.address[dst.0];
                for i in 0..count {
                    memory[srcadd + i as usize] = memory[dstadd + i as usize]
                }
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::MemSet { dst, src, count } => {
                let srcadd = self.address[src.0];
                let dstadd = self.address[dst.0];
                for i in 0..count {
                    memory[dstadd + i as usize] = memory[srcadd as usize]
                }
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
        }
    }

    pub fn step(&mut self, memory: &mut [u8]) -> Result<(), Trap> {
        let (lo, hi) = self.fetch(memory);
        let instruction = Instruction::decode(lo, hi)?;

        println!("0x{:x} {:?}", self.address[Cpu::PC], instruction);

        let next_pc = self.execute(instruction, memory)?;
        self.address[Cpu::PC] = next_pc;
        Ok(())
    }

    pub fn full_step(&mut self, memory: &mut [u8]) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.address[0] = self.address[Cpu::PC];
                self.registers[0] = Word::new(WordType::Fixnum, trap as WordSize);
                self.address[Cpu::PC] = memory
                    .read_word(MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR)
                    as Address;
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

#[cfg(test)]
mod tests {
    use crate::{cpu::WordType::Fixnum, parse_asm};

    use super::*;

    struct TestMemory {}

    impl TestMemory {
        fn new() -> Vec<u8> {
            let mut data = vec![0 as u8; 0x10000];
            data.as_mut_slice().write_word(0, Word::symbol(0).into());
            data.as_mut_slice().write_word(8, Word::symbol(1).into());
            data
        }

        fn load_instructions(data: &mut [u8], start: Address, instructions: Vec<Instruction>) {
            let mut pos = 0;
            for instr in instructions {
                let (lo, hi) = instr.encode();
                data.write_word(start + pos * Cpu::WORD_SIZE, lo);
                pos += 1;
                data.write_word(start + pos * Cpu::WORD_SIZE, hi);
                pos += 1;
            }
        }
    }

    #[test]
    fn test_halt() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        TestMemory::load_instructions(
            &mut memory,
            cpu.address[Cpu::PC] as Address,
            vec![Instruction::Halt],
        );
        cpu.full_step(memory.as_mut_slice());
        assert_eq!(cpu.address[Cpu::PC], 0x1000);
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        TestMemory::load_instructions(
            &mut memory,
            cpu.address[Cpu::PC] as Address,
            vec![Instruction::Nop],
        );
        cpu.full_step(memory.as_mut_slice());
        assert_eq!(cpu.address[Cpu::PC], 0x1010);
        Ok(())
    }

    #[test]
    fn test_multiple_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        TestMemory::load_instructions(
            &mut memory,
            cpu.address[Cpu::PC] as Address,
            parse_asm! {
                NOP;
                NOP;
                NOP;
                NOP;
                NOP;
                NOP;
                NOP;
                NOP;
                HALT;
            },
        );

        while !cpu.halted {
            cpu.full_step(memory.as_mut_slice());
        }
        assert_eq!(cpu.address[Cpu::PC], 0x1080);
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_jump_adr() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        cpu.address[0] = 0x2000;
        TestMemory::load_instructions(
            &mut memory,
            cpu.address[Cpu::PC] as Address,
            parse_asm! {
                JUMP [A 0];
            },
        );
        cpu.full_step(memory.as_mut_slice());
        assert_eq!(cpu.address[Cpu::PC], 0x2000);
        Ok(())
    }

    // #[test]
    fn test_cons_builds_cell() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();

        let cons_hook = 0x2000;
        let trap_hook = 0x2100;
        let cons_cell = 0x3000;

        memory
            .as_mut_slice()
            .write_word(MemoryLayout::RESET_VECTOR, cons_hook);
        memory.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_VECTOR,
            cons_hook,
        );
        memory.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_CONS_VECTOR,
            cons_hook,
        );
        memory.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR,
            trap_hook,
        );

        TestMemory::load_instructions(
            &mut memory,
            MemoryLayout::RESET_VECTOR,
            parse_asm! {
                MOV A Cpu::SP, 0x800;
                MOV R 0, Word::fixnum(42);
                MOV R 1, Word::fixnum(99);
                CONS;
                NOP;
                HALT;
            },
        );

        // Fake CONS_HOOK
        TestMemory::load_instructions(
            &mut memory,
            cons_hook as Address,
            parse_asm! {
                MOV A 0, cons_cell;
                IRETURN;
            },
        );

        TestMemory::load_instructions(
            &mut memory,
            trap_hook as Address,
            parse_asm! {
                HALT;
            },
        );

        cpu.reset(&mut memory);
        while !cpu.halted {
            cpu.full_step(memory.as_mut_slice());
        }

        assert_eq!(
            cpu.registers[0],
            Word::new(WordType::Cons, cons_cell as u64)
        );

        assert_eq!(
            Word::try_from(memory.as_mut_slice().read_word(cons_cell))
                .expect("couldn't parse word"),
            Word::new(WordType::Fixnum, 42)
        );

        assert_eq!(
            Word::try_from(memory.as_mut_slice().read_word(cons_cell + WORD_SIZE))
                .expect("couldn't parse word"),
            Word::new(WordType::Fixnum, 99).into()
        );

        assert!(cpu.halted);

        Ok(())
    }
}
