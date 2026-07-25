mod assembler;
mod encoding;

use int_enum::IntEnum;

use crate::memory::Memory;

type MachineAddressSize = u64;
type WordSize = u64;

pub enum MemoryLayout {}
impl MemoryLayout {
    // Standard addresses.
    pub const NIL_ROOT: u64 = 0x0000000000000000;
    pub const T_ROOT: u64 = 0x0000000000000001;

    pub const RESET_VECTOR: u64 = 0x000000000000ef0;
    pub const INTERRUPT_TABLE: u64 = 0x000000000000f00;
    pub const CPU_RESERVED_END: u64 = 0x000000000001000;
}

pub enum InterruptTableOffset {}
impl InterruptTableOffset {
    pub const TRAP_VECTOR: u64 = 0x00000000000001;
    pub const ALLOC_VECTOR: u64 = 0x00000000000002;
    pub const ALLOC_CONS_VECTOR: u64 = 0x00000000000003;
}

pub enum SymbolLayout {}
impl SymbolLayout {
    pub const NAME_OFFSET: u64 = 0;
    pub const PLIST_OFFSET: u64 = 1;
}

pub enum StringLayout {}
impl StringLayout {
    pub const LENGTH_OFFSET: u64 = 0;
    pub const DATA_OFFSET: u64 = 1;
}

pub enum ConsLayout {}
impl ConsLayout {
    pub const CAR_OFFSET: u64 = 0;
    pub const CDR_OFFSET: u64 = 1;
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
    payload: u64,
}

impl Word {
    pub fn undefined() -> Word {
        Word {
            tag: WordType::Undefined,
            payload: 0,
        }
    }

    pub const fn new(tag: WordType, payload: u64) -> Self {
        Self {
            tag,
            payload: payload & 0x00ff_ffff_ffff_ffff,
        }
    }

    // Convenience methods
    pub const fn nil() -> Self {
        Self::symbol(0)
    }

    pub const fn t() -> Self {
        Self::symbol(1)
    }

    pub const fn symbol(value: u64) -> Self {
        Self::new(WordType::Symbol, value)
    }

    pub const fn fixnum(value: u64) -> Self {
        Self::new(WordType::Fixnum, value)
    }

    pub const fn cons(address: u64) -> Self {
        Self::new(WordType::Cons, address)
    }
}

#[derive(Debug)]
pub enum WordParseError {
    InvalidPrefix,
}
impl TryFrom<u64> for Word {
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        const DATA_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;
        let raw_tag = value >> 56;
        let payload = value & DATA_MASK;
        Ok(Word {
            tag: WordType::try_from(raw_tag as u8).map_err(|_| WordParseError::InvalidPrefix)?,
            payload,
        })
    }
    type Error = WordParseError;
}
impl From<Word> for u64 {
    fn from(value: Word) -> u64 {
        let tag = value.tag;
        let payload = value.payload;

        (tag as u8 as u64) << 56 | payload
    }
}

#[derive(Debug)]
pub struct Cpu {
    /// Common registers.
    registers: [Word; 16],

    /// Address registers
    address: [u64; 8],

    pub halted: bool,
}
impl Cpu {
    pub const SP: usize = 5;
    pub const PC: usize = 6;
    pub const ENV: usize = 7;
    pub const INSTRUCTION_SIZE: u64 = 2;
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
pub struct Register(pub usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AddressRegister(pub usize);

#[derive(Debug, PartialEq, Eq)]
pub enum JumpAddressing {
    /// Jumping relative to current PC is done by JUMPing to [adr=PC=0x06]+offset
    AddressRegister {
        condition: AddressRegister,
        adr: Option<AddressRegister>,
        offset: i64,
    },
    Register {
        condition: Register,
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
pub struct ThreeAddrs {
    pub dst: AddressRegister,
    pub op1: AddressRegister,
    pub op2: Option<AddressRegister>,
    pub imm: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct TwoRegs {
    dst: Register,
    src: Register,
}

#[derive(Debug)]
struct TwoAddrs {
    src: AddressRegister,
    dst: AddressRegister,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Location {
    Register(Register),
    Address(AddressRegister),
    IndirectRegister(Register),
    IndirectAddress(AddressRegister, i64),
    Absolute(i64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Instruction {
    // Lower level
    /// Halts execution
    Halt,

    /// Do nothing.
    Nop,

    /// Jumps unconditionally
    Jump(JumpAddressing),

    /// Jump if its Register is 't
    JumpIf(JumpAddressing),

    /// Jump if its Register is 'nil
    JumpIfNot(JumpAddressing),

    /// PUSHes all Registers and Addresses to the stack, then the PC, and jumps.
    /// For some internal INTs (<0x80, like CONS) it may perform additional work.
    Int(u64),

    /// POPs the PC and and all Registers lower than (count) are restored.
    /// All registers are popped, but the count indicates which ones are kept.
    /// For some internal INTs, it may perform additional work.
    IReturn,

    /// Push raw data onto the stack.
    PushA {
        src: AddressRegister,
    },
    /// Pop raw data from the stack.
    PopA {
        dst: AddressRegister,
    },
    /// Load a literal WORD onto a register. Validate it.
    LoadLiteral {
        dst: Register,
        val: Word,
    },
    /// Load a raw Word
    LoadAddress {
        dst: AddressRegister,
        val: u64,
    },
    Mov {
        dst: Location,
        src: Location,
    },
    AAdd(ThreeAddrs),
    ASub(ThreeAddrs),
    SetTag {
        dst: Register,
        src: AddressRegister,
    },
    GetTag {
        dst: AddressRegister,
        src: Register,
    },
    SetPayload {
        dst: Register,
        src: AddressRegister,
    },
    GetPayload {
        dst: AddressRegister,
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

    /// I have no idea what this does yet.
    MakeClosure {
        dst: Register,
        code: AddressRegister,
    },
    /// Push a word onto the stack
    PushR {
        src: Register,
    },
    /// Pop a word onto the stack - and validate it's a word.
    PopR {
        dst: Register,
    },

    /// PUSHes PC on the stack and jumps
    Call(JumpAddressing),

    /// POPs PC from the stack
    Return,
}

impl Cpu {
    pub fn reset<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &M) {
        self.address[Cpu::PC] = memory.read_word(MemoryLayout::RESET_VECTOR);
    }

    fn fetch<M: Memory<MachineAddressSize, WordSize>>(&self, memory: &M) -> (u64, u64) {
        let instruction_low = memory.read_word(self.address[Cpu::PC]);
        let instruction_high = memory.read_word(self.address[Cpu::PC] + 1);
        (instruction_low, instruction_high)
    }

    fn read_word<M: Memory<MachineAddressSize, WordSize>>(
        memory: &M,
        address: MachineAddressSize,
    ) -> Result<Word, Trap> {
        memory
            .read_word(address)
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

    fn nil<M: Memory<MachineAddressSize, WordSize>>(memory: &M) -> Result<Word, Trap> {
        Self::read_word(memory, MemoryLayout::NIL_ROOT)
    }
    fn t<M: Memory<MachineAddressSize, WordSize>>(memory: &M) -> Result<Word, Trap> {
        Self::read_word(memory, MemoryLayout::T_ROOT)
    }

    fn to_machine_bool<M: Memory<MachineAddressSize, WordSize>>(
        memory: &M,
        val: bool,
    ) -> Result<Word, Trap> {
        if val {
            Self::t(memory)
        } else {
            Self::nil(memory)
        }
    }

    fn push<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &mut M, val: u64) {
        memory.write_word(self.address[Cpu::SP], val);
        self.address[Cpu::SP] -= 1;
    }

    fn pop<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &mut M) -> u64 {
        self.address[Cpu::SP] += 1;
        memory.read_word(self.address[Cpu::SP])
    }
    fn pop_word<M: Memory<MachineAddressSize, WordSize>>(
        &mut self,
        memory: &mut M,
    ) -> Result<Word, Trap> {
        self.address[Cpu::SP] += 1;
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

    fn addr_with_offset(&self, op: Option<AddressRegister>, off: i64) -> u64 {
        match op {
            Some(AddressRegister(r)) => (self.address[r] as i64 + off) as u64,
            None => off as u64,
        }
    }

    fn ensure(w: Word, word_type: WordType) -> Result<Word, Trap> {
        if w.tag == word_type {
            Ok(w)
        } else {
            Err(Trap::TypeError)
        }
    }

    fn execute<M: Memory<MachineAddressSize, WordSize>>(
        &mut self,
        instruction: Instruction,
        memory: &mut M,
    ) -> Result<u64, Trap> {
        match instruction {
            // Control flow
            Instruction::Halt => {
                self.halted = true;
                Ok(self.address[Cpu::PC])
            }
            Instruction::Nop => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),

            Instruction::Jump(JumpAddressing::AddressRegister {
                adr,
                condition: _,
                offset,
            }) => Ok(self.addr_with_offset(adr, offset)),
            Instruction::Jump(JumpAddressing::Register { condition: _, adr }) => {
                Err(Trap::Unimplemented)
            }

            Instruction::JumpIf(JumpAddressing::AddressRegister {
                condition,
                adr,
                offset,
            }) => {
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value == nil {
                    Ok(self.address[Cpu::SP] + Cpu::INSTRUCTION_SIZE)
                } else {
                    Ok(self.addr_with_offset(adr, offset))
                }
            }
            Instruction::JumpIf(JumpAddressing::Register { condition, adr }) => {
                Err(Trap::Unimplemented)
            }

            Instruction::JumpIfNot(JumpAddressing::AddressRegister {
                condition,
                adr,
                offset,
            }) => {
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value != nil {
                    Ok(self.address[Cpu::SP] + Cpu::INSTRUCTION_SIZE)
                } else {
                    Ok(self.addr_with_offset(adr, offset))
                }
            }
            Instruction::JumpIfNot(JumpAddressing::Register { condition, adr }) => {
                Err(Trap::Unimplemented)
            }

            Instruction::Call(target) => Err(Trap::Unimplemented),

            Instruction::Return => {
                let return_address = self.pop(memory);
                Ok(return_address)
            }
            Instruction::MakeClosure { dst, code } => Err(Trap::Unimplemented),

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

                self.push(memory, interruption);
                self.push(memory, next_pc);
                let location = memory.read_word(MemoryLayout::INTERRUPT_TABLE + interruption);
                Ok(location)
            }
            Instruction::IReturn => {
                let return_address = self.pop(memory);

                let interrupt = self.pop(memory);
                if interrupt == 3 {
                    let cdr = self.pop(memory);
                    let car = self.pop(memory);
                    memory.write_word(self.address[0] + ConsLayout::CAR_OFFSET, car);
                    memory.write_word(self.address[0] + ConsLayout::CDR_OFFSET, cdr);
                    self.registers[0] = Word::cons(self.address[0]);
                }
                Ok(return_address)
            }
            Instruction::Car(TwoRegs { dst, src }) => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CAR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Cdr(TwoRegs { dst, src }) => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CDR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetCar(TwoRegs { dst, src }) => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[src.0];

                memory.write_word(dst_obj.payload + ConsLayout::CAR_OFFSET, val_obj.into());
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetCdr(TwoRegs { dst, src }) => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[src.0];

                memory.write_word(dst_obj.payload + ConsLayout::CDR_OFFSET, val_obj.into());
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[car.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CAR_OFFSET)?;
                self.registers[cdr.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CDR_OFFSET)?;
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
                self.address[dst.0] = result;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::PushA { src } => {
                self.push(memory, self.address[src.0]);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            Instruction::LoadAddress { dst, val: address } => {
                self.address[dst.0] = address;

                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::AAdd(ThreeAddrs { dst, op1, op2, imm }) => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::ASub(ThreeAddrs { dst, op1, op2, imm }) => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::GetPayload { dst, src } => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::GetTag { src, dst } => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),
            Instruction::LoadLiteral { dst, val } => {
                self.registers[dst.0] = val;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetPayload { dst, src } => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetTag { src, dst } => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),
            Instruction::Mov { dst, src } => {
                let value = match src {
                    Location::Absolute(a) => memory.read_word(a as u64),
                    Location::Address(AddressRegister(r)) => self.address[r],
                    Location::IndirectAddress(AddressRegister(r), off) => {
                        memory.read_word((self.address[r] as i64 + off) as u64)
                    }
                    Location::IndirectRegister(Register(r)) => {
                        memory.read_word(self.registers[r].payload)
                    }
                    Location::Register(Register(r)) => self.registers[r].into(),
                };
                match dst {
                    Location::Absolute(a) => memory.write_word(a as u64, value),
                    Location::Address(AddressRegister(r)) => self.address[r] = value,
                    Location::IndirectAddress(AddressRegister(r), off) => {
                        memory.write_word((self.address[r] as i64 + off) as u64, value)
                    }
                    Location::IndirectRegister(Register(r)) => {
                        memory.write_word(self.registers[r].payload, value)
                    }
                    Location::Register(Register(r)) => {
                        self.registers[r] = Word::try_from(value).map_err(|_| Trap::TypeError)?
                    }
                }
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
        }
    }

    pub fn step<M: Memory<MachineAddressSize, WordSize>>(
        &mut self,
        memory: &mut M,
    ) -> Result<(), Trap> {
        let (lo, hi) = self.fetch(memory);
        let instruction = Instruction::decode(lo, hi)?;

        println!("{} - {:?}", self.address[Cpu::PC], instruction);
        let next_pc = self.execute(instruction, memory)?;
        self.address[Cpu::PC] = next_pc;
        Ok(())
    }

    pub fn full_step<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &mut M) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.address[0] = self.address[Cpu::PC];
                self.registers[0] = Word::new(WordType::Fixnum, trap as u64);
                self.address[Cpu::PC] = memory
                    .read_word(MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR);
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

    struct TestMemory {
        data: Vec<u64>,
    }
    impl Memory<MachineAddressSize, WordSize> for TestMemory {
        fn read_word(&self, addr: MachineAddressSize) -> WordSize {
            self.data[addr as usize]
        }
        fn write_word(&mut self, addr: MachineAddressSize, data: WordSize) {
            self.data[addr as usize] = data
        }
    }

    impl TestMemory {
        const NIL: Word = Word::new(WordType::Symbol, 0);
        const T: Word = Word::new(WordType::Symbol, 1);
        fn new() -> Self {
            let mut data = vec![0; 0x4000];
            data[0] = TestMemory::NIL.into();
            data[1] = TestMemory::T.into();
            Self { data }
        }

        fn load_instructions(&mut self, start: usize, instructions: Vec<Instruction>) {
            let mut pos = start;
            for instr in instructions {
                let (lo, hi) = instr.encode();
                self.data[pos] = lo;
                pos += 1;
                self.data[pos] = hi;
                pos += 1;
            }
        }
    }

    #[test]
    fn test_halt() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        memory.load_instructions(cpu.address[Cpu::PC] as usize, vec![Instruction::Halt]);
        cpu.full_step(&mut memory);
        assert_eq!(cpu.address[Cpu::PC], 0x1000);
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        memory.load_instructions(cpu.address[Cpu::PC] as usize, vec![Instruction::Nop]);
        cpu.full_step(&mut memory);
        assert_eq!(cpu.address[Cpu::PC], 0x1002);
        Ok(())
    }

    #[test]
    fn test_multiple_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        memory.load_instructions(
            cpu.address[Cpu::PC] as usize,
            vec![
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
            cpu.full_step(&mut memory);
        }
        assert_eq!(cpu.address[Cpu::PC], 0x1010);
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_jump_adr() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.address[Cpu::PC] = 0x1000;
        cpu.address[0] = 0x2000;
        memory.load_instructions(
            cpu.address[Cpu::PC] as usize,
            vec![Instruction::Jump(JumpAddressing::AddressRegister {
                condition: AddressRegister(0),
                adr: Some(AddressRegister(0)),
                offset: 0,
            })],
        );
        cpu.full_step(&mut memory);
        assert_eq!(cpu.address[Cpu::PC], 0x2000);
        Ok(())
    }

    #[test]
    fn test_cons_builds_cell() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();

        cpu.address[Cpu::PC] = 0x1000;

        let cons_hook = 0x2000;
        let trap_hook = 0x2100;
        let cons_cell = 0x3000;

        memory.write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_VECTOR,
            cons_hook,
        );
        memory.write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_CONS_VECTOR,
            cons_hook,
        );
        memory.write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR,
            trap_hook,
        );

        memory.load_instructions(
            cpu.address[Cpu::PC] as usize,
            parse_asm! {
                MOV A Cpu::SP, 0x800;
                MOV R 0, Word::fixnum(42);
                MOV R 1, Word::fixnum(99);
                CONS;
                NOP;
                HALT;
            },
        );
        println!(
            "{:?}",
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
        memory.load_instructions(
            cons_hook as usize,
            parse_asm! {
                MOV A 0, cons_cell;
                IRETURN;
            },
        );

        memory.load_instructions(
            trap_hook as usize,
            parse_asm! {
                HALT;
            },
        );

        while !cpu.halted {
            cpu.step(&mut memory).map_err(|t| format!("Trap {:?}", t))?;
        }

        assert_eq!(cpu.registers[0], Word::new(WordType::Cons, cons_cell));

        assert_eq!(
            Word::try_from(memory.read_word(cons_cell)).expect("couldn't parse word"),
            Word::new(WordType::Fixnum, 42)
        );

        assert_eq!(
            Word::try_from(memory.read_word(cons_cell + 1)).expect("couldn't parse word"),
            Word::new(WordType::Fixnum, 99).into()
        );

        assert!(cpu.halted);

        Ok(())
    }
}
