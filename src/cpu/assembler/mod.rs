pub mod parser;
pub mod tokenizer;

use std::collections::HashMap;

use crate::{
    bus,
    cpu::{
        self,
        assembler::{
            parser::*,
            tokenizer::{TokenizeError, tokenize},
        },
    },
};

#[allow(dead_code)]
#[derive(Debug)]
pub enum AssemblerError<'a> {
    InvalidInstruction,
    TokenizerError(TokenizeError<'a>),
    ParserError(ParserError<'a>),
    UnresolvedReference(&'a str),
    IncompleteResolution,
    InvalidState,
    OverlappingSection(usize),
    DuplicateLabel(&'a str),
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

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub data: Vec<u8>,
    pub base: usize,
}

pub fn assemble<'a>(input: &'a str) -> Result<Vec<Section>, AssemblerError<'a>> {
    let tokens = tokenize(input)?;
    let ast = parse(&tokens)?;
    let mut sections = layout(ast)?;
    resolve(&mut sections)?;
    let source = emit(&sections)?;
    Ok(source)
}

pub struct LayoutSection<'a> {
    pub base: usize,
    pub size: usize,
    pub lines: Vec<AssemblyLine<'a>>,
}

pub struct Layout<'a> {
    pub symbols: HashMap<&'a str, usize>,
    pub sections: Vec<LayoutSection<'a>>,
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
                if symbols.contains_key(label) {
                    return Err(AssemblerError::DuplicateLabel(label));
                }
                symbols.insert(label, position);
                false
            }
            AssemblyLine::Equ(label, value) => {
                if symbols.contains_key(label) {
                    return Err(AssemblerError::DuplicateLabel(label));
                }
                symbols.insert(label, *value);
                false
            }
            AssemblyLine::Org(p) => {
                if let Some(s) = current_section_lines.take() {
                    let size = (position - current_section_base).next_multiple_of(8);
                    for section in &sections {
                        if !(current_section_base > (section.base + section.size)
                            || position < section.base)
                        {
                            return Err(AssemblerError::OverlappingSection(current_section_base));
                        }
                    }
                    let section = LayoutSection {
                        lines: s,
                        base: current_section_base,
                        size,
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
        let size = (position - current_section_base).next_multiple_of(8);
        for section in &sections {
            if !(current_section_base > (section.base + section.size) || position < section.base) {
                return Err(AssemblerError::OverlappingSection(current_section_base));
            }
        }
        let section = LayoutSection {
            lines: s,
            base: current_section_base,
            size,
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
        .map(|r| resolve_reference(r, labels))
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
            .ok_or(AssemblerError::UnresolvedReference(r))?),
    }
}

pub fn resolve_opt_data<'a>(
    data: Option<Native<'a>>,
    labels: &HashMap<&'a str, usize>,
) -> Result<Option<u64>, AssemblerError<'a>> {
    data.as_ref().map(|d| resolve_data(d, labels)).transpose()
}
pub fn resolve_data<'a>(
    data: &Native<'a>,
    symbols: &HashMap<&'a str, usize>,
) -> Result<u64, AssemblerError<'a>> {
    Ok(match data {
        Native::Raw(refr) => resolve_reference(refr, symbols)? as u64,
        Native::Char(refr) => cpu::LispWord::char(resolve_reference(refr, symbols)? as u64).0,
        Native::Symbol(refr) => cpu::LispWord::symbol(resolve_reference(refr, symbols)? as u64).0,
        Native::Cons(refr) => cpu::LispWord::cons(resolve_reference(refr, symbols)? as u64).0,
        Native::Fixnum(refr) => cpu::LispWord::fixnum(resolve_reference(refr, symbols)? as u64).0,
        Native::String(refr) => cpu::LispWord::string(resolve_reference(refr, symbols)? as u64).0,
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
        Location::Literal(l) => cpu::Location::Literal(cpu::Native(resolve_data(l, labels)?)),
        Location::Machine(m) => cpu::Location::Machine(*m),
        Location::Register(r) => cpu::Location::Register(*r),
    })
}

fn resolve<'a>(layout: &mut Layout<'a>) -> Result<(), AssemblerError<'a>> {
    let symbols = &layout.symbols;
    for section in &mut layout.sections {
        for line in &mut section.lines {
            match line {
                AssemblyLine::Org(_) => {}
                AssemblyLine::Label(_) => {}
                AssemblyLine::Equ(_, _) => {}
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
                            op3: resolve_opt_data(op3.clone(), symbols)?.map(cpu::LispWord),
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
                    target: JumpTarget::Register(r),
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                        target: cpu::JumpTarget::Register(*r),
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                    target: JumpTarget::IndirectRegister(r),
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                        target: cpu::JumpTarget::IndirectRegister(*r),
                    })
                }
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
                            op3: resolve_opt_data(op3.clone(), symbols)?.map(cpu::LispWord),
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
                    condition: c,
                    target: JumpTarget::Register(r),
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                        condition: *c,
                        target: cpu::JumpTarget::Register(*r),
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                    condition: c,
                    target: JumpTarget::IndirectRegister(i),
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                        condition: *c,
                        target: cpu::JumpTarget::IndirectRegister(*i),
                    })
                }
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
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Comparison {
                        op: *op,
                        dst: *dst,
                        operands: cpu::EitherSource::Reg(cpu::RegSource {
                            op1: *op1,
                            op2: *op2,
                            op3: resolve_opt_data(op3.clone(), symbols)?.map(cpu::LispWord),
                        }),
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                    op,
                    dst,
                    operands: EitherSource::Mach(MachSource { op1, op2, op3 }),
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Comparison {
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
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Typep {
                    dst,
                    src,
                    compare,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Typep {
                        dst: *dst,
                        src: *src,
                        compare: resolve_reference(compare, symbols)
                            .map(|c| cpu::Native(c as u64))?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MemCpy {
                    dst,
                    src,
                    count: RegAndOffset { op1, op2 },
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::MemCpy {
                        dst: *dst,
                        src: *src,
                        count: cpu::RegAndOff {
                            op1: *op1,
                            off: resolve_opt_data(op2.clone(), symbols)?.map(cpu::LispWord),
                        },
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
        let mut result: Vec<u8> = vec![0; section.size.next_multiple_of(8) + 8];
        // Padding to an extra word.

        for line in &section.lines {
            match line {
                // These were elided on layout
                AssemblyLine::Label(_) => {}
                AssemblyLine::Org(_) => {}
                AssemblyLine::Equ(_, _) => {}
                AssemblyLine::ResolvedData(d) => {
                    result[position..][..d.len()].copy_from_slice(d);
                    position += d.len();
                    position = position.next_multiple_of(8);
                }
                AssemblyLine::ResolvedInstruction(i) => {
                    position = position.next_multiple_of(16);
                    let (lo, hi) = i.encode().map_err(|_| AssemblerError::InvalidInstruction)?;
                    result[position..][..8].copy_from_slice(&lo.to_le_bytes());
                    position += 8;
                    result[position..][..8].copy_from_slice(&hi.to_le_bytes());
                    position += 8;
                }
                AssemblyLine::UnresolvedData(_) => {
                    return Err(AssemblerError::IncompleteResolution);
                }
                AssemblyLine::UnresolvedInstruction(_) => {
                    return Err(AssemblerError::IncompleteResolution);
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

#[cfg(test)]
pub mod test {
    use crate::cpu::{
        self,
        assembler::{
            parser::{self},
            tokenizer,
        },
        *,
    };
    pub const SOURCE: &str = r#"
        .org 128
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
            CONS R1, R2, R3
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
            GTE R0, A1, A2
        "#;

    pub fn expected_tokens<'a>() -> Vec<tokenizer::AssemblyToken<'a>> {
        vec![
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Dot,
            tokenizer::AssemblyToken::Identifier("org"),
            tokenizer::AssemblyToken::Number(128),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("HALT"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("NOP"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; A comment"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Two comments in a row"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("RETURN"),
            tokenizer::AssemblyToken::Comment("; And an inline one"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("loop"),
            tokenizer::AssemblyToken::Colon,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("INT"),
            tokenizer::AssemblyToken::Number(42),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("INT"),
            tokenizer::AssemblyToken::Quote,
            tokenizer::AssemblyToken::Identifier("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("IRETURN"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Jump variants"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMP"),
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMP"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMP"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMP"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMP"),
            tokenizer::AssemblyToken::Quote,
            tokenizer::AssemblyToken::Identifier("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Quote,
            tokenizer::AssemblyToken::Identifier("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Quote,
            tokenizer::AssemblyToken::Identifier("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CALL"),
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CALL"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CALL"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CALL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CALL"),
            tokenizer::AssemblyToken::Quote,
            tokenizer::AssemblyToken::Identifier("loop"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Stack variants"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("PUSH"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("POP"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("PUSH"),
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("POP"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Loading & Memory"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(1234),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(2147483647),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(7)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(64),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(24),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Machine Arithmetic"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(4),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(2),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(12),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Tag Operations"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SETTAG"),
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GETTAG"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SETPAYLOAD"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GETPAYLOAD"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Lisp Destructuring Primitives"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CONS"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("UNCONS"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CAR"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CDR"),
            tokenizer::AssemblyToken::Register(Register(6)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(7)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SETCAR"),
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SETCDR"),
            tokenizer::AssemblyToken::Register(Register(10)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(11)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment(
                "; Arithmetic (assembler::ThreeRegs layouts using Option types, Word Immediates)",
            ),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MUL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MUL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MUL"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("EQ"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("EQ"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("EQ"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("NE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("NE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("NE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("LT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("LT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("LT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("LTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("LTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("LTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GT"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GTE"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(10),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("DIV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("DIV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("DIV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Hash,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Advanced Operations"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MAKECLOSURE"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MEMCPY"),
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(1),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("TYPEP"),
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(3),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GTE"),
            tokenizer::AssemblyToken::Register(Register(0)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::MachineRegister(MachineRegister(2)),
            tokenizer::AssemblyToken::Newline,
        ]
    }

    pub fn expected_lines<'a>() -> Vec<parser::AssemblyLine<'a>> {
        vec![
            parser::AssemblyLine::Org(128),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Halt),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Nop),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Return),
            parser::AssemblyLine::Label("loop"),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Int(
                assembler::Reference::Resolved(42),
            )),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Int(
                assembler::Reference::Unresolved("loop"),
            )),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::IReturn),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::JumpTarget::Absolute(assembler::Reference::Resolved(16)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(16),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::JumpTarget::Register(cpu::Register(1)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::JumpTarget::Absolute(assembler::Reference::Unresolved("loop")),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: assembler::JumpTarget::Absolute(assembler::Reference::Resolved(16)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(16),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: assembler::JumpTarget::Register(cpu::Register(1)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: assembler::JumpTarget::Absolute(assembler::Reference::Unresolved("loop")),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: assembler::JumpTarget::Absolute(assembler::Reference::Resolved(16)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(16),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: assembler::JumpTarget::Register(cpu::Register(1)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: assembler::JumpTarget::Absolute(assembler::Reference::Unresolved("loop")),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::JumpTarget::Absolute(assembler::Reference::Resolved(16)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::JumpTarget::Machine(
                    cpu::MachineRegister(1),
                    assembler::Reference::Resolved(16),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::JumpTarget::Register(cpu::Register(1)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::JumpTarget::Absolute(assembler::Reference::Unresolved("loop")),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::PushA {
                src: cpu::MachineRegister(1),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::PopA {
                dst: cpu::MachineRegister(2),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::PushR {
                src: cpu::Register(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::PopR {
                dst: cpu::Register(4),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Register(cpu::Register(1)),
                src: assembler::Location::Literal(assembler::Native::Fixnum(
                    assembler::Reference::Resolved(1234),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Machine(cpu::MachineRegister(3)),
                src: assembler::Location::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(2147483647),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Register(cpu::Register(5)),
                src: assembler::Location::Register(cpu::Register(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Register(cpu::Register(5)),
                src: assembler::Location::IndirectRegister(cpu::Register(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::IndirectRegister(cpu::Register(6)),
                src: assembler::Location::Register(cpu::Register(7)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Machine(cpu::MachineRegister(4)),
                src: assembler::Location::Machine(cpu::MachineRegister(5)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Machine(cpu::MachineRegister(1)),
                src: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(2),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Machine(cpu::MachineRegister(1)),
                src: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(2),
                    assembler::Reference::Resolved(16),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Machine(cpu::MachineRegister(1)),
                src: assembler::Location::Absolute(assembler::Reference::Resolved(64)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(2),
                    assembler::Reference::Resolved(0),
                ),
                src: assembler::Location::Machine(cpu::MachineRegister(3)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(2),
                    assembler::Reference::Resolved(8),
                ),
                src: assembler::Location::Machine(cpu::MachineRegister(3)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Absolute(assembler::Reference::Resolved(24)),
                src: assembler::Location::Machine(cpu::MachineRegister(4)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Machine(cpu::MachineRegister(5)),
                src: assembler::Location::Register(cpu::Register(8)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Register(cpu::Register(9)),
                src: assembler::Location::Machine(cpu::MachineRegister(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(5),
                    assembler::Reference::Resolved(0),
                ),
                src: assembler::Location::Register(cpu::Register(8)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(5),
                    assembler::Reference::Resolved(8),
                ),
                src: assembler::Location::Register(cpu::Register(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Register(cpu::Register(5)),
                src: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(6),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: assembler::Location::Register(cpu::Register(5)),
                src: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(6),
                    assembler::Reference::Resolved(8),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: assembler::Location::Machine(cpu::MachineRegister(3)),
                src: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(4),
                    assembler::Reference::Resolved(0),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: assembler::Location::Machine(cpu::MachineRegister(3)),
                src: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(4),
                    assembler::Reference::Resolved(5),
                ),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: assembler::Location::Machine(cpu::MachineRegister(3)),
                src: assembler::Location::Absolute(assembler::Reference::Resolved(5)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(4),
                    assembler::Reference::Resolved(0),
                ),
                src: assembler::Location::Machine(cpu::MachineRegister(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: assembler::Location::IndirectMachine(
                    cpu::MachineRegister(4),
                    assembler::Reference::Resolved(5),
                ),
                src: assembler::Location::Machine(cpu::MachineRegister(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: assembler::Location::Absolute(assembler::Reference::Resolved(5)),
                src: assembler::Location::Machine(cpu::MachineRegister(6)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: cpu::MachineRegister(1),
                operands: parser::MachSource {
                    op1: cpu::MachineRegister(2),
                    op2: Some(cpu::MachineRegister(3)),
                    op3: None,
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: cpu::MachineRegister(1),
                operands: parser::MachSource {
                    op1: cpu::MachineRegister(2),
                    op2: Some(cpu::MachineRegister(3)),
                    op3: Some(assembler::Reference::Resolved(4)),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MBinary {
                op: cpu::MBinaryOp::Add,
                dst: cpu::MachineRegister(1),
                operands: parser::MachSource {
                    op1: cpu::MachineRegister(2),
                    op2: None,
                    op3: Some(assembler::Reference::Resolved(8)),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: cpu::MachineRegister(4),
                operands: parser::MachSource {
                    op1: cpu::MachineRegister(5),
                    op2: Some(cpu::MachineRegister(6)),
                    op3: None,
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: cpu::MachineRegister(4),
                operands: parser::MachSource {
                    op1: cpu::MachineRegister(5),
                    op2: Some(cpu::MachineRegister(6)),
                    op3: Some(assembler::Reference::Resolved(2)),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MBinary {
                op: cpu::MBinaryOp::Sub,
                dst: cpu::MachineRegister(4),
                operands: parser::MachSource {
                    op1: cpu::MachineRegister(5),
                    op2: None,
                    op3: Some(assembler::Reference::Resolved(12)),
                },
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetTag {
                dst: cpu::Register(2),
                src: cpu::MachineRegister(1),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::GetTag {
                dst: cpu::MachineRegister(3),
                src: cpu::Register(2),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetPayload {
                dst: cpu::Register(4),
                src: cpu::MachineRegister(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::GetPayload {
                dst: cpu::MachineRegister(5),
                src: cpu::Register(4),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Cons {
                dst: cpu::Register(1),
                car: cpu::Register(2),
                cdr: cpu::Register(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Uncons {
                car: cpu::Register(1),
                cdr: cpu::Register(2),
                src: cpu::Register(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Car(cpu::TwoRegs {
                dst: cpu::Register(4),
                src: cpu::Register(5),
            })),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Cdr(cpu::TwoRegs {
                dst: cpu::Register(6),
                src: cpu::Register(7),
            })),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCar(cpu::TwoRegs {
                dst: cpu::Register(8),
                src: cpu::Register(9),
            })),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCdr(cpu::TwoRegs {
                dst: cpu::Register(10),
                src: cpu::Register(11),
            })),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                        10,
                    ))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                        10,
                    ))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Mul,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Mul,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Mul,
                dst: cpu::Register(1),
                operands: assembler::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                        10,
                    ))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Eq,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Eq,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Eq,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: None,
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                            10,
                        ))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Ne,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Ne,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Ne,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: None,
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                            10,
                        ))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lt,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lt,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lt,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: None,
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                            10,
                        ))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lte,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lte,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lte,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: None,
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                            10,
                        ))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gt,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gt,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gt,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: None,
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                            10,
                        ))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: Some(cpu::Register(3)),
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: cpu::Register(1),
                    operands: assembler::EitherSource::Reg(assembler::RegSource {
                        op1: cpu::Register(2),
                        op2: None,
                        op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(
                            10,
                        ))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::IDiv {
                div: cpu::Register(9),
                rem: cpu::Register(2),
                operands: assembler::RegSource {
                    op1: cpu::Register(3),
                    op2: Some(cpu::Register(4)),
                    op3: None,
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::IDiv {
                div: cpu::Register(9),
                rem: cpu::Register(2),
                operands: assembler::RegSource {
                    op1: cpu::Register(3),
                    op2: Some(cpu::Register(4)),
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::IDiv {
                div: cpu::Register(9),
                rem: cpu::Register(2),
                operands: assembler::RegSource {
                    op1: cpu::Register(3),
                    op2: None,
                    op3: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                },
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::MakeClosure {
                dst: cpu::Register(5),
                code: cpu::MachineRegister(6),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MemCpy {
                dst: cpu::MachineRegister(1),
                src: cpu::MachineRegister(2),
                count: assembler::RegAndOffset {
                    op1: None,
                    op2: Some(assembler::Native::Raw(assembler::Reference::Resolved(1))),
                },
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Typep {
                dst: cpu::Register(1),
                src: cpu::Register(1),
                compare: assembler::Reference::Resolved(3),
            }),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    dst: cpu::Register(0),
                    op: cpu::Comparison::Gte,
                    operands: parser::EitherSource::Mach(parser::MachSource {
                        op1: cpu::MachineRegister(1),
                        op2: Some(cpu::MachineRegister(2)),
                        op3: None,
                    }),
                },
            ),
        ]
    }

    pub fn expected_resolved<'a>() -> assembler::Layout<'a> {
        let tokens = assembler::tokenize(SOURCE).unwrap();
        let ast = assembler::parse(&tokens).unwrap();
        let mut layout = assembler::layout(ast).unwrap();
        assembler::resolve(&mut layout).unwrap();
        layout
    }

    #[test]
    pub fn instructions() {
        let tokens = assembler::tokenize(SOURCE).unwrap();
        let ast = assembler::parse(&tokens).unwrap();
        let mut layout = assembler::layout(ast).unwrap();
        assembler::resolve(&mut layout).unwrap();
        let r = assembler::emit(&layout).unwrap();

        assert_eq!(
            r,
            vec![assembler::Section {
                data: vec![
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 39, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0,
                    0, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0,
                    0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 64, 0, 0, 0, 0, 0,
                    0, 16, 0, 0, 0, 0, 0, 0, 0, 2, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                    17, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 2, 64, 0, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0, 0, 0, 0, 3, 5, 64, 0,
                    0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 3, 5, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 3, 5, 17, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 3, 5, 1, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 3, 5, 64, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0, 0, 0, 0, 4,
                    5, 64, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 4, 5, 17, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 4, 5, 17, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 4, 5, 1, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 5, 64, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0,
                    0, 0, 0, 38, 64, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 38, 17, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 17, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0,
                    0, 38, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 38, 64, 0, 0, 0, 0, 0, 0,
                    176, 0, 0, 0, 0, 0, 0, 0, 7, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8,
                    18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 35, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 36, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 1, 65, 0, 0,
                    0, 0, 0, 210, 4, 0, 0, 0, 0, 0, 1, 9, 19, 65, 0, 0, 0, 0, 0, 255, 255, 255,
                    127, 0, 0, 0, 0, 9, 5, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 5, 38, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 38, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 9, 20, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 17, 50, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 17, 50, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0,
                    9, 17, 64, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 9, 50, 19, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 9, 50, 19, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 9, 64,
                    20, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 0, 0, 9, 21, 8, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 9, 9, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 53, 8, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 53, 6, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0,
                    0, 9, 5, 54, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 5, 54, 0, 0, 0, 0, 0, 8,
                    0, 0, 0, 0, 0, 0, 0, 10, 19, 52, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 19,
                    52, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 10, 19, 64, 0, 0, 0, 0, 0, 5, 0, 0,
                    0, 0, 0, 0, 0, 10, 52, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 52, 22,
                    0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 10, 64, 22, 0, 0, 0, 0, 0, 5, 0, 0, 0,
                    0, 0, 0, 0, 11, 17, 18, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11, 17, 18, 51,
                    0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 11, 17, 18, 64, 0, 0, 0, 0, 8, 0, 0, 0, 0,
                    0, 0, 0, 12, 20, 21, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 20, 21, 54, 0,
                    0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 12, 20, 21, 64, 0, 0, 0, 0, 12, 0, 0, 0, 0, 0,
                    0, 0, 13, 2, 17, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 14, 19, 2, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 4, 19, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    16, 21, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 17, 1, 2, 3, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 18, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 19, 4, 5,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 21, 8, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 22, 10, 11, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 23, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    23, 1, 2, 35, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 23, 1, 2, 64, 0, 0, 0, 0, 10,
                    0, 0, 0, 0, 0, 0, 1, 28, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28, 1, 2,
                    35, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 28, 1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0,
                    0, 0, 0, 1, 24, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 24, 1, 2, 35, 0,
                    0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 24, 1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0,
                    0, 1, 29, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29, 1, 2, 35, 0, 0, 0,
                    0, 5, 0, 0, 0, 0, 0, 0, 1, 29, 1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 1,
                    30, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 1, 2, 35, 0, 0, 0, 0, 5,
                    0, 0, 0, 0, 0, 0, 1, 30, 1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 1, 33, 1,
                    2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 1, 2, 35, 0, 0, 0, 0, 5, 0, 0, 0,
                    0, 0, 0, 1, 33, 1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 1, 34, 1, 2, 3, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 34, 1, 2, 35, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0,
                    1, 34, 1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 1, 31, 1, 2, 3, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 31, 1, 2, 35, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 31,
                    1, 2, 64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 1, 32, 1, 2, 3, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 32, 1, 2, 35, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 32, 1, 2,
                    64, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 1, 27, 9, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 27, 9, 2, 3, 36, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 27, 9, 2, 3, 64,
                    0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 37, 5, 22, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 41, 17, 18, 64, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 40, 1, 1, 0, 0, 0, 0, 0,
                    3, 0, 0, 0, 0, 0, 0, 0, 32, 0, 17, 18, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0
                ],
                base: 128
            }]
        );
    }
}
