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
    pub const ALLOC_VECTOR: u64 = 0x000000000000fc0;
    pub const ALLOC_CONS_VECTOR: u64 = 0x000000000000fd0;
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Debug)]
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

#[derive(PartialEq, Eq, Debug)]
struct Register(usize);

#[derive(Debug)]
struct AddressRegister(usize);

#[derive(Debug)]
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
        target: AddressRegister,
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
    LoadAddress {
        dst: AddressRegister,
        address: u64,
    },
    Store {
        address: AddressRegister,
        value: Register,
    },
    StoreOffset {
        base: AddressRegister,
        offset: i64,
        value: Register,
    },
    LoadCons {
        dst: Register,
        address: AddressRegister,
    },
}

// TODO: Find a real encoding/decoding.
impl Instruction {
    fn decode(lo: u64, hi: u64) -> Result<Self, Trap> {
        let [opcode, r0, r1, r2, r3, r4, r5, r6] = lo.to_le_bytes();
        match opcode {
            0 => Ok(Self::Halt),
            1 => Ok(Self::Nop),
            2 => Ok(Self::JumpAdr {
                target: AddressRegister(hi as usize - 16),
            }),
            3 => Ok(Self::JumpReg {
                target: Register(r0 as usize),
            }),
            4 => Ok(Self::JumpImm { target: hi }),
            5 => Ok(Self::JumpRel { offset: hi as i64 }),
            6 => Ok(Self::JumpIfAdr {
                target: AddressRegister(hi as usize - 16),
                condition: Register(r1 as usize),
            }),
            7 => Ok(Self::JumpIfReg {
                target: Register(r0 as usize),
                condition: Register(r1 as usize),
            }),
            8 => Ok(Self::JumpIfImm {
                target: hi,
                condition: Register(r1 as usize),
            }),
            9 => Ok(Self::JumpIfRel {
                offset: hi as i64,
                condition: Register(r1 as usize),
            }),
            10 => Ok(Self::JumpIfNotAdr {
                target: AddressRegister(hi as usize - 16),
                condition: Register(r1 as usize),
            }),
            11 => Ok(Self::JumpIfNotReg {
                target: Register(r0 as usize),
                condition: Register(r1 as usize),
            }),
            12 => Ok(Self::JumpIfNotImm {
                target: hi,
                condition: Register(r1 as usize),
            }),
            13 => Ok(Self::JumpIfNotRel {
                offset: hi as i64,
                condition: Register(r1 as usize),
            }),
            14 => Ok(Self::CallAdr {
                target: AddressRegister(hi as usize - 16),
            }),
            15 => Ok(Self::CallReg {
                target: Register(r0 as usize),
            }),
            16 => Ok(Self::CallImm { target: hi }),
            17 => Ok(Self::CallRel { offset: hi as i64 }),

            18 => Ok(Self::Return),
            19 => Ok(Self::MakeClosure {
                dst: Register(r0 as usize),
                code: AddressRegister(r0 as usize - 16),
            }),

            20 => Ok(Self::Eq {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            21 => Ok(Self::Ne {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            22 => Ok(Self::Gt {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            23 => Ok(Self::Gte {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            24 => Ok(Self::Lt {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            25 => Ok(Self::Lte {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),

            26 => Ok(Self::Cons {
                car: Register(r0 as usize),
                cdr: Register(r0 as usize),
            }),
            27 => Ok(Self::Car {
                dst: Register(r0 as usize),
                src: Register(r0 as usize),
            }),
            28 => Ok(Self::Cdr {
                dst: Register(r0 as usize),
                src: Register(r0 as usize),
            }),
            29 => Ok(Self::SetCar {
                dst: Register(r0 as usize),
                val: Register(r0 as usize),
            }),
            30 => Ok(Self::SetCdr {
                dst: Register(r0 as usize),
                val: Register(r0 as usize),
            }),
            31 => Ok(Self::Uncons {
                car: Register(r0 as usize),
                cdr: Register(r0 as usize),
                src: Register(r0 as usize),
            }),
            32 => Ok(Self::Add {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            33 => Ok(Self::Sub {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            34 => Ok(Self::Mul {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            35 => Ok(Self::Div {
                dst: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            36 => Ok(Self::IDiv {
                div: Register(r0 as usize),
                rem: Register(r0 as usize),
                op1: Register(r0 as usize),
                op2: Register(r0 as usize),
            }),
            37 => Ok(Self::LoadPc {
                dst: AddressRegister(r0 as usize - 16),
            }),
            38 => Ok(Self::PushPc),
            39 => Ok(Self::PopR {
                dst: Register(r0 as usize),
            }),
            40 => Ok(Self::PushR {
                src: Register(r0 as usize),
            }),
            41 => Ok(Self::PopA {
                dst: AddressRegister(r0 as usize - 16),
            }),
            42 => Ok(Self::PushA {
                src: AddressRegister(r0 as usize - 16),
            }),
            43 => Ok(Self::LoadChar {
                dst: Register(r0 as usize),
                val: hi,
            }),
            44 => Ok(Self::LoadFixnum {
                dst: Register(r0 as usize),
                val: hi,
            }),
            45 => match hi {
                0 => Ok(Self::LoadRoot {
                    dst: Register(r0 as usize),
                    root: Root::NIL,
                }),
                1 => Ok(Self::LoadRoot {
                    dst: Register(r0 as usize),
                    root: Root::T,
                }),
                _ => Err(Trap::InvalidInstruction),
            },
            46 => Ok(Instruction::LoadAddress {
                dst: AddressRegister(r0 as usize - 16),
                address: hi,
            }),
            47 => Ok(Instruction::Store {
                address: AddressRegister(r0 as usize - 16),
                value: Register(r1 as usize),
            }),
            48 => Ok(Self::StoreOffset {
                base: AddressRegister(r0 as usize - 16),
                offset: hi as i64,
                value: Register(r1 as usize),
            }),
            49 => Ok(Self::LoadCons {
                dst: Register(r0 as usize),
                address: AddressRegister(r1 as usize - 16),
            }),
            _ => Err(Trap::InvalidInstruction),
        }
    }

    fn encode(&self) -> (u64, u64) {
        match self {
            // Control flow
            Instruction::Halt => (u64::from_le_bytes([0, 0, 0, 0, 0, 0, 0, 0]), 0),
            Instruction::Nop => (u64::from_le_bytes([1, 0, 0, 0, 0, 0, 0, 0]), 0),

            Instruction::JumpAdr { target } => (
                u64::from_le_bytes([2, target.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::JumpReg { target } => {
                (u64::from_le_bytes([3, target.0 as u8, 0, 0, 0, 0, 0, 0]), 0)
            }
            Instruction::JumpImm { target } => {
                (u64::from_le_bytes([4, 0, 0, 0, 0, 0, 0, 0]), *target)
            }
            Instruction::JumpRel { offset } => {
                (u64::from_le_bytes([5, 0, 0, 0, 0, 0, 0, 0]), *offset as u64)
            }

            Instruction::JumpIfAdr { target, condition } => (
                u64::from_le_bytes([6, target.0 as u8 + 16, condition.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::JumpIfReg { target, condition } => (
                u64::from_le_bytes([7, target.0 as u8, condition.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::JumpIfImm { target, condition } => (
                u64::from_le_bytes([8, condition.0 as u8, 0, 0, 0, 0, 0, 0]),
                *target,
            ),
            Instruction::JumpIfRel { offset, condition } => (
                u64::from_le_bytes([9, condition.0 as u8, 0, 0, 0, 0, 0, 0]),
                *offset as u64,
            ),

            Instruction::JumpIfNotAdr { target, condition } => (
                u64::from_le_bytes([10, target.0 as u8 + 16, condition.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::JumpIfNotReg { target, condition } => (
                u64::from_le_bytes([11, target.0 as u8, condition.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::JumpIfNotImm { target, condition } => (
                u64::from_le_bytes([12, condition.0 as u8, 0, 0, 0, 0, 0, 0]),
                *target,
            ),
            Instruction::JumpIfNotRel { offset, condition } => (
                u64::from_le_bytes([13, condition.0 as u8, 0, 0, 0, 0, 0, 0]),
                *offset as u64,
            ),

            Instruction::CallAdr { target } => (
                u64::from_le_bytes([14, target.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::CallReg { target } => (
                u64::from_le_bytes([15, target.0 as u8, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::CallImm { target } => {
                (u64::from_le_bytes([16, 0, 0, 0, 0, 0, 0, 0]), *target)
            }
            Instruction::CallRel { offset } => (
                u64::from_le_bytes([17, 0, 0, 0, 0, 0, 0, 0]),
                *offset as u64,
            ),

            Instruction::Return => (u64::from_le_bytes([18, 0, 0, 0, 0, 0, 0, 0]), 0),
            Instruction::MakeClosure { dst, code } => (
                u64::from_le_bytes([19, dst.0 as u8, code.0 as u8 + 16, 0, 0, 0, 0, 0]),
                0,
            ),

            // Comparison
            Instruction::Eq { dst, op1, op2 } => (
                u64::from_le_bytes([20, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Ne { dst, op1, op2 } => (
                u64::from_le_bytes([21, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Gt { dst, op1, op2 } => (
                u64::from_le_bytes([22, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Gte { dst, op1, op2 } => (
                u64::from_le_bytes([23, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Lt { dst, op1, op2 } => (
                u64::from_le_bytes([24, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Lte { dst, op1, op2 } => (
                u64::from_le_bytes([25, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),

            // Cons
            Instruction::Cons { car, cdr } => (
                u64::from_le_bytes([26, car.0 as u8, cdr.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Car { dst, src } => (
                u64::from_le_bytes([27, dst.0 as u8, src.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Cdr { dst, src } => (
                u64::from_le_bytes([28, dst.0 as u8, src.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::SetCar { dst, val } => (
                u64::from_le_bytes([29, dst.0 as u8, val.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::SetCdr { dst, val } => (
                u64::from_le_bytes([30, dst.0 as u8, val.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Uncons { car, cdr, src } => (
                u64::from_le_bytes([31, car.0 as u8, cdr.0 as u8, src.0 as u8, 0, 0, 0, 0]),
                0,
            ),

            // Arithmetic
            Instruction::Add { dst, op1, op2 } => (
                u64::from_le_bytes([32, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Sub { dst, op1, op2 } => (
                u64::from_le_bytes([33, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Mul { dst, op1, op2 } => (
                u64::from_le_bytes([34, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Div { dst, op1, op2 } => (
                u64::from_le_bytes([35, dst.0 as u8, op1.0 as u8, op2.0 as u8, 0, 0, 0, 0]),
                0,
            ),
            Instruction::IDiv { div, rem, op1, op2 } => (
                u64::from_le_bytes([
                    36,
                    div.0 as u8,
                    rem.0 as u8,
                    op1.0 as u8,
                    op2.0 as u8,
                    0,
                    0,
                    0,
                ]),
                0,
            ),

            // Load
            Instruction::LoadPc { dst } => (
                u64::from_le_bytes([37, dst.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::PushPc => (u64::from_le_bytes([38, 0, 0, 0, 0, 0, 0, 0]), 0),

            Instruction::PopR { dst } => {
                (u64::from_le_bytes([39, dst.0 as u8, 0, 0, 0, 0, 0, 0]), 0)
            }
            Instruction::PushR { src } => {
                (u64::from_le_bytes([40, src.0 as u8, 0, 0, 0, 0, 0, 0]), 0)
            }
            Instruction::PopA { dst } => (
                u64::from_le_bytes([41, dst.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::PushA { src } => (
                u64::from_le_bytes([42, src.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),

            Instruction::LoadChar { dst, val } => (
                u64::from_le_bytes([43, dst.0 as u8, 0, 0, 0, 0, 0, 0]),
                *val as u64,
            ),
            Instruction::LoadFixnum { dst, val } => (
                u64::from_le_bytes([44, dst.0 as u8, 0, 0, 0, 0, 0, 0]),
                *val as u64,
            ),
            Instruction::LoadRoot { dst, root } => (
                u64::from_le_bytes([45, dst.0 as u8, 0, 0, 0, 0, 0, 0]),
                *root as u64,
            ),

            Instruction::LoadAddress { dst, address } => (
                u64::from_le_bytes([46, dst.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                *address,
            ),
            Instruction::Store { address, value } => (
                u64::from_le_bytes([47, address.0 as u8 + 16, value.0 as u8, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::StoreOffset {
                base,
                offset,
                value,
            } => (
                u64::from_le_bytes([48, base.0 as u8 + 16, value.0 as u8, 0, 0, 0, 0, 0]),
                *offset as u64,
            ),
            Instruction::LoadCons { dst, address } => (
                u64::from_le_bytes([49, dst.0 as u8, address.0 as u8 + 16, 0, 0, 0, 0, 0]),
                0,
            ),
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

            Instruction::Return => {
                let return_address = self.pop(memory);
                Ok(return_address)
            }
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
            Instruction::Cons { car, cdr } => {
                let next_pc = self.pc + INSTRUCTION_SIZE;
                self.push(memory, next_pc);
                self.push(memory, self.registers[cdr.0].into());
                self.push(memory, self.registers[car.0].into());
                self.push(memory, 2);
                self.push(memory, WordType::Cons as u64);

                // ALLOC_CONS_VECTOR has special semantics and it handles cleaning up
                // the params.
                let location = memory.read_word(MemoryLayout::ALLOC_CONS_VECTOR);
                Ok(location)
            }
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

            Instruction::LoadAddress { dst, address } => {
                self.address[dst.0] = address;

                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::Store { address, value } => {
                let value_obj = self.registers[value.0];
                memory.write_word(self.address[address.0], value_obj.into());

                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::StoreOffset {
                base,
                offset,
                value,
            } => {
                let value_obj = self.registers[value.0];

                memory.write_word(
                    (self.address[base.0 as usize] as i64 + offset) as u64,
                    value_obj.into(),
                );

                Ok(self.pc + INSTRUCTION_SIZE)
            }
            Instruction::LoadCons { dst, address } => {
                self.registers[dst.0] = Word::new(WordType::Cons, self.address[address.0]);

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

    #[test]
    fn test_cons_builds_cell() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut memory = TestMemory::new();

        cpu.pc = 0x1000;
        cpu.sp = 0x200;

        let cons_hook = 0x2000;
        let cons_cell = 0x3000;

        memory.write_word(MemoryLayout::ALLOC_CONS_VECTOR, cons_hook);

        memory.load_instructions(
            cpu.pc as usize,
            vec![
                Instruction::LoadFixnum {
                    dst: Register(1),
                    val: 42,
                },
                Instruction::LoadFixnum {
                    dst: Register(2),
                    val: 99,
                },
                // R0 <- Cons(R1, R2)
                Instruction::Cons {
                    car: Register(1),
                    cdr: Register(2),
                },
                Instruction::Halt,
            ],
        );

        // Fake CONS_HOOK
        memory.load_instructions(
            cons_hook as usize,
            vec![
                // A real hook would call an allocator.
                // For testing, directly create the object.
                Instruction::LoadAddress {
                    dst: AddressRegister(0),
                    address: cons_cell,
                },
                Instruction::Store {
                    address: AddressRegister(0),
                    value: Register(1),
                },
                Instruction::StoreOffset {
                    base: AddressRegister(0),
                    offset: 1,
                    value: Register(2),
                },
                Instruction::PopA {
                    dst: AddressRegister(1),
                },
                Instruction::PopA {
                    dst: AddressRegister(1),
                },
                Instruction::PopA {
                    dst: AddressRegister(1),
                },
                Instruction::PopA {
                    dst: AddressRegister(1),
                },
                Instruction::LoadCons {
                    dst: Register(0),
                    address: AddressRegister(0),
                },
                Instruction::Return,
            ],
        );

        while !cpu.halted {
            cpu.full_step(&mut memory);
        }

        let result = cpu.registers[0];

        assert_eq!(result, Word::new(WordType::Cons, cons_cell));

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
