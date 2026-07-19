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

    pub const fn new(tag: WordType, payload: u64) -> Self {
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

    JumpAdr {
        target: AddressRegister,
    },
    JumpReg {
        target: Register,
    },
    JumpImm {
        target: u64,
    },
    JumpRel {
        offset: i64,
    },
    JumpIfAdr {
        condition: Register,
        target: AddressRegister,
    },
    JumpIfReg {
        condition: Register,
        target: Register,
    },
    JumpIfImm {
        condition: Register,
        target: u64,
    },
    JumpIfRel {
        condition: Register,
        offset: i64,
    },
    JumpIfNotAdr {
        condition: Register,
        target: AddressRegister,
    },
    JumpIfNotReg {
        condition: Register,
        target: Register,
    },
    JumpIfNotImm {
        condition: Register,
        target: u64,
    },
    JumpIfNotRel {
        condition: Register,
        offset: i64,
    },
    CallAdr {
        target: Register,
    },
    CallReg {
        target: Register,
    },
    CallImm {
        target: u64,
    },
    CallRel {
        offset: i64,
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

// TODO: Find a real encoding/decoding.
impl Instruction {
    fn decode(lo: u64, hi: u64) -> Result<Self, Trap> {
        match lo {
            0 => Ok(Self::Halt),
            1 => Ok(Self::Nop),
            2 => Ok(Self::JumpAdr {
                target: AddressRegister(hi as usize),
            }),
            3 => Ok(Self::JumpIfAdr {
                condition: Register(((hi >> 8) & 0xff) as usize),
                target: AddressRegister(((hi >> 0) & 0xff) as usize),
            }),
            4 => Ok(Self::JumpIfNotAdr {
                condition: Register(((hi >> 8) & 0xff) as usize),
                target: AddressRegister(((hi >> 0) & 0xff) as usize),
            }),
            5 => Ok(Self::JumpIfReg {
                condition: Register(((hi >> 8) & 0xff) as usize),
                target: Register(((hi >> 8) & 0xff) as usize),
            }),
            6 => Ok(Self::JumpIfNotReg {
                condition: Register(((hi >> 8) & 0xff) as usize),
                target: Register(((hi >> 8) & 0xff) as usize),
            }),
            7 => Ok(Self::JumpReg {
                target: Register(((hi >> 8) & 0xff) as usize),
            }),
            8 => Ok(Self::JumpRel {
                offset: ((hi >> 8) & 0xff) as i64,
            }),
            9 => Ok(Self::JumpIfRel {
                condition: Register(((hi >> 8) & 0xff) as usize),
                offset: ((hi >> 8) & 0xff) as i64,
            }),
            10 => Ok(Self::JumpIfNotRel {
                condition: Register(((hi >> 8) & 0xff) as usize),
                offset: (((hi >> 8) & 0xff) as i64),
            }),
            11 => Ok(Self::Eq {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            12 => Ok(Self::Ne {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            13 => Ok(Self::Gt {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            14 => Ok(Self::Gte {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            15 => Ok(Self::Lt {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            16 => Ok(Self::Lte {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            17 => Ok(Self::Cons {
                dst: Register(((hi >> 8) & 0xff) as usize),
                car: Register(((hi >> 8) & 0xff) as usize),
                cdr: Register(((hi >> 8) & 0xff) as usize),
            }),
            18 => Ok(Self::Car {
                dst: Register(((hi >> 8) & 0xff) as usize),
                src: Register(((hi >> 8) & 0xff) as usize),
            }),
            19 => Ok(Self::Cdr {
                dst: Register(((hi >> 8) & 0xff) as usize),
                src: Register(((hi >> 8) & 0xff) as usize),
            }),
            20 => Ok(Self::SetCar {
                dst: Register(((hi >> 8) & 0xff) as usize),
                val: Register(((hi >> 8) & 0xff) as usize),
            }),
            21 => Ok(Self::SetCdr {
                dst: Register(((hi >> 8) & 0xff) as usize),
                val: Register(((hi >> 8) & 0xff) as usize),
            }),
            22 => Ok(Self::Uncons {
                car: Register(((hi >> 8) & 0xff) as usize),
                cdr: Register(((hi >> 8) & 0xff) as usize),
                src: Register(((hi >> 8) & 0xff) as usize),
            }),
            23 => Ok(Self::Add {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            24 => Ok(Self::Sub {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            25 => Ok(Self::Mul {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            26 => Ok(Self::Div {
                dst: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            27 => Ok(Self::IDiv {
                div: Register(((hi >> 8) & 0xff) as usize),
                rem: Register(((hi >> 8) & 0xff) as usize),
                op1: Register(((hi >> 8) & 0xff) as usize),
                op2: Register(((hi >> 8) & 0xff) as usize),
            }),
            28 => Ok(Self::CallReg {
                target: Register(((hi >> 8) & 0xff) as usize),
            }),
            29 => Ok(Self::CallAdr {
                target: Register(((hi >> 8) & 0xff) as usize),
            }),
            30 => Ok(Self::Return),
            31 => Ok(Self::MakeClosure {
                dst: Register(((hi >> 8) & 0xff) as usize),
                code: AddressRegister(((hi >> 8) & 0xff) as usize),
            }),
            32 => Ok(Self::LoadPc {
                dst: AddressRegister(((hi >> 8) & 0xff) as usize),
            }),
            33 => Ok(Self::PushPc),
            34 => Ok(Self::PopR {
                dst: Register(((hi >> 8) & 0xff) as usize),
            }),
            35 => Ok(Self::PushR {
                src: Register(((hi >> 8) & 0xff) as usize),
            }),
            36 => Ok(Self::PopA {
                dst: AddressRegister(((hi >> 8) & 0xff) as usize),
            }),
            37 => Ok(Self::PushA {
                src: AddressRegister(((hi >> 8) & 0xff) as usize),
            }),
            38 => Ok(Self::LoadChar {
                dst: Register(((hi >> 8) & 0xff) as usize),
                val: ((hi >> 8) & 0xff),
            }),
            39 => Ok(Self::LoadFixnum {
                dst: Register(((hi >> 8) & 0xff) as usize),
                val: ((hi >> 8) & 0xff),
            }),
            40 => match ((hi >> 8) & 0xff) {
                0 => Ok(Self::LoadRoot {
                    dst: Register(((hi >> 8) & 0xff) as usize),
                    root: Root::NIL,
                }),
                1 => Ok(Self::LoadRoot {
                    dst: Register(((hi >> 8) & 0xff) as usize),
                    root: Root::T,
                }),
                _ => Err(Trap::InvalidInstruction),
            },
            _ => Err(Trap::InvalidInstruction),
        }
    }

    fn encode(&self) -> (u64, u64) {
        match self {
            // Control flow
            Instruction::Halt => (0, 0),
            Instruction::Nop => (1, 0),

            Instruction::JumpAdr { target } => (2, 0),
            Instruction::JumpReg { target } => (7, 0),
            Instruction::JumpImm { target } => (0, 0),
            Instruction::JumpRel { offset } => (8, 0),

            Instruction::JumpIfAdr { condition, target } => (3, 0),
            Instruction::JumpIfReg { condition, target } => (5, 0),
            Instruction::JumpIfImm { condition, target } => (5, 0),
            Instruction::JumpIfRel { condition, offset } => (9, 0),

            Instruction::JumpIfNotAdr { condition, target } => (4, 0),
            Instruction::JumpIfNotReg { condition, target } => (6, 0),
            Instruction::JumpIfNotImm { condition, target } => (6, 0),
            Instruction::JumpIfNotRel { condition, offset } => (10, 0),

            Instruction::CallAdr { target } => (28, 0),
            Instruction::CallReg { target } => (29, 0),
            Instruction::CallImm { target } => (29, 0),
            Instruction::CallRel { offset } => (29, 0),

            Instruction::Return => (30, 0),
            Instruction::MakeClosure { dst, code } => (31, 0),

            // Comparison
            Instruction::Eq { dst, op1, op2 } => (11, 0),
            Instruction::Ne { dst, op1, op2 } => (12, 0),
            Instruction::Gt { dst, op1, op2 } => (13, 0),
            Instruction::Gte { dst, op1, op2 } => (14, 0),
            Instruction::Lt { dst, op1, op2 } => (15, 0),
            Instruction::Lte { dst, op1, op2 } => (16, 0),

            // Cons
            Instruction::Cons { dst, car, cdr } => (17, 0),
            Instruction::Car { dst, src } => (18, 0),
            Instruction::Cdr { dst, src } => (19, 0),
            Instruction::SetCar { dst, val } => (20, 0),
            Instruction::SetCdr { dst, val } => (21, 0),
            Instruction::Uncons { car, cdr, src } => (22, 0),

            // Arithmetic
            Instruction::Add { dst, op1, op2 } => (23, 0),
            Instruction::Sub { dst, op1, op2 } => (24, 0),
            Instruction::Mul { dst, op1, op2 } => (25, 0),
            Instruction::Div { dst, op1, op2 } => (26, 0),
            Instruction::IDiv { div, rem, op1, op2 } => (27, 0),

            // Load
            Instruction::LoadPc { dst } => (32, 0),
            Instruction::PushPc => (33, 0),

            Instruction::PopR { dst } => (34, 0),
            Instruction::PushR { src } => (35, 0),
            Instruction::PopA { dst } => (36, 0),
            Instruction::PushA { src } => (37, 0),

            Instruction::LoadChar { dst, val } => (38, 0),
            Instruction::LoadFixnum { dst, val } => (39, 0),
            Instruction::LoadRoot { dst, root } => (40, 0),
        }
    }
}

impl Cpu {
    pub fn reset<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &M) {
        self.pc = memory.read_word(MemoryLayout::RESET_VECTOR);
    }

    fn fetch<M: Memory<MachineAddressSize, WordSize>>(&self, memory: &M) -> (u64, u64) {
        let instruction_low = memory.read_word(self.pc);
        let instruction_high = memory.read_word(self.pc + 1);
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

            Instruction::JumpAdr { target } => {
                let target_address = self.address[target.0];
                Ok(target_address)
            }
            Instruction::JumpReg { target } => Err(Trap::Unimplemented),
            Instruction::JumpImm { target } => Err(Trap::Unimplemented),
            Instruction::JumpRel { offset } => Err(Trap::Unimplemented),

            Instruction::JumpIfAdr { condition, target } => {
                let target_address = self.address[target.0];
                let condition_value = self.registers[condition.0];
                let nil = Self::nil(memory)?;
                if condition_value == nil {
                    Ok(self.sp + INSTRUCTION_SIZE)
                } else {
                    Ok(target_address)
                }
            }
            Instruction::JumpIfReg { condition, target } => Err(Trap::Unimplemented),
            Instruction::JumpIfRel { condition, offset } => Err(Trap::Unimplemented),
            Instruction::JumpIfImm { condition, target } => Err(Trap::Unimplemented),

            Instruction::JumpIfNotAdr { condition, target } => {
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
            Instruction::JumpIfNotImm { condition, target } => Err(Trap::Unimplemented),
            Instruction::JumpIfNotRel { condition, offset } => Err(Trap::Unimplemented),

            Instruction::CallReg { target: dst } => Err(Trap::Unimplemented),
            Instruction::CallAdr { target: dst } => Err(Trap::Unimplemented),
            Instruction::CallImm { target: dst } => Err(Trap::Unimplemented),
            Instruction::CallRel { offset: dst } => Err(Trap::Unimplemented),

            Instruction::Return => Err(Trap::Unimplemented),
            Instruction::MakeClosure { dst, code } => Err(Trap::Unimplemented),

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

    pub fn step<M: Memory<MachineAddressSize, WordSize>>(
        &mut self,
        memory: &mut M,
    ) -> Result<(), Trap> {
        let (lo, hi) = self.fetch(memory);
        let instruction = Instruction::decode(lo, hi)?;

        let next_pc = self.execute(instruction, memory)?;
        self.pc = next_pc;
        Ok(())
    }

    pub fn full_step<M: Memory<MachineAddressSize, WordSize>>(&mut self, memory: &mut M) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.address[0] = self.pc;
                self.registers[0] = Word::new(WordType::Fixnum, trap as u64);
                self.pc = memory.read_word(MemoryLayout::TRAP_VECTOR);
            }
        };
    }
}

pub enum Trap {
    TypeError,
    InvalidInstruction,
    Unimplemented,
}

#[cfg(test)]
mod tests {
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
            let mut data = vec![0; 0x2000];
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
        cpu.pc = 0x1000;
        memory.load_instructions(cpu.pc as usize, vec![Instruction::Halt]);
        cpu.full_step(&mut memory);
        assert_eq!(cpu.pc, 0x1000);
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.pc = 0x1000;
        memory.load_instructions(cpu.pc as usize, vec![Instruction::Nop]);
        cpu.full_step(&mut memory);
        assert_eq!(cpu.pc, 0x1002);
        Ok(())
    }

    #[test]
    fn test_multiple_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.pc = 0x1000;
        memory.load_instructions(
            cpu.pc as usize,
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
        assert_eq!(cpu.pc, 0x1010);
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_jump_adr() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();
        cpu.pc = 0x1000;
        cpu.address[0] = 0x2000;
        memory.load_instructions(
            cpu.pc as usize,
            vec![Instruction::JumpAdr {
                target: AddressRegister(0),
            }],
        );
        cpu.full_step(&mut memory);
        assert_eq!(cpu.pc, 0x2000);
        Ok(())
    }
}
