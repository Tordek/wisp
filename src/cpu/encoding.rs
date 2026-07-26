use int_enum::IntEnum;

use crate::cpu::{
    self, Instruction, JumpAddressing, Location, MachineRegister, Register, ThreeMachs, ThreeRegs,
    Trap, TwoRegs, Word,
};

impl cpu::Register {
    fn encode(&self) -> u8 {
        self.0 as u8
    }

    fn decode(reg: u8) -> Self {
        Self(reg as usize)
    }

    const NONE: u8 = 0x55;
}

impl MachineRegister {
    fn encode(&self) -> u8 {
        self.0 as u8 + 16
    }
    fn try_decode(reg: u8) -> Option<Self> {
        if reg - 16 < 8 {
            Some(Self(reg as usize - 16))
        } else {
            None
        }
    }
    fn decode(reg: u8) -> Result<Self, Trap> {
        if reg - 16 < 8 {
            Ok(Self(reg as usize - 16))
        } else {
            Err(Trap::InvalidInstruction)
        }
    }
}

impl TwoRegs {
    fn decode(r1: u8, r2: u8) -> TwoRegs {
        TwoRegs {
            dst: Register(r1 as usize),
            src: Register(r2 as usize),
        }
    }
}

impl ThreeRegs {
    fn decode(r1: u8, r2: u8, r3: u8, imm: u64) -> Result<ThreeRegs, Trap> {
        Ok(ThreeRegs {
            dst: Register(r1 as usize),
            op1: Register(r2 as usize),
            op2: if r3 == Register::NONE {
                cpu::OffsetRegister::Absolute {
                    pos: cpu::Word::try_from(imm).map_err(|_| Trap::InvalidInstruction)?,
                }
            } else {
                cpu::OffsetRegister::Relative {
                    base: Register::decode(r3),
                    off: cpu::Word::try_from(imm).map_err(|_| Trap::InvalidInstruction)?,
                }
            },
        })
    }
}

impl cpu::JumpAddressing {
    fn decode(cond: u8, adr: u8, offset: u64) -> (Register, Self) {
        if adr == Register::NONE {
            (
                Register(cond as usize),
                cpu::JumpAddressing::Absolute { pos: offset },
            )
        } else if adr < 16 {
            (
                Register(cond as usize),
                cpu::JumpAddressing::Register {
                    adr: Register(adr as usize),
                },
            )
        } else {
            (
                Register(cond as usize),
                cpu::JumpAddressing::MachineRegister {
                    adr: MachineRegister::try_decode(adr)
                        .expect("somehow you encoded something invalid"),
                    offset: offset as i64,
                },
            )
        }
    }
}

impl cpu::OffsetAddress {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Absolute { pos } => (Register::NONE, *pos as u64),
            Self::Relative { base, off } => (base.encode(), *off as u64),
        }
    }
    fn decode(reg: u8, imm: u64) -> Result<Self, Trap> {
        if reg == Register::NONE {
            Ok(Self::Absolute { pos: imm as usize })
        } else {
            Ok(Self::Relative {
                base: MachineRegister::decode(reg)?,
                off: imm as usize,
            })
        }
    }
}
impl cpu::OffsetRegister {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Absolute { pos } => (Register::NONE, u64::from(*pos)),
            Self::Relative { base, off } => (base.encode(), u64::from(*off)),
        }
    }
    fn decode(reg: u8, imm: u64) -> Result<Self, Trap> {
        if reg == Register::NONE {
            Ok(Self::Absolute {
                pos: Word::try_from(imm).map_err(|_| Trap::InvalidInstruction)?,
            })
        } else {
            Ok(Self::Relative {
                base: Register::decode(reg),
                off: Word::try_from(imm).map_err(|_| Trap::InvalidInstruction)?,
            })
        }
    }
}

impl cpu::Location {
    fn encode(&self) -> (u8, u64) {
        match self {
            Self::Absolute(a) => (Register::NONE, *a as u64),
            Self::Machine(r) => (r.encode(), 0),
            Self::Register(r) => (r.encode(), 0),
            Self::IndirectMachine(a, d) => (a.encode() + 24, *d as u64),
            Self::IndirectRegister(a) => (a.encode() + 24, 0),
        }
    }

    fn decode(reg: u8, off: u64) -> Result<Self, Trap> {
        if reg == Register::NONE {
            Ok(Self::Absolute(off as usize))
        } else if reg < 16 {
            Ok(Self::Register(Register::decode(reg)))
        } else if reg < 24 {
            Ok(Self::Machine(MachineRegister::decode(reg)?))
        } else if reg < 40 {
            Ok(Self::IndirectRegister(Register::decode(reg - 24)))
        } else {
            Ok(Self::IndirectMachine(
                MachineRegister::decode(reg - 24)?,
                off as i64,
            ))
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
    LoadLiteral,
    LoadMachine,
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
}

impl Default for Register {
    fn default() -> Self {
        Register(0)
    }
}

// TODO: Find a real encoding/decoding.
impl Instruction {
    pub fn decode(lo: u64, hi: u64) -> Result<Self, Trap> {
        let [opcode, r0, r1, r2, r3, ..] = lo.to_le_bytes();
        match Opcode::try_from(opcode).map_err(|_| Trap::InvalidInstruction)? {
            Opcode::Nop => Ok(Self::Nop),
            Opcode::Jump => {
                let (_, target) = JumpAddressing::decode(r0, r1, hi);
                Ok(Self::Jump { target })
            }
            Opcode::JumpIf => {
                let (condition, target) = JumpAddressing::decode(r0, r1, hi);
                Ok(Self::JumpIf { condition, target })
            }
            Opcode::JumpIfNot => {
                let (condition, target) = JumpAddressing::decode(r0, r1, hi);
                Ok(Self::JumpIfNot { condition, target })
            }
            Opcode::Call => {
                let (_, target) = JumpAddressing::decode(r0, r1, hi);
                Ok(Self::Call { target })
            }

            Opcode::Return => Ok(Self::Return),
            Opcode::MakeClosure => Ok(Self::MakeClosure {
                dst: Register::decode(r0),
                code: MachineRegister::try_decode(r1).expect("Shit happened!"),
            }),

            Opcode::Eq => Ok(Self::Eq(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Ne => Ok(Self::Ne(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Gt => Ok(Self::Gt(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Gte => Ok(Self::Gte(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Lt => Ok(Self::Lt(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Lte => Ok(Self::Lte(ThreeRegs::decode(r0, r1, r2, hi)?)),

            Opcode::Car => Ok(Self::Car(TwoRegs::decode(r0, r1))),
            Opcode::Cdr => Ok(Self::Cdr(TwoRegs::decode(r0, r1))),
            Opcode::SetCar => Ok(Self::SetCar(TwoRegs::decode(r0, r1))),
            Opcode::SetCdr => Ok(Self::SetCdr(TwoRegs::decode(r0, r1))),
            Opcode::Uncons => Ok(Self::Uncons {
                car: Register::decode(r0),
                cdr: Register::decode(r1),
                src: Register::decode(r2),
            }),
            Opcode::Add => Ok(Self::Add(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Sub => Ok(Self::Sub(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::Mul => Ok(Self::Mul(ThreeRegs::decode(r0, r1, r2, hi)?)),
            Opcode::IDiv => Ok(Self::IDiv {
                div: Register::decode(r0),
                rem: Register::decode(r1),
                op1: Register::decode(r2),
                op2: if r3 == Register::NONE {
                    cpu::OffsetRegister::Absolute {
                        pos: Word::try_from(hi).map_err(|_| Trap::InvalidInstruction)?,
                    }
                } else {
                    cpu::OffsetRegister::Relative {
                        base: Register::decode(r3),
                        off: Word::try_from(hi).map_err(|_| Trap::InvalidInstruction)?,
                    }
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
            Opcode::LoadMachine => Ok(Instruction::LoadMachine {
                dst: MachineRegister::decode(r0)?,
                val: hi as usize,
            }),
            Opcode::IReturn => Ok(Self::IReturn),
            Opcode::AAdd => Ok(Instruction::AAdd(cpu::ThreeMachs {
                dst: MachineRegister::decode(r0)?,
                op1: MachineRegister::decode(r1)?,
                op2: cpu::OffsetAddress::decode(r2, hi)?,
            })),
            Opcode::ASub => Ok(Instruction::ASub(cpu::ThreeMachs {
                dst: MachineRegister::decode(r0)?,
                op1: MachineRegister::decode(r1)?,
                op2: cpu::OffsetAddress::decode(r2, hi)?,
            })),
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
            Opcode::LoadLiteral => Ok(Instruction::LoadLiteral {
                dst: Register(r0 as usize),
                val: Word::try_from(hi).map_err(|_| Trap::InvalidInstruction)?,
            }),
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
        }
    }

    fn encode_op_jump_cond(
        opcode: Opcode,
        condition: Register,
        target: &JumpAddressing,
    ) -> (u64, u64) {
        match target {
            JumpAddressing::Absolute { pos } => (
                u64::from_le_bytes([
                    opcode.into(),
                    condition.encode(),
                    Register::NONE,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                *pos as u64,
            ),
            JumpAddressing::MachineRegister {
                adr: MachineRegister(r),
                offset,
            } => (
                u64::from_le_bytes([
                    opcode.into(),
                    condition.encode(),
                    (r + 16) as u8,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                *offset as u64,
            ),
            JumpAddressing::Register { adr } => (
                u64::from_le_bytes([
                    opcode.into(),
                    condition.encode(),
                    adr.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
        }
    }

    fn encode_three_adrs(opcode: Opcode, target: &ThreeMachs) -> (u64, u64) {
        match target {
            &ThreeMachs { dst, op1, op2 } => {
                let (op2e, hi) = op2.encode();
                (
                    u64::from_le_bytes([
                        opcode.into(),
                        dst.encode(),
                        op1.encode(),
                        op2e,
                        0,
                        0,
                        0,
                        0,
                    ]),
                    hi,
                )
            }
        }
    }
    fn encode_three_regs(opcode: Opcode, target: &ThreeRegs) -> (u64, u64) {
        match target {
            &ThreeRegs { dst, op1, op2 } => {
                let (op2e, hi) = op2.encode();
                (
                    u64::from_le_bytes([
                        opcode.into(),
                        dst.encode(),
                        op1.encode(),
                        op2e,
                        0,
                        0,
                        0,
                        0,
                    ]),
                    hi,
                )
            }
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

            Instruction::Jump { target: addressing } => {
                Self::encode_op_jump_cond(Opcode::Jump, Register(0), addressing)
            }
            Instruction::JumpIf { condition, target } => {
                Self::encode_op_jump_cond(Opcode::JumpIf, *condition, target)
            }
            Instruction::JumpIfNot { condition, target } => {
                Self::encode_op_jump_cond(Opcode::JumpIfNot, *condition, target)
            }
            Instruction::Call { target } => {
                Self::encode_op_jump_cond(Opcode::Call, Register(0), target)
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
            Instruction::Eq(regs) => Self::encode_three_regs(Opcode::Eq, regs),
            Instruction::Ne(regs) => Self::encode_three_regs(Opcode::Ne, regs),
            Instruction::Gt(regs) => Self::encode_three_regs(Opcode::Gt, regs),
            Instruction::Gte(regs) => Self::encode_three_regs(Opcode::Gte, regs),
            Instruction::Lt(regs) => Self::encode_three_regs(Opcode::Lt, regs),
            Instruction::Lte(regs) => Self::encode_three_regs(Opcode::Lte, regs),

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

            // Arithmetic
            Instruction::Add(regs) => Self::encode_three_regs(Opcode::Add, regs),
            Instruction::Sub(regs) => Self::encode_three_regs(Opcode::Sub, regs),
            Instruction::Mul(regs) => Self::encode_three_regs(Opcode::Mul, regs),

            Instruction::IDiv { div, rem, op1, op2 } => {
                let (op2e, hi) = op2.encode();
                (
                    u64::from_le_bytes([
                        Opcode::IDiv.into(),
                        div.encode(),
                        rem.encode(),
                        op1.encode(),
                        op2e,
                        0,
                        0,
                        0,
                    ]),
                    hi,
                )
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
            Instruction::LoadMachine { dst, val: address } => (
                u64::from_le_bytes([Opcode::LoadMachine.into(), dst.encode(), 0, 0, 0, 0, 0, 0]),
                *address as u64,
            ),
            Instruction::IReturn => (
                u64::from_le_bytes([Opcode::IReturn.into(), 0, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::AAdd(regs) => Self::encode_three_adrs(Opcode::AAdd, regs),
            Instruction::ASub(regs) => Self::encode_three_adrs(Opcode::ASub, regs),
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
            Instruction::LoadLiteral { dst, val } => (
                u64::from_le_bytes([Opcode::LoadLiteral.into(), dst.encode(), 0, 0, 0, 0, 0, 0]),
                (*val).into(),
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
            Instruction::MemCpy { dst, src, count } => todo!(),
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
        }
    }
}
