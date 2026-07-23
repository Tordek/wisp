use int_enum::IntEnum;

use crate::cpu::{
    self, AddressRegister, Instruction, JumpAddressing, Register, Root, ThreeAddrs, ThreeRegs,
    Trap, TwoRegs, Word,
};

impl cpu::Register {
    fn encode(&self) -> u8 {
        self.0 as u8
    }
    fn decode(reg: u8) -> Self {
        Self(reg as usize)
    }
}

impl AddressRegister {
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
            op2: Some(Register(r3 as usize)),
            imm: cpu::Word::try_from(imm).map_err(|_| Trap::InvalidInstruction)?,
        })
    }
}

impl cpu::JumpAddressing {
    fn decode(cond: u8, adr: u8, offset: u64) -> Self {
        if adr < 16 {
            cpu::JumpAddressing::Register {
                condition: Register(cond as usize),
                adr: Register(adr as usize),
            }
        } else {
            cpu::JumpAddressing::AddressRegister {
                condition: AddressRegister(cond as usize - 16),
                adr: AddressRegister::try_decode(adr),
                offset: offset as i64,
            }
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
    LoadRoot,
    LoadAddress,
    ReadReg,
    StoreReg,
    ReadOffsetAdr,
    StoreOffsetAdr,
    MovAdr,
    MovAR,
    MovRA,
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
            Opcode::Jump => Ok(Self::Jump(JumpAddressing::decode(r0, r1, hi))),
            Opcode::JumpIf => Ok(Self::JumpIf(JumpAddressing::decode(r0, r1, hi))),
            Opcode::JumpIfNot => Ok(Self::JumpIfNot(JumpAddressing::decode(r0, r1, hi))),
            Opcode::Call => Ok(Self::Call(JumpAddressing::decode(r0, r1, hi))),

            Opcode::Return => Ok(Self::Return),
            Opcode::MakeClosure => Ok(Self::MakeClosure {
                dst: Register::decode(r1),
                code: AddressRegister::try_decode(r1).expect("Shit happened!"),
            }),

            Opcode::Eq => Ok(Self::Eq(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Ne => Ok(Self::Ne(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Gt => Ok(Self::Gt(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Gte => Ok(Self::Gte(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Lt => Ok(Self::Lt(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Lte => Ok(Self::Lte(ThreeRegs::decode(r1, r2, r3, hi)?)),

            Opcode::Car => Ok(Self::Car(TwoRegs::decode(r1, r2))),
            Opcode::Cdr => Ok(Self::Cdr(TwoRegs::decode(r1, r2))),
            Opcode::SetCar => Ok(Self::SetCar(TwoRegs::decode(r1, r2))),
            Opcode::SetCdr => Ok(Self::SetCdr(TwoRegs::decode(r1, r2))),
            Opcode::Uncons => Ok(Self::Uncons {
                car: Register::decode(r1),
                cdr: Register::decode(r1),
                src: Register::decode(r1),
            }),
            Opcode::Add => Ok(Self::Add(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Sub => Ok(Self::Sub(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::Mul => Ok(Self::Mul(ThreeRegs::decode(r1, r2, r3, hi)?)),
            Opcode::IDiv => Ok(Self::IDiv {
                div: Register::decode(01),
                rem: Register::decode(r1),
                op1: Register::decode(r2),
                op2: Some(Register::decode(r3)),
                imm: Word::try_from(hi).map_err(|_| Trap::InvalidInstruction)?,
            }),
            Opcode::PopR => Ok(Self::PopR {
                dst: Register::decode(r1),
            }),
            Opcode::PushR => Ok(Self::PushR {
                src: Register::decode(r1),
            }),
            Opcode::PopA => Ok(Self::PopA {
                dst: AddressRegister::decode(r0)?,
            }),
            Opcode::PushA => Ok(Self::PushA {
                src: AddressRegister::decode(r0)?,
            }),
            Opcode::LoadRoot => match hi {
                0 => Ok(Self::LoadRoot {
                    dst: Register::decode(r0),
                    root: Root::NIL,
                }),
                1 => Ok(Self::LoadRoot {
                    dst: Register::decode(r0),
                    root: Root::T,
                }),
                _ => Err(Trap::InvalidInstruction),
            },
            Opcode::LoadAddress => Ok(Instruction::LoadAddress {
                dst: AddressRegister::decode(r0)?,
                address: hi,
            }),
            Opcode::StoreReg => Ok(Self::StoreReg {
                base: Register::decode(r1),
                src: Register(r1 as usize),
            }),
            Opcode::IReturn => Ok(Self::IReturn { count: hi }),
            Opcode::AAdd => Ok(Instruction::AAdd(cpu::ThreeAddrs {
                dst: AddressRegister::decode(r0)?,
                op1: AddressRegister::decode(r1)?,
                op2: AddressRegister::try_decode(r2),
                imm: hi,
            })),
            Opcode::ASub => Ok(Instruction::ASub(cpu::ThreeAddrs {
                dst: AddressRegister::decode(r0)?,
                op1: AddressRegister::decode(r1)?,
                op2: AddressRegister::try_decode(r2),
                imm: hi,
            })),
            Opcode::GetPayload => Err(Trap::TypeError),
            Opcode::GetTag => Err(Trap::TypeError),
            Opcode::Halt => Err(Trap::TypeError),
            Opcode::Int => Err(Trap::TypeError),
            Opcode::LoadLiteral => Err(Trap::TypeError),
            Opcode::MovAR => Err(Trap::TypeError),
            Opcode::MovRA => Err(Trap::TypeError),
            Opcode::ReadOffsetAdr => Err(Trap::TypeError),
            Opcode::ReadReg => Err(Trap::TypeError),
            Opcode::SetPayload => Err(Trap::TypeError),
            Opcode::SetTag => Err(Trap::TypeError),
            Opcode::StoreOffsetAdr => Err(Trap::TypeError),
            Opcode::MovAdr => Err(Trap::TypeError),
        }
    }

    fn encode_op_jump_cond(opcode: Opcode, target: &JumpAddressing) -> (u64, u64) {
        match target {
            JumpAddressing::AddressRegister {
                condition,
                adr: None,
                offset,
            } => (
                u64::from_le_bytes([opcode.into(), condition.encode(), 24, 0, 0, 0, 0, 0]),
                *offset as u64,
            ),
            JumpAddressing::AddressRegister {
                condition,
                adr: Some(AddressRegister(r)),
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
            JumpAddressing::Register { condition, adr } => (
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

    fn encode_three_adrs(opcode: Opcode, target: &ThreeAddrs) -> (u64, u64) {
        match target {
            &ThreeAddrs {
                dst,
                op1,
                op2: None,
                imm,
            } => (
                u64::from_le_bytes([opcode.into(), dst.encode(), op1.encode(), 24, 0, 0, 0, 0]),
                imm.into(),
            ),
            &ThreeAddrs {
                dst,
                op1,
                op2: Some(op2),
                imm,
            } => (
                u64::from_le_bytes([
                    opcode.into(),
                    dst.encode(),
                    op1.encode(),
                    op2.encode(),
                    0,
                    0,
                    0,
                    0,
                ]),
                imm.into(),
            ),
        }
    }
    fn encode_three_regs(opcode: Opcode, target: &ThreeRegs) -> (u64, u64) {
        match target {
            &ThreeRegs {
                dst,
                op1,
                op2: None,
                imm,
            } => (
                u64::from_le_bytes([opcode.into(), dst.encode(), op1.encode(), 24, 0, 0, 0, 0]),
                imm.into(),
            ),
            &ThreeRegs {
                dst,
                op1,
                op2: Some(op2),
                imm,
            } => (
                u64::from_le_bytes([
                    opcode.into(),
                    dst.encode(),
                    op1.encode(),
                    op2.encode(),
                    0,
                    0,
                    0,
                    0,
                ]),
                imm.into(),
            ),
        }
    }
    fn encode_two_regs(opcode: Opcode, target: &TwoRegs) -> (u64, u64) {
        match target {
            &TwoRegs { dst, src } => (
                u64::from_le_bytes([opcode.into(), dst.encode(), src.encode(), 24, 0, 0, 0, 0]),
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

            Instruction::Jump(addressing) => Self::encode_op_jump_cond(Opcode::Jump, addressing),
            Instruction::JumpIf(addressing) => {
                Self::encode_op_jump_cond(Opcode::JumpIf, addressing)
            }
            Instruction::JumpIfNot(addressing) => {
                Self::encode_op_jump_cond(Opcode::JumpIf, addressing)
            }
            Instruction::Call(addressing) => Self::encode_op_jump_cond(Opcode::Call, addressing),

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

            Instruction::IDiv {
                div,
                rem,
                op1,
                op2,
                imm,
            } => (
                u64::from_le_bytes([
                    Opcode::IDiv.into(),
                    div.encode(),
                    rem.encode(),
                    op1.encode(),
                    op2.unwrap_or_default().encode(),
                    0,
                    0,
                    0,
                ]),
                u64::from(*imm),
            ),

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

            Instruction::LoadRoot { dst, root } => (
                u64::from_le_bytes([Opcode::LoadRoot.into(), dst.encode(), 0, 0, 0, 0, 0, 0]),
                *root as u64,
            ),

            Instruction::LoadAddress { dst, address } => (
                u64::from_le_bytes([Opcode::LoadAddress.into(), dst.encode(), 0, 0, 0, 0, 0, 0]),
                *address,
            ),
            Instruction::ReadOffsetAdr {
                dst,
                base: None,
                offset,
            } => (
                u64::from_le_bytes([
                    Opcode::ReadOffsetAdr.into(),
                    dst.encode(),
                    24,
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                *offset as u64,
            ),
            Instruction::ReadOffsetAdr {
                dst,
                base: Some(adr),
                offset,
            } => (
                u64::from_le_bytes([
                    Opcode::ReadOffsetAdr.into(),
                    dst.encode(),
                    adr.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                *offset as u64,
            ),
            Instruction::ReadReg { dst, base } => (
                u64::from_le_bytes([
                    Opcode::LoadAddress.into(),
                    dst.encode(),
                    base.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::StoreOffsetAdr {
                base: None,
                offset,
                value,
            } => (
                u64::from_le_bytes([
                    Opcode::StoreOffsetAdr.into(),
                    24,
                    value.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                *offset as u64,
            ),
            Instruction::StoreOffsetAdr {
                base: Some(base),
                offset,
                value,
            } => (
                u64::from_le_bytes([
                    Opcode::StoreOffsetAdr.into(),
                    base.encode(),
                    value.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                *offset as u64,
            ),
            Instruction::StoreReg { base, src } => (
                u64::from_le_bytes([
                    Opcode::StoreReg.into(),
                    base.encode(),
                    src.encode(),
                    0,
                    0,
                    0,
                    0,
                    0,
                ]),
                0,
            ),
            Instruction::IReturn { count } => (
                u64::from_le_bytes([Opcode::IReturn.into(), 0, 0, 0, 0, 0, 0, 0]),
                *count,
            ),
            Instruction::MovAR { dst, src } => (
                u64::from_le_bytes([
                    Opcode::MovAR.into(),
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
            Instruction::MovRA { dst, src } => (
                u64::from_le_bytes([
                    Opcode::MovRA.into(),
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
                    src.encode(),
                    dst.encode(),
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
            Instruction::MovAdr { dst, src } => (
                u64::from_le_bytes([
                    Opcode::MovAdr.into(),
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
                    src.encode(),
                    dst.encode(),
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
}
