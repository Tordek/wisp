use int_enum::IntEnum;

use crate::memory::Memory;

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

type Register = usize;

enum Instruction {
    Halt,
    Nop,
    Uncons {
        car: Register,
        cdr: Register,
        src: Register,
    },
    Eq {
        dst: Register,
        op1: Register,
        op2: Register,
    },
}
impl Instruction {
    fn decode(raw: u128) -> Self {
        match raw {
            0 => Self::Halt,
            1 => Self::Nop,
            2 => Self::Uncons {
                car: 0,
                cdr: 1,
                src: 2,
            },
            _ => Self::Eq {
                dst: 0,
                op1: 1,
                op2: 2,
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

    fn execute<M: Memory<MachineAddressSize, WordSize>>(
        &mut self,
        instruction: Instruction,
        memory: &mut M,
    ) -> Result<u64, Trap> {
        match instruction {
            Instruction::Halt => {
                self.halted = true;
                Ok(self.pc)
            }
            Instruction::Nop => Ok(self.pc + INSTRUCTION_SIZE),
            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.registers[src];

                if src_obj.tag != WordType::Cons {
                    return Err(Trap::TypeError);
                }

                let src_cons = src_obj.payload;

                self.registers[car] = memory
                    .read_word(src_cons + ConsLayout::CAR_OFFSET)
                    .try_into()
                    .map_err(|_| Trap::TypeError)?;
                self.registers[cdr] = memory
                    .read_word(src_cons + ConsLayout::CDR_OFFSET)
                    .try_into()
                    .map_err(|_| Trap::TypeError)?;
                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Eq { dst, op1, op2 } => {
                let op1_obj = self.registers[op1];
                let op2_obj = self.registers[op2];
                let result = if op1_obj == op2_obj {
                    MemoryLayout::T_ROOT
                } else {
                    MemoryLayout::NIL_ROOT
                };

                self.registers[dst] = memory
                    .read_word(result)
                    .try_into()
                    .map_err(|_| Trap::TypeError)?;
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
            Err(Trap::TypeError) => {
                self.address[0] = self.pc;
                self.registers[0] = Word {
                    tag: WordType::Fixnum,
                    payload: Trap::TypeError as u64,
                };
                self.pc = memory.read_word(MemoryLayout::TRAP_VECTOR);
            }
        }
    }
}

pub enum Trap {
    TypeError,
}
