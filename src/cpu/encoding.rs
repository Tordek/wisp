use int_enum::IntEnum;

use crate::{
    cpu::{
        self, Count, EitherSource, Instruction, LispWord, Location, MachSource, MachineRegister,
        Native, RegSource, Register, Trap, TwoRegs,
    },
    bus::{Address, Offset},
};

impl cpu::Register {
    fn encode(&self) -> u8 {
        self.0
    }

    fn decode(reg: u8) -> Self {
        Self(reg)
    }

    const NONE: u8 = 0x55;
}

impl MachineRegister {
    fn encode(&self) -> u8 {
        self.0
    }
    fn decode(reg: u8) -> Self {
        Self(reg)
    }
}

impl TwoRegs {
    fn decode(r1: u8, r2: u8) -> TwoRegs {
        TwoRegs {
            dst: Register(r1),
            src: Register(r2),
        }
    }
}

impl RegSource {
    fn encode(&self) -> (u8, u8, u64) {
        EitherSource::encode_reg_source(*self)
    }

    fn decode(r1: u8, r2: u8, imm: u64) -> RegSource {
        EitherSource::decode_reg_source(r1, r2, imm)
    }
}

impl cpu::Location {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Literal(l) => (Register::NONE, l.0),
            Self::Absolute(l) => (Register::NONE + 1, l.0),
            Self::Register(r) => (r.encode(), 0),
            Self::Machine(r) => (r.encode() + 16, 0),
            Self::IndirectRegister(a) => (a.encode() + 24, 0),
            Self::IndirectMachine(a, d) => (a.encode() + 40, d.0 as u64),
        }
    }

    fn decode(reg: u8, off: u64) -> Result<Self, Trap> {
        if reg == Register::NONE {
            Ok(Self::Literal(Native(off)))
        } else if reg == Register::NONE + 1 {
            Ok(Self::Absolute(Address(off)))
        } else if reg < 16 {
            Ok(Self::Register(Register::decode(reg)))
        } else if reg < 24 {
            Ok(Self::Machine(MachineRegister::decode(reg - 16)))
        } else if reg < 40 {
            Ok(Self::IndirectRegister(Register::decode(reg - 24)))
        } else {
            Ok(Self::IndirectMachine(
                MachineRegister::decode(reg - 40),
                cpu::Offset(off as i64),
            ))
        }
    }
}

impl cpu::JumpTarget {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Absolute(a) => (Register::NONE, a.0),
            Self::Register(r) => (r.encode(), 0),
            Self::Machine(r, off) => (r.encode() + 16, off.0 as u64),
            Self::IndirectRegister(a) => (a.encode() + 24, 0),
            Self::IndirectMachine(a, off) => (a.encode() + 40, off.0 as u64),
        }
    }

    fn decode(reg: u8, off: u64) -> Result<Self, Trap> {
        if reg == Register::NONE {
            Ok(Self::Absolute(cpu::Address(off)))
        } else if reg < 16 {
            Ok(Self::Register(Register::decode(reg)))
        } else if reg < 24 {
            Ok(Self::Machine(
                MachineRegister::decode(reg - 16),
                Offset(off as i64),
            ))
        } else if reg < 40 {
            Ok(Self::IndirectRegister(Register::decode(reg - 24)))
        } else {
            Ok(Self::IndirectMachine(
                MachineRegister::decode(reg - 40),
                cpu::Offset(off as i64),
            ))
        }
    }
}

impl MachSource {
    fn encode(&self) -> (u8, u8, u64) {
        EitherSource::encode_mach_source(*self)
    }
    fn decode(r1: u8, r2: u8, imm: u64) -> Self {
        EitherSource::decode_mach_source(r1, r2, imm)
    }
}

impl cpu::EitherSource {
    fn encode_reg_source(RegSource { op1, op2, op3 }: RegSource) -> (u8, u8, u64) {
        Self::encode_either_source(op1.encode(), op2.map(|v| v.encode()), op3.map(|v| v.0))
    }

    fn encode_mach_source(MachSource { op1, op2, op3 }: MachSource) -> (u8, u8, u64) {
        let (op1, op2, op3) =
            Self::encode_either_source(op1.encode(), op2.map(|v| v.encode()), op3.map(|v| v.0));
        (op1 + 128, op2 + 128, op3)
    }

    fn encode_either_source(op1: u8, op2: Option<u8>, op3: Option<u64>) -> (u8, u8, u64) {
        match (op2, op3) {
            (None, None) => (op1, 64, 0),
            (None, Some(o)) => (op1, 65, o),
            (Some(r), None) => (op1, r + 48, 0),
            (Some(r), Some(o)) => (op1, r, o),
        }
    }

    fn decode_reg_source(r1: u8, r2: u8, imm: u64) -> RegSource {
        if r2 == 64 {
            RegSource {
                op1: Register::decode(r1),
                op2: None,
                op3: None,
            }
        } else if r2 == 65 {
            RegSource {
                op1: Register::decode(r1),
                op2: None,
                op3: Some(LispWord(imm)),
            }
        } else if r2 >= 48 {
            RegSource {
                op1: Register::decode(r1),
                op2: Some(Register::decode(r2 - 48)),
                op3: None,
            }
        } else {
            RegSource {
                op1: Register::decode(r1),
                op2: Some(Register::decode(r2)),
                op3: Some(LispWord(imm)),
            }
        }
    }

    fn decode_mach_source(r1: u8, r2: u8, imm: u64) -> MachSource {
        let (r1, r2) = (r1 - 128, r2 - 128);
        if r2 == 64 {
            MachSource {
                op1: MachineRegister::decode(r1),
                op2: None,
                op3: None,
            }
        } else if r2 == 65 {
            MachSource {
                op1: MachineRegister::decode(r1),
                op2: None,
                op3: Some(Native(imm)),
            }
        } else if r2 >= 48 {
            MachSource {
                op1: MachineRegister::decode(r1),
                op2: Some(MachineRegister::decode(r2 - 48)),
                op3: None,
            }
        } else {
            MachSource {
                op1: MachineRegister::decode(r1),
                op2: Some(MachineRegister::decode(r2)),
                op3: Some(Native(imm)),
            }
        }
    }

    fn decode(r1: u8, r2: u8, hi: u64) -> Result<Self, Trap> {
        if r1 >= 24 {
            Ok(EitherSource::Mach(MachSource::decode(r1, r2, hi)))
        } else {
            Ok(EitherSource::Reg(RegSource::decode(r1, r2, hi)))
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
}

// TODO: Find a real encoding/decoding.
impl Instruction {
    pub fn decode(lo: u64, hi: u64) -> Result<Self, Trap> {
        let [opcode, r0, r1, r2, r3, ..] = lo.to_le_bytes();
        match Opcode::try_from(opcode).map_err(|_| Trap::InvalidInstruction)? {
            Opcode::Nop => Ok(Self::Nop),
            Opcode::Jump => Ok(Self::Jump {
                condition: cpu::Condition::Always,
                target: cpu::JumpTarget::decode(r0, hi)?,
            }),
            Opcode::JumpIf => Ok(Self::Jump {
                condition: cpu::Condition::True(Register::decode(r0)),
                target: cpu::JumpTarget::decode(r1, hi)?,
            }),
            Opcode::JumpIfNot => Ok(Self::Jump {
                condition: cpu::Condition::False(Register::decode(r0)),
                target: cpu::JumpTarget::decode(r1, hi)?,
            }),
            Opcode::Call => Ok(Self::Call {
                target: cpu::JumpTarget::decode(r0, hi)?,
            }),

            Opcode::Return => Ok(Self::Return),
            Opcode::MakeClosure => Ok(Self::MakeClosure {
                dst: Register::decode(r0),
                code: MachineRegister::decode(r1),
            }),

            Opcode::Eq => Ok(Self::MComparison {
                op: crate::cpu::Comparison::Eq,
                dst: Register::decode(r0),
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Ne => Ok(Self::MComparison {
                op: crate::cpu::Comparison::Ne,
                dst: Register::decode(r0),
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Gt => Ok(Self::MComparison {
                op: crate::cpu::Comparison::Gt,
                dst: Register::decode(r0),
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Gte => Ok(Self::MComparison {
                op: crate::cpu::Comparison::Gte,
                dst: Register::decode(r0),
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Lt => Ok(Self::MComparison {
                op: crate::cpu::Comparison::Lt,
                dst: Register::decode(r0),
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Lte => Ok(Self::MComparison {
                op: crate::cpu::Comparison::Lte,
                dst: Register::decode(r0),
                operands: cpu::EitherSource::decode(r1, r2, hi)?,
            }),
            Opcode::Add => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Add,
                dst: Register::decode(r0),
                operands: RegSource::decode(r1, r2, hi),
            }),
            Opcode::Sub => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Sub,
                dst: Register::decode(r0),
                operands: RegSource::decode(r1, r2, hi),
            }),
            Opcode::Mul => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Mul,
                dst: Register::decode(r0),
                operands: RegSource::decode(r1, r2, hi),
            }),
            Opcode::Shl => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shl,
                dst: Register::decode(r0),
                operands: RegSource::decode(r1, r2, hi),
            }),
            Opcode::Shr => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shr,
                dst: Register::decode(r0),
                operands: RegSource::decode(r1, r2, hi),
            }),

            Opcode::Car => Ok(Self::Car(TwoRegs::decode(r0, r1))),
            Opcode::Cdr => Ok(Self::Cdr(TwoRegs::decode(r0, r1))),
            Opcode::SetCar => Ok(Self::SetCar(TwoRegs::decode(r0, r1))),
            Opcode::SetCdr => Ok(Self::SetCdr(TwoRegs::decode(r0, r1))),
            Opcode::Uncons => Ok(Self::Uncons {
                car: Register::decode(r0),
                cdr: Register::decode(r1),
                src: Register::decode(r2),
            }),
            Opcode::IDiv => Ok(Self::IDiv {
                div: Register::decode(r0),
                rem: Register::decode(r1),
                operands: RegSource::decode(r2, r3, hi),
            }),
            Opcode::PopR => Ok(Self::PopR {
                dst: Register::decode(r0),
            }),
            Opcode::PushR => Ok(Self::PushR {
                src: Register::decode(r0),
            }),
            Opcode::PopA => Ok(Self::PopA {
                dst: MachineRegister::decode(r0),
            }),
            Opcode::PushA => Ok(Self::PushA {
                src: MachineRegister::decode(r0),
            }),
            Opcode::IReturn => Ok(Self::IReturn),
            Opcode::AAdd => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: cpu::MachineRegister::decode(r0),
                operands: cpu::MachSource::decode(r1, r2, hi),
            }),
            Opcode::ASub => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: cpu::MachineRegister::decode(r0),
                operands: cpu::MachSource::decode(r1, r2, hi),
            }),
            Opcode::GetPayload => Ok(Instruction::GetPayload {
                dst: MachineRegister::decode(r0),
                src: Register::decode(r1),
            }),
            Opcode::GetTag => Ok(Instruction::GetTag {
                dst: MachineRegister::decode(r0),
                src: Register::decode(r1),
            }),
            Opcode::Halt => Ok(Instruction::Halt),
            Opcode::Int => Ok(Instruction::Int(hi)),
            Opcode::SetPayload => Ok(Instruction::SetPayload {
                dst: Register::decode(r0),
                src: MachineRegister::decode(r1),
            }),
            Opcode::SetTag => Ok(Instruction::SetTag {
                dst: Register::decode(r0),
                src: MachineRegister::decode(r1),
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
                dst: Register::decode(r0),
                src: Register::decode(r1),
                compare: Native(hi),
            }),
            Opcode::MemCpy => Ok(Instruction::MemCpy {
                dst: MachineRegister::decode(r0),
                src: MachineRegister::decode(r1),
                count: Count(hi),
            }),
        }
    }

    fn encode_div(div: Register, rem: Register, r: RegSource) -> (u64, u64) {
        let (r2, r3, hi) = r.encode();
        (
            u64::from_le_bytes([
                Opcode::IDiv.into(),
                div.encode(),
                rem.encode(),
                r2,
                r3,
                0,
                0,
                0,
            ]),
            hi,
        )
    }
    fn encode_three_adrs(opcode: Opcode, dst: u8, target: &MachSource) -> (u64, u64) {
        let (r1, r2, hi) = target.encode();
        (
            u64::from_le_bytes([opcode.into(), dst, r1, r2, 0, 0, 0, 0]),
            hi,
        )
    }
    fn encode_three_regs(opcode: Opcode, dst: u8, target: &RegSource) -> (u64, u64) {
        let (r1, r2, hi) = target.encode();
        (
            u64::from_le_bytes([opcode.into(), dst, r1, r2, 0, 0, 0, 0]),
            hi,
        )
    }
    fn encode_three_either(opcode: Opcode, dst: u8, target: &EitherSource) -> (u64, u64) {
        match target {
            EitherSource::Mach(m) => Self::encode_three_adrs(opcode, dst, m),
            EitherSource::Reg(r) => Self::encode_three_regs(opcode, dst, r),
        }
    }
    fn encode_two_regs(opcode: Opcode, target: &TwoRegs) -> (u64, u64) {
        match target {
            &TwoRegs { dst, src } => (
                u64::from_le_bytes([
                    opcode.into(),
                    dst.encode(),
                    src.encode(),
                    Register::NONE,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
        }
    }

    pub fn encode(&self) -> (u64, u64) {
        match self {
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
                    u64::from_le_bytes([Opcode::JumpIf.into(), reg.encode(), r1, 0, 0, 0, 0, 0]),
                    hi,
                )
            }
            Instruction::Jump {
                condition: crate::cpu::Condition::False(reg),
                target: addressing,
            } => {
                let (r1, hi) = addressing.encode();
                (
                    u64::from_le_bytes([Opcode::JumpIfNot.into(), reg.encode(), r1, 0, 0, 0, 0, 0]),
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
                    dst.encode(),
                    code.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),

            // Comparison
            Instruction::MComparison {
                op: crate::cpu::Comparison::Eq,
                dst,
                operands,
            } => Self::encode_three_either(Opcode::Eq, dst.encode(), operands),
            Instruction::MComparison {
                op: crate::cpu::Comparison::Ne,
                dst,
                operands,
            } => Self::encode_three_either(Opcode::Ne, dst.encode(), operands),
            Instruction::MComparison {
                op: crate::cpu::Comparison::Gt,
                dst,
                operands,
            } => Self::encode_three_either(Opcode::Gt, dst.encode(), operands),
            Instruction::MComparison {
                op: crate::cpu::Comparison::Gte,
                dst,
                operands,
            } => Self::encode_three_either(Opcode::Gte, dst.encode(), operands),
            Instruction::MComparison {
                op: crate::cpu::Comparison::Lt,
                dst,
                operands,
            } => Self::encode_three_either(Opcode::Lt, dst.encode(), operands),
            Instruction::MComparison {
                op: crate::cpu::Comparison::Lte,
                dst,
                operands,
            } => Self::encode_three_either(Opcode::Lte, dst.encode(), operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Add,
                dst,
                operands,
            } => Self::encode_three_regs(Opcode::Add, dst.encode(), operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Sub,
                dst,
                operands,
            } => Self::encode_three_regs(Opcode::Sub, dst.encode(), operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Mul,
                dst,
                operands,
            } => Self::encode_three_regs(Opcode::Mul, dst.encode(), operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Shl,
                dst,
                operands,
            } => Self::encode_three_regs(Opcode::Shl, dst.encode(), operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Shr,
                dst,
                operands,
            } => Self::encode_three_regs(Opcode::Shr, dst.encode(), operands),

            // Cons
            Instruction::Car(regs) => Self::encode_two_regs(Opcode::Car, regs),
            Instruction::Cdr(regs) => Self::encode_two_regs(Opcode::Cdr, regs),
            Instruction::SetCar(regs) => Self::encode_two_regs(Opcode::SetCar, regs),
            Instruction::SetCdr(regs) => Self::encode_two_regs(Opcode::SetCdr, regs),
            Instruction::Uncons { car, cdr, src } => (
                u64::from_le_bytes([
                    Opcode::Uncons.into(),
                    car.encode(),
                    cdr.encode(),
                    src.encode(),
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),

            &Instruction::IDiv { div, rem, operands } => {
                Instruction::encode_div(div, rem, operands)
            }
            Instruction::PopR { dst } => (
                u64::from_le_bytes([Opcode::PopR.into(), dst.encode(), 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::PushR { src } => (
                u64::from_le_bytes([Opcode::PushR.into(), src.encode(), 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::PopA { dst } => (
                u64::from_le_bytes([Opcode::PopA.into(), dst.encode(), 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::PushA { src } => (
                u64::from_le_bytes([Opcode::PushA.into(), src.encode(), 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::IReturn => (
                u64::from_le_bytes([Opcode::IReturn.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Add,
                dst,
                operands,
            } => Self::encode_three_adrs(Opcode::AAdd, dst.encode(), operands),
            Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Sub,
                dst,
                operands,
            } => Self::encode_three_adrs(Opcode::ASub, dst.encode(), operands),
            Instruction::GetPayload { dst, src } => (
                u64::from_le_bytes([
                    Opcode::GetPayload.into(),
                    dst.encode(),
                    src.encode(),
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
                    dst.encode(),
                    src.encode(),
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
                let (edst, doff) = dst.encode();
                let (esrc, soff) = src.encode();
                if doff != 0 && soff != 0 {
                    panic!("Somehow you managed to construct an instruction with two offsets");
                }
                (
                    u64::from_le_bytes([Opcode::Mov.into(), edst, esrc, 0, 0, 0, 0, 0]),
                    doff + soff,
                )
            }
            Instruction::SetPayload { dst, src } => (
                u64::from_le_bytes([
                    Opcode::SetPayload.into(),
                    dst.encode(),
                    src.encode(),
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
                    dst.encode(),
                    src.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::MemCpy { dst, src, count } => (
                u64::from_le_bytes([
                    Opcode::MemCpy.into(),
                    dst.encode(),
                    src.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                count.0,
            ),
            Instruction::MemSet { dst, src, count } => todo!(),
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
                    dst.encode(),
                    src.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                compare.0,
            ),
        }
    }
}
