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
    Fixnum = 0,
    Symbol = 2,
    Cons = 3,
    Function = 4,
    Macro = 5,
    Character = 6,
    String = 7,
    Vector = 8,
    Float = 9,
    Undefined = 0xff,
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

impl From<Native> for LispWord {
    fn from(value: Native) -> Self {
        LispWord(value.0)
    }
}
impl From<LispWord> for Native {
    fn from(value: LispWord) -> Native {
        Native(value.0)
    }
}

#[derive(Debug)]
pub struct Cpu {
    registers: [WordSize; 256],

    pending_interrupt: Option<u64>,

    pub halted: bool,
    interrupts_disabled: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            registers: [0; 256],
            pending_interrupt: None,
            halted: false,
            interrupts_disabled: false,
        }
    }
}

impl Cpu {
    pub const NIL: Register = Register(246);
    pub const T: Register = Register(247);
    pub const CONS_FREE: Register = Register(248);
    pub const CONS_END: Register = Register(249);
    pub const GEN_FREE: Register = Register(250);
    pub const GEN_END: Register = Register(251);
    pub const FP: Register = Register(252);
    pub const SP: Register = Register(253);
    pub const PC: Register = Register(254);
    pub const VBR: Register = Register(255);

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
        write!(f, "V{}", self.0)
    }
}
impl Register {
    fn offset(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RegAndOff {
    op1: Register,
    off: Option<Native>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RValue {
    Literal(Native),
    LPointer(RegAndOff),
    Register(RegAndOff),
    Absolute(Address),
    Indirect(RegAndOff),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LValue {
    Absolute(Address),
    Register(Register),
    Indirect(RegAndOff),
    LPointer(RegAndOff),
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

    Jump {
        condition: Condition,
        target: RValue,
    },

    /// PUSHes all Registers and Machinees to the stack, then the PC, and jumps.
    /// For some internal INTs (<0x80, like CONS) it may perform additional work.
    Int(WordSize),

    /// POPs the PC and and all Registers lower than (count) are restored.
    /// All registers are popped, but the count indicates which ones are kept.
    /// For some internal INTs, it may perform additional work.
    IReturn,

    Mov {
        dst: LValue,
        src: RValue,
    },
    Mov8 {
        dst: LValue,
        src: RValue,
    },
    MBinary {
        op: MBinaryOp,
        dst: Register,
        op1: Register,
        op2: RValue,
    },
    Comparison {
        op: Comparison,
        dst: Register,
        op1: Register,
        op2: RValue,
    },
    SetTag {
        dst: Register,
        src: Register,
    },
    GetTag {
        dst: Register,
        src: Register,
    },
    SetPayload {
        dst: Register,
        src: Register,
    },
    GetPayload {
        dst: Register,
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
    Car {
        dst: Register,
        src: Register,
    },
    /// Gets the second part of a cons - typechecks.
    Cdr {
        dst: Register,
        src: Register,
    },
    /// Sets the first part of a cons - typechecks.
    SetCar {
        dst: Register,
        src: Register,
    },
    /// Sets the second part of a cons - typechecks.
    SetCdr {
        dst: Register,
        src: Register,
    },
    /// Calculates div <- op1 / (op2 + imm), rem <- op1 % (op2 + imm)
    IDiv {
        div: Register,
        rem: Register,
        op1: Register,
        op2: RValue,
    },
    Binary {
        op: BinaryOp,
        dst: Register,
        op1: Register,
        op2: RValue,
    },
    MakeClosure {
        dst: Register,
        code: Register,
    },
    /// Push a word onto the stack
    Push {
        src: Register,
    },
    /// Pop a word onto the stack - and validate it's a word.
    Pop {
        dst: Register,
    },
    Typep {
        dst: Register,
        src: Register,
        compare: Native,
    },
    /// while (count--) *dst++ = *src++
    MemCpy {
        dst: Register,
        src: Register,
        count: RValue,
    },
    /// while (count--) *dst++ = src
    // MemSet {
    //     dst: Register,
    //     src: Register,
    //     count: Count,
    // },

    /// PUSHes PC on the stack and jumps
    Call {
        target: RValue,
    },

    /// POPs PC from the stack
    Return,
}

impl Cpu {
    pub fn reset(&mut self) {
        self.set_reg(Cpu::PC, Native::from(MemoryLayout::RESET_VECTOR));
        self.set_lreg(Cpu::T, LispWord(1));
        self.set_lreg(Cpu::NIL, LispWord(0));
        self.interrupts_disabled = true;
        self.halted = false;
        self.pending_interrupt = None;
    }

    fn lreg(&self, r: Register) -> LispWord {
        LispWord(self.registers[r.offset()])
    }

    fn set_lreg(&mut self, r: Register, data: LispWord) {
        self.registers[r.offset()] = data.0
    }

    fn reg(&self, r: Register) -> Native {
        Native(self.registers[r.offset()])
    }

    fn set_reg(&mut self, r: Register, data: Native) {
        self.registers[r.offset()] = data.0
    }

    fn fetch(&self, memory: &Bus) -> (Native, Native) {
        let instruction_low = memory.read_word(Address::from(self.reg(Cpu::PC)));
        let instruction_high =
            memory.read_word(Address::from(self.reg(Cpu::PC)) + Offset(Self::WORD_SIZE as i64));
        (instruction_low, instruction_high)
    }

    fn read_word(memory: &Bus, address: Address) -> LispWord {
        memory.read_word(address).into()
    }

    fn to_machine_bool(&self, val: bool) -> LispWord {
        if val {
            self.lreg(Cpu::T)
        } else {
            self.lreg(Cpu::NIL)
        }
    }

    fn push(&mut self, memory: &mut Bus, val: Native) {
        self.set_reg(
            Cpu::SP,
            (Address::from(self.reg(Cpu::SP)) - Offset(Self::WORD_SIZE as i64)).into(),
        );
        memory.write_word(Address::from(self.reg(Cpu::SP)), val);
    }

    fn pop(&mut self, memory: &mut Bus) -> Native {
        let res = memory.read_word(Address::from(self.reg(Cpu::SP)));
        self.set_reg(
            Cpu::SP,
            (Address::from(self.reg(Cpu::SP)) + Offset(Self::WORD_SIZE as i64)).into(),
        );
        res
    }

    fn get_offset_raw(&self, RegAndOff { op1, off }: RegAndOff) -> Native {
        self.reg(op1) + off.unwrap_or(Native(0))
    }

    // TODO: require pointer?
    fn get_offset_lisp(&self, RegAndOff { op1, off }: RegAndOff) -> LispWord {
        let base = self.lreg(op1).payload();
        let off = off
            .map(LispWord::from)
            .unwrap_or(LispWord::fixnum(0))
            .payload();
        LispWord::fixnum(base as i64 + off as i64)
    }

    fn read_rval(&self, bus: &Bus, rval: RValue) -> Native {
        match rval {
            RValue::Literal(l) => l,
            RValue::Register(r) => self.get_offset_raw(r),
            RValue::Absolute(adr) => bus.read_word(adr),
            RValue::Indirect(pos) => bus.read_word(Address::from(self.get_offset_raw(pos))),
            RValue::LPointer(r) => bus.read_word(Address(self.get_offset_lisp(r).payload())),
        }
    }

    fn read_rval_byte(&self, bus: &Bus, src: RValue) -> u8 {
        match src {
            RValue::Literal(v) => v.0 as u8,
            RValue::Register(pos) => self.get_offset_raw(pos).0 as u8,
            RValue::Absolute(adr) => bus.read_byte(adr),
            RValue::Indirect(pos) => bus.read_byte(Address::from(self.get_offset_raw(pos))),
            RValue::LPointer(r) => bus.read_byte(Address(self.get_offset_lisp(r).payload())),
        }
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
            Address::from(self.reg(Cpu::VBR))
                + Offset(interruption as i64 * Self::WORD_SIZE as i64),
        ));
        Ok(location)
    }

    fn execute(&mut self, instruction: Instruction, memory: &mut Bus) -> Result<Address, Trap> {
        let next_pc = Address(self.reg(Cpu::PC).0) + Offset(Cpu::INSTRUCTION_SIZE as i64);
        match instruction {
            // Control flow
            Instruction::Halt => {
                self.halted = true;
            }
            Instruction::Nop => {}

            Instruction::Jump { condition, target } => {
                let nil = self.lreg(Cpu::NIL);

                let should_jump = match condition {
                    Condition::Always => true,
                    Condition::False(r) => self.lreg(r) == nil,
                    Condition::True(r) => self.lreg(r) != nil,
                };

                if should_jump {
                    return Ok(Address::from(self.read_rval(memory, target)));
                }
            }

            // Creates a new frame containing:
            // Sets:
            // - FP = SP
            // - PC = target
            // And creates a frame containing:
            // - Previous Frame Pointer
            // - Previous Environment
            // - Previous Program Counter
            Instruction::Call { target } => {
                let prev_fp = self.reg(Cpu::FP);
                self.set_reg(Cpu::FP, self.reg(Cpu::SP));

                self.push(memory, prev_fp);
                self.push(memory, Native::from(next_pc));
                return Ok(Address::from(self.read_rval(memory, target)));
            }

            // Consumes a frame.
            // Sets:
            // - SP = FP
            // - PC = [FP - 16]
            // - FP = [FP - 8]
            Instruction::Return => {
                let fp = self.reg(Cpu::FP);
                let return_address = memory.read_word(Address::from(fp) - Offset(24)); // FP + 8 points to return value
                let prev_fp = memory.read_word(Address::from(fp) - Offset(8)); // FP points to previous FP.
                self.set_reg(Cpu::SP, fp);
                self.set_reg(Cpu::FP, prev_fp);
                return Ok(Address::from(return_address));
            }

            Instruction::MakeClosure { dst: _, code: _ } => todo!(),

            // Comparison
            Instruction::Comparison { op, dst, op1, op2 } => {
                let op1_obj = self.reg(op1);
                let op2_obj = self.read_rval(memory, op2);

                let result = match op {
                    Comparison::Eq => op1_obj == op2_obj,
                    Comparison::Ne => op1_obj != op2_obj,
                    Comparison::Gt => op1_obj > op2_obj,
                    Comparison::Gte => op1_obj >= op2_obj,
                    Comparison::Lt => op1_obj < op2_obj,
                    Comparison::Lte => op1_obj <= op2_obj,
                };

                self.set_lreg(dst, self.to_machine_bool(result));
            }

            Instruction::Binary { op, dst, op1, op2 } => {
                let op1_obj = self.lreg(op1).as_fixnum()?;
                let op2_obj = LispWord::from(self.read_rval(memory, op2)).as_fixnum()?;

                let result = LispWord::fixnum(match op {
                    BinaryOp::Add => op1_obj + op2_obj,
                    BinaryOp::Sub => op1_obj - op2_obj,
                    BinaryOp::Mul => op1_obj * op2_obj,
                    BinaryOp::Shl => op1_obj << op2_obj,
                    BinaryOp::Shr => op1_obj >> op2_obj,
                });

                self.set_lreg(dst, result);
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

            Instruction::Car { dst, src } => {
                let src_obj = self.lreg(src).ensure(WordType::Cons)?;

                self.set_lreg(
                    dst,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET),
                );
            }
            Instruction::Cdr { dst, src } => {
                let src_obj = self.lreg(src).ensure(WordType::Cons)?;

                self.set_lreg(
                    dst,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET),
                );
            }
            Instruction::SetCar { dst, src } => {
                let dst_obj = self.lreg(dst).ensure(WordType::Cons)?;
                let val_obj = self.lreg(src);

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CAR_OFFSET,
                    val_obj.into(),
                );
            }
            Instruction::SetCdr { dst, src } => {
                let dst_obj = self.lreg(dst).ensure(WordType::Cons)?;
                let val_obj = self.lreg(src);

                memory.write_word(
                    Address(dst_obj.payload()) + ConsLayout::CDR_OFFSET,
                    val_obj.into(),
                );
            }
            Instruction::Cons { dst, car, cdr } => {
                let free = self.reg(Self::CONS_FREE);
                if free.0 + 16 <= self.reg(Self::CONS_END).0 {
                    memory.write_word(
                        Address::from(free) + ConsLayout::CAR_OFFSET,
                        self.lreg(car).into(),
                    );
                    memory.write_word(
                        Address::from(free) + ConsLayout::CDR_OFFSET,
                        self.lreg(cdr).into(),
                    );
                    self.set_lreg(dst, LispWord::cons(free.0));
                    self.set_reg(Self::CONS_FREE, free + Native(16));
                } else {
                    return self.run_interrupt(memory, 0x03, Address::from(self.reg(Self::PC))); // Come back.
                }
            }

            Instruction::Req {
                dst,
                prototype,
                size,
            } => {
                let sizew = self.lreg(size).as_fixnum()? as u64;
                let sizeb = sizew * 8;
                let free = self.reg(Self::GEN_FREE);
                if free.0 + sizeb <= self.reg(Self::GEN_END).0 {
                    for i in 0..sizew {
                        memory.write_word(
                            Address::from(free) + Offset(i as i64 * 8),
                            self.lreg(Register(i as u8)).into(),
                        )
                    }
                    let prototype = self.lreg(prototype);
                    let result = LispWord::new(prototype.tag(), free.0);
                    self.set_lreg(dst, result);
                    self.set_reg(Self::GEN_FREE, free + Native(sizeb));
                } else {
                    return self.run_interrupt(memory, 0x03, Address::from(self.reg(Self::PC))); // Come back.
                }
            }

            Instruction::Uncons { car, cdr, src } => {
                let src_obj = self.lreg(src).ensure(WordType::Cons)?;

                self.set_lreg(
                    car,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CAR_OFFSET),
                );
                self.set_lreg(
                    cdr,
                    Self::read_word(memory, Address(src_obj.payload()) + ConsLayout::CDR_OFFSET),
                );
            }

            Instruction::IDiv { div, rem, op1, op2 } => {
                let op1_obj = self.lreg(op1).as_fixnum()?;
                let op2_obj = LispWord::from(self.read_rval(memory, op2)).as_fixnum()?;
                self.set_lreg(div, LispWord::fixnum(op1_obj / op2_obj));
                self.set_lreg(rem, LispWord::fixnum(op1_obj % op2_obj));
            }

            Instruction::Pop { dst } => {
                let result = self.pop(memory);
                self.set_reg(dst, result);
            }
            Instruction::Push { src } => {
                self.push(memory, self.reg(src));
            }

            Instruction::MBinary { op, dst, op1, op2 } => {
                let val1 = self.reg(op1);
                let val2 = self.read_rval(memory, op2);
                let result = match op {
                    MBinaryOp::Add => val1 + val2,
                    MBinaryOp::Sub => val1 - val2,
                };
                self.set_reg(dst, result);
            }
            Instruction::GetPayload { dst, src } => {
                self.set_reg(dst, Native(self.lreg(src).payload()));
            }
            Instruction::GetTag { src, dst } => {
                self.set_reg(dst, Native(self.lreg(src).tag() as WordSize));
            }
            Instruction::SetPayload { dst, src } => {
                self.set_lreg(dst, LispWord::new(self.lreg(dst).tag(), self.reg(src).0));
            }
            Instruction::SetTag { src, dst } => {
                self.set_lreg(
                    dst,
                    LispWord::new(self.reg(src).0 as u8, self.lreg(dst).payload()),
                );
            }
            Instruction::Mov8 { dst, src } => {
                let value = self.read_rval_byte(memory, src);
                match dst {
                    LValue::Absolute(a) => memory.write_byte(a, value),
                    LValue::Register(r) => {
                        self.set_reg(r, Native(value as u64));
                    }
                    LValue::Indirect(target) => {
                        memory.write_byte(Address::from(self.get_offset_raw(target)), value);
                    }
                    LValue::LPointer(target) => {
                        memory.write_byte(Address(self.get_offset_lisp(target).payload()), value);
                    }
                }
            }
            Instruction::Mov { dst, src } => {
                let value = self.read_rval(memory, src);
                match dst {
                    LValue::Absolute(a) => memory.write_word(a, value),
                    LValue::Register(r) => self.set_lreg(r, LispWord::from(value)),
                    LValue::Indirect(target) => {
                        memory.write_word(Address::from(self.get_offset_raw(target)), value)
                    }
                    LValue::LPointer(target) => {
                        memory.write_word(Address(self.get_offset_lisp(target).payload()), value);
                    }
                }
            }
            Instruction::MemCpy { dst, src, count } => {
                let srcadd = Address::from(self.reg(src));
                let dstadd = Address::from(self.reg(dst));
                let count = LispWord::from(self.read_rval(memory, count)).as_fixnum()?;

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
                let src_obj = self.lreg(src);
                self.set_lreg(dst, self.to_machine_bool(src_obj.tag() as u64 == compare.0));
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
                    Address::from(self.reg(Cpu::PC)),
                )?);
                self.set_reg(Cpu::PC, next_pc)
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
            self.reg(Cpu::PC).0,
            self.reg(Cpu::SP).0,
            instruction
        );
        let next_pc = self.execute(instruction, memory)?;
        self.set_reg(Cpu::PC, Native::from(next_pc));
        Ok(())
    }

    pub fn full_step(&mut self, memory: &mut Bus) {
        match self.step(memory) {
            Ok(()) => {}
            Err(trap) => {
                self.set_reg(Register(0), self.reg(Cpu::PC));
                self.set_lreg(Register(0), LispWord::fixnum(trap as i64));
                self.set_reg(
                    Cpu::PC,
                    memory.read_word(
                        Address::from(self.reg(Cpu::VBR)) + InterruptTableOffset::TRAP_VECTOR,
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
        cpu.set_reg(Cpu::PC, Native(0x1000));
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.reg(Cpu::PC)),
            vec![Instruction::Halt],
        );
        cpu.full_step(&mut test_bus);
        assert_eq!(cpu.reg(Cpu::PC), Native(0x1010));
        assert!(cpu.halted);
        Ok(())
    }

    #[test]
    fn test_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.set_reg(Cpu::PC, Native(0x1000));
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.reg(Cpu::PC)),
            vec![Instruction::Nop],
        );
        cpu.full_step(&mut test_bus);
        assert_eq!(cpu.reg(Cpu::PC), Native(0x1010));
        assert!(!cpu.halted);
        Ok(())
    }

    #[test]
    fn test_multiple_nop() -> Result<(), String> {
        let mut cpu = Cpu::default();
        let mut test_bus = test_setup();
        cpu.set_reg(Cpu::PC, Native(0x1000));
        TestMemory::load_instructions(
            &mut test_bus,
            Address::from(cpu.reg(Cpu::PC)),
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
        assert_eq!(cpu.reg(Cpu::PC), Native(0x10a0));
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
