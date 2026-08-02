use int_enum::IntEnum;

use crate::{
    cpu::{
        self, Count, Instruction, LispWord, Location, MachineRegister, Native, Register,
        ThreeMachs, ThreeRegs, Trap, TwoRegs,
    },
    memory::{Address, Offset},
};

impl cpu::Register {
    fn encode(&self) -> u8 {
        self.0 as u8
    }

    fn encode_indirect(&self) -> u8 {
        self.encode() + 24
    }

    fn decode(reg: u8) -> Self {
        Self(reg as u64)
    }

    fn decode_indirect(reg: u8) -> Self {
        Self(reg as u64 - 24)
    }

    fn try_decode(reg: u8) -> Option<Register> {
        if reg == Self::NONE {
            None
        } else {
            Some(Self::decode(reg))
        }
    }

    const NONE: u8 = 0x55;
}

impl MachineRegister {
    fn encode(&self) -> u8 {
        self.0 as u8 + 16
    }
    fn encode_indirect(&self) -> u8 {
        self.encode() + 24
    }
    fn try_decode(reg: u8) -> Option<Self> {
        if reg - 16 < 8 {
            Some(Self(reg as u64 - 16))
        } else {
            None
        }
    }
    fn decode(reg: u8) -> Result<Self, Trap> {
        if reg - 16 < 8 {
            Ok(Self(reg as u64 - 16))
        } else {
            Err(Trap::InvalidInstruction)
        }
    }
}

impl TwoRegs {
    fn decode(r1: u8, r2: u8) -> TwoRegs {
        TwoRegs {
            dst: Register(r1 as u64),
            src: Register(r2 as u64),
        }
    }
}

impl ThreeRegs {
    fn encode(&self) -> (u8, u8, u8, u64) {
        match (self.op2, self.op3) {
            (None, None) => (self.dst.encode(), self.op1.encode(), 64, 0),
            (None, Some(o)) => (self.dst.encode(), self.op1.encode(), 65, o.0),
            (Some(r), None) => (self.dst.encode(), self.op1.encode(), r.encode() + 24, 0),
            (Some(r), Some(o)) => (self.dst.encode(), self.op1.encode(), r.encode(), o.0),
        }
    }
    fn decode(r0: u8, r1: u8, r2: u8, imm: u64) -> ThreeRegs {
        if r2 == 64 {
            ThreeRegs {
                dst: Register::decode(r0),
                op1: Register::decode(r1),
                op2: None,
                op3: None,
            }
        } else if r2 == 65 {
            ThreeRegs {
                dst: Register::decode(r0),
                op1: Register::decode(r1),
                op2: None,
                op3: Some(LispWord(imm)),
            }
        } else if r2 >= 24 {
            ThreeRegs {
                dst: Register::decode(r0),
                op1: Register::decode(r1),
                op2: Some(Register::decode(r2 - 24)),
                op3: None,
            }
        } else {
            ThreeRegs {
                dst: Register::decode(r0),
                op1: Register::decode(r1),
                op2: Some(Register::decode(r2)),
                op3: Some(LispWord(imm)),
            }
        }
    }
}

impl cpu::Location {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Literal(l) => (Register::NONE, l.0),
            Self::Absolute(l) => (Register::NONE + 1, l.0),
            Self::Machine(r) => (r.encode(), 0),
            Self::Register(r) => (r.encode(), 0),
            Self::IndirectMachine(a, d) => (a.encode() + 24, d.0 as u64),
            Self::IndirectRegister(a) => (a.encode() + 24, 0),
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
            Ok(Self::Machine(MachineRegister::decode(reg)?))
        } else if reg < 40 {
            Ok(Self::IndirectRegister(Register::decode(reg - 24)))
        } else {
            Ok(Self::IndirectMachine(
                MachineRegister::decode(reg - 24)?,
                cpu::Offset(off as i64),
            ))
        }
    }
}

impl cpu::JumpTarget {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Absolute(a) => (Register::NONE, a.0),
            Self::Machine(r, off) => (r.encode(), off.0 as u64),
            Self::Register(r) => (r.encode(), 0),
            Self::IndirectMachine(a, off) => (a.encode(), off.0 as u64),
            Self::IndirectRegister(a) => (a.encode() + 24, 0),
        }
    }

    fn decode(reg: u8, off: u64) -> Result<Self, Trap> {
        if reg == Register::NONE {
            Ok(Self::Absolute(cpu::Address(off)))
        } else if reg < 16 {
            Ok(Self::Register(Register::decode(reg)))
        } else if reg < 24 {
            Ok(Self::Machine(
                MachineRegister::decode(reg)?,
                Offset(off as i64),
            ))
        } else if reg < 40 {
            Ok(Self::IndirectRegister(Register::decode(reg - 24)))
        } else {
            Ok(Self::IndirectMachine(
                MachineRegister::decode(reg - 24)?,
                cpu::Offset(off as i64),
            ))
        }
    }
}

impl ThreeMachs {
    fn encode(&self) -> (u8, u8, u8, u64) {
        match (self.op2, self.op3) {
            (None, None) => (self.dst.encode(), self.op1.encode(), 64, 0),
            (None, Some(o)) => (self.dst.encode(), self.op1.encode(), 65, o.0),
            (Some(r), None) => (self.dst.encode(), self.op1.encode(), r.encode() + 24, 0),
            (Some(r), Some(o)) => (self.dst.encode(), self.op1.encode(), r.encode(), o.0),
        }
    }
    fn decode(r0: u8, r1: u8, r2: u8, imm: u64) -> Result<Self, Trap> {
        Ok(if r2 == 64 {
            Self {
                dst: MachineRegister::decode(r0)?,
                op1: MachineRegister::decode(r1)?,
                op2: None,
                op3: None,
            }
        } else if r2 == 65 {
            Self {
                dst: MachineRegister::decode(r0)?,
                op1: MachineRegister::decode(r1)?,
                op2: None,
                op3: Some(Native(imm)),
            }
        } else if r2 >= 24 {
            Self {
                dst: MachineRegister::decode(r0)?,
                op1: MachineRegister::decode(r1)?,
                op2: Some(MachineRegister::decode(r2 - 24)?),
                op3: None,
            }
        } else {
            Self {
                dst: MachineRegister::decode(r0)?,
                op1: MachineRegister::decode(r1)?,
                op2: Some(MachineRegister::decode(r2)?),
                op3: Some(Native(imm)),
            }
        })
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
                code: MachineRegister::try_decode(r1).expect("Shit happened!"),
            }),

            Opcode::Eq => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Eq,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Ne => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Ne,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Gt => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Gt,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Gte => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Gte,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Lt => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Lt,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Lte => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Lte,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Add => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Add,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Sub => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Sub,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Mul => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Mul,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Shl => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shl,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
            }),
            Opcode::Shr => Ok(Self::Binary {
                op: crate::cpu::BinaryOp::Shr,
                operands: ThreeRegs::decode(r0, r1, r2, hi),
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
                op1: Register::decode(r2),
                op2: if r3 == 64 || r3 == 65 {
                    None
                } else if r3 >= 24 {
                    Some(Register::decode(r3 - 24))
                } else {
                    Some(Register::decode(r3))
                },
                op3: if r3 == 65 || r3 < 24 {
                    Some(LispWord(hi))
                } else {
                    None
                },
            }),
            Opcode::PopR => Ok(Self::PopR {
                dst: Register::decode(r0),
            }),
            Opcode::PushR => Ok(Self::PushR {
                src: Register::decode(r0),
            }),
            Opcode::PopA => Ok(Self::PopA {
                dst: MachineRegister::decode(r0)?,
            }),
            Opcode::PushA => Ok(Self::PushA {
                src: MachineRegister::decode(r0)?,
            }),
            Opcode::IReturn => Ok(Self::IReturn),
            Opcode::AAdd => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Add,
                operands: cpu::ThreeMachs::decode(r0, r1, r2, hi)?,
            }),
            Opcode::ASub => Ok(Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                operands: cpu::ThreeMachs::decode(r0, r1, r2, hi)?,
            }),
            Opcode::GetPayload => Ok(Instruction::GetPayload {
                dst: MachineRegister::decode(r0)?,
                src: Register::decode(r1),
            }),
            Opcode::GetTag => Ok(Instruction::GetTag {
                dst: MachineRegister::decode(r0)?,
                src: Register::decode(r1),
            }),
            Opcode::Halt => Ok(Instruction::Halt),
            Opcode::Int => Ok(Instruction::Int(hi)),
            Opcode::SetPayload => Ok(Instruction::SetPayload {
                dst: Register::decode(r0),
                src: MachineRegister::decode(r1)?,
            }),
            Opcode::SetTag => Ok(Instruction::SetTag {
                dst: Register::decode(r0),
                src: MachineRegister::decode(r1)?,
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
                dst: MachineRegister::decode(r0)?,
                src: MachineRegister::decode(r1)?,
                count: Count(hi),
            }),
        }
    }

    fn encode_div(
        div: Register,
        rem: Register,
        op1: Register,
        op2: Option<Register>,
        op3: Option<LispWord>,
    ) -> (u64, u64) {
        let (r0, r1, r2, r3, hi) = match (op2, op3) {
            (None, None) => (div.encode(), rem.encode(), op1.encode(), 64, 0),
            (None, Some(o)) => (div.encode(), rem.encode(), op1.encode(), 65, o.0),
            (Some(r), None) => (div.encode(), rem.encode(), op1.encode(), r.encode() + 24, 0),
            (Some(r), Some(o)) => (div.encode(), rem.encode(), op1.encode(), r.encode(), o.0),
        };
        (
            u64::from_le_bytes([Opcode::IDiv.into(), r0, r1, r2, r3, 0, 0, 0]),
            hi,
        )
    }
    fn encode_three_adrs(opcode: Opcode, target: &ThreeMachs) -> (u64, u64) {
        let (r0, r1, r2, hi) = target.encode();
        (
            u64::from_le_bytes([opcode.into(), r0, r1, r2, 0, 0, 0, 0]),
            hi,
        )
    }
    fn encode_three_regs(opcode: Opcode, target: &ThreeRegs) -> (u64, u64) {
        let (r0, r1, r2, hi) = target.encode();
        (
            u64::from_le_bytes([opcode.into(), r0, r1, r2, 0, 0, 0, 0]),
            hi,
        )
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
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Eq,
                operands,
            } => Self::encode_three_regs(Opcode::Eq, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Ne,
                operands,
            } => Self::encode_three_regs(Opcode::Ne, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Gt,
                operands,
            } => Self::encode_three_regs(Opcode::Gt, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Gte,
                operands,
            } => Self::encode_three_regs(Opcode::Gte, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Lt,
                operands,
            } => Self::encode_three_regs(Opcode::Lt, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Lte,
                operands,
            } => Self::encode_three_regs(Opcode::Lte, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Add,
                operands,
            } => Self::encode_three_regs(Opcode::Add, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Sub,
                operands,
            } => Self::encode_three_regs(Opcode::Sub, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Mul,
                operands,
            } => Self::encode_three_regs(Opcode::Mul, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Shl,
                operands,
            } => Self::encode_three_regs(Opcode::Shl, operands),
            Instruction::Binary {
                op: crate::cpu::BinaryOp::Shr,
                operands,
            } => Self::encode_three_regs(Opcode::Shr, operands),

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

            Instruction::IDiv {
                div,
                rem,
                op1,
                op2,
                op3,
            } => Instruction::encode_div(*div, *rem, *op1, *op2, *op3),
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
                operands,
            } => Self::encode_three_adrs(Opcode::AAdd, operands),
            Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Sub,
                operands,
            } => Self::encode_three_adrs(Opcode::ASub, operands),
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
