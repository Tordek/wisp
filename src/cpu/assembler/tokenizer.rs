use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_until},
    character::{
        anychar,
        complete::{alpha1, alphanumeric1, digit1, hex_digit1, multispace0, space0},
    },
    combinator::{recognize, value},
    multi::{many0, many0_count},
    sequence::{delimited, pair, preceded, terminated},
};

use crate::cpu;

#[derive(Clone, PartialEq, Debug)]
pub enum AssemblyToken<'a> {
    Colon,
    Dot,
    Number(i64),
    Register(cpu::Register),
    MachineRegister(cpu::MachineRegister),
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
    preceded(tag("R"), decimal)
        .map(|c| AssemblyToken::Register(cpu::Register(c as u8)))
        .parse(input)
}
fn machineregister(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    alt((
        preceded(tag("A"), decimal)
            .map(|c| AssemblyToken::MachineRegister(cpu::MachineRegister(c as u8))),
        value(AssemblyToken::MachineRegister(cpu::Cpu::PC), tag("PC")),
        value(AssemblyToken::MachineRegister(cpu::Cpu::SP), tag("SP")),
        value(AssemblyToken::MachineRegister(cpu::Cpu::VBR), tag("VBR")),
    ))
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
fn instruction(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    ident.map(AssemblyToken::Identifier).parse(input)
}
fn label(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Colon, tag(":")).parse(input)
}
fn reference(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    value(AssemblyToken::Quote, tag("'")).parse(input)
}
fn raw_string(input: &str) -> IResult<&str, AssemblyToken<'_>> {
    delimited(tag("\""), recognize(take_until("\"")), tag("\""))
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
            label,
            bang,
            literal,
            raw_char,
            raw_string,
            reference,
            directive,
            number,
            register,
            machineregister,
            openbracket,
            closebracket,
            plus,
            minus,
            comment,
            instruction,
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
