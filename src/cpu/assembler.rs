use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::{
        anychar,
        complete::{alpha1, alphanumeric1, digit1, hex_digit1, none_of, one_of, space0, space1},
    },
    combinator::{opt, recognize, value},
    multi::{many0, many0_count},
    sequence::{delimited, pair, preceded, terminated},
};

use std::collections::HashMap;

use crate::{
    cpu::{self, LispWord, Native, TwoRegs},
    memory::{self, Address, Offset},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ThreeRegs {
    pub dst: crate::cpu::Register,
    pub op1: crate::cpu::Register,
    pub op2: Option<crate::cpu::Register>,
    pub op3: Option<Reference>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ThreeMachs {
    pub dst: crate::cpu::MachineRegister,
    pub op1: crate::cpu::MachineRegister,
    pub op2: Option<crate::cpu::MachineRegister>,
    pub op3: Option<Reference>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Location {
    Literal(Reference),
    Absolute(Reference),
    Register(crate::cpu::Register),
    Machine(crate::cpu::MachineRegister),
    IndirectRegister(crate::cpu::Register),
    IndirectMachine(crate::cpu::MachineRegister, Reference),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum JumpTarget {
    Absolute(Reference),
    Register(crate::cpu::Register),
    Machine(crate::cpu::MachineRegister, Reference),
    IndirectRegister(crate::cpu::Register),
    IndirectMachine(crate::cpu::MachineRegister, Reference),
}

impl Location {
    fn try_resolve(&self) -> Option<cpu::Location> {
        match self {
            &Location::Literal(Reference::Resolved(r)) => {
                Some(cpu::Location::Literal(cpu::Native(r as u64)))
            }
            &Location::Absolute(Reference::Resolved(r)) => {
                Some(cpu::Location::Absolute(cpu::Address(r as u64)))
            }
            &Location::Machine(reg) => Some(cpu::Location::Machine(reg)),
            &Location::Register(reg) => Some(cpu::Location::Register(reg)),
            &Location::IndirectMachine(reg, Reference::Resolved(off)) => {
                Some(cpu::Location::IndirectMachine(reg, Offset(off)))
            }
            &Location::IndirectRegister(r) => Some(cpu::Location::IndirectRegister(r)),
            _ => None,
        }
    }
}
impl JumpTarget {
    fn try_resolve(&self) -> Option<cpu::JumpTarget> {
        match self {
            &JumpTarget::Absolute(Reference::Resolved(r)) => {
                Some(cpu::JumpTarget::Absolute(Address(r as u64)))
            }
            &JumpTarget::Machine(reg, Reference::Resolved(off)) => {
                Some(cpu::JumpTarget::Machine(reg, Offset(off)))
            }
            &JumpTarget::Register(reg) => Some(cpu::JumpTarget::Register(reg)),
            &JumpTarget::IndirectMachine(reg, Reference::Resolved(off)) => {
                Some(cpu::JumpTarget::IndirectMachine(reg, Offset(off)))
            }
            &JumpTarget::IndirectRegister(r) => Some(cpu::JumpTarget::IndirectRegister(r)),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UnresolvedInstruction {
    Jump {
        condition: crate::cpu::Condition,
        target: JumpTarget,
    },
    Int(Reference),
    Mov {
        dst: Location,
        src: Location,
    },
    Mov8 {
        dst: Location,
        src: Location,
    },
    MBinary {
        op: crate::cpu::MBinaryOp,
        operands: ThreeMachs,
    },
    IDiv {
        div: crate::cpu::Register,
        rem: crate::cpu::Register,
        op1: crate::cpu::Register,
        op2: Option<crate::cpu::Register>,
        op3: Option<Reference>,
    },
    Binary {
        op: crate::cpu::BinaryOp,
        operands: ThreeRegs,
    },
    MemCpy {
        dst: crate::cpu::MachineRegister,
        src: crate::cpu::MachineRegister,
        count: Reference,
    },
    MemSet {
        dst: crate::cpu::MachineRegister,
        src: crate::cpu::MachineRegister,
        count: Reference,
    },
    Call {
        target: JumpTarget,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Reference {
    UnresolvedNeg(String),
    UnresolvedPos(String),
    Resolved(i64),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Data {
    Symbol(Reference),
    Literal(Reference),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AssemblyToken {
    Ord(usize),
    UnresolvedData(Data),
    ResolvedData(Vec<u8>),
    ResolvedInstruction(crate::cpu::Instruction),
    UnresolvedInstruction(UnresolvedInstruction),
    Label(String),
}

fn ident(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))
    .parse(input)
}

fn reference(input: &str) -> IResult<&str, Reference> {
    preceded(tag("'"), ident)
        .map(|name| Reference::UnresolvedPos(name.to_string()))
        .parse(input)
}

fn nop(input: &str) -> IResult<&str, AssemblyToken> {
    value(
        AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Nop),
        tag("NOP"),
    )
    .parse(input)
}

fn halt(input: &str) -> IResult<&str, AssemblyToken> {
    tag("HALT")
        .map(|_| AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Halt))
        .parse(input)
}

fn return_op(input: &str) -> IResult<&str, AssemblyToken> {
    tag("RETURN")
        .map(|_| AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Return))
        .parse(input)
}

fn ireturn_op(input: &str) -> IResult<&str, AssemblyToken> {
    tag("IRETURN")
        .map(|_| AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::IReturn))
        .parse(input)
}

fn fixnum(input: &str) -> IResult<&str, Reference> {
    preceded(tag("#"), number)
        .map(|n| Reference::Resolved(LispWord::fixnum(n as u64).0 as i64))
        .parse(input)
}

fn charlit(input: &str) -> IResult<&str, Reference> {
    preceded(
        tag("\\#"),
        alt((
            value(LispWord::char('\n' as u64), tag("Newline")),
            anychar.map(|n| LispWord::char(n as u64)),
        )),
    )
    .map(|c| Reference::Resolved(c.0 as i64))
    .parse(input)
}

fn any_value(input: &str) -> IResult<&str, Reference> {
    alt((
        number.map(|v| Reference::Resolved(v)),
        reference,
        fixnum,
        charlit,
    ))
    .parse(input)
}

fn interrupt(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("INT").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, target) = any_value.parse(rest)?;
    match target {
        Reference::Resolved(v) => Ok((
            rest,
            AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Int(v as u64)),
        )),
        unresolved => Ok((
            rest,
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Int(unresolved)),
        )),
    }
}

fn register(input: &str) -> IResult<&str, crate::cpu::Register> {
    preceded(tag("R"), digit1)
        .map_res(|n: &str| n.parse().map(crate::cpu::Register))
        .parse(input)
}

// TODO: Handle negatives
fn offset(input: &str) -> IResult<&str, Reference> {
    let (rest, sign) = one_of("+-").parse(input)?;
    let (rest, _) = space0.parse(rest)?;
    any_value
        .map(|lit| {
            if sign == '+' {
                lit
            } else {
                match lit {
                    Reference::Resolved(r) => Reference::Resolved(-r),
                    Reference::UnresolvedNeg(r) => Reference::UnresolvedPos(r),
                    Reference::UnresolvedPos(r) => Reference::UnresolvedNeg(r),
                }
            }
        })
        .parse(rest)
}

fn machineregister(input: &str) -> IResult<&str, crate::cpu::MachineRegister> {
    let normalreg =
        preceded(tag("A"), digit1).map_res(|n: &str| n.parse().map(crate::cpu::MachineRegister));
    let sp = tag("SP").map(|_| cpu::Cpu::SP);
    let pc = tag("PC").map(|_| cpu::Cpu::PC);
    let env = tag("ENV").map(|_| cpu::Cpu::ENV);
    alt((normalreg, sp, pc, env)).parse(input)
}

fn machine_and_offset(input: &str) -> IResult<&str, (cpu::MachineRegister, Reference)> {
    let (rest, adr) = machineregister.parse(input)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, offset) = opt(offset).parse(rest)?;
    Ok((rest, (adr, offset.unwrap_or(Reference::Resolved(0)))))
}

fn jump(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("JUMP").parse(input)?;
    let (rest, condname) = opt(alt((tag("IFNOT"), tag("IF")))).parse(rest)?;
    let (rest, condition) = match condname {
        Some("IF") => {
            let (rest, _) = space1.parse(rest)?;
            let (rest, register) = register.parse(rest)?;
            let (rest, _) = tag(",").parse(rest)?;
            (rest, crate::cpu::Condition::True(register))
        }
        Some("IFNOT") => {
            let (rest, _) = space1.parse(rest)?;
            let (rest, register) = register.parse(rest)?;
            let (rest, _) = tag(",").parse(rest)?;
            (rest, crate::cpu::Condition::False(register))
        }
        None => (rest, crate::cpu::Condition::Always),
        _ => {
            return Err(nom::Err::Error(nom::error::Error::new(
                rest,
                nom::error::ErrorKind::Fail,
            )));
        }
    };

    let (rest, _) = space1.parse(rest)?;
    let (rest, jumptarget) = jumptarget.parse(rest)?;
    Ok((
        rest,
        match jumptarget.try_resolve() {
            Some(resolved) => AssemblyToken::ResolvedInstruction(cpu::Instruction::Jump {
                condition,
                target: resolved,
            }),
            None => AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: jumptarget,
            }),
        },
    ))
}
fn call(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("CALL").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, jumptarget) = jumptarget.parse(rest)?;
    Ok((
        rest,
        match jumptarget {
            JumpTarget::Absolute(Reference::Resolved(target)) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                    target: cpu::JumpTarget::Absolute(Address(target as u64)),
                })
            }
            JumpTarget::Machine(reg, Reference::Resolved(target)) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                    target: cpu::JumpTarget::Machine(reg, Offset(target)),
                })
            }
            JumpTarget::Register(reg) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                    target: cpu::JumpTarget::Register(reg),
                })
            }

            JumpTarget::IndirectMachine(mr, Reference::Resolved(target)) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                    target: cpu::JumpTarget::IndirectMachine(mr, Offset(target)),
                })
            }

            JumpTarget::IndirectRegister(reg) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                    target: cpu::JumpTarget::IndirectRegister(reg),
                })
            }
            unresolved => AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: unresolved,
            }),
        },
    ))
}

fn pusha(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("PUSH").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    machineregister
        .map(|r| AssemblyToken::ResolvedInstruction(cpu::Instruction::PushA { src: r }))
        .parse(rest)
}
fn pushr(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("PUSH").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    register
        .map(|r| AssemblyToken::ResolvedInstruction(cpu::Instruction::PushR { src: r }))
        .parse(rest)
}
fn popa(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("POP").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    machineregister
        .map(|r| AssemblyToken::ResolvedInstruction(cpu::Instruction::PopA { dst: r }))
        .parse(rest)
}
fn popr(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("POP").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    register
        .map(|r| AssemblyToken::ResolvedInstruction(cpu::Instruction::PopR { dst: r }))
        .parse(rest)
}

fn location(input: &str) -> IResult<&str, Location> {
    alt((
        any_value.map(|r| Location::Literal(r)),
        register.map(|r| Location::Register(r)),
        machineregister.map(|reg| Location::Machine(reg)),
        delimited(tag("["), any_value, tag("]")).map(|l| Location::Absolute(l)),
        delimited(tag("["), register, tag("]")).map(|r| Location::IndirectRegister(r)),
        delimited(tag("["), machine_and_offset, tag("]"))
            .map(|(reg, refr)| Location::IndirectMachine(reg, refr)),
    ))
    .parse(input)
}

fn jumptarget(input: &str) -> IResult<&str, JumpTarget> {
    alt((
        any_value.map(|r| JumpTarget::Absolute(r)),
        register.map(|r| JumpTarget::Register(r)),
        machine_and_offset.map(|(reg, off)| JumpTarget::Machine(reg, off)),
        delimited(tag("["), register, tag("]")).map(|r| JumpTarget::IndirectRegister(r)),
        delimited(tag("["), machine_and_offset, tag("]"))
            .map(|(reg, refr)| JumpTarget::IndirectMachine(reg, refr)),
    ))
    .parse(input)
}

fn mov(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("MOV").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, target) = location.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, source) = location.parse(rest)?;
    Ok((
        rest,
        match (target.try_resolve(), source.try_resolve()) {
            (Some(rtarget), Some(rsource)) => {
                AssemblyToken::ResolvedInstruction(cpu::Instruction::Mov {
                    dst: rtarget,
                    src: rsource,
                })
            }
            _ => AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Mov {
                dst: target,
                src: source,
            }),
        },
    ))
}

fn mov8(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("MOV8").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, target) = location.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, source) = location.parse(rest)?;
    Ok((
        rest,
        match (target.try_resolve(), source.try_resolve()) {
            (Some(rtarget), Some(rsource)) => {
                AssemblyToken::ResolvedInstruction(cpu::Instruction::Mov8 {
                    dst: rtarget,
                    src: rsource,
                })
            }
            _ => AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Mov8 {
                dst: target,
                src: source,
            }),
        },
    ))
}

fn threemachs(input: &str) -> IResult<&str, ThreeMachs> {
    let (rest, dst) = machineregister.parse(input)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, op1) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, op2) = opt(machineregister).parse(rest)?;

    let (rest, op3) = if op2.is_some() {
        opt(preceded((space0, tag("+"), space0), any_value)).parse(rest)?
    } else {
        opt(any_value).parse(rest)?
    };

    Ok((rest, ThreeMachs { dst, op1, op2, op3 }))
}

fn mbin(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, op) = alt((
        tag("ADD").map(|_| cpu::MBinaryOp::Add),
        tag("SUB").map(|_| cpu::MBinaryOp::Sub),
    ))
    .parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, threemachs) = threemachs.parse(rest)?;

    Ok((
        rest,
        match threemachs {
            ThreeMachs {
                dst,
                op1,
                op2,
                op3: None,
            } => AssemblyToken::ResolvedInstruction(cpu::Instruction::MBinary {
                op,
                operands: cpu::ThreeMachs {
                    dst,
                    op1,
                    op2,
                    op3: None,
                },
            }),
            ThreeMachs {
                dst,
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            } => AssemblyToken::ResolvedInstruction(cpu::Instruction::MBinary {
                op,
                operands: cpu::ThreeMachs {
                    dst,
                    op1,
                    op2,
                    op3: Some(cpu::Native(r as u64)),
                },
            }),
            unresolved => AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MBinary {
                op,
                operands: unresolved,
            }),
        },
    ))
}

fn settag(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("SETTAG").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::SetTag { dst, src }),
    ))
}
fn setpayload(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("SETPAYLOAD").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::SetPayload { dst, src }),
    ))
}
fn gettag(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("GETTAG").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::GetTag { dst, src }),
    ))
}
fn getpayload(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("GETPAYLOAD").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::GetPayload { dst, src }),
    ))
}

fn cons(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("CONS").parse(input)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::Int(0x03)),
    ))
}
fn uncons(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("UNCONS").parse(input)?;
    let (rest, _) = space1.parse(rest)?;

    let (rest, car) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, cdr) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::Uncons { car, cdr, src }),
    ))
}

fn tworegs(input: &str) -> IResult<&str, cpu::TwoRegs> {
    let (rest, dst) = register.parse(input)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((rest, cpu::TwoRegs { dst, src }))
}

fn car(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("CAR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::Car(tworegs)),
    ))
}

fn cdr(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("CDR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::Cdr(tworegs)),
    ))
}

fn setcar(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("SETCAR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::SetCar(tworegs)),
    ))
}

fn setcdr(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("SETCDR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::SetCdr(tworegs)),
    ))
}
fn threeregs(input: &str) -> IResult<&str, ThreeRegs> {
    let (rest, dst) = register.parse(input)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, op1) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, op2) = opt(register).parse(rest)?;

    let (rest, op3) = if op2.is_some() {
        opt(preceded((space0, tag("+"), space0), any_value)).parse(rest)?
    } else {
        opt(any_value).parse(rest)?
    };

    Ok((rest, ThreeRegs { dst, op1, op2, op3 }))
}

fn bin(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, op) = alt((
        tag("ADD").map(|_| cpu::BinaryOp::Add),
        tag("SUB").map(|_| cpu::BinaryOp::Sub),
        tag("MUL").map(|_| cpu::BinaryOp::Mul),
        tag("EQ").map(|_| cpu::BinaryOp::Eq),
        tag("NE").map(|_| cpu::BinaryOp::Ne),
        tag("GTE").map(|_| cpu::BinaryOp::Gte),
        tag("GT").map(|_| cpu::BinaryOp::Gt),
        tag("LTE").map(|_| cpu::BinaryOp::Lte),
        tag("LT").map(|_| cpu::BinaryOp::Lt),
    ))
    .parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, threeregs) = threeregs.parse(rest)?;

    Ok((
        rest,
        match threeregs {
            ThreeRegs {
                dst,
                op1,
                op2,
                op3: None,
            } => AssemblyToken::ResolvedInstruction(cpu::Instruction::Binary {
                op,
                operands: cpu::ThreeRegs {
                    dst,
                    op1,
                    op2,
                    op3: None,
                },
            }),
            ThreeRegs {
                dst,
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            } => AssemblyToken::ResolvedInstruction(cpu::Instruction::Binary {
                op,
                operands: cpu::ThreeRegs {
                    dst,
                    op1,
                    op2,
                    op3: Some(cpu::LispWord(r as u64)),
                },
            }),
            unresolved => AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Binary {
                op,
                operands: unresolved,
            }),
        },
    ))
}

fn div(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("DIV").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, div) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, threeregs) = threeregs.parse(rest)?;
    Ok((
        rest,
        match threeregs {
            ThreeRegs {
                dst,
                op1,
                op2,
                op3: None,
            } => AssemblyToken::ResolvedInstruction(cpu::Instruction::IDiv {
                div,
                rem: dst,
                op1,
                op2,
                op3: None,
            }),
            ThreeRegs {
                dst,
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            } => AssemblyToken::ResolvedInstruction(cpu::Instruction::IDiv {
                div,
                rem: dst,
                op1,
                op2,
                op3: Some(LispWord(r as u64)),
            }),
            ThreeRegs { dst, op1, op2, op3 } => {
                AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::IDiv {
                    div,
                    rem: dst,
                    op1,
                    op2,
                    op3,
                })
            }
        },
    ))
}
fn makeclosure(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("MAKECLOSURE").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::MakeClosure { code: src, dst }),
    ))
}

fn number(input: &str) -> IResult<&str, i64> {
    alt((hex, decimal)).parse(input)
}

fn typep(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("TYPEP").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, compare) = number.parse(rest)?;

    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::Typep {
            dst,
            src,
            compare: cpu::Native(compare as u64),
        }),
    ))
}

fn memcpy(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("MEMCPY").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, count) = number.map(|c| cpu::Count(c as u64)).parse(rest)?;

    Ok((
        rest,
        AssemblyToken::ResolvedInstruction(cpu::Instruction::MemCpy { dst, src, count }),
    ))
}

fn instruction(input: &str) -> IResult<&str, AssemblyToken> {
    alt((
        alt((
            nop, halt, return_op, ireturn_op, interrupt, jump, call, pusha, popa, pushr, popr, mov,
            mov8, mbin, settag, gettag, setpayload, getpayload, cons, uncons, car,
        )),
        alt((cdr, setcar, setcdr, bin, div, makeclosure, typep, memcpy)),
    ))
    .parse(input)
}

fn decimal(input: &str) -> IResult<&str, i64> {
    let (rest, value) = digit1.parse(input)?;
    Ok((rest, value.parse::<i64>().unwrap()))
}

fn hex(input: &str) -> IResult<&str, i64> {
    // TODO: hex_u64
    let (rest, _) = tag("0x").parse(input)?;
    let (rest, v1) = recognize(hex_digit1).parse(rest)?;
    let v = i64::from_str_radix(v1, 16).unwrap();
    Ok((rest, v as i64))
}

fn ord(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("ord").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) = number.parse(rest)?;
    Ok((rest, AssemblyToken::Ord(pos as usize)))
}

fn symbol(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("symbol").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) = any_value.parse(rest)?;
    Ok((
        rest,
        match pos {
            Reference::Resolved(r) => {
                AssemblyToken::ResolvedData(LispWord::symbol(r as u64).0.to_le_bytes().to_vec())
            }
            unresolved => AssemblyToken::UnresolvedData(Data::Symbol(unresolved)),
        },
    ))
}

fn w(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("w").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) = any_value.parse(rest)?;
    Ok((
        rest,
        match pos {
            Reference::Resolved(r) => AssemblyToken::ResolvedData(r.to_le_bytes().to_vec()),
            unresolved => AssemblyToken::UnresolvedData(Data::Literal(unresolved)),
        },
    ))
}

fn string(input: &str) -> IResult<&str, AssemblyToken> {
    let (rest, _) = tag("str").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) =
        delimited(tag("\""), recognize(many0(none_of("\""))), tag("\"")).parse(rest)?;
    let lenblock = LispWord::fixnum(pos.len() as u64);

    let mut encoded = lenblock.0.to_le_bytes().to_vec();
    encoded.extend(pos.as_bytes().to_vec());
    Ok((rest, AssemblyToken::ResolvedData(encoded)))
}

fn directive(input: &str) -> IResult<&str, AssemblyToken> {
    preceded(tag("."), alt((ord, symbol, string, w))).parse(input)
}

fn label(input: &str) -> IResult<&str, AssemblyToken> {
    terminated(ident, tag(":"))
        .map(|name| AssemblyToken::Label(name.to_owned()))
        .parse(input)
}

fn comment(input: &str) -> IResult<&str, ()> {
    preceded(tag(";"), many0(none_of("\n")))
        .map(|_| ())
        .parse(input)
}

fn asm_line(input: &str) -> IResult<&str, Vec<AssemblyToken>> {
    let mut tokens = Vec::<AssemblyToken>::new();
    let (rest, _) = space0.parse(input)?;
    let (rest, label) = opt(label).parse(rest)?;
    if let Some(label) = label {
        tokens.push(label)
    }
    let (rest, _) = space0.parse(rest)?;
    let (rest, directive) = opt(directive).parse(rest)?;
    if let Some(line) = directive {
        tokens.push(line)
    }
    let (rest, instruction) = opt(instruction).parse(rest)?;
    if let Some(line) = instruction {
        tokens.push(line)
    }
    let (rest, _) = space0.parse(rest)?;
    let (rest, _) = opt(comment).parse(rest)?;
    let (rest, _) = tag("\n").parse(rest)?;

    Ok((rest, tokens))
}

fn asm_lines(input: &str) -> IResult<&str, Vec<AssemblyToken>> {
    let (rest, lines) = many0(asm_line).parse(input)?;
    let (rest, _) = space0.parse(rest)?;
    Ok((rest, lines.concat()))
}

pub fn parse(input: &str) -> Result<Vec<AssemblyToken>, String> {
    let (rest, assembly_lines) = asm_lines(input).map_err(|e| format!("parse error: {}", e))?;

    if rest != "" {
        return Err(rest.to_string());
    }

    Ok(assembly_lines)
}

pub fn layout(assembly_lines: &[AssemblyToken]) -> HashMap<String, usize> {
    let mut position: usize = 0;
    let mut labels = HashMap::new();
    for line in assembly_lines {
        match line {
            AssemblyToken::ResolvedData(d) => position = position.next_multiple_of(8) + d.len(),
            AssemblyToken::UnresolvedData(Data::Symbol(_)) => position += 8,
            AssemblyToken::UnresolvedData(Data::Literal(_)) => position += 8,
            AssemblyToken::ResolvedInstruction(_) => {
                position = position.next_multiple_of(16) + 16;
            }
            AssemblyToken::UnresolvedInstruction(_) => {
                position = position.next_multiple_of(16) + 16;
            }
            AssemblyToken::Label(label) => {
                // Maybe should look at next line to decide alignment?
                labels.insert(label.clone(), position.next_multiple_of(8));
            }
            AssemblyToken::Ord(p) => position = *p,
        }
    }
    labels
}

pub fn resolve_opt_reference(
    reference: &Option<Reference>,
    labels: &HashMap<String, usize>,
) -> Result<Option<usize>, String> {
    reference
        .clone()
        .map(|r| resolve_reference(&r, labels))
        .transpose()
}

pub fn resolve_reference(
    reference: &Reference,
    labels: &HashMap<String, usize>,
) -> Result<usize, String> {
    match reference {
        Reference::Resolved(_) => todo!("This shouldn't happen"),
        Reference::UnresolvedNeg(r) => {
            Ok(-(*labels.get(r).ok_or(format!("Symbol {} not found", r))? as isize) as usize)
        }
        Reference::UnresolvedPos(r) => {
            Ok(*labels.get(r).ok_or(format!("Symbol {} not found", r))? as usize)
        }
    }
}

pub fn resolve_location(
    location: &Location,
    labels: &HashMap<String, usize>,
) -> Result<cpu::Location, String> {
    Ok(match location {
        Location::Absolute(r) => {
            cpu::Location::Absolute(Address(resolve_reference(&r, labels)? as u64))
        }
        Location::IndirectMachine(m, r) => {
            cpu::Location::IndirectMachine(*m, memory::Offset(resolve_reference(r, labels)? as i64))
        }
        Location::IndirectRegister(r) => cpu::Location::IndirectRegister(*r),
        Location::Literal(r) => {
            cpu::Location::Literal(Native(resolve_reference(&r, labels)? as u64))
        }
        Location::Machine(m) => cpu::Location::Machine(*m),
        Location::Register(r) => cpu::Location::Register(*r),
    })
}

pub fn resolve(
    assembly_lines: &[AssemblyToken],
    labels: &HashMap<String, usize>,
) -> Result<Vec<AssemblyToken>, String> {
    let mut result = vec![];
    for line in assembly_lines {
        result.push(match line {
            AssemblyToken::Ord(o) => AssemblyToken::Ord(*o),
            AssemblyToken::Label(l) => AssemblyToken::Label(l.clone()),
            AssemblyToken::UnresolvedData(Data::Symbol(refr)) => AssemblyToken::ResolvedData(
                LispWord::symbol(resolve_reference(&refr, labels)? as u64)
                    .0
                    .to_le_bytes()
                    .to_vec(),
            ),
            AssemblyToken::UnresolvedData(Data::Literal(refr)) => AssemblyToken::ResolvedData(
                (resolve_reference(&refr, labels)? as u64)
                    .to_le_bytes()
                    .to_vec(),
            ),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Binary {
                op,
                operands: ThreeRegs { dst, op1, op2, op3 },
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: *op,
                operands: crate::cpu::ThreeRegs {
                    dst: *dst,
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(&op3, labels)?.map(|w| LispWord(w as u64)),
                },
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Absolute(refr),
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: cpu::JumpTarget::Absolute(
                    Address(resolve_reference(&refr, labels)? as u64),
                ),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Machine(m, refr),
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: cpu::JumpTarget::Machine(
                    *m,
                    Offset(resolve_reference(&refr, labels)? as i64),
                ),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::IndirectMachine(m, refr),
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: cpu::JumpTarget::IndirectMachine(
                    *m,
                    Offset(resolve_reference(&refr, labels)? as i64),
                ),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Register(_),
            }) => return Err("can't happen".to_string()),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::IndirectRegister(_),
            }) => return Err("can't happen".to_string()),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::IDiv {
                div,
                rem,
                op1,
                op2,
                op3,
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::IDiv {
                div: *div,
                rem: *rem,
                op1: *op1,
                op2: *op2,
                op3: resolve_opt_reference(&op3, labels)?.map(|v| LispWord(v as u64)),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Int(i)) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Int(resolve_reference(
                    &i, labels,
                )?
                    as u64))
            }
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::Absolute(refr),
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: *condition,
                target: cpu::JumpTarget::Absolute(
                    Address(resolve_reference(&refr, labels)? as u64),
                ),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::Machine(m, refr),
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: *condition,
                target: cpu::JumpTarget::Machine(
                    *m,
                    Offset(resolve_reference(&refr, labels)? as i64),
                ),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::IndirectMachine(m, refr),
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: *condition,
                target: cpu::JumpTarget::IndirectMachine(
                    *m,
                    Offset(resolve_reference(&refr, labels)? as i64),
                ),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition: _,
                target: JumpTarget::Register(_),
            }) => return Err("can't happen".to_string()),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition: _,
                target: JumpTarget::IndirectRegister(_),
            }) => return Err("can't happen".to_string()),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MBinary {
                op,
                operands: ThreeMachs { dst, op1, op2, op3 },
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: *op,
                operands: cpu::ThreeMachs {
                    dst: *dst,
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(&op3, labels)?.map(|v| Native(v as u64)),
                },
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MemCpy {
                dst,
                src,
                count,
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MemCpy {
                dst: *dst,
                src: *src,
                count: cpu::Count(resolve_reference(&count, labels)? as u64),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MemSet {
                dst,
                src,
                count,
            }) => AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MemSet {
                dst: *dst,
                src: *src,
                count: cpu::Count(resolve_reference(&count, labels)? as u64),
            }),
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Mov { dst, src }) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                    dst: resolve_location(&dst, labels)?,
                    src: resolve_location(&src, labels)?,
                })
            }
            AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::Mov8 { dst, src }) => {
                AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                    dst: resolve_location(&dst, labels)?,
                    src: resolve_location(&src, labels)?,
                })
            }

            AssemblyToken::ResolvedInstruction(r) => AssemblyToken::ResolvedInstruction(*r),
            AssemblyToken::ResolvedData(d) => AssemblyToken::ResolvedData(d.clone()),
        })
    }
    Ok(result)
}

pub fn assemble(assembly_lines: &[AssemblyToken]) -> Result<Vec<u8>, String> {
    let mut result: Vec<u8> = vec![];
    let mut position: usize = 0;

    for line in assembly_lines {
        match line {
            AssemblyToken::Label(_) => {}
            AssemblyToken::Ord(p) => position = *p,
            AssemblyToken::ResolvedData(d) => {
                position = position.next_multiple_of(8);
                result.resize((result.len().max(position)) + d.len(), 0);
                result[position..][..d.len()].copy_from_slice(d);
                position += d.len()
            }
            AssemblyToken::ResolvedInstruction(i) => {
                position = position.next_multiple_of(16);
                let (lo, hi) = i.encode();
                result.resize((result.len().max(position)) + 16, 0);
                result[position..][..8].copy_from_slice(&lo.to_le_bytes());
                position += 8;
                result[position..][..8].copy_from_slice(&hi.to_le_bytes());
                position += 8;
            }
            AssemblyToken::UnresolvedData(_) => return Err("Forgot to resolve".to_string()),
            AssemblyToken::UnresolvedInstruction(_) => return Err("Forgot to resolve".to_string()),
        }
    }

    Ok(result)
}

mod test {
    use crate::cpu::{
        self,
        assembler::{self, parse},
    };

    fn scaffold() -> (Vec<assembler::AssemblyToken>, Vec<assembler::AssemblyToken>) {
        let asm = assembler::parse(
            r#"
            .symbol 'loop
            HALT
            NOP
        ; A comment
        ; Two comments in a row
            RETURN ; And an inline one
        loop:
            INT 42
            INT 'loop
            IRETURN
        ; Jump variants
            JUMP 16
            JUMP A1
            JUMP A1 + 16
            JUMP R1
            JUMP 'loop
            JUMPIF R5, 16
            JUMPIF R5, A1
            JUMPIF R5, A1 + 16
            JUMPIF R5, R1
            JUMPIF R5, 'loop
            JUMPIFNOT R5, 16
            JUMPIFNOT R5, A1
            JUMPIFNOT R5, A1 + 16
            JUMPIFNOT R5, R1
            JUMPIFNOT R5, 'loop
            CALL 16
            CALL A1
            CALL A1 + 16
            CALL R1
            CALL 'loop
            ; Stack variants
            PUSH A1
            POP A2
            PUSH R3
            POP R4
            ; Loading & Memory
            MOV R1, #1234
            MOV A3, 0x7FFFFFFF
            MOV R5, R6
            MOV R5, [R6]
            MOV [R6], R7
            MOV A4, A5
            MOV A1, [A2]
            MOV A1, [A2 + 16]
            MOV A1, [64]
            MOV [A2], A3
            MOV [A2 + 8], A3
            MOV [24], A4
            MOV A5, R8
            MOV R9, A6
            MOV [A5], R8
            MOV [A5 + 8], R6
            MOV R5, [A6]
            MOV R5, [A6 + 8]
            MOV8 A3, [A4]
            MOV8 A3, [A4 + 5]
            MOV8 A3, [5]
            MOV8 [A4], A6
            MOV8 [A4 + 5], A6
            MOV8 [5], A6
        ; Machine Arithmetic
            ADD A1, A2, A3
            ADD A1, A2, A3 + 4
            ADD A1, A2, 8
            SUB A4, A5, A6
            SUB A4, A5, A6 + 2
            SUB A4, A5, 12
        ; Tag Operations
            SETTAG R2, A1
            GETTAG A3, R2
            SETPAYLOAD R4, A3
            GETPAYLOAD A5, R4
        ; Lisp Destructuring Primitives
            CONS
            UNCONS R1, R2, R3
            CAR R4, R5
            CDR R6, R7
            SETCAR R8, R9
            SETCDR R10, R11
        ; Arithmetic (assembler::ThreeRegs layouts using Option types, Word Immediates)
            ADD R1, R2, R3
            ADD R1, R2, R3 + #5
            ADD R1, R2, #10
            SUB R1, R2, R3
            SUB R1, R2, R3 + #5
            SUB R1, R2, #10
            MUL R1, R2, R3
            MUL R1, R2, R3 + #5
            MUL R1, R2, #10
            EQ R1, R2, R3
            EQ R1, R2, R3 + #5
            EQ R1, R2, #10
            NE R1, R2, R3
            NE R1, R2, R3 + #5
            NE R1, R2, #10
            LT R1, R2, R3
            LT R1, R2, R3 + #5
            LT R1, R2, #10
            LTE R1, R2, R3
            LTE R1, R2, R3 + #5
            LTE R1, R2, #10
            GT R1, R2, R3
            GT R1, R2, R3 + #5
            GT R1, R2, #10
            GTE R1, R2, R3
            GTE R1, R2, R3 + #5
            GTE R1, R2, #10
            DIV R9, R2, R3, R4
            DIV R9, R2, R3, R4 + #5
            DIV R9, R2, R3, #5
        ; Advanced Operations
            MAKECLOSURE R5, A6
            MEMCPY A1, A2, 1
        "#,
        );

        let expected = vec![
            assembler::AssemblyToken::UnresolvedData(assembler::Data::Symbol(
                assembler::Reference::UnresolvedPos("loop".to_string()),
            )),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Halt),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Nop),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Return),
            assembler::AssemblyToken::Label("loop".to_string()),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Int(42)),
            assembler::AssemblyToken::UnresolvedInstruction(assembler::UnresolvedInstruction::Int(
                assembler::Reference::UnresolvedPos("loop".to_string()),
            )),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::IReturn),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::Always,
                target: crate::cpu::JumpTarget::Absolute(crate::memory::Address(16)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::Always,
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::Always,
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(16),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::Always,
                target: crate::cpu::JumpTarget::Register(crate::cpu::Register(1)),
            }),
            assembler::AssemblyToken::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Jump {
                    condition: crate::cpu::Condition::Always,
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::True(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Absolute(crate::memory::Address(16)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::True(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::True(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(16),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::True(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Register(crate::cpu::Register(1)),
            }),
            assembler::AssemblyToken::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Jump {
                    condition: crate::cpu::Condition::True(crate::cpu::Register(5)),
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::False(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Absolute(crate::memory::Address(16)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::False(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::False(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(16),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Jump {
                condition: crate::cpu::Condition::False(crate::cpu::Register(5)),
                target: crate::cpu::JumpTarget::Register(crate::cpu::Register(1)),
            }),
            assembler::AssemblyToken::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Jump {
                    condition: crate::cpu::Condition::False(crate::cpu::Register(5)),
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: crate::cpu::JumpTarget::Absolute(crate::memory::Address(16)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: crate::cpu::JumpTarget::Machine(
                    crate::cpu::MachineRegister(1),
                    crate::memory::Offset(16),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Call {
                target: crate::cpu::JumpTarget::Register(crate::cpu::Register(1)),
            }),
            assembler::AssemblyToken::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Call {
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::PushA {
                src: crate::cpu::MachineRegister(1),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::PopA {
                dst: crate::cpu::MachineRegister(2),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::PushR {
                src: crate::cpu::Register(3),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::PopR {
                dst: crate::cpu::Register(4),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(1)),
                src: crate::cpu::Location::Literal(crate::cpu::Native(
                    crate::cpu::LispWord::fixnum(1234).0,
                )),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(3)),
                src: crate::cpu::Location::Literal(crate::cpu::Native(0x7fffffff)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(5)),
                src: crate::cpu::Location::Register(crate::cpu::Register(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(5)),
                src: crate::cpu::Location::IndirectRegister(crate::cpu::Register(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectRegister(crate::cpu::Register(6)),
                src: crate::cpu::Location::Register(crate::cpu::Register(7)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(4)),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(5)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(1)),
                src: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(2),
                    crate::cpu::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(1)),
                src: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(2),
                    crate::cpu::Offset(16),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(1)),
                src: crate::cpu::Location::Absolute(crate::cpu::Address(64)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(2),
                    crate::memory::Offset(0),
                ),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(3)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(2),
                    crate::memory::Offset(8),
                ),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(3)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Absolute(crate::memory::Address(24)),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(4)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(5)),
                src: crate::cpu::Location::Register(crate::cpu::Register(8)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(9)),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(5),
                    crate::memory::Offset(0),
                ),
                src: crate::cpu::Location::Register(crate::cpu::Register(8)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(5),
                    crate::memory::Offset(8),
                ),
                src: crate::cpu::Location::Register(crate::cpu::Register(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(5)),
                src: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(6),
                    crate::memory::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(5)),
                src: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(6),
                    crate::memory::Offset(8),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(3)),
                src: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(4),
                    crate::memory::Offset(0),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(3)),
                src: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(4),
                    crate::memory::Offset(5),
                ),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                dst: crate::cpu::Location::Machine(crate::cpu::MachineRegister(3)),
                src: crate::cpu::Location::Absolute(crate::memory::Address(5)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                dst: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(4),
                    crate::memory::Offset(0),
                ),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                dst: crate::cpu::Location::IndirectMachine(
                    crate::cpu::MachineRegister(4),
                    crate::memory::Offset(5),
                ),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Mov8 {
                dst: crate::cpu::Location::Absolute(crate::memory::Address(5)),
                src: crate::cpu::Location::Machine(crate::cpu::MachineRegister(6)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Add,
                operands: crate::cpu::ThreeMachs {
                    dst: crate::cpu::MachineRegister(1),
                    op1: crate::cpu::MachineRegister(2),
                    op2: Some(crate::cpu::MachineRegister(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Add,
                operands: crate::cpu::ThreeMachs {
                    dst: crate::cpu::MachineRegister(1),
                    op1: crate::cpu::MachineRegister(2),
                    op2: Some(crate::cpu::MachineRegister(3)),
                    op3: Some(crate::cpu::Native(4)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Add,
                operands: crate::cpu::ThreeMachs {
                    dst: crate::cpu::MachineRegister(1),
                    op1: crate::cpu::MachineRegister(2),
                    op2: None,
                    op3: Some(crate::cpu::Native(8)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Sub,
                operands: crate::cpu::ThreeMachs {
                    dst: crate::cpu::MachineRegister(4),
                    op1: crate::cpu::MachineRegister(5),
                    op2: Some(crate::cpu::MachineRegister(6)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Sub,
                operands: crate::cpu::ThreeMachs {
                    dst: crate::cpu::MachineRegister(4),
                    op1: crate::cpu::MachineRegister(5),
                    op2: Some(crate::cpu::MachineRegister(6)),
                    op3: Some(crate::cpu::Native(2)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MBinary {
                op: crate::cpu::MBinaryOp::Sub,
                operands: crate::cpu::ThreeMachs {
                    dst: crate::cpu::MachineRegister(4),
                    op1: crate::cpu::MachineRegister(5),
                    op2: None,
                    op3: Some(crate::cpu::Native(12)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::SetTag {
                dst: crate::cpu::Register(2),
                src: crate::cpu::MachineRegister(1),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::GetTag {
                dst: crate::cpu::MachineRegister(3),
                src: crate::cpu::Register(2),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::SetPayload {
                dst: crate::cpu::Register(4),
                src: crate::cpu::MachineRegister(3),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::GetPayload {
                dst: crate::cpu::MachineRegister(5),
                src: crate::cpu::Register(4),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Int(0x03)),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Uncons {
                car: crate::cpu::Register(1),
                cdr: crate::cpu::Register(2),
                src: crate::cpu::Register(3),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Car(
                assembler::TwoRegs {
                    dst: crate::cpu::Register(4),
                    src: crate::cpu::Register(5),
                },
            )),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Cdr(
                assembler::TwoRegs {
                    dst: crate::cpu::Register(6),
                    src: crate::cpu::Register(7),
                },
            )),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::SetCar(
                assembler::TwoRegs {
                    dst: crate::cpu::Register(8),
                    src: crate::cpu::Register(9),
                },
            )),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::SetCdr(
                assembler::TwoRegs {
                    dst: crate::cpu::Register(10),
                    src: crate::cpu::Register(11),
                },
            )),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Add,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Add,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Add,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Sub,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Sub,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Sub,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Mul,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Mul,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Mul,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Eq,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Eq,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Eq,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Ne,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Ne,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Ne,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Lt,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Lt,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Lt,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Lte,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Lte,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Lte,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Gt,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Gt,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Gt,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Gte,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Gte,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: Some(crate::cpu::Register(3)),
                    op3: Some(crate::cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::Binary {
                op: crate::cpu::BinaryOp::Gte,
                operands: crate::cpu::ThreeRegs {
                    dst: crate::cpu::Register(1),
                    op1: crate::cpu::Register(2),
                    op2: None,
                    op3: Some(crate::cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::IDiv {
                div: crate::cpu::Register(9),
                rem: crate::cpu::Register(2),
                op1: crate::cpu::Register(3),
                op2: Some(crate::cpu::Register(4)),
                op3: None,
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::IDiv {
                div: crate::cpu::Register(9),
                rem: crate::cpu::Register(2),
                op1: crate::cpu::Register(3),
                op2: Some(crate::cpu::Register(4)),
                op3: Some(crate::cpu::LispWord::fixnum(5)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::IDiv {
                div: crate::cpu::Register(9),
                rem: crate::cpu::Register(2),
                op1: crate::cpu::Register(3),
                op2: None,
                op3: Some(crate::cpu::LispWord::fixnum(5)),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MakeClosure {
                dst: crate::cpu::Register(5),
                code: crate::cpu::MachineRegister(6),
            }),
            assembler::AssemblyToken::ResolvedInstruction(crate::cpu::Instruction::MemCpy {
                dst: crate::cpu::MachineRegister(1),
                src: crate::cpu::MachineRegister(2),
                count: crate::cpu::Count(1),
            }),
        ];
        (asm.unwrap(), expected)
    }

    #[test]
    fn test_assembler_parse() {
        let (actual, expected) = scaffold();

        for i in 0..actual.len() {
            assert_eq!((i, &actual[i]), (i, &expected[i]));
        }
    }

    #[test]
    fn test_locate() {
        let (actual, _) = scaffold();
        let labels = assembler::layout(&actual);
        assert_eq!(labels.len(), 1);
        assert_eq!(labels.get("loop"), Some(&64));
    }

    #[test]
    fn test_resolve() -> Result<(), String> {
        let (actual, _) = scaffold();
        let labels = assembler::layout(&actual);
        let resolved = assembler::resolve(&actual, &labels).map_err(|e| e.to_string())?;

        for line in resolved {
            if let assembler::AssemblyToken::UnresolvedInstruction(i) = line {
                return Err(format!("{:?}", i).to_string());
            }
        }

        Ok(())
    }

    #[test]
    fn test_encoder_decoder() -> Result<(), String> {
        let (_, expected) = scaffold();
        let labels = assembler::layout(&expected);
        let resolved = assembler::resolve(&expected, &labels).map_err(|e| e.to_string())?;

        for i in 0..resolved.len() {
            match expected[i] {
                assembler::AssemblyToken::ResolvedInstruction(inst) => {
                    let (lo, hi) = inst.encode();
                    assert_eq!(
                        (
                            i,
                            cpu::Instruction::decode(lo, hi).map_err(|t| format!(
                                "Error when decoding {:?}: {:?} {:08x}:{:08x}",
                                inst, t, lo, hi
                            ))?
                        ),
                        (i, inst)
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[test]
    fn test_encoder_decoder_twice() -> Result<(), String> {
        let (_, expected) = scaffold();
        let labels = assembler::layout(&expected);
        let resolved = assembler::resolve(&expected, &labels).map_err(|e| e.to_string())?;

        for i in 0..resolved.len() {
            match expected[i] {
                assembler::AssemblyToken::ResolvedInstruction(inst) => {
                    let (lo, hi) = inst.encode();
                    let decoded = cpu::Instruction::decode(lo, hi).map_err(|t| {
                        format!(
                            "Error when decoding {:?}: {:?} {:08x}:{:08x}",
                            inst, t, lo, hi
                        )
                    })?;
                    let (lo, hi) = decoded.encode();
                    assert_eq!(
                        (
                            i,
                            cpu::Instruction::decode(lo, hi).map_err(|t| format!(
                                "Error when decoding {:?}: {:?} {:08x}:{:08x}",
                                inst, t, lo, hi
                            ))?
                        ),
                        (i, inst)
                    );
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[test]
    fn test_assemble() -> Result<(), String> {
        let (_, expected) = scaffold();
        let labels = assembler::layout(&expected);
        let resolved = assembler::resolve(&expected, &labels).map_err(|e| e.to_string())?;
        let result = assembler::assemble(&resolved)?;

        assert_eq!(result.len(), 1648);
        Ok(())
    }

    #[test]
    fn test_invalid() -> Result<(), String> {
        let invalid_instructions = vec!["MOV [SP], 0x0123\n"];

        for inst in invalid_instructions {
            let result = parse(inst);
            assert_eq!(result, Err(inst.to_string()));
        }
        Ok(())
    }
}
