use crate::cpu::{AddressRegister, Instruction, Register, Root, Trap};

impl Register {
    fn encode(&self) -> u8 {
        self.0 as u8
    }
    fn decode(reg: u8) -> Self {
        Self(reg as usize)
    }
}

impl AddressRegister {
    fn encode(&self) -> u8 {
        return self.0 as u8 + 16;
    }
    fn decode(reg: u8) -> Self {
        Self(reg as usize - 16)
    }
}

// TODO: Find a real encoding/decoding.
impl Instruction {
    pub fn decode(lo: u64, hi: u64) -> Result<Self, Trap> {
        let [opcode, r0, r1, ..] = lo.to_le_bytes();
        match opcode {
            255 => Ok(Self::Halt),
            1 => Ok(Self::Nop),
            2 => Ok(Self::JumpAdr {
                target: AddressRegister::decode(r0),
            }),
            3 => Ok(Self::JumpReg {
                target: Register::decode(r0),
            }),
            4 => Ok(Self::JumpImm { target: hi }),
            5 => Ok(Self::JumpRel { offset: hi as i64 }),
            6 => Ok(Self::JumpIfAdr {
                target: AddressRegister::decode(r1),
                condition: Register(r1 as usize),
            }),
            7 => Ok(Self::JumpIfReg {
                target: Register::decode(r0),
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
                target: AddressRegister::decode(r1),
                condition: Register(r1 as usize),
            }),
            11 => Ok(Self::JumpIfNotReg {
                target: Register::decode(r0),
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
                target: AddressRegister::decode(r1),
            }),
            15 => Ok(Self::CallReg {
                target: Register::decode(r0),
            }),
            16 => Ok(Self::CallImm { target: hi }),
            17 => Ok(Self::CallRel { offset: hi as i64 }),

            18 => Ok(Self::Return),
            19 => Ok(Self::MakeClosure {
                dst: Register::decode(r0),
                code: AddressRegister::decode(r0),
            }),

            20 => Ok(Self::Eq {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            21 => Ok(Self::Ne {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            22 => Ok(Self::Gt {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            23 => Ok(Self::Gte {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            24 => Ok(Self::Lt {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            25 => Ok(Self::Lte {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),

            26 => Ok(Self::Cons {
                car: Register::decode(r0),
                cdr: Register::decode(r1),
            }),
            27 => Ok(Self::Car {
                dst: Register::decode(r0),
                src: Register::decode(r0),
            }),
            28 => Ok(Self::Cdr {
                dst: Register::decode(r0),
                src: Register::decode(r0),
            }),
            29 => Ok(Self::SetCar {
                dst: Register::decode(r0),
                val: Register::decode(r0),
            }),
            30 => Ok(Self::SetCdr {
                dst: Register::decode(r0),
                val: Register::decode(r0),
            }),
            31 => Ok(Self::Uncons {
                car: Register::decode(r0),
                cdr: Register::decode(r0),
                src: Register::decode(r0),
            }),
            32 => Ok(Self::Add {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            33 => Ok(Self::Sub {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            34 => Ok(Self::Mul {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            35 => Ok(Self::Div {
                dst: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            36 => Ok(Self::IDiv {
                div: Register::decode(r0),
                rem: Register::decode(r0),
                op1: Register::decode(r0),
                op2: Register::decode(r0),
            }),
            37 => Ok(Self::LoadPc {
                dst: AddressRegister::decode(r0),
            }),
            38 => Ok(Self::PushPc),
            39 => Ok(Self::LoadSp {
                dst: AddressRegister::decode(r0),
            }),
            42 => Ok(Self::StoreSp {
                src: AddressRegister::decode(r0),
            }),
            43 => Ok(Self::PopR {
                dst: Register::decode(r0),
            }),
            44 => Ok(Self::PushR {
                src: Register::decode(r0),
            }),
            45 => Ok(Self::PopA {
                dst: AddressRegister::decode(r0),
            }),
            46 => Ok(Self::PushA {
                src: AddressRegister::decode(r0),
            }),
            47 => Ok(Self::LoadChar {
                dst: Register::decode(r0),
                val: hi,
            }),
            48 => Ok(Self::LoadFixnum {
                dst: Register::decode(r0),
                val: hi,
            }),
            49 => match hi {
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
            50 => Ok(Instruction::LoadAddress {
                dst: AddressRegister::decode(r0),
                address: hi,
            }),
            52 => Ok(Self::StoreOffsetReg {
                base: AddressRegister::decode(r0),
                offset: hi as i64,
                value: Register(r1 as usize),
            }),
            53 => Ok(Self::LoadCons {
                dst: Register::decode(r0),
                address: AddressRegister(r1 as usize - 16),
            }),
            254 => Ok(Self::IReturn { count: hi }),
            _ => Err(Trap::InvalidInstruction),
        }
    }

    pub fn encode(&self) -> (u64, u64) {
        match self {
            // Control flow
            Instruction::Halt => (u64::from_le_bytes([255, 0, 0, 0, 0, 0, 0, 0]), 0),
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

            Instruction::LoadSp { dst } => (
                u64::from_le_bytes([39, dst.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::StoreSp { src } => (
                u64::from_le_bytes([42, src.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),

            Instruction::PopR { dst } => {
                (u64::from_le_bytes([43, dst.0 as u8, 0, 0, 0, 0, 0, 0]), 0)
            }
            Instruction::PushR { src } => {
                (u64::from_le_bytes([44, src.0 as u8, 0, 0, 0, 0, 0, 0]), 0)
            }
            Instruction::PopA { dst } => (
                u64::from_le_bytes([45, dst.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::PushA { src } => (
                u64::from_le_bytes([46, src.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                0,
            ),

            Instruction::LoadChar { dst, val } => (
                u64::from_le_bytes([47, dst.0 as u8, 0, 0, 0, 0, 0, 0]),
                *val as u64,
            ),
            Instruction::LoadFixnum { dst, val } => (
                u64::from_le_bytes([48, dst.0 as u8, 0, 0, 0, 0, 0, 0]),
                *val as u64,
            ),
            Instruction::LoadRoot { dst, root } => (
                u64::from_le_bytes([49, dst.0 as u8, 0, 0, 0, 0, 0, 0]),
                *root as u64,
            ),

            Instruction::LoadAddress { dst, address } => (
                u64::from_le_bytes([50, dst.0 as u8 + 16, 0, 0, 0, 0, 0, 0]),
                *address,
            ),
            Instruction::LoadCons { dst, address } => (
                u64::from_le_bytes([53, dst.0 as u8, address.0 as u8 + 16, 0, 0, 0, 0, 0]),
                0,
            ),
            Instruction::ReadOffsetAdr { dst, base, offset } => (0, 0),
            Instruction::ReadOffsetReg { dst, base, offset } => (0, 0),
            Instruction::StoreOffsetAdr {
                base,
                offset,
                value,
            } => (0, 0),
            Instruction::StoreOffsetReg {
                base,
                offset,
                value,
            } => (0, 0),
            Instruction::IReturn { count } => (
                u64::from_le_bytes([254, 0, 0, 0, 0, 0, 0, 0]),
                *count as u64,
            ),
            Instruction::MovAR { dst, src } => (0, 0),
            Instruction::MovRA { dst, src } => (0, 0),
            Instruction::AAdd { dst, op1, op2 } => (0, 0),
            Instruction::GetPayload { dst, src } => (0, 0),
        }
    }
}
