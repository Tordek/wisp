use int_enum::IntEnum;

use crate::{cpu::WordType::Fixnum, memory::Memory};

type MachineAddressSize = u64;
type WordSize = u64;

pub const INSTRUCTION_SIZE: u64 = 2;
pub enum MemoryLayout {}
impl MemoryLayout {
    // Standard addresses.
    pub const NIL_ROOT: u64 = 0x0000000000000000;
    pub const T_ROOT: u64 = 0x0000000000000001;
    pub const RESET_VECTOR: u64 = 0x000000000000fe0;
    pub const TRAP_VECTOR: u64 = 0x000000000000ff0;
    pub const CPU_RESERVED_END: u64 = 0x000000000001000;
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

pub enum Root {
    NIL,
    T,
    CONS,
    FIXNUM,
    SYMBOL,
}

#[repr(u8)]
#[derive(PartialEq, Eq, Clone, Copy, IntEnum)]
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

#[derive(PartialEq, Eq, Clone, Copy)]
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

    pub fn new(tag: WordType, payload: u64) -> Self {
        Self {
            tag,
            payload: payload & 0x00ff_ffff_ffff_ffff,
        }
    }
}

impl TryFrom<u64> for Word {
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        const DATA_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;
        let raw_tag = value >> 56;
        let payload = value & DATA_MASK;
        Ok(Word {
            tag: WordType::try_from(raw_tag as u8)?,
            payload,
        })
    }

    type Error = u8;
}
impl From<Word> for u64 {
    fn from(value: Word) -> u64 {
        let tag = value.tag;
        let payload = value.payload;

        (tag as u8 as u64) << 56 | payload
    }
}

pub struct Cpu {
    /// Common registers.
    registers: [Word; 16],

    /// Address registers
    address: [u64; 4],

    /// Program counter
    pc: u64,

    /// Stack pointer
    sp: u64,

    /// Environment pointer
    env: u64,

    pub halted: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            registers: [Word::undefined(); 16],
            address: [0; 4],
            pc: 0,
            sp: 0,
            env: 0,
            halted: false,
        }
    }
}

#[derive(PartialEq, Eq)]
struct Register(usize);

struct AddressRegister(usize);

enum Instruction {
    Halt,
    Nop,
    JumpAbs {
        target: AddressRegister,
    },
    JumpReg {
        target: Register,
    },
    JumpRegRel {
        offset: Register,
    },
    JumpIfAbs {
        condition: Register,
        target: AddressRegister,
    },
    JumpIfReg {
        condition: Register,
        target: Register,
    },
    JumpIfRegRel {
        condition: Register,
        offset: Register,
    },
    JumpIfNotAbs {
        condition: Register,
        target: AddressRegister,
    },
    JumpIfNotReg {
        condition: Register,
        target: Register,
    },
    JumpIfNotRegRel {
        condition: Register,
        target: Register,
    },
    Cons {
        dst: Register,
        car: Register,
        cdr: Register,
    },
    Uncons {
        car: Register,
        cdr: Register,
        src: Register,
    },
    Car {
        dst: Register,
        src: Register,
    },
    Cdr {
        dst: Register,
        src: Register,
    },
    SetCar {
        dst: Register,
        val: Register,
    },
    SetCdr {
        dst: Register,
        val: Register,
    },
    Add {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Mul {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    IDiv {
        div: Register,
        rem: Register,
        op1: Register,
        op2: Register,
    },
    Div {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Sub {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Call {
        target: Register,
    },
    CallAbs {
        target: AddressRegister,
    },
    Return,
    MakeClosure {
        dst: Register,
        code: AddressRegister,
    },
    Eq {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Ne {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Gt {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Gte {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Lt {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    Lte {
        dst: Register,
        op1: Register,
        op2: Register,
    },
    PushR {
        src: Register,
    },
    PushA {
        src: AddressRegister,
    },
    PopR {
        dst: Register,
    },
    PopA {
        dst: AddressRegister,
    },
    PushPc,
    LoadPc {
        dst: AddressRegister,
    },
    LoadChar {
        dst: Register,
        val: u64,
    },
    LoadFixnum {
        dst: Register,
        val: u64,
    },
    LoadRoot {
        dst: Register,
        root: Root,
    },
}
impl Instruction {
    fn decode(raw: u128) -> Self {
        match raw {
            0 => Self::Halt,
            1 => Self::Nop,
            2 => Self::Uncons {
                car: Register(0),
                cdr: Register(1),
                src: Register(1),
            },
            _ => Self::Eq {
                dst: Register(0),
                op1: Register(1),
                op2: Register(2),
            },
        }
    }
}

impl Cpu {
    pub fn reset<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &M) {
        self.pc = memory.read_word(MemoryLayout::RESET_VECTOR);
    }

    fn fetch<M: Memory<MachineAddressSize, WordSize>>(&self, memory: &M) -> u128 {
        let instruction_low = memory.read_word(self.pc) as u128;
        let instruction_high = memory.read_word(self.pc + 1) as u128;
        instruction_high << 64 | instruction_low
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
        memory.write_word(self.sp, val);
        self.sp += 1;
    }

    fn pop<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &mut M) -> u64 {
        self.sp -= 1;
        memory.read_word(self.sp)
    }
    fn pop_word<M: Memory<MachineAddressSize, WordSize>>(
        &mut self,
        memory: &mut M,
    ) -> Result<Word, Trap> {
        self.sp -= 1;
        Self::read_word(memory, self.sp)
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
                Ok(self.pc)
            }
            Instruction::Nop => Ok(self.pc + INSTRUCTION_SIZE),
            Instruction::JumpAbs { target } => {
                let target_address = self.address[target.0];
                Ok(target_address)
            }
            Instruction::JumpIfAbs { condition, target } => {
                let target_address = self.address[target.0];
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value == nil {
                    Ok(self.sp + INSTRUCTION_SIZE)
                } else {
                    Ok(target_address)
                }
            }
            Instruction::JumpIfNotAbs { condition, target } => {
                let target_address = self.address[target.0];
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value != nil {
                    Ok(self.sp + INSTRUCTION_SIZE)
                } else {
                    Ok(target_address)
                }
            }
            Instruction::JumpIfNotReg { condition, target } => Err(Trap::Unimplemented),
            Instruction::JumpIfReg { condition, target } => Err(Trap::Unimplemented),
            Instruction::JumpReg { target } => Err(Trap::Unimplemented),
            Instruction::JumpIfNotRegRel { condition, target } => Err(Trap::Unimplemented),
            Instruction::JumpIfRegRel { condition, offset } => Err(Trap::Unimplemented),
            Instruction::JumpRegRel { offset } => Err(Trap::Unimplemented),

            // Comparison
            Instruction::Eq { dst, op1, op2 } => {
                let op1_obj = self.registers[op1.0];
                let op2_obj = self.registers[op2.0];

                self.registers[dst.0] = Self::to_machine_bool(memory, op1_obj == op2_obj)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Ne { dst, op1, op2 } => {
                let op1_obj = self.registers[op1.0];
                let op2_obj = self.registers[op2.0];
                self.registers[dst.0] = Self::to_machine_bool(memory, op1_obj != op2_obj)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Gt { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload > op2_obj.payload)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Gte { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload >= op2_obj.payload)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Lt { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload < op2_obj.payload)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Lte { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload >= op2_obj.payload)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }

            // Cons
            Instruction::Cons { dst, car, cdr } => Err(Trap::Unimplemented),
            Instruction::Car { dst, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CAR_OFFSET)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Cdr { dst, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CDR_OFFSET)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::SetCar { dst, val } => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[val.0];

                memory.write_word(dst_obj.payload + ConsLayout::CAR_OFFSET, val_obj.into());
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::SetCdr { dst, val } => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[val.0];

                memory.write_word(dst_obj.payload + ConsLayout::CDR_OFFSET, val_obj.into());
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[car.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CAR_OFFSET)?;
                self.registers[cdr.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CDR_OFFSET)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }

            // Arithmetic
            Instruction::Add { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload + op2_obj.payload);
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Sub { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload - op2_obj.payload);
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Mul { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload * op2_obj.payload);
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Div { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload / op2_obj.payload);
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::IDiv { div, rem, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[div.0] = Word::new(Fixnum, op1_obj.payload / op2_obj.payload);
                self.registers[rem.0] = Word::new(Fixnum, op1_obj.payload % op2_obj.payload);
                Ok(self.pc + INSTRUCTION_SIZE)
            }

            // Call
            Instruction::Call { target: dst } => Err(Trap::Unimplemented),
            Instruction::CallAbs { target: dst } => Err(Trap::Unimplemented),
            Instruction::Return => Err(Trap::Unimplemented),
            Instruction::MakeClosure { dst, code } => Err(Trap::Unimplemented),

            // Load
            Instruction::LoadPc { dst } => {
                let next_pc = self.pc + INSTRUCTION_SIZE;
                self.address[dst.0] = next_pc;
                Ok(next_pc)
            }
            Instruction::PushPc => {
                let next_pc = self.pc + INSTRUCTION_SIZE;
                self.push(memory, next_pc);
                Ok(next_pc)
            }

            Instruction::PopR { dst } => {
                let result = self.pop_word(memory)?;
                self.registers[dst.0] = result;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::PushR { src } => {
                self.push(memory, self.registers[src.0].into());
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::PopA { dst } => {
                let result = self.pop(memory);
                self.address[dst.0] = result;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::PushA { src } => {
                self.push(memory, self.address[src.0]);
                Ok(self.pc + INSTRUCTION_SIZE)
            }

            Instruction::LoadChar { dst, val } => {
                let num = Word::new(WordType::Character, val);
                self.registers[dst.0] = num;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::LoadFixnum { dst, val } => {
                let num = Word::new(WordType::Fixnum, val);
                self.registers[dst.0] = num;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::LoadRoot { dst, root } => {
                let root_obj = Self::read_word(memory, root as u64)?;
                self.registers[dst.0] = root_obj;

                Ok(self.pc + INSTRUCTION_SIZE)
            }
        }
    }

    pub fn step<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &mut M) {
        let instruction_raw = self.fetch(memory);
        let instruction = Instruction::decode(instruction_raw);

        match self.execute(instruction, memory) {
            Ok(next_pc) => {
                self.pc = next_pc;
            }
            Err(trap) => {
                self.address[0] = self.pc;
                self.registers[0] = Word::new(WordType::Fixnum, trap as u64);
                self.pc = memory.read_word(MemoryLayout::TRAP_VECTOR);
            }
        }
    }
}

pub enum Trap {
    TypeError,
    Unimplemented,
}
