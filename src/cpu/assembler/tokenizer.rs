use nom::{
    AsChar, IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_until},
    character::{
        anychar,
        complete::{alpha1, alphanumeric1, digit1, hex_digit1, multispace0, satisfy, space0},
    },
    combinator::{peek, recognize, value},
    multi::{many0, many0_count},
    sequence::{delimited, pair, preceded, terminated},
};

use crate::cpu::{self, assembler};

#[derive(Clone, PartialEq, Debug)]
pub enum AssemblyToken<'a> {
    Colon,
    Dot,
    Number(i64),
    Register(cpu::Register),
    OpenBracket,
    CloseBracket,
    Plus,
    Minus,
    Comment(&'a str),
    Identifier(&'a str),
    Cash,
    Quote,
    String(&'a str),
    Character(u8),
    Hash,
    Newline,
    Comma,
    Bang,
}

// Convenience methods
fn ident(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("-"), tag("_")))),
    ))
    .parse(input)
}

fn number_string(input: &str) -> IResult<&str, i64> {
    alt((hex, decimal)).parse(input)
}

fn decimal(input: &str) -> IResult<&str, i64> {
    digit1.map_res(|v: &str| v.parse::<i64>()).parse(input)
}

fn hex(input: &str) -> IResult<&str, i64> {
    preceded(tag("0x"), recognize(hex_digit1))
        .map_res(|str| u64::from_str_radix(str, 16).map(|c| c as i64))
        .parse(input)
}

// Tokens
fn directive(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Dot, tag(".")).parse(input)
}

fn number(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    number_string.map(AssemblyToken::Number).parse(input)
}

fn register(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    terminated(
        alt((
            alt([
                value(AssemblyToken::Register(cpu::Cpu::NIL), tag("NIL")),
                value(AssemblyToken::Register(cpu::Cpu::T), tag("T")),
                value(AssemblyToken::Register(cpu::Cpu::CONS_END), tag("CONS_END")),
                value(
                    AssemblyToken::Register(cpu::Cpu::CONS_FREE),
                    tag("CONS_FREE"),
                ),
                value(AssemblyToken::Register(cpu::Cpu::GEN_END), tag("GEN_END")),
                value(AssemblyToken::Register(cpu::Cpu::GEN_FREE), tag("GEN_FREE")),
                value(AssemblyToken::Register(cpu::Cpu::PC), tag("PC")),
                value(AssemblyToken::Register(cpu::Cpu::FP), tag("FP")),
                value(AssemblyToken::Register(cpu::Cpu::SP), tag("SP")),
                value(AssemblyToken::Register(cpu::Cpu::VBR), tag("VBR")),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A0),
                    tag("A0"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A1),
                    tag("A1"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A2),
                    tag("A2"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A3),
                    tag("A3"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A4),
                    tag("A4"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A5),
                    tag("A5"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A6),
                    tag("A6"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::A7),
                    tag("A7"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::AN),
                    tag("AN"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::R0),
                    tag("R0"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::R1),
                    tag("R1"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::R2),
                    tag("R2"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::R3),
                    tag("R3"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::RN),
                    tag("RN"),
                ),
                value(
                    AssemblyToken::Register(assembler::AbiRegisters::RX),
                    tag("RX"),
                ),
            ]),
            preceded(tag("V"), decimal).map(|c| AssemblyToken::Register(cpu::Register(c as u8))),
        )),
        peek(satisfy(|c| !c.is_alphanum())),
    )
    .parse(input)
}
fn openbracket(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::OpenBracket, tag("[")).parse(input)
}
fn closebracket(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::CloseBracket, tag("]")).parse(input)
}
fn plus(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Plus, tag("+")).parse(input)
}
fn minus(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Minus, tag("-")).parse(input)
}
fn comment(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    recognize(preceded(tag(";"), take_until("\n")))
        .map(AssemblyToken::Comment)
        .parse(input)
}
fn newline(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(
        AssemblyToken::Newline,
        nom::character::complete::line_ending,
    )
    .parse(input)
}
fn identifier(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    ident.map(AssemblyToken::Identifier).parse(input)
}
fn colon(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Colon, tag(":")).parse(input)
}
fn quote(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Quote, tag("'")).parse(input)
}
fn raw_string(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    delimited(
        tag("\""),
        recognize(many0(alt((
            satisfy(|c| c != '\\' && c != '"'),
            preceded(satisfy(|c| c == '\\'), anychar),
        )))),
        tag("\""),
    )
    .map(AssemblyToken::String)
    .parse(input)
}

fn cash(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Cash, tag("$")).parse(input)
}
fn comma(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Comma, tag(",")).parse(input)
}
fn raw_char(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    preceded(
        tag("\\"),
        alt((
            value(' ', tag("Space")),
            value('\n', tag("Newline")),
            anychar,
        )),
    )
    .map(|c| AssemblyToken::Character(c as u8))
    .parse(input)
}
fn literal(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Hash, tag("#")).parse(input)
}
fn bang(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Bang, tag("!")).parse(input)
}

fn token(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    preceded(
        space0,
        alt((
            colon,
            bang,
            literal,
            raw_char,
            raw_string,
            quote,
            directive,
            number,
            register,
            openbracket,
            closebracket,
            plus,
            minus,
            comment,
            identifier,
            cash,
            comma,
            newline,
        )),
    )
    .parse(input)
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum TokenizeError<'a> {
    UnexpectedInput { remaining: &'a str },
}

pub fn tokenize<'a>(input: &'a str) -> Result<Vec<AssemblyToken<'a>>, TokenizeError<'a>> {
    let (rest, tokens) = terminated(many0(token), multispace0)
        .parse(input)
        .map_err(|_| TokenizeError::UnexpectedInput { remaining: input })?;

    if !rest.is_empty() {
        return Err(TokenizeError::UnexpectedInput { remaining: rest });
    }
    Ok(tokens)
}

#[cfg(test)]
mod test {
    use crate::cpu::assembler::{self, test::expected_tokens, tokenizer::*};

    #[test]
    fn test_tokenize() {
        let parsed = tokenize(assembler::test::SOURCE).unwrap();
        let expected = expected_tokens();
        for i in 0..parsed.len() {
            assert_eq!(parsed[i], expected[i])
        }
    }
}
