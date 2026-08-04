mod parser;
mod tokenizer;

use std::collections::HashMap;

use crate::{
    bus,
    cpu::{
        self, LispWord,
        assembler::{
            parser::*,
            tokenizer::{TokenizeError, tokenize},
        },
    },
};

#[derive(Debug)]
pub enum AssemblerError<'a> {
    TokenizerError(TokenizeError<'a>),
    ParserError(ParserError<'a>),
    UnresolvedReference(&'a str),
    InvalidState,
}

impl<'a> From<TokenizeError<'a>> for AssemblerError<'a> {
    fn from(value: TokenizeError<'a>) -> Self {
        AssemblerError::TokenizerError(value)
    }
}

impl<'a> From<ParserError<'a>> for AssemblerError<'a> {
    fn from(value: ParserError<'a>) -> Self {
        AssemblerError::ParserError(value)
    }
}

#[derive(Debug)]
pub struct Section {
    pub data: Vec<u8>,
    pub base: usize,
}

pub fn assemble<'a>(input: &'a str) -> Result<Vec<Section>, AssemblerError<'a>> {
    let tokens = tokenize(input)?;
    let ast = parser_new(&tokens)?;
    let mut sections = layout(ast)?;
    resolve(&mut sections)?;
    let source = emit(&sections)?;
    Ok(source)
}

struct LayoutSection<'a> {
    base: usize,
    size: usize,
    lines: Vec<AssemblyLine<'a>>,
}

struct Layout<'a> {
    symbols: HashMap<&'a str, usize>,
    sections: Vec<LayoutSection<'a>>,
}

fn layout<'a>(lines: Vec<AssemblyLine<'a>>) -> Result<Layout<'a>, AssemblerError<'a>> {
    let mut position = 0_usize;
    let mut symbols = HashMap::<&'a str, usize>::new();
    let mut sections: Vec<LayoutSection> = vec![];

    let mut current_section_lines: Option<Vec<AssemblyLine<'a>>> = None;
    let mut current_section_base = 0_usize;

    for line in lines {
        let should_push = match &line {
            AssemblyLine::ResolvedData(d) => {
                position = (position + d.len()).next_multiple_of(8);
                true
            }
            AssemblyLine::UnresolvedData(_) => {
                position += 8;
                true
            }
            AssemblyLine::ResolvedInstruction(_) => {
                position = position.next_multiple_of(16) + 16;
                true
            }
            AssemblyLine::UnresolvedInstruction(_) => {
                position = position.next_multiple_of(16) + 16;
                true
            }
            AssemblyLine::Label(label) => {
                symbols.insert(label, position);
                false
            }
            AssemblyLine::Org(p) => {
                if let Some(s) = current_section_lines.take() {
                    let section = LayoutSection {
                        lines: s,
                        base: current_section_base,
                        size: (position - current_section_base).next_multiple_of(8),
                    };
                    sections.push(section)
                }
                position = *p;
                current_section_base = position;
                current_section_lines = Some(vec![]);
                false
            }
        };
        if should_push {
            match current_section_lines {
                Some(ref mut l) => l.push(line),
                None => return Err(AssemblerError::InvalidState),
            }
        }
    }

    if let Some(s) = current_section_lines.take() {
        let section = LayoutSection {
            lines: s,
            base: current_section_base,
            size: position - current_section_base,
        };
        sections.push(section)
    }
    Ok(Layout { symbols, sections })
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
        Reference::Resolved(r) => Ok(*r as usize),
        Reference::Unresolved(r) => Ok(*labels
            .get(r)
            .ok_or(AssemblerError::UnresolvedReference(r))?
            as usize),
    }
}

pub fn resolve_opt_data<'a>(
    data: Option<Native<'a>>,
    labels: &HashMap<&'a str, usize>,
) -> Result<Option<u64>, AssemblerError<'a>> {
    data.as_ref().map(|d| resolve_data(&d, labels)).transpose()
}
pub fn resolve_data<'a>(
    data: &Native<'a>,
    symbols: &HashMap<&'a str, usize>,
) -> Result<u64, AssemblerError<'a>> {
    Ok(match data {
        Native::Char(refr) => cpu::LispWord::char(resolve_reference(refr, symbols)? as u64).0,
        Native::Symbol(refr) => cpu::LispWord::symbol(resolve_reference(refr, symbols)? as u64).0,
        Native::Raw(refr) => resolve_reference(refr, symbols)? as u64,
        Native::Cons(refr) => cpu::LispWord::cons(resolve_reference(refr, symbols)? as u64).0,
        Native::Fixnum(refr) => cpu::LispWord::fixnum(resolve_reference(refr, symbols)? as u64).0,
    })
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
            cpu::Location::IndirectMachine(*m, bus::Offset(resolve_reference(r, labels)? as i64))
        }
        Location::IndirectRegister(r) => cpu::Location::IndirectRegister(*r),
        Location::Literal(Native::Cons(r)) => cpu::Location::Literal(cpu::Native(
            LispWord::cons(resolve_reference(r, labels)? as u64).0,
        )),
        Location::Literal(Native::Symbol(r)) => cpu::Location::Literal(cpu::Native(
            LispWord::symbol(resolve_reference(r, labels)? as u64).0,
        )),
        Location::Literal(Native::Fixnum(r)) => cpu::Location::Literal(cpu::Native(
            LispWord::fixnum(resolve_reference(r, labels)? as u64).0,
        )),
        Location::Literal(Native::Raw(r)) => {
            cpu::Location::Literal(cpu::Native(resolve_reference(r, labels)? as u64))
        }
        Location::Literal(Native::Char(r)) => cpu::Location::Literal(cpu::Native(
            LispWord::char(resolve_reference(r, labels)? as u64).0,
        )),
        Location::Machine(m) => cpu::Location::Machine(*m),
        Location::Register(r) => cpu::Location::Register(*r),
    })
}

fn resolve<'a>(layout: &mut Layout<'a>) -> Result<(), AssemblerError<'a>> {
    let symbols = &layout.symbols;
    for section in &mut layout.sections {
        for line in &mut section.lines {
            match line {
                AssemblyLine::Org(o) => *line = AssemblyLine::Org(*o),
                AssemblyLine::Label(l) => *line = AssemblyLine::Label(l),
                AssemblyLine::UnresolvedData(d) => {
                    *line =
                        AssemblyLine::ResolvedData(resolve_data(d, symbols)?.to_le_bytes().to_vec())
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
                            op3: resolve_opt_data(op3.clone(), symbols)?
                                .map(|w| cpu::LispWord(w as u64)),
                        },
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                    target: JumpTarget::Absolute(refr),
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                        target: cpu::JumpTarget::Absolute(cpu::Address(resolve_reference(
                            refr, symbols,
                        )?
                            as u64)),
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
                    operands: RegSource { op1, op2, op3 },
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                        div: *div,
                        rem: *rem,
                        operands: cpu::RegSource {
                            op1: *op1,
                            op2: *op2,
                            op3: resolve_opt_data(op3.clone(), symbols)?
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
                        target: cpu::JumpTarget::Absolute(cpu::Address(resolve_reference(
                            refr, symbols,
                        )?
                            as u64)),
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
                            op3: resolve_opt_data(op3.clone(), symbols)?
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

                AssemblyLine::ResolvedInstruction(_) => {}
                AssemblyLine::ResolvedData(_) => {}
            }
        }
    }
    Ok(())
}

fn emit<'a>(layout: &Layout<'a>) -> Result<Vec<Section>, AssemblerError<'a>> {
    let mut result_sections = vec![];
    for section in &layout.sections {
        let mut position: usize = 0;
        let mut result: Vec<u8> = Vec::new();
        result.resize(section.size, 0);

        for line in &section.lines {
            match line {
                // These were elided on layout
                AssemblyLine::Label(_) => {}
                AssemblyLine::Org(_) => {}
                AssemblyLine::ResolvedData(d) => {
                    result[position..][..d.len()].copy_from_slice(d);
                    position += d.len();
                    position = position.next_multiple_of(8);
                }
                AssemblyLine::ResolvedInstruction(i) => {
                    position = position.next_multiple_of(16);
                    let (lo, hi) = i.encode();
                    result[position..][..8].copy_from_slice(&lo.to_le_bytes());
                    position += 8;
                    result[position..][..8].copy_from_slice(&hi.to_le_bytes());
                    position += 8;
                }
                AssemblyLine::UnresolvedData(_) => {
                    return Err(AssemblerError::UnresolvedReference(""));
                }
                AssemblyLine::UnresolvedInstruction(_) => {
                    return Err(AssemblerError::UnresolvedReference(""));
                }
            }
        }

        result_sections.push(Section {
            base: section.base,
            data: result,
        })
    }

    Ok(result_sections)
}

pub mod test {
    use crate::cpu::{assembler::tokenizer, *};
    pub const SOURCE: &str = r#"
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
            TYPEP R1, R1, 0x03
        "#;

    pub fn expected_tokens<'a>() -> Vec<tokenizer::AssemblyToken<'a>> {
        vec![
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("HALT"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("NOP"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; A comment"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Two comments in a row"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("RETURN"),
            tokenizer::AssemblyToken::Comment("; And an inline one"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Label("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("INT"),
            tokenizer::AssemblyToken::Number(42),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("INT"),
            tokenizer::AssemblyToken::Reference("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("IRETURN"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Jump variants"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMP"),
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMP"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMP"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMP"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMP"),
            tokenizer::AssemblyToken::Reference("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Reference("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Reference("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CALL"),
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CALL"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CALL"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CALL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CALL"),
            tokenizer::AssemblyToken::Reference("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Stack variants"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("PUSH"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("POP"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("PUSH"),
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("POP"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Loading & Memory"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(1234),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(2147483647),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(7)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(64),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(24),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV8"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV8"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV8"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Machine Arithmetic"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("ADD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("ADD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(4),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("ADD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SUB"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SUB"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(2),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SUB"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(12),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Tag Operations"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SETTAG"),
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GETTAG"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SETPAYLOAD"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GETPAYLOAD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Lisp Destructuring Primitives"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CONS"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("UNCONS"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CAR"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("CDR"),
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(7)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SETCAR"),
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SETCDR"),
            tokenizer::AssemblyToken::Register(Register(10)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(11)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment(
                "; Arithmetic (assembler::ThreeRegs layouts using Option types, Word Immediates)",
            ),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("ADD"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("ADD"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("ADD"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SUB"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SUB"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("SUB"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MUL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MUL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MUL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("EQ"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("EQ"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("EQ"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("NE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("NE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("NE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("LT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("LT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("LT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("LTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("LTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("LTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("GTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("DIV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("DIV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("DIV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::LispLiteral,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Advanced Operations"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MAKECLOSURE"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("MEMCPY"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(1),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Instruction("TYPEP"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(3),
            tokenizer::AssemblyToken::Newline,
        ]
    }
}
