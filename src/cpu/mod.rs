mod encoding;

use int_enum::IntEnum;

use crate::{cpu::WordType::Fixnum, memory::Memory};

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
    address: [u64; 8],

    pub halted: bool,
}
impl Cpu {
    const SP: usize = 5;
    const PC: usize = 6;
    const ENV: usize = 7;
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

#[derive(PartialEq, Eq, Debug)]
pub struct Register(pub usize);

#[derive(Debug)]
pub struct AddressRegister(pub usize);

#[derive(Debug)]
pub enum Instruction {
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
    IReturn {
        count: u64,
    },
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
    LoadSp {
        dst: AddressRegister,
    },
    StoreSp {
        src: AddressRegister,
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
    ReadOffsetAdr {
        dst: AddressRegister,
        base: AddressRegister,
        offset: i64,
    },
    ReadOffsetReg {
        dst: Register,
        base: AddressRegister,
        offset: i64,
    },
    StoreOffsetAdr {
        base: AddressRegister,
        offset: i64,
        value: AddressRegister,
    },
    StoreOffsetReg {
        base: AddressRegister,
        offset: i64,
        value: Register,
    },
    LoadCons {
        dst: Register,
        address: AddressRegister,
    },
    MovAR {
        dst: AddressRegister,
        src: Register,
    },
    MovRA {
        dst: Register,
        src: AddressRegister,
    },
    AAdd {
        dst: AddressRegister,
        op1: AddressRegister,
        op2: AddressRegister,
    },
    GetPayload {
        dst: AddressRegister,
        src: Register,
    },
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
                    Ok(self.address[Cpu::SP] + Cpu::INSTRUCTION_SIZE)
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
                    Ok(self.address[Cpu::SP] + Cpu::INSTRUCTION_SIZE)
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
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Ne { dst, op1, op2 } => {
                let op1_obj = self.registers[op1.0];
                let op2_obj = self.registers[op2.0];
                self.registers[dst.0] = Self::to_machine_bool(memory, op1_obj != op2_obj)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Gt { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload > op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Gte { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload >= op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Lt { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload < op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Lte { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                self.registers[dst.0] =
                    Self::to_machine_bool(memory, op1_obj.payload >= op2_obj.payload)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            // Cons
            Instruction::Cons { car, cdr } => {
                let next_pc = self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE;
                self.push(memory, self.registers[car.0].into());
                self.push(memory, self.registers[cdr.0].into());
                self.push(memory, 0x02);
                for i in 0..4 {
                    self.push(memory, self.registers[i].into());
                    self.push(memory, self.address[i].into());
                }
                for i in 4..16 {
                    self.push(memory, self.registers[i].into());
                }
                self.push(memory, next_pc);
                self.push(memory, Word::new(WordType::Fixnum, 2).into()); // Size: 2
                self.push(memory, Word::new(WordType::Fixnum, 2).into()); // Type: Cons

                let location = memory.read_word(
                    MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_CONS_VECTOR,
                );
                Ok(location)
            }
            Instruction::Car { dst, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CAR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Cdr { dst, src } => {
                let src_obj = self.read_register_as(src, WordType::Cons)?;

                self.registers[dst.0] =
                    Self::read_word(memory, src_obj.payload + ConsLayout::CDR_OFFSET)?;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetCar { dst, val } => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[val.0];

                memory.write_word(dst_obj.payload + ConsLayout::CAR_OFFSET, val_obj.into());
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::SetCdr { dst, val } => {
                let dst_obj = self.read_register_as(dst, WordType::Cons)?;
                let val_obj = self.registers[val.0];

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
            Instruction::Add { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload + op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Sub { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload - op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Mul { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload * op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::Div { dst, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[dst.0] = Word::new(Fixnum, op1_obj.payload / op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::IDiv { div, rem, op1, op2 } => {
                let op1_obj = self.read_register_as(op1, WordType::Fixnum)?;
                let op2_obj = self.read_register_as(op2, WordType::Fixnum)?;
                // TODO: When fetching numbers, extend the sign bit.
                self.registers[div.0] = Word::new(Fixnum, op1_obj.payload / op2_obj.payload);
                self.registers[rem.0] = Word::new(Fixnum, op1_obj.payload % op2_obj.payload);
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            // Load
            Instruction::LoadPc { dst } => {
                let next_pc = self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE;
                self.address[dst.0] = next_pc;
                Ok(next_pc)
            }
            Instruction::PushPc => {
                let next_pc = self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE;
                self.push(memory, next_pc);
                Ok(next_pc)
            }
            Instruction::LoadSp { dst } => {
                self.address[dst.0] = self.address[Cpu::SP];
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::StoreSp { src } => {
                self.address[Cpu::SP] = self.address[src.0];
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

            Instruction::LoadChar { dst, val } => {
                let num = Word::new(WordType::Character, val);
                self.registers[dst.0] = num;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::LoadFixnum { dst, val } => {
                let num = Word::new(WordType::Fixnum, val);
                self.registers[dst.0] = num;
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::LoadRoot { dst, root } => {
                let root_obj = Self::read_word(memory, root as u64)?;
                self.registers[dst.0] = root_obj;

                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }

            Instruction::LoadAddress { dst, address } => {
                self.address[dst.0] = address;

                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::StoreOffsetAdr {
                base,
                offset,
                value,
            } => {
                let value_obj = self.registers[value.0];

                memory.write_word(
                    (self.address[base.0 as usize] as i64 + offset) as u64,
                    value_obj.into(),
                );

                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            } // Instruction::LoadCons { dst, address } => {
            //     self.registers[dst.0] = Word::new(WordType::Cons, self.address[address.0]);

            //     Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            // }
            Instruction::IReturn { count } => {
                let return_address = self.pop(memory);

                for i in (4..16).rev() {
                    let r = self.pop_word(memory)?;
                    if count <= i {
                        self.registers[i as usize] = r;
                    }
                }
                for i in (0..4).rev() {
                    let r = self.pop_word(memory)?;
                    let a = self.pop(memory);
                    if count <= i {
                        self.registers[i as usize] = r;
                        self.address[i as usize] = a;
                    }
                }
                let interrupt = self.pop(memory);
                if interrupt == 2 {
                    let cdr = self.pop(memory);
                    let car = self.pop(memory);
                    memory.write_word(self.address[0] + ConsLayout::CAR_OFFSET, car);
                    memory.write_word(self.address[0] + ConsLayout::CDR_OFFSET, cdr);
                    self.registers[0] = Word::new(WordType::Cons, self.address[0]);
                }
                Ok(return_address)
            }
            Instruction::LoadCons { dst, address } => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::ReadOffsetAdr { dst, base, offset } => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::ReadOffsetReg { dst, base, offset } => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::StoreOffsetReg {
                base,
                offset,
                value,
            } => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),
            Instruction::MovAR { dst, src } => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),
            Instruction::MovRA { dst, src } => Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE),
            Instruction::AAdd { dst, op1, op2 } => {
                Ok(self.address[Cpu::PC] + Cpu::INSTRUCTION_SIZE)
            }
            Instruction::GetPayload { dst, src } => {
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
            vec![Instruction::JumpAdr {
                target: AddressRegister(0),
            }],
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
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_CONS_VECTOR,
            cons_hook,
        );
        memory.write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR,
            trap_hook,
        );

        memory.load_instructions(
            cpu.address[Cpu::PC] as usize,
            vec![
                Instruction::LoadAddress {
                    dst: AddressRegister(0),
                    address: 0x800,
                },
                Instruction::StoreSp {
                    src: AddressRegister(0),
                },
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
                Instruction::Nop,
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
                Instruction::PopA {
                    dst: AddressRegister(1),
                },
                Instruction::IReturn { count: 1 },
            ],
        );

        memory.load_instructions(trap_hook as usize, vec![Instruction::Halt]);

        while !cpu.halted {
            cpu.step(&mut memory).map_err(|_| "Trap ")?;
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
