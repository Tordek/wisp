pub mod parser;
pub mod tokenizer;

use std::collections::HashMap;

use crate::cpu::{
    self,
    assembler::{
        parser::*,
        tokenizer::{TokenizeError, tokenize},
    },
};

pub struct AbiRegisters {}
impl AbiRegisters {
    // Argument registers: Quick arguments for functions.
    const A0: cpu::Register = cpu::Register(226);
    const A1: cpu::Register = cpu::Register(227);
    const A2: cpu::Register = cpu::Register(228);
    const A3: cpu::Register = cpu::Register(229);
    const A4: cpu::Register = cpu::Register(230);
    const A5: cpu::Register = cpu::Register(231);
    const A6: cpu::Register = cpu::Register(232);
    const A7: cpu::Register = cpu::Register(233);
    // Argument counter: If >8, the rest are on the stack
    const AN: cpu::Register = cpu::Register(234);
    // Result registers: Return values
    const R0: cpu::Register = cpu::Register(235);
    const R1: cpu::Register = cpu::Register(236);
    const R2: cpu::Register = cpu::Register(237);
    const R3: cpu::Register = cpu::Register(238);
    // Result value counter
    const RN: cpu::Register = cpu::Register(239);
    // Pointer to extra value area.
    const RX: cpu::Register = cpu::Register(240);
}

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

pub fn resolve_data<'a>(
    data: &Native<'a>,
    symbols: &HashMap<&'a str, usize>,
) -> Result<cpu::Native, AssemblerError<'a>> {
    Ok(match data {
        Native::Raw(refr) => cpu::Native(resolve_reference(refr, symbols)? as u64),
        Native::Char(refr) => {
            cpu::Native::from(cpu::LispWord::char(resolve_reference(refr, symbols)? as u64))
        }
        Native::Symbol(refr) => cpu::Native::from(cpu::LispWord::symbol(resolve_reference(
            refr, symbols,
        )? as u64)),
        Native::Cons(refr) => {
            cpu::Native::from(cpu::LispWord::cons(resolve_reference(refr, symbols)? as u64))
        }
        Native::Fixnum(refr) => cpu::Native::from(cpu::LispWord::fixnum(resolve_reference(
            refr, symbols,
        )? as i64)),
        Native::String(refr) => cpu::Native::from(cpu::LispWord::string(resolve_reference(
            refr, symbols,
        )? as u64)),
    })
}

pub fn resolve_lvalue<'a>(
    location: &LValue<'a>,
    labels: &HashMap<&'a str, usize>,
) -> Result<cpu::LValue, AssemblerError<'a>> {
    Ok(match location {
        LValue::Absolute(r) => cpu::LValue::Absolute(cpu::Address(resolve_data(r, labels)?.0)),
        LValue::Indirect(r) => cpu::LValue::Indirect(resolve_reg_and_off(r, labels)?),
        LValue::Register(r) => cpu::LValue::Register(*r),
    })
}

pub fn resolve_rvalue<'a>(
    location: &RValue<'a>,
    labels: &HashMap<&'a str, usize>,
) -> Result<cpu::RValue, AssemblerError<'a>> {
    Ok(match location {
        RValue::Literal(l) => cpu::RValue::Literal(resolve_data(l, labels)?),
        RValue::Register(r) => cpu::RValue::Register(resolve_reg_and_off(r, labels)?),
        RValue::Absolute(r) => cpu::RValue::Absolute(cpu::Address::from(resolve_data(r, labels)?)),
        RValue::Indirect(r) => cpu::RValue::Indirect(resolve_reg_and_off(r, labels)?),
    })
}

pub fn resolve_reg_and_off<'a>(
    rando: &RegAndOff<'a>,
    labels: &HashMap<&'a str, usize>,
) -> Result<cpu::RegAndOff, AssemblerError<'a>> {
    Ok(cpu::RegAndOff {
        op1: rando.op1,
        off: rando
            .off
            .clone()
            .map(|off| resolve_data(&off, labels))
            .transpose()?,
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
                    *line = AssemblyLine::ResolvedData(
                        resolve_data(d, symbols)?.0.to_le_bytes().to_vec(),
                    )
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Binary {
                    op,
                    dst,
                    op1,
                    op2,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                        op: *op,
                        dst: *dst,
                        op1: *op1,
                        op2: resolve_rvalue(op2, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call { target }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                        target: resolve_rvalue(target, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::IDiv {
                    div,
                    rem,
                    op1,
                    op2,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                        div: *div,
                        rem: *rem,
                        op1: *op1,
                        op2: resolve_rvalue(op2, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Int(i)) => {
                    *line = {
                        AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(
                            resolve_data(i, symbols)?.0 as u64,
                        ))
                    }
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                    condition,
                    target,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                        condition: *condition,
                        target: resolve_rvalue(target, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MBinary {
                    op,
                    dst,
                    op1,
                    op2,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                        op: *op,
                        dst: *dst,
                        op1: *op1,
                        op2: resolve_rvalue(op2, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                    op,
                    dst,
                    op1,
                    op2,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Comparison {
                        op: *op,
                        dst: *dst,
                        op1: *op1,
                        op2: resolve_rvalue(op2, symbols)?,
                    })
                }
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
                        dst: resolve_lvalue(dst, symbols)?,
                        src: resolve_rvalue(src, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov8 { dst, src }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                        dst: resolve_lvalue(dst, symbols)?,
                        src: resolve_rvalue(src, symbols)?,
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
                        compare: resolve_data(compare, symbols)?,
                    })
                }
                AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MemCpy {
                    dst,
                    src,
                    count,
                }) => {
                    *line = AssemblyLine::ResolvedInstruction(cpu::Instruction::MemCpy {
                        dst: *dst,
                        src: *src,
                        count: resolve_rvalue(count, symbols)?,
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
            JUMP V1
            JUMP V1 + 16
            JUMP V1
            JUMP 'loop
            JUMPIF V5, 16
            JUMPIF V5, V1
            JUMPIF V5, V1 + 16
            JUMPIF V5, V1
            JUMPIF V5, 'loop
            JUMPIFNOT V5, 16
            JUMPIFNOT V5, V1
            JUMPIFNOT V5, V1 + 16
            JUMPIFNOT V5, V1
            JUMPIFNOT V5, 'loop
            CALL 16
            CALL V1
            CALL V1 + 16
            CALL V1
            CALL 'loop
            ; Stack variants
            PUSH V1
            POP V2
            PUSH V3
            POP V4
            ; Loading & Memory
            MOV V1, #1234
            MOV V3, 0x7FFFFFFF
            MOV V5, V6
            MOV V5, [V6]
            MOV [V6], V7
            MOV V4, V5
            MOV V1, [V2]
            MOV V1, [V2 + 16]
            MOV V1, [64]
            MOV [V2], V3
            MOV [V2 + 8], V3
            MOV [24], V4
            MOV V5, V8
            MOV V9, V6
            MOV [V5], V8
            MOV [V5 + 8], V6
            MOV V5, [V6]
            MOV V5, [V6 + 8]
            MOV8 V3, [V4]
            MOV8 V3, [V4 + 5]
            MOV8 V3, [5]
            MOV8 [V4], V6
            MOV8 [V4 + 5], V6
            MOV8 [5], V6
        ; Register Arithmetic
            ADD V1, V2, V3
            ADD V1, V2, V3 + 4
            ADD V1, V2, 8
            SUB V4, V5, V6
            SUB V4, V5, V6 + 2
            SUB V4, V5, 12
        ; Tag Operations
            SETTAG V2, V1
            GETTAG V3, V2
            SETPAYLOAD V4, V3
            GETPAYLOAD V5, V4
        ; Lisp Destructuring Primitives
            CONS V1, V2, V3
            UNCONS V1, V2, V3
            CAR V4, V5
            CDR V6, V7
            SETCAR V8, V9
            SETCDR V10, V11
        ; Arithmetic (assembler::ThreeRegs layouts using Option types, Word Immediates)
            ADD V1, V2, V3
            ADD V1, V2, V3 + #5
            ADD V1, V2, #10
            SUB V1, V2, V3
            SUB V1, V2, V3 + #5
            SUB V1, V2, #10
            MUL V1, V2, V3
            MUL V1, V2, V3 + #5
            MUL V1, V2, #10
            EQ V1, V2, V3
            EQ V1, V2, V3 + #5
            EQ V1, V2, #10
            NE V1, V2, V3
            NE V1, V2, V3 + #5
            NE V1, V2, #10
            LT V1, V2, V3
            LT V1, V2, V3 + #5
            LT V1, V2, #10
            LTE V1, V2, V3
            LTE V1, V2, V3 + #5
            LTE V1, V2, #10
            GT V1, V2, V3
            GT V1, V2, V3 + #5
            GT V1, V2, #10
            GTE V1, V2, V3
            GTE V1, V2, V3 + #5
            GTE V1, V2, #10
            DIV V9, V2, V3, V4
            DIV V9, V2, V3, V4 + #5
            DIV V9, V2, V3, #5
        ; Advanced Operations
            MAKECLOSURE V5, V6
            MEMCPY V1, V2, 1
            TYPEP V1, V1, 0x03
            GTE V0, V1, V2
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
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMP"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIF"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("JUMPIFNOT"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("CALL"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("POP"),
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(16),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(64),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(24),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(9)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(8)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV"),
            tokenizer::AssemblyToken::Register(Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MOV8"),
            tokenizer::AssemblyToken::OpenBracket,
            tokenizer::AssemblyToken::Number(5),
            tokenizer::AssemblyToken::CloseBracket,
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Register Arithmetic"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(4),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("ADD"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(8),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Plus,
            tokenizer::AssemblyToken::Number(2),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SUB"),
            tokenizer::AssemblyToken::Register(cpu::Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Number(12),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Comment("; Tag Operations"),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SETTAG"),
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GETTAG"),
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(Register(2)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("SETPAYLOAD"),
            tokenizer::AssemblyToken::Register(Register(4)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(3)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("GETPAYLOAD"),
            tokenizer::AssemblyToken::Register(cpu::Register(5)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(6)),
            tokenizer::AssemblyToken::Newline,
            tokenizer::AssemblyToken::Identifier("MEMCPY"),
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
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
            tokenizer::AssemblyToken::Register(cpu::Register(1)),
            tokenizer::AssemblyToken::Comma,
            tokenizer::AssemblyToken::Register(cpu::Register(2)),
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
                assembler::Native::Raw(assembler::Reference::Resolved(42)),
            )),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Int(
                assembler::Native::Raw(assembler::Reference::Unresolved("loop")),
            )),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::IReturn),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(16),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(16))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::Always,
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Unresolved("loop"),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(Register(5)),
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(16),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(Register(5)),
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(Register(5)),
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(16))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(Register(5)),
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::True(Register(5)),
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Unresolved("loop"),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(Register(5)),
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(16),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(Register(5)),
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(Register(5)),
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(16))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(Register(5)),
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Jump {
                condition: cpu::Condition::False(Register(5)),
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Unresolved("loop"),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(16),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(16))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(1),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Call {
                target: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Unresolved("loop"),
                )),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Push { src: Register(1) }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Pop { dst: Register(2) }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Push { src: Register(3) }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Pop { dst: Register(4) }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(1)),
                src: assembler::RValue::Literal(parser::Native::Fixnum(
                    assembler::Reference::Resolved(1234),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(3)),
                src: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(2147483647),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(5)),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(5)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(7),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(4)),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(5),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(1)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(2),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(1)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(2),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(16))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(1)),
                src: parser::RValue::Absolute(assembler::Native::Raw(
                    assembler::Reference::Resolved(64),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(2),
                    off: None,
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(2),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(8))),
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Absolute(assembler::Native::Raw(
                    assembler::Reference::Resolved(24),
                )),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(4),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(5)),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(8),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(9)),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(5),
                    off: None,
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(8),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(5),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(8))),
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(5)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov {
                dst: parser::LValue::Register(Register(5)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(6),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(8))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: parser::LValue::Register(Register(3)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(4),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: parser::LValue::Register(Register(3)),
                src: parser::RValue::Indirect(parser::RegAndOff {
                    op1: Register(4),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(5))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: parser::LValue::Register(Register(3)),
                src: parser::RValue::Absolute(assembler::Native::Raw(
                    assembler::Reference::Resolved(5),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(4),
                    off: None,
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: parser::LValue::Indirect(parser::RegAndOff {
                    op1: Register(4),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(5))),
                }),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Mov8 {
                dst: parser::LValue::Absolute(assembler::Native::Raw(
                    assembler::Reference::Resolved(5),
                )),
                src: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(4))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: Register(1),
                op1: Register(2),
                op2: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(8),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: Register(4),
                op1: Register(5),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: Register(4),
                op1: Register(5),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(6),
                    off: Some(assembler::Native::Raw(assembler::Reference::Resolved(2))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: Register(4),
                op1: Register(5),
                op2: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(12),
                )),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetTag {
                dst: Register(2),
                src: Register(1),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::GetTag {
                dst: Register(3),
                src: Register(2),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetPayload {
                dst: Register(4),
                src: Register(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::GetPayload {
                dst: Register(5),
                src: Register(4),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Cons {
                dst: Register(1),
                car: Register(2),
                cdr: Register(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Uncons {
                car: Register(1),
                cdr: Register(2),
                src: Register(3),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Car {
                dst: Register(4),
                src: Register(5),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::Cdr {
                dst: Register(6),
                src: Register(7),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCar {
                dst: Register(8),
                src: Register(9),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCdr {
                dst: Register(10),
                src: Register(11),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Add,
                dst: Register(1),
                op1: Register(2),
                op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                    assembler::Reference::Resolved(10),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Sub,
                dst: Register(1),
                op1: Register(2),
                op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                    assembler::Reference::Resolved(10),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Mul,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Mul,
                dst: Register(1),
                op1: Register(2),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(3),
                    off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Binary {
                op: cpu::BinaryOp::Mul,
                dst: Register(1),
                op1: Register(2),
                op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                    assembler::Reference::Resolved(10),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Eq,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Eq,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Eq,
                    dst: Register(1),
                    op1: Register(2),
                    op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                        assembler::Reference::Resolved(10),
                    )),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Ne,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Ne,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Ne,
                    dst: Register(1),
                    op1: Register(2),
                    op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                        assembler::Reference::Resolved(10),
                    )),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lt,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lt,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lt,
                    dst: Register(1),
                    op1: Register(2),
                    op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                        assembler::Reference::Resolved(10),
                    )),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lte,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lte,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Lte,
                    dst: Register(1),
                    op1: Register(2),
                    op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                        assembler::Reference::Resolved(10),
                    )),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gt,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gt,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gt,
                    dst: Register(1),
                    op1: Register(2),
                    op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                        assembler::Reference::Resolved(10),
                    )),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: None,
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: Register(1),
                    op1: Register(2),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(3),
                        off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                    }),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: Register(1),
                    op1: Register(2),
                    op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                        assembler::Reference::Resolved(10),
                    )),
                },
            ),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::IDiv {
                div: Register(9),
                rem: Register(2),
                op1: Register(3),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(4),
                    off: None,
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::IDiv {
                div: Register(9),
                rem: Register(2),
                op1: Register(3),
                op2: parser::RValue::Register(parser::RegAndOff {
                    op1: Register(4),
                    off: Some(assembler::Native::Fixnum(assembler::Reference::Resolved(5))),
                }),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::IDiv {
                div: Register(9),
                rem: Register(2),
                op1: Register(3),
                op2: assembler::RValue::Literal(assembler::Native::Fixnum(
                    assembler::Reference::Resolved(5),
                )),
            }),
            parser::AssemblyLine::ResolvedInstruction(cpu::Instruction::MakeClosure {
                dst: Register(5),
                code: Register(6),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::MemCpy {
                dst: Register(1),
                src: Register(2),
                count: assembler::RValue::Literal(assembler::Native::Raw(
                    assembler::Reference::Resolved(1),
                )),
            }),
            parser::AssemblyLine::UnresolvedInstruction(parser::UnresolvedInstruction::Typep {
                dst: Register(1),
                src: Register(1),
                compare: assembler::Native::Raw(assembler::Reference::Resolved(3)),
            }),
            parser::AssemblyLine::UnresolvedInstruction(
                parser::UnresolvedInstruction::MComparison {
                    op: cpu::Comparison::Gte,
                    dst: Register(0),
                    op1: Register(1),
                    op2: parser::RValue::Register(parser::RegAndOff {
                        op1: Register(2),
                        off: None,
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
                    0, 0, 0, 0, 0, 0, 38, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 32, 0, 0,
                    0, 0, 0, 0, 42, 0, 0, 0, 0, 0, 0, 0, 5, 32, 0, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0,
                    0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 38, 0, 0, 0, 0, 0,
                    0, 16, 0, 0, 0, 0, 0, 0, 0, 2, 2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                    34, 0, 1, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 2, 2, 0, 1, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 2, 38, 0, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0, 0, 0, 0, 3, 39, 5, 0,
                    0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 3, 3, 5, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 3, 35, 5, 1, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 3, 3, 5, 1, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 3, 39, 5, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0, 0, 0, 0, 4,
                    39, 5, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 4, 3, 5, 1, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 4, 35, 5, 1, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 4, 3, 5, 1, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 39, 5, 0, 0, 0, 0, 0, 176, 0, 0, 0, 0, 0,
                    0, 0, 37, 35, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 37, 1, 1, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 37, 33, 1, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0,
                    37, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 37, 35, 0, 0, 0, 0, 0, 0, 176,
                    0, 0, 0, 0, 0, 0, 0, 34, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 35, 1, 2,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 34, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 35, 1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 45, 1, 0, 0, 0, 0,
                    0, 0, 210, 4, 0, 0, 0, 0, 0, 7, 45, 3, 0, 0, 0, 0, 0, 255, 255, 255, 127, 0, 0,
                    0, 0, 7, 5, 5, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 9, 5, 0, 0, 6, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 7, 6, 0, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 5,
                    4, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 9, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 7, 41, 1, 0, 0, 2, 0, 0, 16, 0, 0, 0, 0, 0, 0, 0, 7, 33, 1, 0, 0,
                    0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 7, 6, 0, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 7, 38, 0, 2, 3, 0, 0, 1, 8, 0, 0, 0, 0, 0, 0, 0, 7, 36, 0, 0, 4, 0, 0, 1,
                    24, 0, 0, 0, 0, 0, 0, 0, 7, 5, 5, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 5,
                    9, 0, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 6, 0, 5, 8, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 7, 38, 0, 5, 6, 0, 0, 1, 8, 0, 0, 0, 0, 0, 0, 0, 7, 9, 5, 0, 0, 6,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 41, 5, 0, 0, 6, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0,
                    8, 9, 3, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 41, 3, 0, 0, 4, 0, 0, 5, 0,
                    0, 0, 0, 0, 0, 0, 8, 33, 3, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 8, 6, 0, 4,
                    6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 38, 0, 4, 6, 0, 0, 1, 5, 0, 0, 0, 0, 0,
                    0, 0, 8, 36, 0, 0, 6, 0, 0, 1, 5, 0, 0, 0, 0, 0, 0, 0, 22, 7, 1, 2, 3, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 22, 39, 1, 2, 3, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 22,
                    47, 1, 2, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 27, 7, 4, 5, 6, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 27, 39, 4, 5, 6, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 27, 47, 4, 5,
                    0, 0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 11, 3, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 12, 3, 3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 3, 4, 3, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 14, 3, 5, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 7,
                    1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 17, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 18, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 19, 3, 6, 7, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 3, 8, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    21, 3, 10, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 22, 7, 1, 2, 3, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 22, 39, 1, 2, 3, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 22, 47,
                    1, 2, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 27, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 27, 39, 1, 2, 3, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 27, 47, 1, 2, 0,
                    0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 23, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 23, 39, 1, 2, 3, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 23, 47, 1, 2, 0, 0, 0, 0,
                    0, 10, 0, 0, 0, 0, 0, 0, 28, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 28,
                    39, 1, 2, 3, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 28, 47, 1, 2, 0, 0, 0, 0, 0, 10,
                    0, 0, 0, 0, 0, 0, 29, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 29, 39, 1,
                    2, 3, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 29, 47, 1, 2, 0, 0, 0, 0, 0, 10, 0, 0,
                    0, 0, 0, 0, 32, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 32, 39, 1, 2, 3,
                    0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 32, 47, 1, 2, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0,
                    0, 0, 33, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 39, 1, 2, 3, 0, 0,
                    0, 0, 5, 0, 0, 0, 0, 0, 0, 33, 47, 1, 2, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0,
                    30, 7, 1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 39, 1, 2, 3, 0, 0, 0, 0,
                    5, 0, 0, 0, 0, 0, 0, 30, 47, 1, 2, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 31, 7,
                    1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 31, 39, 1, 2, 3, 0, 0, 0, 0, 5, 0, 0,
                    0, 0, 0, 0, 31, 47, 1, 2, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 0, 26, 15, 9, 2, 3,
                    4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 26, 47, 9, 2, 3, 4, 0, 0, 0, 5, 0, 0, 0, 0, 0,
                    0, 26, 63, 9, 2, 3, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 36, 3, 5, 6, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 40, 47, 1, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 39,
                    35, 1, 1, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 31, 7, 0, 1, 2, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ],
                base: 128
            }]
        );
    }
}
