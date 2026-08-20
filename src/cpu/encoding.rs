use bitflags::bitflags;
use int_enum::IntEnum;

use crate::{
    bus::Address,
    cpu::{self, Instruction, Native, RegAndOff, Register},
};

#[derive(Debug)]
pub enum EncoderError {
    BadInstruction,
}

#[derive(Debug)]
pub enum DecoderError {
    BadInstruction,
}

bitflags! {
    #[derive(PartialEq, Debug)]
    pub struct Tiebreaker: u8 {
        const NONE = 0;
        const RVALUE_OFF = 1;
        const LVALUE_OFF = 2;
        const RVALUE_LISP = 4;
        const LVALUE_LISP = 8;
        const RVALUE_IND = 16;
        const LVALUE_IND = 32;
        const RVALUE_LITERAL = 64;
    }
}

impl cpu::RValue {
    fn encode(&self) -> (Option<Register>, Native, Tiebreaker) {
        match self {
            Self::Literal(l) => (None, *l, Tiebreaker::RVALUE_LITERAL),
            Self::Absolute(l) => (None, Native(l.0), Tiebreaker::RVALUE_OFF),
            Self::Register(RegAndOff { op1, off }) => (
                Some(*op1),
                off.unwrap_or(Native(0)),
                if off.is_some() {
                    Tiebreaker::RVALUE_OFF
                } else {
                    Tiebreaker::NONE
                },
            ),
            Self::Indirect(RegAndOff { op1, off }) => (
                Some(*op1),
                off.unwrap_or(Native(0)),
                Tiebreaker::RVALUE_IND
                    | if off.is_some() {
                        Tiebreaker::RVALUE_OFF
                    } else {
                        Tiebreaker::NONE
                    },
            ),
            Self::LPointer(RegAndOff { op1, off }) => (
                Some(*op1),
                off.unwrap_or(Native(0)),
                Tiebreaker::RVALUE_LISP
                    | if off.is_some() {
                        Tiebreaker::RVALUE_OFF
                    } else {
                        Tiebreaker::NONE
                    },
            ),
        }
    }

    fn decode(
        register: Option<Register>,
        off: Native,
        tiebreak: &Tiebreaker,
    ) -> Result<Self, DecoderError> {
        match register {
            Some(op1) if tiebreak.contains(Tiebreaker::RVALUE_LISP) => {
                Ok(cpu::RValue::LPointer(RegAndOff {
                    op1,
                    off: tiebreak.contains(Tiebreaker::RVALUE_OFF).then_some(off),
                }))
            }
            Some(op1) if tiebreak.contains(Tiebreaker::RVALUE_IND) => {
                Ok(cpu::RValue::Indirect(RegAndOff {
                    op1,
                    off: tiebreak.contains(Tiebreaker::RVALUE_OFF).then_some(off),
                }))
            }
            Some(op1) => Ok(cpu::RValue::Register(RegAndOff {
                op1,
                off: tiebreak.contains(Tiebreaker::RVALUE_OFF).then_some(off),
            })),
            None if *tiebreak == Tiebreaker::RVALUE_LITERAL => Ok(cpu::RValue::Literal(off)),
            None => Ok(cpu::RValue::Absolute(Address::from(off))),
        }
    }

    fn needs_second_word(&self) -> bool {
        match self {
            cpu::RValue::Literal(_) => true,
            cpu::RValue::Absolute(_) => true,
            cpu::RValue::Register(RegAndOff { op1: _, off }) => off.is_some(),
            cpu::RValue::Indirect(RegAndOff { op1: _, off }) => off.is_some(),
            cpu::RValue::LPointer(RegAndOff { op1: _, off }) => off.is_some(),
        }
    }
}

impl cpu::LValue {
    fn encode(&self) -> (Option<Register>, Native, Tiebreaker) {
        match self {
            Self::Absolute(a) => (None, Native(a.0), Tiebreaker::NONE),
            Self::Register(r) => (Some(*r), Native(0), Tiebreaker::NONE),
            Self::Indirect(RegAndOff { op1, off }) => (
                Some(*op1),
                off.unwrap_or(Native(0)),
                Tiebreaker::LVALUE_IND
                    | if off.is_some() {
                        Tiebreaker::LVALUE_OFF
                    } else {
                        Tiebreaker::NONE
                    },
            ),
            Self::LPointer(RegAndOff { op1, off }) => (
                Some(*op1),
                off.unwrap_or(Native(0)),
                Tiebreaker::LVALUE_IND
                    | Tiebreaker::LVALUE_LISP
                    | if off.is_some() {
                        Tiebreaker::LVALUE_OFF
                    } else {
                        Tiebreaker::NONE
                    },
            ),
        }
    }

    fn decode(
        reg: Option<Register>,
        off: Native,
        tiebreak: &Tiebreaker,
    ) -> Result<Self, DecoderError> {
        match reg {
            None => Ok(cpu::LValue::Absolute(Address::from(off))),
            Some(op1) if tiebreak.contains(Tiebreaker::LVALUE_LISP) => {
                Ok(cpu::LValue::LPointer(RegAndOff {
                    op1,
                    off: tiebreak.contains(Tiebreaker::LVALUE_OFF).then_some(off),
                }))
            }

            Some(op1) if tiebreak.contains(Tiebreaker::LVALUE_IND) => {
                Ok(cpu::LValue::Indirect(RegAndOff {
                    op1,
                    off: tiebreak.contains(Tiebreaker::LVALUE_OFF).then_some(off),
                }))
            }
            Some(d) => Ok(cpu::LValue::Register(d)),
        }
    }

    fn needs_second_word(&self) -> bool {
        match self {
            cpu::LValue::Absolute(_) => true,
            cpu::LValue::Register(_) => false,
            cpu::LValue::Indirect(RegAndOff { op1: _, off }) => off.is_some(),
            cpu::LValue::LPointer(RegAndOff { op1: _, off }) => off.is_some(),
        }
    }
}

#[repr(u8)]
#[derive(IntEnum, Debug)]
enum Opcode {
    Halt,
    Nop,
    Jump,
    JumpIf,
    JumpIfNot,
    Int,
    IReturn,
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
    Push,
    Pop,
    MakeClosure,
    Call,
    Return,
    Typep,
    MemCpy,
    DisableInterrupts,
    EnableInterrupts,
}

bitflags! {
    #[derive(PartialEq, Debug)]
    pub struct ParamShape: u8 {
        const NONE = 0;
        const P0 = 1;
        const P1 = 2;
        const P2 = 4;
        const P3 = 8;
        const P4 = 16;
    }
}
impl Instruction {
    #[allow(clippy::type_complexity)]
    fn decode_params(
        lo: u64,
        off: u64,
    ) -> Result<
        (
            Opcode,
            Option<Register>,
            Option<Register>,
            Option<Register>,
            Option<Register>,
            Option<Register>,
            Tiebreaker,
            Native,
        ),
        DecoderError,
    > {
        let [opcode, param_shape, p0, p1, p2, p3, p4, tiebreak] = lo.to_le_bytes();

        let param_shape = ParamShape::from_bits_retain(param_shape);

        let r0 = if param_shape.contains(ParamShape::P0) {
            Some(Register(p0))
        } else {
            None
        };
        let r1 = if param_shape.contains(ParamShape::P1) {
            Some(Register(p1))
        } else {
            None
        };
        let r2 = if param_shape.contains(ParamShape::P2) {
            Some(Register(p2))
        } else {
            None
        };
        let r3 = if param_shape.contains(ParamShape::P3) {
            Some(Register(p3))
        } else {
            None
        };
        let r4 = if param_shape.contains(ParamShape::P4) {
            Some(Register(p4))
        } else {
            None
        };

        Ok((
            Opcode::try_from(opcode).map_err(|_| DecoderError::BadInstruction)?,
            r0,
            r1,
            r2,
            r3,
            r4,
            Tiebreaker::from_bits_truncate(tiebreak),
            Native(off),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_params(
        opcode: Opcode,
        p0: Option<Register>,
        p1: Option<Register>,
        p2: Option<Register>,
        p3: Option<Register>,
        p4: Option<Register>,
        tiebreak: Tiebreaker,
        off: Native,
    ) -> (u64, u64) {
        let mut shape = ParamShape::NONE;
        let r0 = match p0 {
            None => 0,
            Some(r) => {
                shape |= ParamShape::P0;
                r.0
            }
        };
        let r1 = match p1 {
            None => 0,
            Some(r) => {
                shape |= ParamShape::P1;
                r.0
            }
        };
        let r2 = match p2 {
            None => 0,
            Some(r) => {
                shape |= ParamShape::P2;
                r.0
            }
        };
        let r3 = match p3 {
            None => 0,
            Some(r) => {
                shape |= ParamShape::P3;
                r.0
            }
        };
        let r4 = match p4 {
            None => 0,
            Some(r) => {
                shape |= ParamShape::P4;
                r.0
            }
        };

        (
            u64::from_le_bytes([
                opcode.into(),
                shape.bits(),
                r0,
                r1,
                r2,
                r3,
                r4,
                tiebreak.bits(),
            ]),
            off.0,
        )
    }

    pub fn decode(lo: u64, hi: u64) -> Result<Self, DecoderError> {
        let (opcode, r0, r1, r2, r3, _, tiebreak, off) = Self::decode_params(lo, hi)?;

        match opcode {
            Opcode::Nop => Ok(Self::Nop),
            Opcode::Jump => Ok(Self::Jump {
                condition: cpu::Condition::Always,
                target: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::JumpIf => Ok(Self::Jump {
                condition: cpu::Condition::True(r0.ok_or(DecoderError::BadInstruction)?),
                target: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::JumpIfNot => Ok(Self::Jump {
                condition: cpu::Condition::False(r0.ok_or(DecoderError::BadInstruction)?),
                target: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Call => Ok(Self::Call {
                target: cpu::RValue::decode(r0, off, &tiebreak)?,
            }),

            Opcode::Return => Ok(Self::Return),
            Opcode::MakeClosure => Ok(Self::MakeClosure {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                code: r1.ok_or(DecoderError::BadInstruction)?,
            }),

            Opcode::Eq => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Eq,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Ne => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Ne,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Gt => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Gt,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Gte => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Gte,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Lt => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Lt,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Lte => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Lte,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Add => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Add,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Sub => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Sub,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Mul => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Mul,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Shl => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shl,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::Shr => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shr,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),

            Opcode::Car => Ok(Self::Car {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Cdr => Ok(Self::Cdr {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::SetCar => Ok(Self::SetCar {
                dst: cpu::RValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::SetCdr => Ok(Self::SetCdr {
                dst: cpu::RValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Cons => Ok(Self::Cons {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                car: r1.ok_or(DecoderError::BadInstruction)?,
                cdr: r2.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Uncons => Ok(Self::Uncons {
                car: r0.ok_or(DecoderError::BadInstruction)?,
                cdr: r1.ok_or(DecoderError::BadInstruction)?,
                src: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::IDiv => Ok(Self::IDiv {
                div: cpu::LValue::decode(r0, off, &tiebreak)?,
                rem: r1,
                op1: r2.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r3, off, &tiebreak)?,
            }),
            Opcode::Pop => Ok(Self::Pop {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
            }),
            Opcode::Push => Ok(Self::Push {
                src: cpu::RValue::decode(r0, off, &tiebreak)?,
            }),
            Opcode::IReturn => Ok(Self::IReturn),
            Opcode::AAdd => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::ASub => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::GetPayload => Ok(Instruction::GetPayload {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::GetTag => Ok(Instruction::GetTag {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Halt => Ok(Instruction::Halt),
            Opcode::Int => Ok(Instruction::Int(hi)),
            Opcode::SetPayload => Ok(Instruction::SetPayload {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::SetTag => Ok(Instruction::SetTag {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Mov => Ok(Instruction::Mov {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Mov8 => Ok(Instruction::Mov8 {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
            }),
            Opcode::Typep => Ok(Instruction::Typep {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                src: cpu::RValue::decode(r1, off, &tiebreak)?,
                compare: lo.to_le_bytes()[4],
            }),
            Opcode::MemCpy => Ok(Instruction::MemCpy {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
                count: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
            Opcode::DisableInterrupts => Ok(Instruction::DisableInterrupts),
            Opcode::EnableInterrupts => Ok(Instruction::EnableInterrupts),
            Opcode::Req => Ok(Instruction::Req {
                dst: cpu::LValue::decode(r0, off, &tiebreak)?,
                prototype: r1.ok_or(DecoderError::BadInstruction)?,
                size: cpu::RValue::decode(r2, off, &tiebreak)?,
            }),
        }
    }

    pub fn encode(&self) -> Result<(u64, u64), EncoderError> {
        Ok(match self {
            // Control flow
            // No params:
            i @ (Instruction::Halt
            | Instruction::Nop
            | Instruction::EnableInterrupts
            | Instruction::DisableInterrupts
            | Instruction::IReturn
            | Instruction::Return) => {
                let opcode = match i {
                    Instruction::Halt => Opcode::Halt,
                    Instruction::Nop => Opcode::Nop,
                    Instruction::Return => Opcode::Return,
                    Instruction::IReturn => Opcode::IReturn,
                    Instruction::EnableInterrupts => Opcode::EnableInterrupts,
                    Instruction::DisableInterrupts => Opcode::DisableInterrupts,
                    _ => panic!("Missing branch"),
                };
                Instruction::encode_params(
                    opcode,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Tiebreaker::NONE,
                    Native(0),
                )
            }

            // 1 arg
            Instruction::Pop { dst } => {
                let (op1, off, tiebreak) = dst.encode();
                Instruction::encode_params(Opcode::Pop, op1, None, None, None, None, tiebreak, off)
            }
            Instruction::Push { src } => {
                let (src, off, tiebreak) = src.encode();
                Instruction::encode_params(Opcode::Push, src, None, None, None, None, tiebreak, off)
            }
            Instruction::Int(i) => Instruction::encode_params(
                Opcode::Int,
                None,
                None,
                None,
                None,
                None,
                Tiebreaker::NONE,
                Native(*i),
            ),

            // 2 args:
            i @ (Instruction::Car { dst, src }
            | Instruction::Cdr { dst, src }
            | Instruction::GetPayload { dst, src }
            | Instruction::GetTag { dst, src }
            | Instruction::SetPayload { dst, src }
            | Instruction::SetTag { dst, src }) => {
                let opcode = match i {
                    Instruction::Car { dst: _, src: _ } => Opcode::Car,
                    Instruction::Cdr { dst: _, src: _ } => Opcode::Cdr,
                    Instruction::SetCar { dst: _, src: _ } => Opcode::SetCar,
                    Instruction::SetCdr { dst: _, src: _ } => Opcode::SetCdr,
                    Instruction::GetPayload { dst: _, src: _ } => Opcode::GetPayload,
                    Instruction::SetPayload { dst: _, src: _ } => Opcode::SetPayload,
                    Instruction::GetTag { dst: _, src: _ } => Opcode::GetTag,
                    Instruction::SetTag { dst: _, src: _ } => Opcode::SetTag,
                    _ => panic!("Missing branch"),
                };
                let (dst, doff, dtiebreak) = dst.encode();
                let (src, soff, stiebreak) = src.encode();
                Instruction::encode_params(
                    opcode,
                    dst,
                    src,
                    None,
                    None,
                    None,
                    stiebreak | dtiebreak,
                    Native(soff.0 + doff.0),
                )
            }

            i @ (Instruction::SetCar { dst, src } | Instruction::SetCdr { dst, src }) => {
                let opcode = match i {
                    Instruction::SetCar { dst: _, src: _ } => Opcode::SetCar,
                    Instruction::SetCdr { dst: _, src: _ } => Opcode::SetCdr,
                    _ => panic!("Missing branch"),
                };
                let (dst, doff, dtiebreak) = dst.encode();
                let (src, soff, stiebreak) = src.encode();
                Instruction::encode_params(
                    opcode,
                    dst,
                    src,
                    None,
                    None,
                    None,
                    stiebreak | dtiebreak,
                    Native(soff.0 + doff.0),
                )
            }

            // 3 + off args
            Instruction::Binary { op, dst, op1, op2 } => {
                let opcode = match op {
                    cpu::BinaryOp::Add => Opcode::Add,
                    cpu::BinaryOp::Mul => Opcode::Mul,
                    cpu::BinaryOp::Shl => Opcode::Shl,
                    cpu::BinaryOp::Shr => Opcode::Shr,
                    cpu::BinaryOp::Sub => Opcode::Sub,
                };
                let (dst, doff, dtiebreak) = dst.encode();
                let (op2, soff, stiebreak) = op2.encode();
                Instruction::encode_params(
                    opcode,
                    dst,
                    Some(*op1),
                    op2,
                    None,
                    None,
                    stiebreak | dtiebreak,
                    Native(soff.0 + doff.0),
                )
            }
            Instruction::MBinary { op, dst, op1, op2 } => {
                let opcode = match op {
                    cpu::MBinaryOp::Add => Opcode::AAdd,
                    cpu::MBinaryOp::Sub => Opcode::ASub,
                };
                let (dst, doff, dtiebreak) = dst.encode();
                let (op2, soff, stiebreak) = op2.encode();
                Instruction::encode_params(
                    opcode,
                    dst,
                    Some(*op1),
                    op2,
                    None,
                    None,
                    stiebreak | dtiebreak,
                    Native(soff.0 + doff.0),
                )
            }
            Instruction::Comparison { op, dst, op1, op2 } => {
                let opcode = match op {
                    cpu::Comparison::Eq => Opcode::Eq,
                    cpu::Comparison::Ne => Opcode::Ne,
                    cpu::Comparison::Lt => Opcode::Lt,
                    cpu::Comparison::Lte => Opcode::Lte,
                    cpu::Comparison::Gt => Opcode::Gt,
                    cpu::Comparison::Gte => Opcode::Gte,
                };
                let (dst, doff, dtiebreak) = dst.encode();
                let (op2, soff, stiebreak) = op2.encode();
                Instruction::encode_params(
                    opcode,
                    dst,
                    Some(*op1),
                    op2,
                    None,
                    None,
                    stiebreak | dtiebreak,
                    Native(soff.0 + doff.0),
                )
            }

            Instruction::Call { target } => {
                let (direct, off, tiebreak) = target.encode();
                Instruction::encode_params(
                    Opcode::Call,
                    direct,
                    None,
                    None,
                    None,
                    None,
                    tiebreak,
                    off,
                )
            }
            Instruction::Jump { condition, target } => {
                let (direct, off, tiebreak) = target.encode();
                let (opcode, cond) = match condition {
                    cpu::Condition::Always => (Opcode::Jump, None),
                    cpu::Condition::False(r) => (Opcode::JumpIfNot, Some(*r)),
                    cpu::Condition::True(r) => (Opcode::JumpIf, Some(*r)),
                };
                Instruction::encode_params(opcode, cond, direct, None, None, None, tiebreak, off)
            }
            Instruction::Cons { dst, car, cdr } => {
                let (dst, doff, dtiebreak) = dst.encode();
                Instruction::encode_params(
                    Opcode::Cons,
                    dst,
                    Some(*car),
                    Some(*cdr),
                    None,
                    None,
                    dtiebreak,
                    doff,
                )
            }
            Instruction::Uncons { car, cdr, src } => {
                let (src, soff, stiebreak) = src.encode();
                Instruction::encode_params(
                    Opcode::Uncons,
                    Some(*car),
                    Some(*cdr),
                    src,
                    None,
                    None,
                    stiebreak,
                    soff,
                )
            }
            Instruction::IDiv { div, rem, op1, op2 } => {
                let (div, doff, dtiebreak) = div.encode();
                let (direct, off, tiebreak) = op2.encode();
                Instruction::encode_params(
                    Opcode::IDiv,
                    div,
                    *rem,
                    Some(*op1),
                    direct,
                    None,
                    dtiebreak | tiebreak,
                    Native(doff.0 + off.0),
                )
            }
            Instruction::MakeClosure { dst, code } => {
                let (dst, doff, dtiebreak) = dst.encode();
                Instruction::encode_params(
                    Opcode::MakeClosure,
                    dst,
                    Some(*code),
                    None,
                    None,
                    None,
                    dtiebreak,
                    doff,
                )
            }
            Instruction::MemCpy { dst, src, count } => {
                let (direct, off, tiebreak) = count.encode();
                Instruction::encode_params(
                    Opcode::MemCpy,
                    Some(*dst),
                    Some(*src),
                    direct,
                    None,
                    None,
                    tiebreak,
                    off,
                )
            }
            Instruction::Mov { dst, src } => {
                if dst.needs_second_word() && src.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (ddst, odst, tdst) = dst.encode();
                let (dsrc, osrc, tsrc) = src.encode();

                Instruction::encode_params(
                    Opcode::Mov,
                    ddst,
                    dsrc,
                    None,
                    None,
                    None,
                    tdst | tsrc,
                    Native(odst.0 + osrc.0),
                )
            }
            Instruction::Mov8 { dst, src } => {
                if dst.needs_second_word() && src.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (ddst, odst, tdst) = dst.encode();
                let (dsrc, osrc, tsrc) = src.encode();

                Instruction::encode_params(
                    Opcode::Mov8,
                    ddst,
                    dsrc,
                    None,
                    None,
                    None,
                    tdst | tsrc,
                    Native(odst.0 + osrc.0),
                )
            }
            Instruction::Req {
                dst,
                prototype,
                size,
            } => {
                if dst.needs_second_word() && size.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (ddst, odst, tdst) = dst.encode();
                let (dsrc, osrc, tsrc) = size.encode();
                Instruction::encode_params(
                    Opcode::Req,
                    ddst,
                    Some(*prototype),
                    dsrc,
                    None,
                    None,
                    tdst | tsrc,
                    Native(odst.0 + osrc.0),
                )
            }
            Instruction::Typep { dst, src, compare } => {
                if dst.needs_second_word() && src.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (ddst, odst, tdst) = dst.encode();
                let (dsrc, osrc, tsrc) = src.encode();
                Instruction::encode_params(
                    Opcode::Typep,
                    ddst,
                    dsrc,
                    Some(Register(*compare)),
                    None,
                    None,
                    tdst | tsrc,
                    Native(odst.0 + osrc.0),
                )
            }
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
                        dbg!(r);
                        let (lo, hi) = r.encode().map_err(|_| format!("Error encoding {:?}", r))?;
                        dbg!(lo.to_le_bytes(), hi);
                        let decoded = cpu::Instruction::decode(lo, hi)
                            .map_err(|e| format!("Error decoding {:?} {:?}", r, e))?;
                        dbg!(i);
                        dbg!(decoded);
                        assert_eq!(r, decoded);
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
