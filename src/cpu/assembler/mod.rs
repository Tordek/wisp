mod parser;
mod tokenizer;

use std::collections::HashMap;

use crate::{
    cpu::{
        self,
        assembler::{
            parser::*,
            tokenizer::{TokenizeError, tokenize},
        },
    },
    memory,
};

#[derive(Debug)]
pub enum AssemblerError<'a> {
    TokenizerError(TokenizeError<'a>),
    ParserError(ParserError<'a>),
    UnresolvedReference(&'a str),
    InvalidState,
}

pub fn assemble<'a>(input: &'a str) -> Result<Vec<u8>, AssemblerError<'a>> {
    let tokens = tokenize(input).map_err(AssemblerError::TokenizerError)?;
    let mut ast = parser_new(&tokens).map_err(AssemblerError::ParserError)?;
    let symbols = layout(&ast)?;
    resolve(&mut ast, &symbols)?;
    let source = encode(&ast)?;
    Ok(source)
}

fn layout<'a>(lines: &[AssemblyLine<'a>]) -> Result<HashMap<&'a str, usize>, AssemblerError<'a>> {
    let mut position: usize = 0;
    let mut labels = HashMap::<&'a str, usize>::new();
    for line in lines {
        match line {
            AssemblyLine::ResolvedData(d) => position = position.next_multiple_of(8) + d.len(),
            AssemblyLine::UnresolvedData(Data::Symbol(_)) => position += 8,
            AssemblyLine::UnresolvedData(Data::Literal(_)) => position += 8,
            AssemblyLine::UnresolvedData(Data::Cons(..)) => position += 16,
            AssemblyLine::ResolvedInstruction(_) => {
                position = position.next_multiple_of(16) + 16;
            }
            AssemblyLine::UnresolvedInstruction(_) => {
                position = position.next_multiple_of(16) + 16;
            }
            AssemblyLine::Label(label) => {
                // Maybe should look at next line to decide alignment?
                labels.insert(label, position.next_multiple_of(8));
            }
            AssemblyLine::Ord(p) => position = *p,
        }
    }
    Ok(labels)
}

fn encode<'a>(lines: &[AssemblyLine<'a>]) -> Result<Vec<u8>, AssemblerError<'a>> {
    let mut result: Vec<u8> = vec![];
    let mut position: usize = 0;

    for line in lines {
        match line {
            AssemblyLine::Label(_) => {}
            AssemblyLine::Ord(p) => position = *p,
            AssemblyLine::ResolvedData(d) => {
                position = position.next_multiple_of(8);
                result.resize((result.len().max(position)) + d.len(), 0);
                result[position..][..d.len()].copy_from_slice(d);
                position += d.len()
            }
            AssemblyLine::ResolvedInstruction(i) => {
                position = position.next_multiple_of(16);
                let (lo, hi) = i.encode();
                result.resize((result.len().max(position)) + 16, 0);
                result[position..][..8].copy_from_slice(&lo.to_le_bytes());
                position += 8;
                result[position..][..8].copy_from_slice(&hi.to_le_bytes());
                position += 8;
            }
            AssemblyLine::UnresolvedData(d) => return Err(AssemblerError::UnresolvedReference("")), // TODO
            AssemblyLine::UnresolvedInstruction(d) => {
                return Err(AssemblerError::UnresolvedReference(""));
            } // TODO
        }
    }

    Ok(result)
}

pub fn resolve_opt_reference<'a>(
    reference: Option<Reference<'a>>,
    labels: &HashMap<&'a str, usize>,
) -> Result<Option<usize>, AssemblerError<'a>> {
    reference
        .as_ref()
        .map(|r| resolve_reference(&r, labels))
        .transpose()
}

pub fn resolve_reference<'a>(
    reference: &Reference<'a>,
    labels: &HashMap<&'a str, usize>,
) -> Result<usize, AssemblerError<'a>> {
    match reference {
        Reference::Resolved(_) => todo!("This shouldn't happen"),
        Reference::UnresolvedNeg(r) => Ok(-(*labels
            .get(r)
            .ok_or(AssemblerError::UnresolvedReference(r))?
            as isize) as usize),
        Reference::UnresolvedPos(r) => Ok(*labels
            .get(r)
            .ok_or(AssemblerError::UnresolvedReference(r))?
            as usize),
    }
}

pub fn resolve_location<'a>(
    location: &Location<'a>,
    labels: &HashMap<&'a str, usize>,
) -> Result<cpu::Location, AssemblerError<'a>> {
    Ok(match location {
        Location::Absolute(r) => {
            cpu::Location::Absolute(cpu::Address(resolve_reference(r, labels)? as u64))
        }
        Location::IndirectMachine(m, r) => {
            cpu::Location::IndirectMachine(*m, memory::Offset(resolve_reference(r, labels)? as i64))
        }
        Location::IndirectRegister(r) => cpu::Location::IndirectRegister(*r),
        Location::Literal(r) => {
            cpu::Location::Literal(cpu::Native(resolve_reference(r, labels)? as u64))
        }
        Location::Machine(m) => cpu::Location::Machine(*m),
        Location::Register(r) => cpu::Location::Register(*r),
    })
}

fn resolve<'a>(
    lines: &mut [AssemblyLine<'a>],
    symbols: &HashMap<&'a str, usize>,
) -> Result<(), AssemblerError<'a>> {
    for line in lines {
        match line {
            AssemblyLine::Ord(o) => *line = AssemblyLine::Ord(*o),
            AssemblyLine::Label(l) => *line = AssemblyLine::Label(l),
            AssemblyLine::UnresolvedData(Data::Cons(car, cdr)) => {
                let mut vec = resolve_reference(car, symbols)?.to_le_bytes().to_vec();
                vec.extend(resolve_reference(cdr, symbols)?.to_be_bytes());

                *line = AssemblyLine::ResolvedData(vec)
            }
            AssemblyLine::UnresolvedData(Data::Symbol(refr)) => {
                *line = AssemblyLine::ResolvedData(
                    cpu::LispWord::symbol(resolve_reference(refr, symbols)? as u64)
                        .0
                        .to_le_bytes()
                        .to_vec(),
                )
            }
            AssemblyLine::UnresolvedData(Data::Literal(refr)) => {
                *line = AssemblyLine::ResolvedData(
                    (resolve_reference(refr, symbols)? as u64)
                        .to_le_bytes()
                        .to_vec(),
                )
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Binary {
                op,
                dst,
                operands: RegSource { op1, op2, op3 },
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                    op: *op,
                    dst: *dst,
                    operands: cpu::RegSource {
                        op1: *op1,
                        op2: *op2,
                        op3: resolve_opt_reference(op3.clone(), symbols)?
                            .map(|w| cpu::LispWord(w as u64)),
                    },
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Absolute(refr),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::Absolute(cpu::Address(
                        resolve_reference(refr, symbols)? as u64,
                    )),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Machine(m, refr),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::Machine(
                        *m,
                        cpu::Offset(resolve_reference(refr, symbols)? as i64),
                    ),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::IndirectMachine(m, refr),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                    target: cpu::JumpTarget::IndirectMachine(
                        *m,
                        cpu::Offset(resolve_reference(refr, symbols)? as i64),
                    ),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Register(_),
            }) => return Err(AssemblerError::InvalidState),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::IndirectRegister(_),
            }) => return Err(AssemblerError::InvalidState),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::IDiv {
                div,
                rem,
                op1,
                op2,
                op3,
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                    div: *div,
                    rem: *rem,
                    operands: cpu::RegSource {
                        op1: *op1,
                        op2: *op2,
                        op3: resolve_opt_reference(op3.clone(), symbols)?
                            .map(|v| cpu::LispWord(v as u64)),
                    },
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Int(i)) => {
                *line = {
                    AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(resolve_reference(
                        i, symbols,
                    )?
                        as u64))
                }
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::Absolute(refr),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                    condition: *condition,
                    target: cpu::JumpTarget::Absolute(cpu::Address(
                        resolve_reference(refr, symbols)? as u64,
                    )),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::Machine(m, refr),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                    condition: *condition,
                    target: cpu::JumpTarget::Machine(
                        *m,
                        cpu::Offset(resolve_reference(refr, symbols)? as i64),
                    ),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::IndirectMachine(m, refr),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                    condition: *condition,
                    target: cpu::JumpTarget::IndirectMachine(
                        *m,
                        cpu::Offset(resolve_reference(refr, symbols)? as i64),
                    ),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition: _,
                target: JumpTarget::Register(_),
            }) => return Err(AssemblerError::InvalidState),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition: _,
                target: JumpTarget::IndirectRegister(_),
            }) => return Err(AssemblerError::InvalidState),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MBinary {
                op,
                dst,
                operands: MachSource { op1, op2, op3 },
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                    op: *op,
                    dst: *dst,
                    operands: cpu::MachSource {
                        op1: *op1,
                        op2: *op2,
                        op3: resolve_opt_reference(op3.clone(), symbols)?
                            .map(|v| cpu::Native(v as u64)),
                    },
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                op,
                dst,
                operands: EitherSource::Reg(RegSource { op1, op2, op3 }),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                    op: *op,
                    dst: *dst,
                    operands: cpu::EitherSource::Reg(cpu::RegSource {
                        op1: *op1,
                        op2: *op2,
                        op3: resolve_opt_reference(op3.clone(), symbols)?
                            .map(|v| cpu::LispWord(v as u64)),
                    }),
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                op,
                dst,
                operands: EitherSource::Mach(MachSource { op1, op2, op3 }),
            }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                    op: *op,
                    dst: *dst,
                    operands: cpu::EitherSource::Mach(cpu::MachSource {
                        op1: *op1,
                        op2: *op2,
                        op3: resolve_opt_reference(op3.clone(), symbols)?
                            .map(|v| cpu::Native(v as u64)),
                    }),
                })
            }
            // AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MemCpy {
            //     dst,
            //     src,
            //     count,
            // }) => *line = AssemblyToken::ResolvedInstruction(cpu::Instruction::MemCpy {
            //     dst: *dst,
            //     src: *src,
            //     count: cpu::Count(resolve_reference(&count, labels)? as u64),
            // }),
            // AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MemSet {
            //     dst,
            //     src,
            //     count,
            // }) => *line = AssemblyToken::ResolvedInstruction(cpu::Instruction::MemSet {
            //     dst: *dst,
            //     src: *src,
            //     count: cpu::Count(resolve_reference(count, labels)? as u64),
            // }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov { dst, src }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                    dst: resolve_location(dst, symbols)?,
                    src: resolve_location(src, symbols)?,
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov8 { dst, src }) => {
                *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                    dst: resolve_location(dst, symbols)?,
                    src: resolve_location(src, symbols)?,
                })
            }

            AssemblyLine::ResolvedInstruction(r) => *line = AssemblyLine::ResolvedInstruction(*r),
            AssemblyLine::ResolvedData(d) => *line = AssemblyLine::ResolvedData(d.clone()),
        }
    }
    Ok(())
}
