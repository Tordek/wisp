use int_enum::IntEnum;

use crate::{
    bus::{Address, Offset},
    cpu::{
        self, EitherSource, Instruction, LispWord, Location, MachSource, MachineRegister, Native,
        RegAndOff, RegSource, Register, TwoRegs,
    },
};

#[derive(Debug)]
pub enum EncoderError {
    BadInstruction,
}

#[derive(Debug)]
pub enum DecoderError {
    BadInstruction,
}

enum ARegister {
    Register(Register),
    MRegister(MachineRegister),
    IRegister(Register),
    IMRegister(MachineRegister),
    None,
    Imm,
}

impl ARegister {
    fn decode(r: u8) -> Result<ARegister, DecoderError> {
        if r < 16 {
            Ok(ARegister::Register(Register::from_offset(r as usize)))
        } else if r < 32 {
            Ok(ARegister::MRegister(MachineRegister::from_offset(
                r as usize,
            )))
        } else if r < 48 {
            Ok(ARegister::IRegister(Register::from_offset(r as usize - 32)))
        } else if r < 64 {
            Ok(ARegister::IMRegister(MachineRegister::from_offset(
                r as usize - 32,
            )))
        } else if r == 64 {
            Ok(ARegister::None)
        } else if r == 65 {
            Ok(ARegister::Imm)
        } else {
            Err(DecoderError::BadInstruction)
        }
    }

    fn encode(&self) -> u8 {
        match self {
            ARegister::Imm => 65,
            ARegister::None => 64,
            ARegister::Register(r) => r.offset() as u8,
            ARegister::MRegister(r) => r.offset() as u8,
            ARegister::IRegister(r) => r.offset() as u8 + 32,
            ARegister::IMRegister(r) => r.offset() as u8 + 32,
        }
    }

    fn register(self) -> Result<Register, DecoderError> {
        match self {
            ARegister::Register(r) => Ok(r),
            _ => Err(DecoderError::BadInstruction),
        }
    }

    fn mregister(&self) -> Result<MachineRegister, DecoderError> {
        match self {
            ARegister::MRegister(r) => Ok(*r),
            _ => Err(DecoderError::BadInstruction),
        }
    }
}

struct MachAndOff {
    op1: Option<MachineRegister>,
    off: Option<Native>,
}
impl MachAndOff {
    fn encode(&self) -> Result<(u8, u64), EncoderError> {
        match (self.op1, self.off) {
            (Some(o1), Some(off)) => Ok((ARegister::IMRegister(o1).encode(), off.0 as u64)),
            (Some(o1), None) => Ok((ARegister::MRegister(o1).encode(), 0)),
            (None, Some(off)) => Ok((ARegister::None.encode(), off.0 as u64)),
            (None, None) => Err(EncoderError::BadInstruction),
        }
    }

    fn decode(r1: u8, off: u64) -> Result<Self, DecoderError> {
        let r = ARegister::decode(r1)?;
        match r {
            ARegister::MRegister(r) => Ok(Self {
                op1: Some(r),
                off: None,
            }),
            ARegister::IMRegister(r) => Ok(Self {
                op1: Some(r),
                off: Some(Native(off)),
            }),
            ARegister::None => Ok(Self {
                op1: None,
                off: Some(Native(off)),
            }),
            _ => Err(DecoderError::BadInstruction),
        }
    }
}
impl RegAndOff {
    fn encode(&self) -> Result<(u8, u64), EncoderError> {
        match (self.op1, self.off) {
            (Some(o1), Some(off)) => Ok((ARegister::IRegister(o1).encode(), off.0)),
            (Some(o1), None) => Ok((ARegister::Register(o1).encode(), 0)),
            (None, Some(off)) => Ok((ARegister::None.encode(), off.0)),
            (None, None) => Err(EncoderError::BadInstruction),
        }
    }

    fn decode(r1: u8, off: u64) -> Result<Self, DecoderError> {
        let r = ARegister::decode(r1)?;
        match r {
            ARegister::Register(r) => Ok(RegAndOff {
                op1: Some(r),
                off: None,
            }),
            ARegister::IRegister(r) => Ok(RegAndOff {
                op1: Some(r),
                off: Some(LispWord(off)),
            }),
            ARegister::None => Ok(RegAndOff {
                op1: None,
                off: Some(LispWord(off)),
            }),
            _ => Err(DecoderError::BadInstruction),
        }
    }
}

impl TwoRegs {
    fn encode(&self) -> (u8, u8) {
        (
            ARegister::Register(self.dst).encode(),
            ARegister::Register(self.src).encode(),
        )
    }

    fn decode(r1: u8, r2: u8) -> Result<Self, DecoderError> {
        let reg1 = ARegister::decode(r1)?.register()?;
        let reg2 = ARegister::decode(r2)?.register()?;
        Ok(TwoRegs {
            dst: reg1,
            src: reg2,
        })
    }
}

impl RegSource {
    fn encode(&self) -> Result<(u8, u8, u64), EncoderError> {
        let op1 = ARegister::Register(self.op1).encode();
        let op2 = RegAndOff {
            op1: self.op2,
            off: self.op3,
        }
        .encode()?;
        Ok((op1, op2.0, op2.1))
    }

    fn decode(r1: u8, r2: u8, imm: u64) -> Result<RegSource, DecoderError> {
        let op1 = ARegister::decode(r1)?.register()?;
        let op2 = RegAndOff::decode(r2, imm)?;
        Ok(RegSource {
            op1,
            op2: op2.op1,
            op3: op2.off,
        })
    }
}

impl cpu::Location {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Literal(l) => (ARegister::Imm.encode(), l.0),
            Self::Absolute(l) => (ARegister::None.encode(), l.0),
            Self::Register(r) => (ARegister::Register(*r).encode(), 0),
            Self::Machine(r) => (ARegister::MRegister(*r).encode(), 0),
            Self::IndirectRegister(a) => (ARegister::IRegister(*a).encode(), 0),
            Self::IndirectMachine(a, d) => (ARegister::IMRegister(*a).encode(), d.0 as u64),
        }
    }

    fn decode(reg: u8, off: u64) -> Result<Self, DecoderError> {
        let r = ARegister::decode(reg)?;
        match r {
            ARegister::Imm => Ok(Self::Literal(Native(off))),
            ARegister::None => Ok(Self::Absolute(Address(off))),
            ARegister::IMRegister(reg) => Ok(Self::IndirectMachine(reg, cpu::Offset(off as i64))),
            ARegister::IRegister(reg) => Ok(Self::IndirectRegister(reg)),
            ARegister::Register(reg) => Ok(Self::Register(reg)),
            ARegister::MRegister(reg) => Ok(Self::Machine(reg)),
        }
    }

    fn needs_second_word(&self) -> bool {
        match self {
            Location::Literal(_) => true,
            Location::Absolute(_) => true,
            Location::Machine(_) => false,
            Location::Register(_) => false,
            Location::IndirectMachine(_, _) => true,
            Location::IndirectRegister(_) => false,
        }
    }
}

impl cpu::JumpTarget {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Absolute(a) => (ARegister::None.encode(), a.0),
            Self::Register(r) => (ARegister::Register(*r).encode(), 0),
            Self::Machine(r, off) => (ARegister::MRegister(*r).encode(), off.0 as u64),
            Self::IndirectRegister(a) => (ARegister::IRegister(*a).encode() + 32, 0),
            Self::IndirectMachine(a, off) => {
                (ARegister::IMRegister(*a).encode() + 32, off.0 as u64)
            }
        }
    }

    fn decode(reg: u8, off: u64) -> Result<Self, DecoderError> {
        let r = ARegister::decode(reg)?;
        match r {
            ARegister::None => Ok(Self::Absolute(Address(off))),
            ARegister::IMRegister(reg) => Ok(Self::IndirectMachine(reg, cpu::Offset(off as i64))),
            ARegister::IRegister(reg) => Ok(Self::IndirectRegister(reg)),
            ARegister::Register(reg) => Ok(Self::Register(reg)),
            ARegister::MRegister(reg) => Ok(Self::Machine(reg, Offset(off as i64))),
            _ => Err(DecoderError::BadInstruction),
        }
    }
}

impl MachSource {
    fn encode(&self) -> Result<(u8, u8, u64), EncoderError> {
        let op1 = ARegister::MRegister(self.op1).encode();
        let op2 = MachAndOff {
            op1: self.op2,
            off: self.op3,
        }
        .encode()?;
        Ok((op1, op2.0, op2.1))
    }
    fn decode(r1: u8, r2: u8, imm: u64) -> Result<Self, DecoderError> {
        let op1 = ARegister::decode(r1)?.mregister()?;
        let op2 = MachAndOff::decode(r2, imm)?;
        Ok(Self {
            op1: op1,
            op2: op2.op1,
            op3: op2.off,
        })
    }
}

impl cpu::EitherSource {
    fn decode(r1: u8, r2: u8, hi: u64) -> Result<Self, DecoderError> {
        if r1 >= 16 {
            Ok(EitherSource::Mach(MachSource::decode(r1, r2, hi)?))
        } else {
            Ok(EitherSource::Reg(RegSource::decode(r1, r2, hi)?))
        }
    }
}

#[repr(u8)]
#[derive(IntEnum)]
enum Opcode {
    Halt,
    Nop,
    Jump,
    JumpIf,
    JumpIfNot,
    Int,
    IReturn,
    PushA,
    PopA,
    Mov,
    Mov8,
    AAdd,
    ASub,
    SetTag,
    GetTag,
    SetPayload,
    GetPayload,
    Req,
    Cons,
    Uncons,
    Car,
    Cdr,
    SetCar,
    SetCdr,
    Add,
    Mul,
    Shl,
    Shr,
    IDiv,
    Sub,
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    PushR,
    PopR,
    MakeClosure,
    Call,
    Return,
    Typep,
    MemCpy,
    DisableInterrupts,
    EnableInterrupts,
}

impl Instruction {
    pub fn decode(lo: u64, hi: u64) -> Result<Self, DecoderError> {
        let [opcode, r0, r1, r2, r3, ..] = lo.to_le_bytes();
        match Opcode::try_from(opcode).map_err(|_| DecoderError::BadInstruction)? {
            Opcode::Nop => Ok(Self::Nop),
            Opcode::Jump => Ok(Self::Jump {
                condition: cpu::Condition::Always,
                target: cpu::JumpTarget::decode(r0, hi)?,
            }),
            Opcode::JumpIf => Ok(Self::Jump {
                condition: cpu::Condition::True(ARegister::decode(r0)?.register()?),
                target: cpu::JumpTarget::decode(r1, hi)?,
            }),
            Opcode::JumpIfNot => Ok(Self::Jump {
                condition: cpu::Condition::False(ARegister::decode(r0)?.register()?),
                target: cpu::JumpTarget::decode(r1, hi)?,
            }),
            Opcode::Call => Ok(Self::Call {
                target: cpu::JumpTarget::decode(r0, hi)?,
            }),

            Opcode::Return => Ok(Self::Return),
            Opcode::MakeClosure => Ok(Self::MakeClosure {
                dst: ARegister::decode(r0)?.register()?,
                code: ARegister::decode(r1)?.mregister()?,
            }),

            Opcode::Eq => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Eq,
                dst: ARegister::decode(r0)?.register()?,
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Ne => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Ne,
                dst: ARegister::decode(r0)?.register()?,
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Gt => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Gt,
                dst: ARegister::decode(r0)?.register()?,
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Gte => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Gte,
                dst: ARegister::decode(r0)?.register()?,
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Lt => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Lt,
                dst: ARegister::decode(r0)?.register()?,
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Lte => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Lte,
                dst: ARegister::decode(r0)?.register()?,
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Add => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Add,
                dst: ARegister::decode(r0)?.register()?,
                operands: RegSource::decode(r1, r2, hi)?,
            }),
            Opcode::Sub => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Sub,
                dst: ARegister::decode(r0)?.register()?,
                operands: RegSource::decode(r1, r2, hi)?,
            }),
            Opcode::Mul => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Mul,
                dst: ARegister::decode(r0)?.register()?,
                operands: RegSource::decode(r1, r2, hi)?,
            }),
            Opcode::Shl => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shl,
                dst: ARegister::decode(r0)?.register()?,
                operands: RegSource::decode(r1, r2, hi)?,
            }),
            Opcode::Shr => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shr,
                dst: ARegister::decode(r0)?.register()?,
                operands: RegSource::decode(r1, r2, hi)?,
            }),

            Opcode::Car => Ok(Self::Car(TwoRegs::decode(r0, r1)?)),
            Opcode::Cdr => Ok(Self::Cdr(TwoRegs::decode(r0, r1)?)),
            Opcode::SetCar => Ok(Self::SetCar(TwoRegs::decode(r0, r1)?)),
            Opcode::SetCdr => Ok(Self::SetCdr(TwoRegs::decode(r0, r1)?)),
            Opcode::Cons => Ok(Self::Cons {
                dst: ARegister::decode(r0)?.register()?,
                car: ARegister::decode(r1)?.register()?,
                cdr: ARegister::decode(r2)?.register()?,
            }),
            Opcode::Uncons => Ok(Self::Uncons {
                car: ARegister::decode(r0)?.register()?,
                cdr: ARegister::decode(r1)?.register()?,
                src: ARegister::decode(r2)?.register()?,
            }),
            Opcode::IDiv => Ok(Self::IDiv {
                div: ARegister::decode(r0)?.register()?,
                rem: ARegister::decode(r1)?.register()?,
                operands: RegSource::decode(r2, r3, hi)?,
            }),
            Opcode::PopR => Ok(Self::PopR {
                dst: ARegister::decode(r0)?.register()?,
            }),
            Opcode::PushR => Ok(Self::PushR {
                src: ARegister::decode(r0)?.register()?,
            }),
            Opcode::PopA => Ok(Self::PopA {
                dst: ARegister::decode(r0)?.mregister()?,
            }),
            Opcode::PushA => Ok(Self::PushA {
                src: ARegister::decode(r0)?.mregister()?,
            }),
            Opcode::IReturn => Ok(Self::IReturn),
            Opcode::AAdd => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: ARegister::decode(r0)?.mregister()?,
                operands: cpu::MachSource::decode(r1, r2, hi)?,
            }),
            Opcode::ASub => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: ARegister::decode(r0)?.mregister()?,
                operands: cpu::MachSource::decode(r1, r2, hi)?,
            }),
            Opcode::GetPayload => Ok(Instruction::GetPayload {
                dst: ARegister::decode(r0)?.mregister()?,
                src: ARegister::decode(r1)?.register()?,
            }),
            Opcode::GetTag => Ok(Instruction::GetTag {
                dst: ARegister::decode(r0)?.mregister()?,
                src: ARegister::decode(r1)?.register()?,
            }),
            Opcode::Halt => Ok(Instruction::Halt),
            Opcode::Int => Ok(Instruction::Int(hi)),
            Opcode::SetPayload => Ok(Instruction::SetPayload {
                dst: ARegister::decode(r0)?.register()?,
                src: ARegister::decode(r1)?.mregister()?,
            }),
            Opcode::SetTag => Ok(Instruction::SetTag {
                dst: ARegister::decode(r0)?.register()?,
                src: ARegister::decode(r1)?.mregister()?,
            }),
            Opcode::Mov => {
                let dst = Location::decode(r0, hi)?;
                let src = Location::decode(r1, hi)?;

                Ok(Instruction::Mov { dst, src })
            }
            Opcode::Mov8 => {
                let dst = Location::decode(r0, hi)?;
                let src = Location::decode(r1, hi)?;

                Ok(Instruction::Mov8 { dst, src })
            }
            Opcode::Typep => Ok(Instruction::Typep {
                dst: ARegister::decode(r0)?.register()?,
                src: ARegister::decode(r1)?.register()?,
                compare: Native(hi),
            }),
            Opcode::MemCpy => Ok(Instruction::MemCpy {
                dst: ARegister::decode(r0)?.mregister()?,
                src: ARegister::decode(r1)?.mregister()?,
                count: RegAndOff::decode(r2, hi)?,
            }),
            Opcode::DisableInterrupts => Ok(Instruction::DisableInterrupts),
            Opcode::EnableInterrupts => Ok(Instruction::EnableInterrupts),
            Opcode::Req => Ok(Instruction::Req {
                dst: ARegister::decode(r0)?.register()?,
                size: ARegister::decode(r1)?.register()?,
            }),
        }
    }

    fn encode_div(div: Register, rem: Register, r: RegSource) -> Result<(u64, u64), EncoderError> {
        let (r2, r3, hi) = r.encode()?;
        Ok((
            u64::from_le_bytes([
                Opcode::IDiv.into(),
                ARegister::Register(div).encode(),
                ARegister::Register(rem).encode(),
                r2,
                r3,
                0,
                0,
                0,
            ]),
            hi,
        ))
    }

    fn encode_three_adrs(
        opcode: Opcode,
        dst: u8,
        target: &MachSource,
    ) -> Result<(u64, u64), EncoderError> {
        let (r1, r2, hi) = target.encode()?;
        Ok((
            u64::from_le_bytes([opcode.into(), dst, r1, r2, 0, 0, 0, 0]),
            hi,
        ))
    }

    fn encode_three_regs(
        opcode: Opcode,
        dst: u8,
        target: &RegSource,
    ) -> Result<(u64, u64), EncoderError> {
        let (r1, r2, hi) = target.encode()?;
        Ok((
            u64::from_le_bytes([opcode.into(), dst, r1, r2, 0, 0, 0, 0]),
            hi,
        ))
    }

    fn encode_three_either(
        opcode: Opcode,
        dst: u8,
        target: &EitherSource,
    ) -> Result<(u64, u64), EncoderError> {
        match target {
            EitherSource::Mach(m) => Ok(Self::encode_three_adrs(opcode, dst, m)?),
            EitherSource::Reg(r) => Ok(Self::encode_three_regs(opcode, dst, r)?),
        }
    }

    fn encode_two_regs(opcode: Opcode, target: &TwoRegs) -> (u64, u64) {
        let (r1, r2) = target.encode();
        (
            u64::from_le_bytes([opcode.into(), r1, r2, 0, 0, 0, 0, 0]),
            0,
        )
    }

    pub fn encode(&self) -> Result<(u64, u64), EncoderError> {
        Ok(match self {
            // Control flow
            Instruction::Halt => (
                u64::from_le_bytes([Opcode::Halt.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Nop => (
                u64::from_le_bytes([Opcode::Nop.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),

            Instruction::Jump {
                condition: crate::cpu::Condition::Always,
                target: addressing,
            } => {
                let (r0, hi) = addressing.encode();
                (
                    u64::from_le_bytes([Opcode::Jump.into(), r0, 0, 0, 0, 0, 0, 0]),
                    hi,
                )
            }
            Instruction::Jump {
                condition: crate::cpu::Condition::True(reg),
                target: addressing,
            } => {
                let (r1, hi) = addressing.encode();
                (
                    u64::from_le_bytes([
                        Opcode::JumpIf.into(),
                        ARegister::Register(*reg).encode(),
                        r1,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ]),
                    hi,
                )
            }
            Instruction::Jump {
                condition: crate::cpu::Condition::False(reg),
                target: addressing,
            } => {
                let (r1, hi) = addressing.encode();
                (
                    u64::from_le_bytes([
                        Opcode::JumpIfNot.into(),
                        ARegister::Register(*reg).encode(),
                        r1,
                        0,
                        0,
                        0,
                        0,
                        0,
                    ]),
                    hi,
                )
            }
            Instruction::Call { target: addressing } => {
                let (r0, hi) = addressing.encode();
                (
                    u64::from_le_bytes([Opcode::Call.into(), r0, 0, 0, 0, 0, 0, 0]),
                    hi,
                )
            }

            Instruction::Return => (
                u64::from_le_bytes([Opcode::Return.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::MakeClosure { dst, code } => (
                u64::from_le_bytes([
                    Opcode::MakeClosure.into(),
                    ARegister::Register(*dst).encode(),
                    ARegister::MRegister(*code).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),

            // Comparison
            Instruction::Comparison { op, dst, operands } => {
                let opcode = match op {
                    cpu::Comparison::Eq => Opcode::Eq,
                    cpu::Comparison::Gt => Opcode::Gt,
                    cpu::Comparison::Gte => Opcode::Gte,
                    cpu::Comparison::Lt => Opcode::Lt,
                    cpu::Comparison::Lte => Opcode::Lte,
                    cpu::Comparison::Ne => Opcode::Ne,
                };
                Self::encode_three_either(opcode, ARegister::Register(*dst).encode(), operands)?
            }
            Instruction::Binary { op, dst, operands } => {
                let opcode = match op {
                    cpu::BinaryOp::Add => Opcode::Add,
                    cpu::BinaryOp::Mul => Opcode::Mul,
                    cpu::BinaryOp::Shl => Opcode::Shl,
                    cpu::BinaryOp::Shr => Opcode::Shr,
                    cpu::BinaryOp::Sub => Opcode::Sub,
                };
                Self::encode_three_regs(opcode, ARegister::Register(*dst).encode(), operands)?
            }

            // Cons
            Instruction::Car(regs) => Self::encode_two_regs(Opcode::Car, regs),
            Instruction::Cdr(regs) => Self::encode_two_regs(Opcode::Cdr, regs),
            Instruction::SetCar(regs) => Self::encode_two_regs(Opcode::SetCar, regs),
            Instruction::SetCdr(regs) => Self::encode_two_regs(Opcode::SetCdr, regs),
            Instruction::Cons { dst, car, cdr } => (
                u64::from_le_bytes([
                    Opcode::Cons.into(),
                    ARegister::Register(*dst).encode(),
                    ARegister::Register(*car).encode(),
                    ARegister::Register(*cdr).encode(),
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::Uncons { car, cdr, src } => (
                u64::from_le_bytes([
                    Opcode::Uncons.into(),
                    ARegister::Register(*car).encode(),
                    ARegister::Register(*cdr).encode(),
                    ARegister::Register(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),

            &Instruction::IDiv { div, rem, operands } => {
                Instruction::encode_div(div, rem, operands)?
            }
            Instruction::PopR { dst } => (
                u64::from_le_bytes([
                    Opcode::PopR.into(),
                    ARegister::Register(*dst).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::PushR { src } => (
                u64::from_le_bytes([
                    Opcode::PushR.into(),
                    ARegister::Register(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::PopA { dst } => (
                u64::from_le_bytes([
                    Opcode::PopA.into(),
                    ARegister::MRegister(*dst).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::PushA { src } => (
                u64::from_le_bytes([
                    Opcode::PushA.into(),
                    ARegister::MRegister(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::IReturn => (
                u64::from_le_bytes([Opcode::IReturn.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::MBinary { op, dst, operands } => {
                let opcode = match op {
                    cpu::MBinaryOp::Add => Opcode::AAdd,
                    cpu::MBinaryOp::Sub => Opcode::ASub,
                };
                Self::encode_three_adrs(opcode, ARegister::MRegister(*dst).encode(), operands)?
            }
            Instruction::GetPayload { dst, src } => (
                u64::from_le_bytes([
                    Opcode::GetPayload.into(),
                    ARegister::MRegister(*dst).encode(),
                    ARegister::Register(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::GetTag { src, dst } => (
                u64::from_le_bytes([
                    Opcode::GetTag.into(),
                    ARegister::MRegister(*dst).encode(),
                    ARegister::Register(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::Int(count) => (
                u64::from_le_bytes([Opcode::Int.into(), 0, 0, 0, 0, 0, 0, 0]),
                *count,
            ),
            Instruction::Mov { dst, src } => {
                if dst.needs_second_word() && src.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (edst, doff) = dst.encode();
                let (esrc, soff) = src.encode();
                (
                    u64::from_le_bytes([Opcode::Mov.into(), edst, esrc, 0, 0, 0, 0, 0]),
                    doff + soff,
                )
            }
            Instruction::SetPayload { dst, src } => (
                u64::from_le_bytes([
                    Opcode::SetPayload.into(),
                    ARegister::Register(*dst).encode(),
                    ARegister::MRegister(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::SetTag { src, dst } => (
                u64::from_le_bytes([
                    Opcode::SetTag.into(),
                    ARegister::Register(*dst).encode(),
                    ARegister::MRegister(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::MemCpy { dst, src, count } => {
                let (count, off) = count.encode()?;
                (
                    u64::from_le_bytes([
                        Opcode::MemCpy.into(),
                        ARegister::MRegister(*dst).encode(),
                        ARegister::MRegister(*src).encode(),
                        count,
                        0,
                        0,
                        0,
                        0,
                    ]),
                    off,
                )
            }
            // Instruction::MemSet { dst: _, src: _, count: _ } => todo!(),
            Instruction::Mov8 { dst, src } => {
                let (edst, doff) = dst.encode();
                let (esrc, soff) = src.encode();
                if doff != 0 && soff != 0 {
                    panic!("Somehow you managed to construct an instruction with two offsets");
                }
                (
                    u64::from_le_bytes([Opcode::Mov8.into(), edst, esrc, 0, 0, 0, 0, 0]),
                    doff + soff,
                )
            }
            Instruction::Typep { dst, src, compare } => (
                u64::from_le_bytes([
                    Opcode::Typep.into(),
                    ARegister::Register(*dst).encode(),
                    ARegister::Register(*src).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                compare.0,
            ),
            Instruction::DisableInterrupts => (
                u64::from_le_bytes([Opcode::DisableInterrupts.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::EnableInterrupts => (
                u64::from_le_bytes([Opcode::EnableInterrupts.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::Req { dst, size } => (
                u64::from_le_bytes([
                    Opcode::Req.into(),
                    ARegister::Register(*dst).encode(),
                    ARegister::Register(*size).encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
        })
    }
}

#[cfg(test)]
mod test {
    use crate::cpu::{
        self,
        assembler::{parser, test::expected_resolved},
    };

    #[test]
    fn encode_decode() -> Result<(), String> {
        let resolved = expected_resolved();

        for s in resolved.sections {
            for i in s.lines {
                match i {
                    parser::AssemblyLine::ResolvedInstruction(r) => {
                        let (lo, hi) = r.encode().map_err(|_| format!("Error encoding {:?}", r))?;
                        dbg!(lo.to_le_bytes(), hi);
                        let decoded = cpu::Instruction::decode(lo, hi)
                            .map_err(|_| format!("Error decoding {:?}", r))?;
                        let (lo, hi) = decoded
                            .encode()
                            .map_err(|_| format!("Error second encoding {:?}", r))?;
                        let decoded = cpu::Instruction::decode(lo, hi)
                            .map_err(|_| format!("Error second decoding {:?}", r))?;
                        assert_eq!(r, decoded);
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
