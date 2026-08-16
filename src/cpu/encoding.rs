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

impl cpu::RValue {
    fn encode(&self) -> (Option<Register>, Option<Register>, Option<Native>) {
        match self {
            Self::Literal(l) => (Some(Register(0)), Some(Register(0)), Some(*l)),
            Self::Absolute(l) => (None, None, Some(Native(l.0))),
            Self::Register(RegAndOff { op1, off }) => (Some(*op1), None, *off),
            Self::Indirect(RegAndOff { op1, off }) => (None, Some(*op1), *off),
        }
    }

    fn decode(
        direct: Option<Register>,
        indirect: Option<Register>,
        offset: Option<Native>,
    ) -> Result<Self, DecoderError> {
        match (direct, indirect, offset) {
            (Some(op1), None, off) => Ok(cpu::RValue::Register(RegAndOff { op1, off })),
            (None, Some(op1), off) => Ok(cpu::RValue::Indirect(RegAndOff { op1, off })),
            (None, None, Some(abs)) => Ok(cpu::RValue::Absolute(Address::from(abs))),
            (Some(_), Some(_), Some(off)) => Ok(cpu::RValue::Literal(off)),
            _ => Err(DecoderError::BadInstruction),
        }
    }

    fn needs_second_word(&self) -> bool {
        match self {
            cpu::RValue::Literal(_) => true,
            cpu::RValue::Absolute(_) => true,
            cpu::RValue::Register(RegAndOff { op1: _, off }) => off.is_some(),
            cpu::RValue::Indirect(RegAndOff { op1: _, off }) => off.is_some(),
        }
    }
}

impl cpu::LValue {
    fn encode(&self) -> (Option<Register>, Option<Register>, Option<Native>) {
        match self {
            Self::Absolute(a) => (None, None, Some(Native(a.0))),
            Self::Register(r) => (Some(*r), None, None),
            Self::Indirect(RegAndOff { op1, off }) => (None, Some(*op1), *off),
        }
    }

    fn decode(
        direct: Option<Register>,
        indirect: Option<Register>,
        offset: Option<Native>,
    ) -> Result<Self, DecoderError> {
        match (direct, indirect, offset) {
            (Some(d), None, None) => Ok(cpu::LValue::Register(d)),
            (None, None, Some(off)) => Ok(cpu::LValue::Absolute(Address::from(off))),
            (None, Some(op1), off) => Ok(cpu::LValue::Indirect(RegAndOff { op1, off })),

            _ => Err(DecoderError::BadInstruction),
        }
    }

    fn needs_second_word(&self) -> bool {
        match self {
            cpu::LValue::Absolute(_) => true,
            cpu::LValue::Register(_) => false,
            cpu::LValue::Indirect(RegAndOff { op1: _, off }) => off.is_some(),
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

impl Instruction {
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
            Option<Native>,
            u8,
        ),
        DecoderError,
    > {
        let [opcode, param_shape, p0, p1, p2, p3, p4, tiebreak] = lo.to_le_bytes();

        let r0 = if param_shape & 1 != 0 {
            Some(Register(p0))
        } else {
            None
        };
        let r1 = if param_shape & 2 != 0 {
            Some(Register(p1))
        } else {
            None
        };
        let r2 = if param_shape & 4 != 0 {
            Some(Register(p2))
        } else {
            None
        };
        let r3 = if param_shape & 8 != 0 {
            Some(Register(p3))
        } else {
            None
        };
        let r4 = if param_shape & 16 != 0 {
            Some(Register(p4))
        } else {
            None
        };
        let off = if param_shape & 32 != 0 {
            Some(Native(off))
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
            off,
            tiebreak,
        ))
    }

    fn encode_params(
        opcode: Opcode,
        p0: Option<Register>,
        p1: Option<Register>,
        p2: Option<Register>,
        p3: Option<Register>,
        p4: Option<Register>,
        off: Option<Native>,
        tiebreak: u8,
    ) -> (u64, u64) {
        let mut shape = 0;
        let r0 = match p0 {
            None => 0,
            Some(r) => {
                shape |= 1;
                r.0
            }
        };
        let r1 = match p1 {
            None => 0,
            Some(r) => {
                shape |= 2;
                r.0
            }
        };
        let r2 = match p2 {
            None => 0,
            Some(r) => {
                shape |= 4;
                r.0
            }
        };
        let r3 = match p3 {
            None => 0,
            Some(r) => {
                shape |= 8;
                r.0
            }
        };
        let r4 = match p4 {
            None => 0,
            Some(r) => {
                shape |= 16;
                r.0
            }
        };
        let off = match off {
            None => 0,
            Some(r) => {
                shape |= 32;
                r.0
            }
        };

        (
            u64::from_le_bytes([opcode.into(), shape, r0, r1, r2, r3, r4, tiebreak]),
            off,
        )
    }

    pub fn decode(lo: u64, hi: u64) -> Result<Self, DecoderError> {
        let (opcode, r0, r1, r2, r3, r4, off, tiebreak) = Self::decode_params(lo, hi)?;

        match opcode {
            Opcode::Nop => Ok(Self::Nop),
            Opcode::Jump => Ok(Self::Jump {
                condition: cpu::Condition::Always,
                target: cpu::RValue::decode(r1, r2, off)?,
            }),
            Opcode::JumpIf => Ok(Self::Jump {
                condition: cpu::Condition::True(r0.ok_or(DecoderError::BadInstruction)?),
                target: cpu::RValue::decode(r1, r2, off)?,
            }),
            Opcode::JumpIfNot => Ok(Self::Jump {
                condition: cpu::Condition::False(r0.ok_or(DecoderError::BadInstruction)?),
                target: cpu::RValue::decode(r1, r2, off)?,
            }),
            Opcode::Call => Ok(Self::Call {
                target: cpu::RValue::decode(r0, r1, off)?,
            }),

            Opcode::Return => Ok(Self::Return),
            Opcode::MakeClosure => Ok(Self::MakeClosure {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                code: r1.ok_or(DecoderError::BadInstruction)?,
            }),

            Opcode::Eq => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Eq,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Ne => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Ne,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Gt => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Gt,
                dst: r0.ok_or(DecoderError::BadInstruction)?,

                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Gte => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Gte,
                dst: r0.ok_or(DecoderError::BadInstruction)?,

                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Lt => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Lt,
                dst: r0.ok_or(DecoderError::BadInstruction)?,

                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Lte => Ok(Self::Comparison {
                op: crate::cpu::Comparison::Lte,
                dst: r0.ok_or(DecoderError::BadInstruction)?,

                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Add => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Add,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Sub => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Sub,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Mul => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Mul,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Shl => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shl,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::Shr => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shr,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),

            Opcode::Car => Ok(Self::Car {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Cdr => Ok(Self::Cdr {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::SetCar => Ok(Self::SetCar {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::SetCdr => Ok(Self::SetCdr {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Cons => Ok(Self::Cons {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                car: r1.ok_or(DecoderError::BadInstruction)?,
                cdr: r2.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Uncons => Ok(Self::Uncons {
                car: r0.ok_or(DecoderError::BadInstruction)?,
                cdr: r1.ok_or(DecoderError::BadInstruction)?,
                src: r2.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::IDiv => Ok(Self::IDiv {
                div: r0.ok_or(DecoderError::BadInstruction)?,
                rem: r1.ok_or(DecoderError::BadInstruction)?,
                op1: r2.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r3, r4, off)?,
            }),
            Opcode::Pop => Ok(Self::Pop {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Push => Ok(Self::Push {
                src: r0.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::IReturn => Ok(Self::IReturn),
            Opcode::AAdd => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::ASub => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                op1: r1.ok_or(DecoderError::BadInstruction)?,
                op2: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::GetPayload => Ok(Instruction::GetPayload {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::GetTag => Ok(Instruction::GetTag {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Halt => Ok(Instruction::Halt),
            Opcode::Int => Ok(Instruction::Int(hi)),
            Opcode::SetPayload => Ok(Instruction::SetPayload {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::SetTag => Ok(Instruction::SetTag {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
            }),
            Opcode::Mov => Ok(Instruction::Mov {
                dst: cpu::LValue::decode(r0, r1, if tiebreak == 1 { off } else { None })?,
                src: cpu::RValue::decode(r2, r3, if tiebreak == 0 { off } else { None })?,
            }),
            Opcode::Mov8 => Ok(Instruction::Mov8 {
                dst: cpu::LValue::decode(r0, r1, if tiebreak == 1 { off } else { None })?,
                src: cpu::RValue::decode(r2, r3, if tiebreak == 0 { off } else { None })?,
            }),
            Opcode::Typep => Ok(Instruction::Typep {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
                compare: Native(hi),
            }),
            Opcode::MemCpy => Ok(Instruction::MemCpy {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                src: r1.ok_or(DecoderError::BadInstruction)?,
                count: cpu::RValue::decode(r2, r3, off)?,
            }),
            Opcode::DisableInterrupts => Ok(Instruction::DisableInterrupts),
            Opcode::EnableInterrupts => Ok(Instruction::EnableInterrupts),
            Opcode::Req => Ok(Instruction::Req {
                dst: r0.ok_or(DecoderError::BadInstruction)?,
                prototype: r1.ok_or(DecoderError::BadInstruction)?,
                size: r2.ok_or(DecoderError::BadInstruction)?,
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
                Instruction::encode_params(opcode, None, None, None, None, None, None, 0)
            }

            // 1 arg
            Instruction::Pop { dst } => {
                Instruction::encode_params(Opcode::Pop, Some(*dst), None, None, None, None, None, 0)
            }
            Instruction::Push { src } => Instruction::encode_params(
                Opcode::Push,
                Some(*src),
                None,
                None,
                None,
                None,
                None,
                0,
            ),
            Instruction::Int(i) => Instruction::encode_params(
                Opcode::Int,
                None,
                None,
                None,
                None,
                None,
                Some(Native(*i)),
                0,
            ),

            // 2 args:
            i @ (Instruction::Car { dst, src }
            | Instruction::Cdr { dst, src }
            | Instruction::SetCar { dst, src }
            | Instruction::SetCdr { dst, src }
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
                Instruction::encode_params(
                    opcode,
                    Some(*dst),
                    Some(*src),
                    None,
                    None,
                    None,
                    None,
                    0,
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
                let (direct, indirect, off) = op2.encode();
                Instruction::encode_params(
                    opcode,
                    Some(*dst),
                    Some(*op1),
                    direct,
                    indirect,
                    None,
                    off,
                    0,
                )
            }
            Instruction::MBinary { op, dst, op1, op2 } => {
                let opcode = match op {
                    cpu::MBinaryOp::Add => Opcode::AAdd,
                    cpu::MBinaryOp::Sub => Opcode::ASub,
                };
                let (direct, indirect, off) = op2.encode();
                Instruction::encode_params(
                    opcode,
                    Some(*dst),
                    Some(*op1),
                    direct,
                    indirect,
                    None,
                    off,
                    0,
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
                let (direct, indirect, off) = op2.encode();
                Instruction::encode_params(
                    opcode,
                    Some(*dst),
                    Some(*op1),
                    direct,
                    indirect,
                    None,
                    off,
                    0,
                )
            }

            Instruction::Call { target } => {
                let (direct, indirect, off) = target.encode();
                Instruction::encode_params(Opcode::Call, direct, indirect, None, None, None, off, 0)
            }
            Instruction::Jump { condition, target } => {
                let (direct, indirect, off) = target.encode();
                let (opcode, cond) = match condition {
                    cpu::Condition::Always => (Opcode::Jump, None),
                    cpu::Condition::False(r) => (Opcode::JumpIfNot, Some(*r)),
                    cpu::Condition::True(r) => (Opcode::JumpIf, Some(*r)),
                };
                Instruction::encode_params(opcode, cond, direct, indirect, None, None, off, 0)
            }
            Instruction::Cons { dst, car, cdr } => Instruction::encode_params(
                Opcode::Cons,
                Some(*dst),
                Some(*car),
                Some(*cdr),
                None,
                None,
                None,
                0,
            ),
            Instruction::Uncons { car, cdr, src } => Instruction::encode_params(
                Opcode::Uncons,
                Some(*car),
                Some(*cdr),
                Some(*src),
                None,
                None,
                None,
                0,
            ),
            Instruction::IDiv { div, rem, op1, op2 } => {
                let (direct, indirect, off) = op2.encode();
                Instruction::encode_params(
                    Opcode::IDiv,
                    Some(*div),
                    Some(*rem),
                    Some(*op1),
                    direct,
                    indirect,
                    off,
                    0,
                )
            }
            Instruction::MakeClosure { dst, code } => Instruction::encode_params(
                Opcode::MakeClosure,
                Some(*dst),
                Some(*code),
                None,
                None,
                None,
                None,
                0,
            ),
            Instruction::MemCpy { dst, src, count } => {
                let (direct, indirect, off) = count.encode();
                Instruction::encode_params(
                    Opcode::MemCpy,
                    Some(*dst),
                    Some(*src),
                    direct,
                    indirect,
                    None,
                    off,
                    0,
                )
            }
            Instruction::Mov { dst, src } => {
                if dst.needs_second_word() && src.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (ddst, idst, odst) = dst.encode();
                let (dsrc, isrc, osrc) = src.encode();

                Instruction::encode_params(
                    Opcode::Mov,
                    ddst,
                    idst,
                    dsrc,
                    isrc,
                    None,
                    odst.or(osrc),
                    if odst.is_some() { 1 } else { 0 },
                )
            }
            Instruction::Mov8 { dst, src } => {
                if dst.needs_second_word() && src.needs_second_word() {
                    return Err(EncoderError::BadInstruction);
                }
                let (ddst, idst, odst) = dst.encode();
                let (dsrc, isrc, osrc) = src.encode();

                Instruction::encode_params(
                    Opcode::Mov8,
                    ddst,
                    idst,
                    dsrc,
                    isrc,
                    None,
                    odst.or(osrc),
                    if odst.is_some() { 1 } else { 0 },
                )
            }
            Instruction::Req {
                dst,
                prototype,
                size,
            } => Instruction::encode_params(
                Opcode::Req,
                Some(*dst),
                Some(*prototype),
                Some(*size),
                None,
                None,
                None,
                0,
            ),
            Instruction::Typep { dst, src, compare } => Instruction::encode_params(
                Opcode::Typep,
                Some(*dst),
                Some(*src),
                None,
                None,
                None,
                Some(*compare),
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
                        dbg!(r);
                        let (lo, hi) = r.encode().map_err(|_| format!("Error encoding {:?}", r))?;
                        dbg!(lo.to_le_bytes(), hi);
                        let decoded = cpu::Instruction::decode(lo, hi)
                            .map_err(|_| format!("Error decoding {:?}", r))?;
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
