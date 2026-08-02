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

use crate::cpu::assembler::*;

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

fn nop(input: &str) -> IResult<&str, AssemblyLine> {
    value(
        AssemblyLine::ResolvedInstruction(cpu::Instruction::Nop),
        tag("NOP"),
    )
    .parse(input)
}

fn halt(input: &str) -> IResult<&str, AssemblyLine> {
    tag("HALT")
        .map(|_| AssemblyLine::ResolvedInstruction(cpu::Instruction::Halt))
        .parse(input)
}

fn return_op(input: &str) -> IResult<&str, AssemblyLine> {
    tag("RETURN")
        .map(|_| AssemblyLine::ResolvedInstruction(cpu::Instruction::Return))
        .parse(input)
}

fn ireturn_op(input: &str) -> IResult<&str, AssemblyLine> {
    tag("IRETURN")
        .map(|_| AssemblyLine::ResolvedInstruction(cpu::Instruction::IReturn))
        .parse(input)
}

fn fixnum(input: &str) -> IResult<&str, Reference> {
    preceded(tag("#"), number)
        .map(|n| Reference::Resolved(cpu::LispWord::fixnum(n as u64).0 as i64))
        .parse(input)
}

fn charlit(input: &str) -> IResult<&str, Reference> {
    preceded(
        tag("#\\"),
        alt((
            value(cpu::LispWord::char('\n' as u64), tag("Newline")),
            anychar.map(|n| cpu::LispWord::char(n as u64)),
        )),
    )
    .map(|c| Reference::Resolved(c.0 as i64))
    .parse(input)
}

fn any_value(input: &str) -> IResult<&str, Reference> {
    alt((number.map(Reference::Resolved), reference, fixnum, charlit)).parse(input)
}

fn interrupt(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("INT").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, target) = any_value.parse(rest)?;
    match target {
        Reference::Resolved(v) => Ok((
            rest,
            AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(v as u64)),
        )),
        unresolved => Ok((
            rest,
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Int(unresolved)),
        )),
    }
}

fn register(input: &str) -> IResult<&str, cpu::Register> {
    preceded(tag("R"), digit1)
        .map_res(|n: &str| n.parse().map(cpu::Register))
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

fn machineregister(input: &str) -> IResult<&str, cpu::MachineRegister> {
    let normalreg =
        preceded(tag("A"), digit1).map_res(|n: &str| n.parse().map(cpu::MachineRegister));
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

fn jump(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("JUMP").parse(input)?;
    let (rest, condname) = opt(alt((tag("IFNOT"), tag("IF")))).parse(rest)?;
    let (rest, condition) = match condname {
        Some("IF") => {
            let (rest, _) = space1.parse(rest)?;
            let (rest, register) = register.parse(rest)?;
            let (rest, _) = tag(",").parse(rest)?;
            (rest, cpu::Condition::True(register))
        }
        Some("IFNOT") => {
            let (rest, _) = space1.parse(rest)?;
            let (rest, register) = register.parse(rest)?;
            let (rest, _) = tag(",").parse(rest)?;
            (rest, cpu::Condition::False(register))
        }
        None => (rest, cpu::Condition::Always),
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
            Some(resolved) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition,
                target: resolved,
            }),
            None => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: jumptarget,
            }),
        },
    ))
}
fn call(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("CALL").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, jumptarget) = jumptarget.parse(rest)?;
    Ok((
        rest,
        match jumptarget {
            JumpTarget::Absolute(Reference::Resolved(target)) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::Absolute(cpu::Address(target as u64)),
                })
            }
            JumpTarget::Machine(reg, Reference::Resolved(target)) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::Machine(reg, cpu::Offset(target)),
                })
            }
            JumpTarget::Register(reg) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::Register(reg),
                })
            }

            JumpTarget::IndirectMachine(mr, Reference::Resolved(target)) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::IndirectMachine(mr, cpu::Offset(target)),
                })
            }

            JumpTarget::IndirectRegister(reg) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::IndirectRegister(reg),
                })
            }
            unresolved => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: unresolved,
            }),
        },
    ))
}

fn pusha(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("PUSH").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    machineregister
        .map(|r| AssemblyLine::ResolvedInstruction(cpu::Instruction::PushA { src: r }))
        .parse(rest)
}
fn pushr(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("PUSH").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    register
        .map(|r| AssemblyLine::ResolvedInstruction(cpu::Instruction::PushR { src: r }))
        .parse(rest)
}
fn popa(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("POP").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    machineregister
        .map(|r| AssemblyLine::ResolvedInstruction(cpu::Instruction::PopA { dst: r }))
        .parse(rest)
}
fn popr(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("POP").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    register
        .map(|r| AssemblyLine::ResolvedInstruction(cpu::Instruction::PopR { dst: r }))
        .parse(rest)
}

fn location(input: &str) -> IResult<&str, Location> {
    alt((
        any_value.map(Location::Literal),
        register.map(Location::Register),
        machineregister.map(Location::Machine),
        delimited(tag("["), any_value, tag("]")).map(Location::Absolute),
        delimited(tag("["), register, tag("]")).map(Location::IndirectRegister),
        delimited(tag("["), machine_and_offset, tag("]"))
            .map(|(reg, refr)| Location::IndirectMachine(reg, refr)),
    ))
    .parse(input)
}

fn jumptarget(input: &str) -> IResult<&str, JumpTarget> {
    alt((
        any_value.map(JumpTarget::Absolute),
        register.map(JumpTarget::Register),
        machine_and_offset.map(|(reg, off)| JumpTarget::Machine(reg, off)),
        delimited(tag("["), register, tag("]")).map(JumpTarget::IndirectRegister),
        delimited(tag("["), machine_and_offset, tag("]"))
            .map(|(reg, refr)| JumpTarget::IndirectMachine(reg, refr)),
    ))
    .parse(input)
}

fn operand_uses_extra(location: &Location) -> bool {
    match location {
        Location::Absolute(_) => true,
        Location::IndirectMachine(_, _) => true,
        Location::IndirectRegister(_) => false,
        Location::Literal(_) => true,
        Location::Machine(_) => false,
        Location::Register(_) => false,
    }
}

fn mov(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("MOV").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, target) = location.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, source) = location.parse(rest)?;

    if operand_uses_extra(&source) && operand_uses_extra(&target) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Fail,
        )));
    }

    Ok((
        rest,
        match (target.try_resolve(), source.try_resolve()) {
            (Some(rtarget), Some(rsource)) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                    dst: rtarget,
                    src: rsource,
                })
            }
            _ => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov {
                dst: target,
                src: source,
            }),
        },
    ))
}

fn mov8(input: &str) -> IResult<&str, AssemblyLine> {
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
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                    dst: rtarget,
                    src: rsource,
                })
            }
            _ => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov8 {
                dst: target,
                src: source,
            }),
        },
    ))
}

fn threemachs(input: &str) -> IResult<&str, MachSource> {
    let (rest, op1) = machineregister.parse(input)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, op2) = opt(machineregister).parse(rest)?;

    let (rest, op3) = if op2.is_some() {
        opt(preceded((space0, tag("+"), space0), any_value)).parse(rest)?
    } else {
        opt(any_value).parse(rest)?
    };

    Ok((rest, MachSource { op1, op2, op3 }))
}

fn mbin(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, op) = alt((
        tag("ADD").map(|_| cpu::MBinaryOp::Add),
        tag("SUB").map(|_| cpu::MBinaryOp::Sub),
    ))
    .parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, threemachs) = threemachs.parse(rest)?;

    Ok((
        rest,
        match threemachs {
            MachSource {
                op1,
                op2,
                op3: None,
            } => AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op,
                dst,
                operands: cpu::MachSource {
                    op1,
                    op2,
                    op3: None,
                },
            }),
            MachSource {
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            } => AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op,
                dst,
                operands: cpu::MachSource {
                    op1,
                    op2,
                    op3: Some(cpu::Native(r as u64)),
                },
            }),
            unresolved => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MBinary {
                op,
                dst,
                operands: unresolved,
            }),
        },
    ))
}

fn settag(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("SETTAG").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::SetTag { dst, src }),
    ))
}
fn setpayload(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("SETPAYLOAD").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::SetPayload { dst, src }),
    ))
}
fn gettag(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("GETTAG").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::GetTag { dst, src }),
    ))
}
fn getpayload(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("GETPAYLOAD").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = machineregister.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::GetPayload { dst, src }),
    ))
}

fn cons(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("CONS").parse(input)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(0x03)),
    ))
}
fn uncons(input: &str) -> IResult<&str, AssemblyLine> {
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
        AssemblyLine::ResolvedInstruction(cpu::Instruction::Uncons { car, cdr, src }),
    ))
}

fn tworegs(input: &str) -> IResult<&str, cpu::TwoRegs> {
    let (rest, dst) = register.parse(input)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = register.parse(rest)?;
    Ok((rest, cpu::TwoRegs { dst, src }))
}

fn car(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("CAR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::Car(tworegs)),
    ))
}

fn cdr(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("CDR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::Cdr(tworegs)),
    ))
}

fn setcar(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("SETCAR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCar(tworegs)),
    ))
}

fn setcdr(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("SETCDR").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, tworegs) = tworegs.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCdr(tworegs)),
    ))
}
fn threeregs(input: &str) -> IResult<&str, RegSource> {
    let (rest, op1) = register.parse(input)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, op2) = opt(register).parse(rest)?;

    let (rest, op3) = if op2.is_some() {
        opt(preceded((space0, tag("+"), space0), any_value)).parse(rest)?
    } else {
        opt(any_value).parse(rest)?
    };

    Ok((rest, RegSource { op1, op2, op3 }))
}

fn either_source(input: &str) -> IResult<&str, EitherSource> {
    alt((
        threemachs.map(EitherSource::Mach),
        threeregs.map(EitherSource::Reg),
    ))
    .parse(input)
}

fn comparison(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, op) = alt((
        value(cpu::Comparison::Eq, tag("EQ")),
        value(cpu::Comparison::Ne, tag("NE")),
        value(cpu::Comparison::Gte, tag("GTE")),
        value(cpu::Comparison::Gt, tag("GT")),
        value(cpu::Comparison::Lte, tag("LTE")),
        value(cpu::Comparison::Lt, tag("LT")),
    ))
    .parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, either_source) = either_source.parse(rest)?;

    Ok((
        rest,
        match either_source {
            EitherSource::Mach(MachSource {
                op1,
                op2,
                op3: None,
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op,
                dst,
                operands: cpu::EitherSource::Mach(cpu::MachSource {
                    op1,
                    op2,
                    op3: None,
                }),
            }),
            EitherSource::Mach(MachSource {
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op,
                dst,
                operands: cpu::EitherSource::Mach(cpu::MachSource {
                    op1,
                    op2,
                    op3: Some(cpu::Native(r as u64)),
                }),
            }),
            EitherSource::Reg(RegSource {
                op1,
                op2,
                op3: None,
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op,
                dst,
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1,
                    op2,
                    op3: None,
                }),
            }),
            EitherSource::Reg(RegSource {
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op,
                dst,
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1,
                    op2,
                    op3: Some(cpu::LispWord(r as u64)),
                }),
            }),
            unresolved => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                op,
                dst,
                operands: unresolved,
            }),
        },
    ))
}

fn bin(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, op) = alt((
        value(cpu::BinaryOp::Add, tag("ADD")),
        value(cpu::BinaryOp::Sub, tag("SUB")),
        value(cpu::BinaryOp::Mul, tag("MUL")),
    ))
    .parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, threeregs) = threeregs.parse(rest)?;

    Ok((
        rest,
        match threeregs {
            RegSource {
                op1,
                op2,
                op3: None,
            } => AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op,
                dst,
                operands: cpu::RegSource {
                    op1,
                    op2,
                    op3: None,
                },
            }),
            RegSource {
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            } => AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op,
                dst,
                operands: cpu::RegSource {
                    op1,
                    op2,
                    op3: Some(cpu::LispWord(r as u64)),
                },
            }),
            unresolved => AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Binary {
                op,
                dst,
                operands: unresolved,
            }),
        },
    ))
}

fn div(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("DIV").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, div) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, rem) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, operands) = threeregs.parse(rest)?;
    Ok((
        rest,
        match operands {
            RegSource {
                op1,
                op2,
                op3: None,
            } => AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                div,
                rem,
                operands: {
                    cpu::RegSource {
                        op1,
                        op2,
                        op3: None,
                    }
                },
            }),
            RegSource {
                op1,
                op2,
                op3: Some(Reference::Resolved(r)),
            } => AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                div,
                rem,
                operands: cpu::RegSource {
                    op1,
                    op2,
                    op3: Some(cpu::LispWord(r as u64)),
                },
            }),
            RegSource { op1, op2, op3 } => {
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::IDiv {
                    div,
                    rem,
                    op1,
                    op2,
                    op3,
                })
            }
        },
    ))
}
fn makeclosure(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("MAKECLOSURE").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, dst) = register.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, _) = space0.parse(rest)?;
    let (rest, src) = machineregister.parse(rest)?;
    Ok((
        rest,
        AssemblyLine::ResolvedInstruction(cpu::Instruction::MakeClosure { code: src, dst }),
    ))
}

fn number(input: &str) -> IResult<&str, i64> {
    alt((hex, decimal)).parse(input)
}

fn typep(input: &str) -> IResult<&str, AssemblyLine> {
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
        AssemblyLine::ResolvedInstruction(cpu::Instruction::Typep {
            dst,
            src,
            compare: cpu::Native(compare as u64),
        }),
    ))
}

fn memcpy(input: &str) -> IResult<&str, AssemblyLine> {
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
        AssemblyLine::ResolvedInstruction(cpu::Instruction::MemCpy { dst, src, count }),
    ))
}

fn instruction(input: &str) -> IResult<&str, AssemblyLine> {
    alt((
        alt((
            nop, halt, return_op, ireturn_op, interrupt, jump, call, pusha, popa, pushr, popr, mov,
            mov8, mbin, settag, gettag, setpayload, getpayload, cons, uncons, car,
        )),
        alt((
            cdr,
            setcar,
            setcdr,
            bin,
            div,
            makeclosure,
            typep,
            memcpy,
            comparison,
        )),
    ))
    .parse(input)
}

fn decimal(input: &str) -> IResult<&str, i64> {
    let (rest, value) = digit1.parse(input)?;
    Ok((rest, value.parse::<i64>().unwrap()))
}

fn hex(input: &str) -> IResult<&str, i64> {
    preceded(tag("0x"), recognize(hex_digit1))
        .map_res(|str| i64::from_str_radix(str, 16))
        .parse(input)
}

fn ord(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("ord").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) = number.parse(rest)?;
    Ok((rest, AssemblyLine::Ord(pos as usize)))
}

fn symbol(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("symbol").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) = any_value.parse(rest)?;
    Ok((
        rest,
        match pos {
            Reference::Resolved(r) => {
                AssemblyLine::ResolvedData(cpu::LispWord::symbol(r as u64).0.to_le_bytes().to_vec())
            }
            unresolved => AssemblyLine::UnresolvedData(Data::Symbol(unresolved)),
        },
    ))
}

fn w(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("w").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) = any_value.parse(rest)?;
    Ok((
        rest,
        match pos {
            Reference::Resolved(r) => AssemblyLine::ResolvedData(r.to_le_bytes().to_vec()),
            unresolved => AssemblyLine::UnresolvedData(Data::Literal(unresolved)),
        },
    ))
}

fn string(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("str").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, pos) =
        delimited(tag("\""), recognize(many0(none_of("\""))), tag("\"")).parse(rest)?;
    let lenblock = cpu::LispWord::fixnum(pos.len() as u64);

    let mut encoded = lenblock.0.to_le_bytes().to_vec();
    encoded.extend(pos.as_bytes().to_vec());
    Ok((rest, AssemblyLine::ResolvedData(encoded)))
}

fn consord(input: &str) -> IResult<&str, AssemblyLine> {
    let (rest, _) = tag("cons").parse(input)?;
    let (rest, _) = space1.parse(rest)?;
    let (rest, car) = any_value.parse(rest)?;
    let (rest, _) = tag(",").parse(rest)?;
    let (rest, cdr) = any_value.parse(rest)?;

    Ok((rest, AssemblyLine::UnresolvedData(Data::Cons(car, cdr))))
}

fn directive(input: &str) -> IResult<&str, AssemblyLine> {
    preceded(tag("."), alt((ord, symbol, string, w, consord))).parse(input)
}

fn label(input: &str) -> IResult<&str, AssemblyLine> {
    terminated(ident, tag(":"))
        .map(|name| AssemblyLine::Label(name.to_owned()))
        .parse(input)
}

fn comment(input: &str) -> IResult<&str, ()> {
    preceded(tag(";"), many0(none_of("\n")))
        .map(|_| ())
        .parse(input)
}

fn asm_line(input: &str) -> IResult<&str, Vec<AssemblyLine>> {
    let mut tokens = Vec::<AssemblyLine>::new();
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

pub fn asm_lines(input: &str) -> IResult<&str, Vec<AssemblyLine>> {
    let (rest, lines) = many0(asm_line).parse(input)?;
    let (rest, _) = space0.parse(rest)?;
    Ok((rest, lines.concat()))
}


enum ParserError {}

// pub fn parser_new(
//     tokens: &Vec<tokenizer::AssemblyToken>,
// ) -> Result<Vec<AssemblyLine>, ParserError> {
//     let (rest, tokens) = many0(token)
//         .parse(input)
//         .map_err(|_| TokenizeError::UnexpectedInput { remaining: input })?;

//     if !rest.is_empty() {
//         return Err(TokenizeError::UnexpectedInput { remaining: rest });
//     }
//     Ok(tokens)
// }
