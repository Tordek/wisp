use crate::cpu::{
    self,
    assembler::tokenizer::AssemblyToken::{self},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RegAndOff<'input> {
    pub op1: cpu::Register,
    pub off: Option<Native<'input>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RValue<'input> {
    Literal(Native<'input>),
    Absolute(Native<'input>),
    Register(RegAndOff<'input>),
    Indirect(RegAndOff<'input>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LValue<'input> {
    Absolute(Native<'input>),
    Register(cpu::Register),
    Indirect(RegAndOff<'input>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UnresolvedInstruction<'input> {
    Jump {
        condition: cpu::Condition,
        target: RValue<'input>,
    },
    Int(Native<'input>),
    Mov {
        dst: LValue<'input>,
        src: RValue<'input>,
    },
    Mov8 {
        dst: LValue<'input>,
        src: RValue<'input>,
    },
    MBinary {
        op: cpu::MBinaryOp,
        dst: cpu::Register,
        op1: cpu::Register,
        op2: RValue<'input>,
    },
    IDiv {
        div: cpu::Register,
        rem: cpu::Register,
        op1: cpu::Register,
        op2: RValue<'input>,
    },
    Binary {
        op: cpu::BinaryOp,
        dst: cpu::Register,
        op1: cpu::Register,
        op2: RValue<'input>,
    },
    MComparison {
        op: cpu::Comparison,
        dst: cpu::Register,
        op1: cpu::Register,
        op2: RValue<'input>,
    },
    MemCpy {
        dst: cpu::Register,
        src: cpu::Register,
        count: RValue<'input>,
    },
    // MemSet {
    //     dst: cpu::Register,
    //     src: cpu::Register,
    //     count: Native,
    // },
    Call {
        target: RValue<'input>,
    },
    Typep {
        dst: cpu::Register,
        src: cpu::Register,
        compare: Native<'input>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Reference<'input> {
    Unresolved(&'input str),
    Resolved(i64),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Native<'input> {
    Raw(Reference<'input>),
    Char(Reference<'input>),
    Fixnum(Reference<'input>),
    Cons(Reference<'input>),
    Symbol(Reference<'input>),
    String(Reference<'input>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AssemblyLine<'input> {
    Equ(&'input str, usize),
    Org(usize),
    UnresolvedData(Native<'input>),
    ResolvedData(Vec<u8>),
    ResolvedInstruction(cpu::Instruction),
    UnresolvedInstruction(UnresolvedInstruction<'input>),
    Label(&'input str),
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ParserError<'input> {
    InvalidEscape {
        c: char,
        raw: &'input str,
    },
    Expected {
        expected: &'static str,
        rest: Vec<AssemblyToken<'input>>,
    },
    ExpectedDirective {
        expected: &'static str,
        found: Option<AssemblyToken<'input>>,
    },
    ExpectedInstruction {
        expected: &'static str,
        found: Option<AssemblyToken<'input>>,
    },
    UnknownDirective {
        directive: &'input str,
    },
    UnknownInstruction {
        instruction: &'input str,
    },
    InvalidInstruction,
}

fn unescape<'input>(input: &'input str) -> Result<String, ParserError<'input>> {
    let mut result = String::new();
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        result.push(if c == '\\' {
            match chars.next() {
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                Some('\\') => '\\',
                Some('"') => '"',
                Some(c) => {
                    return Err(ParserError::InvalidEscape { c, raw: input });
                }
                None => {
                    return Err(ParserError::InvalidEscape {
                        c: '\\',
                        raw: input,
                    });
                }
            }
        } else {
            c
        })
    }

    Ok(result)
}

struct NParser<'tokens, 'input> {
    tokens: &'tokens [AssemblyToken<'input>],
    position: usize,
}
impl<'tokens, 'input> NParser<'tokens, 'input> {
    fn expect<T>(
        &self,
        value: Option<T>,
        expected: &'static str,
    ) -> Result<T, ParserError<'input>> {
        match value {
            Some(v) => Ok(v),
            None => Err(ParserError::Expected {
                expected,
                rest: self.tokens[self.position..].to_vec(),
            }),
        }
    }
    fn peek(&self) -> Option<&AssemblyToken<'input>> {
        if self.position < self.tokens.len() {
            Some(&self.tokens[self.position])
        } else {
            None
        }
    }

    fn next(&mut self) -> Option<&AssemblyToken<'input>> {
        if self.position < self.tokens.len() {
            let res = Some(&self.tokens[self.position]);
            self.position += 1;
            res
        } else {
            None
        }
    }
    fn consume_if<T>(&mut self, f: impl FnOnce(&AssemblyToken<'input>) -> Option<T>) -> Option<T> {
        let value = self.tokens.get(self.position).and_then(f)?;
        self.position += 1;
        Some(value)
    }

    fn try_plus(&mut self) -> Option<()> {
        self.consume_if(|t| match t {
            AssemblyToken::Plus => Some(()),
            _ => None,
        })
    }
    fn try_number(&mut self) -> Option<i64> {
        self.consume_if(|t| match t {
            AssemblyToken::Number(n) => Some(*n),
            _ => None,
        })
    }

    fn try_register(&mut self) -> Option<cpu::Register> {
        self.consume_if(|t| match t {
            AssemblyToken::Register(l) => Some(*l),
            _ => None,
        })
    }
    fn try_open_bracket(&mut self) -> Option<()> {
        self.consume_if(|t| match t {
            AssemblyToken::OpenBracket => Some(()),
            _ => None,
        })
    }
    fn try_close_bracket(&mut self) -> Option<()> {
        self.consume_if(|t| match t {
            AssemblyToken::CloseBracket => Some(()),
            _ => None,
        })
    }
    fn try_colon(&mut self) -> Option<()> {
        self.consume_if(|t| match t {
            AssemblyToken::Colon => Some(()),
            _ => None,
        })
    }

    fn try_label(&mut self) -> Option<&'input str> {
        let refr = self.consume_if(|t| match t {
            AssemblyToken::Identifier(name) => Some(*name),
            _ => None,
        })?;
        let colon = self.try_colon();
        match colon {
            Some(()) => Some(refr),
            None => {
                self.position -= 1;
                None
            }
        }
    }

    fn try_label_line(&mut self) -> Option<AssemblyLine<'input>> {
        self.try_label().map(AssemblyLine::Label)
    }

    fn try_comment(&mut self) -> Option<()> {
        self.consume_if(|t| match t {
            AssemblyToken::Comment(_) => Some(()),
            _ => None,
        })
    }

    fn try_end(&mut self) -> Option<()> {
        let end = self.peek();
        match end {
            None => Some(()),
            Some(AssemblyToken::Newline) => {
                self.next();
                Some(())
            }
            _ => None,
        }
    }

    fn try_quote(&mut self) -> Option<()> {
        self.consume_if(|reference| match reference {
            AssemblyToken::Quote => Some(()),
            _ => None,
        })
    }

    fn try_identifier(&mut self) -> Option<&'input str> {
        self.consume_if(|reference| match reference {
            AssemblyToken::Identifier(r) => Some(*r),
            _ => None,
        })
    }

    fn try_char(&mut self) -> Option<u8> {
        self.consume_if(|reference| match reference {
            AssemblyToken::Character(c) => Some(*c),
            _ => None,
        })
    }
    fn try_string(&mut self) -> Option<&'input str> {
        self.consume_if(|reference| match reference {
            AssemblyToken::String(s) => Some(*s),
            _ => None,
        })
    }

    fn try_reference(&mut self) -> Option<Reference<'input>> {
        self.try_quote()?;
        let name = self.try_identifier();

        match name {
            Some(name) => Some(Reference::Unresolved(name)),
            None => {
                self.position -= 1;
                None
            }
        }
    }

    fn try_any_machine_value(&mut self) -> Option<Reference<'input>> {
        let next = self.peek();
        match next {
            Some(AssemblyToken::Number(_)) => self.try_number().map(Reference::Resolved),
            Some(AssemblyToken::Character(_)) => {
                self.try_char().map(|c| Reference::Resolved(c as i64))
            }
            Some(AssemblyToken::Quote) => self.try_reference(),
            _ => None,
        }
    }

    fn expect_lisp_value(&mut self) -> Result<Native<'input>, ParserError<'input>> {
        self.consume_if(|t| match t {
            AssemblyToken::Hash => Some(()),
            _ => None,
        });

        let next = self.peek();
        match next {
            Some(AssemblyToken::Character(c)) => {
                let v = *c;
                self.next();
                Ok(Native::Char(Reference::Resolved(v as i64)))
            }
            Some(AssemblyToken::OpenBracket) => {
                self.next();
                let v = self.try_any_machine_value();
                let v = self.expect(v, "A value to encode")?;
                Ok(Native::Cons(v))
            }
            Some(AssemblyToken::Bang) => {
                self.next();
                let v = self.try_any_machine_value();
                let v = self.expect(v, "A value to encode")?;
                Ok(Native::Symbol(v))
            }
            Some(AssemblyToken::Number(n)) => {
                let v = *n;
                self.next();
                Ok(Native::Fixnum(Reference::Resolved(v)))
            }
            Some(AssemblyToken::Quote) => {
                self.next();
                let refr = self.try_identifier();
                let refr = self.expect(refr, "A reference name")?;
                Ok(Native::Fixnum(Reference::Unresolved(refr)))
            }
            Some(AssemblyToken::Cash) => {
                self.next();
                let refr = self.try_any_machine_value();
                let refr = self.expect(refr, "A reference name")?;
                Ok(Native::String(refr))
            }
            _ => Err(ParserError::Expected {
                expected: "A value to convert",
                rest: self.tokens[self.position..].to_vec(),
            }),
        }
    }

    fn expect_any_value(&mut self) -> Result<Native<'input>, ParserError<'input>> {
        let next = self.peek();
        match next {
            Some(AssemblyToken::Number(_)) => {
                let n = self.try_number();
                let n = self.expect(n, "A number")?;
                Ok(Native::Raw(Reference::Resolved(n)))
            }
            Some(AssemblyToken::Quote) => {
                let r = self.try_reference();
                let r = self.expect(r, "A reference")?;
                Ok(Native::Raw(r))
            }
            Some(AssemblyToken::Character(_)) => {
                let r = self.try_char();
                let r = self.expect(r, "A reference")?;
                Ok(Native::Raw(Reference::Resolved(r as i64)))
            }
            Some(AssemblyToken::Hash) => self.expect_lisp_value(),
            _ => Err(ParserError::Expected {
                expected: "Any value",
                rest: self.tokens[self.position..].to_vec(),
            }),
        }
    }

    fn expect_directive(&mut self, name: &'static str) -> Result<(), ParserError<'input>> {
        match self.next() {
            Some(AssemblyToken::Identifier(actual)) if *actual == name => Ok(()),
            found => Err(ParserError::ExpectedDirective {
                expected: name,
                found: found.cloned(),
            }),
        }
    }
    fn expect_identifier(&mut self, name: &'static str) -> Result<(), ParserError<'input>> {
        match self.next() {
            Some(AssemblyToken::Identifier(actual)) if *actual == name => Ok(()),
            found => Err(ParserError::ExpectedInstruction {
                expected: name,
                found: found.cloned(),
            }),
        }
    }
    fn parse_org(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("org")?;
        let position = self.try_number();
        let position = self.expect(position, "an address")?;
        Ok(AssemblyLine::Org(position as usize))
    }
    fn parse_string(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("str")?;
        let str = self.try_string();
        let str = self.expect(str, "A string")?;

        let unescaped = unescape(str)?;
        let lenblock = cpu::LispWord::fixnum(unescaped.len() as i64);

        let mut encoded = lenblock.0.to_le_bytes().to_vec();
        encoded.extend(unescaped.as_bytes().to_vec());
        Ok(AssemblyLine::ResolvedData(encoded))
    }
    fn parse_w(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("w")?;
        let contents = self.expect_any_value()?;
        Ok(AssemblyLine::UnresolvedData(contents))
    }
    fn parse_eq(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("equ")?;
        let name = self.try_label();
        let name = self.expect(name, "A label to assign.")?;
        let value = self.try_number();
        let value = self.expect(value, "A literal value")?;
        Ok(AssemblyLine::Equ(name, value as usize))
    }
    fn try_directive_line(&mut self) -> Result<Option<AssemblyLine<'input>>, ParserError<'input>> {
        let directive_marker = self.consume_if(|d| match d {
            AssemblyToken::Dot => Some(()),
            _ => None,
        });
        if directive_marker.is_none() {
            return Ok(None);
        }
        let directive = self.peek();

        match directive {
            Some(AssemblyToken::Identifier("org")) => Ok(Some(self.parse_org()?)),
            Some(AssemblyToken::Identifier("str")) => Ok(Some(self.parse_string()?)),
            Some(AssemblyToken::Identifier("w")) => Ok(Some(self.parse_w()?)),
            Some(AssemblyToken::Identifier("equ")) => Ok(Some(self.parse_eq()?)),
            Some(AssemblyToken::Identifier(directive)) => {
                Err(ParserError::UnknownDirective { directive })
            }
            _ => Ok(None),
        }
    }

    fn parse_nullary_op(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let identifier = self.try_identifier();
        let identifier = self.expect(identifier, "An identifier")?;
        Ok(AssemblyLine::ResolvedInstruction(match identifier {
            "NOP" => cpu::Instruction::Nop,
            "RETURN" => cpu::Instruction::Return,
            "IRETURN" => cpu::Instruction::IReturn,
            "HALT" => cpu::Instruction::Halt,
            "EI" => cpu::Instruction::EnableInterrupts,
            "DI" => cpu::Instruction::DisableInterrupts,
            _ => {
                return Err(ParserError::Expected {
                    expected: "A nullary op",
                    rest: self.tokens[self.position..].to_vec(),
                });
            }
        }))
    }
    fn parse_interrupt(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("INT")?;
        let target = self.expect_any_value()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Int(target),
        ))
    }

    fn expect_jump_condition(&mut self) -> Result<cpu::Condition, ParserError<'input>> {
        let condition = self.next();
        match condition {
            Some(AssemblyToken::Identifier("JUMP")) => Ok(cpu::Condition::Always),
            Some(AssemblyToken::Identifier("JUMPIF")) => {
                let conditional = self.try_register();
                let conditional = self.expect(conditional, "A register to check")?;
                self.expect_comma()?;
                Ok(cpu::Condition::True(conditional))
            }
            Some(AssemblyToken::Identifier("JUMPIFNOT")) => {
                let conditional = self.try_register();
                let conditional = self.expect(conditional, "A register to check")?;
                self.expect_comma()?;
                Ok(cpu::Condition::False(conditional))
            }
            _ => Err(ParserError::ExpectedInstruction {
                expected: "A jump",
                found: condition.cloned(),
            }),
        }
    }

    fn expect_r_and_offset(&mut self) -> Result<RegAndOff<'input>, ParserError<'input>> {
        let mreg = self.try_register();
        let mreg = self.expect(mreg, "A register")?;
        let plus = self.try_plus();
        let extra = if plus.is_some() {
            Some(self.expect_any_value()?)
        } else {
            None
        };
        Ok(RegAndOff {
            op1: mreg,
            off: extra,
        })
    }

    fn expect_lvalue(&mut self) -> Result<LValue<'input>, ParserError<'input>> {
        let register = self.try_register();
        if let Some(reg) = register {
            return Ok(LValue::Register(reg));
        }

        let indirect = self.try_open_bracket();
        self.expect(indirect, "An indirect address")?;

        let reg = self.try_register();
        Ok(match reg {
            None => {
                let absolute = self.expect_any_value()?;
                let close = self.try_close_bracket();
                self.expect(close, "Close bracket")?;
                LValue::Absolute(absolute)
            }
            Some(_) => {
                self.position -= 1;
                let r = self.expect_r_and_offset()?;
                let close = self.try_close_bracket();
                self.expect(close, "Close bracket")?;
                LValue::Indirect(r)
            }
        })
    }

    fn parse_jump(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let condition = self.expect_jump_condition()?;
        let target = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Jump { condition, target },
        ))
    }
    fn parse_call(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("CALL")?;
        let target = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Call { target },
        ))
    }
    fn parse_push(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("PUSH")?;
        let reg = self.try_register();
        if let Some(r) = reg {
            return Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Push {
                src: r,
            }));
        }
        Err(ParserError::Expected {
            expected: "A location to push from",
            rest: self.tokens[self.position..].to_vec(),
        })
    }
    fn parse_pop(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("POP")?;
        let reg = self.try_register();
        if let Some(r) = reg {
            return Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Pop {
                dst: r,
            }));
        }
        Err(ParserError::Expected {
            expected: "A location to pop to",
            rest: self.tokens[self.position..].to_vec(),
        })
    }

    fn expect_rvalue(&mut self) -> Result<RValue<'input>, ParserError<'input>> {
        let indirect = self.try_open_bracket();
        if indirect.is_none() {
            let register = self.try_register();
            if register.is_some() {
                self.position -= 1;
                let r = self.expect_r_and_offset()?;
                return Ok(RValue::Register(r));
            }

            let absolute = self.expect_any_value()?;
            return Ok(RValue::Literal(absolute));
        }

        let reg = self.try_register();
        match reg {
            None => {
                let v = self.expect_any_value()?;
                let close = self.try_close_bracket();
                self.expect(close, "Close bracket")?;
                Ok(RValue::Absolute(v))
            }
            Some(_) => {
                self.position -= 1;
                let r = self.expect_r_and_offset()?;
                let close = self.try_close_bracket();
                self.expect(close, "Close bracket")?;
                Ok(RValue::Indirect(r))
            }
        }
    }

    fn parse_mov(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("MOV")?;
        let dst = self.expect_lvalue()?;
        self.expect_comma()?;
        let src = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Mov { dst, src },
        ))
    }
    fn parse_mov8(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("MOV8")?;
        let dst = self.expect_lvalue()?;
        self.expect_comma()?;
        let src = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Mov8 { dst, src },
        ))
    }

    fn parse_mbin(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let instr = self.peek().cloned();
        self.next();
        let dst = self.peek().cloned();
        match dst {
            Some(AssemblyToken::Register(dst)) => {
                self.next();
                self.expect_comma()?;
                let op = match instr {
                    Some(AssemblyToken::Identifier("AADD")) => cpu::MBinaryOp::Add,
                    Some(AssemblyToken::Identifier("ASUB")) => cpu::MBinaryOp::Sub,
                    _ => {
                        return Err(ParserError::Expected {
                            expected: "ADD or SUB",
                            rest: self.tokens[self.position..].to_vec(),
                        });
                    }
                };
                let op1 = self.try_register();
                let op1 = self.expect(op1, "A source")?;
                self.expect_comma()?;
                let op2 = self.expect_rvalue()?;
                Ok(AssemblyLine::UnresolvedInstruction(
                    UnresolvedInstruction::MBinary { op, dst, op1, op2 },
                ))
            }
            _ => Err(ParserError::Expected {
                expected: "ADD or SUB",
                rest: self.tokens[self.position..].to_vec(),
            }),
        }
    }
    fn parse_settag(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("SETTAG")?;
        let dst = self.try_register();
        self.expect_comma()?;
        let dst = self.expect(dst, "A register")?;
        let src = self.try_register();
        let src = self.expect(src, "A machine register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::SetTag { dst, src },
        ))
    }
    fn parse_gettag(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("GETTAG")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::GetTag { dst, src },
        ))
    }
    fn parse_setpayload(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("SETPAYLOAD")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A machine register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::SetPayload { dst, src },
        ))
    }
    fn parse_getpayload(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("GETPAYLOAD")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::GetPayload { dst, src },
        ))
    }
    fn parse_req(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("REQ")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let prototype = self.try_register();
        let prototype = self.expect(prototype, "A register")?;
        self.expect_comma()?;
        let size = self.try_register();
        let size = self.expect(size, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Req {
            dst,
            prototype,
            size,
        }))
    }
    fn parse_cons(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("CONS")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let car = self.try_register();
        let car = self.expect(car, "A register")?;
        self.expect_comma()?;
        let cdr = self.try_register();
        let cdr = self.expect(cdr, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Cons {
            dst,
            car,
            cdr,
        }))
    }
    fn parse_uncons(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("UNCONS")?;
        let car = self.try_register();
        let car = self.expect(car, "A register")?;
        self.expect_comma()?;
        let cdr = self.try_register();
        let cdr = self.expect(cdr, "A register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::Uncons { car, cdr, src },
        ))
    }
    fn parse_car(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("CAR")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Car {
            dst,
            src,
        }))
    }
    fn parse_cdr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("CDR")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Cdr {
            dst,
            src,
        }))
    }
    fn parse_setcar(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("SETCAR")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::SetCar { dst, src },
        ))
    }
    fn parse_setcdr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("SETCDR")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::SetCdr { dst, src },
        ))
    }
    fn parse_bin(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let instr = self.peek().cloned();
        self.next();
        let dst = self.try_register();
        let dst = self.expect(dst, "A destination register")?;
        self.expect_comma()?;
        let op = match instr {
            Some(AssemblyToken::Identifier("ADD")) => cpu::BinaryOp::Add,
            Some(AssemblyToken::Identifier("SUB")) => cpu::BinaryOp::Sub,
            Some(AssemblyToken::Identifier("MUL")) => cpu::BinaryOp::Mul,
            Some(AssemblyToken::Identifier("SHL")) => cpu::BinaryOp::Shl,
            Some(AssemblyToken::Identifier("SHR")) => cpu::BinaryOp::Shr,
            _ => {
                return Err(ParserError::Expected {
                    expected: "A binary op",
                    rest: self.tokens[self.position..].to_vec(),
                });
            }
        };
        let op1 = self.try_register();
        let op1 = self.expect(op1, "A source")?;
        self.expect_comma()?;
        let op2 = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Binary { op, dst, op1, op2 },
        ))
    }
    fn parse_div(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("DIV")?;
        let div = self.try_register();
        let div = self.expect(div, "A destination register")?;
        self.expect_comma()?;
        let rem = self.try_register();
        let rem = self.expect(rem, "A destination register")?;
        self.expect_comma()?;
        let op1 = self.try_register();
        let op1 = self.expect(op1, "A source")?;
        self.expect_comma()?;
        let op2 = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::IDiv { div, rem, op1, op2 },
        ))
    }
    fn parse_makeclosure(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("MAKECLOSURE")?;
        let dst = self.try_register();
        self.expect_comma()?;
        let dst = self.expect(dst, "A register")?;
        let code = self.try_register();
        let code = self.expect(code, "A machine register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::MakeClosure { dst, code },
        ))
    }
    fn parse_typep(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("TYPEP")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A place to store result")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A source")?;
        self.expect_comma()?;
        let compare = self.expect_any_value()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Typep { dst, src, compare },
        ))
    }

    fn try_comma(&mut self) -> Option<()> {
        self.consume_if(|token| match token {
            AssemblyToken::Comma => Some(()),
            _ => None,
        })
    }
    fn expect_comma(&mut self) -> Result<(), ParserError<'input>> {
        let comma = self.try_comma();
        self.expect(comma, "operand separator")
    }

    fn parse_memcpy(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_identifier("MEMCPY")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "Register for dst")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "Register for src")?;
        self.expect_comma()?;
        let count = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::MemCpy { dst, src, count },
        ))
    }
    fn parse_comparison(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let instr = self.peek().cloned();
        self.next();
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let op = match instr {
            Some(AssemblyToken::Identifier("EQ")) => cpu::Comparison::Eq,
            Some(AssemblyToken::Identifier("NE")) => cpu::Comparison::Ne,
            Some(AssemblyToken::Identifier("GT")) => cpu::Comparison::Gt,
            Some(AssemblyToken::Identifier("GTE")) => cpu::Comparison::Gte,
            Some(AssemblyToken::Identifier("LT")) => cpu::Comparison::Lt,
            Some(AssemblyToken::Identifier("LTE")) => cpu::Comparison::Lte,
            _ => {
                return Err(ParserError::Expected {
                    expected: "A comparison",
                    rest: self.tokens[self.position..].to_vec(),
                });
            }
        };
        let op1 = self.try_register();
        let op1 = self.expect(op1, "A source")?;
        self.expect_comma()?;
        let op2 = self.expect_rvalue()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::MComparison { op, dst, op1, op2 },
        ))
    }

    fn try_instruction_line(
        &mut self,
    ) -> Result<Option<AssemblyLine<'input>>, ParserError<'input>> {
        let instruction = self.peek();

        match instruction {
            Some(AssemblyToken::Identifier("NOP")) => Ok(Some(self.parse_nullary_op()?)),
            Some(AssemblyToken::Identifier("HALT")) => Ok(Some(self.parse_nullary_op()?)),
            Some(AssemblyToken::Identifier("RETURN")) => Ok(Some(self.parse_nullary_op()?)),
            Some(AssemblyToken::Identifier("IRETURN")) => Ok(Some(self.parse_nullary_op()?)),
            Some(AssemblyToken::Identifier("DI")) => Ok(Some(self.parse_nullary_op()?)),
            Some(AssemblyToken::Identifier("EI")) => Ok(Some(self.parse_nullary_op()?)),

            Some(AssemblyToken::Identifier("INT")) => Ok(Some(self.parse_interrupt()?)),
            Some(AssemblyToken::Identifier("CALL")) => Ok(Some(self.parse_call()?)),
            Some(AssemblyToken::Identifier("JUMP")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Identifier("JUMPIF")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Identifier("JUMPIFNOT")) => Ok(Some(self.parse_jump()?)),

            Some(AssemblyToken::Identifier("PUSH")) => Ok(Some(self.parse_push()?)),
            Some(AssemblyToken::Identifier("POP")) => Ok(Some(self.parse_pop()?)),

            Some(AssemblyToken::Identifier("MOV")) => Ok(Some(self.parse_mov()?)),
            Some(AssemblyToken::Identifier("MOV8")) => Ok(Some(self.parse_mov8()?)),

            Some(AssemblyToken::Identifier("AADD")) => Ok(Some(self.parse_mbin()?)),
            Some(AssemblyToken::Identifier("ASUB")) => Ok(Some(self.parse_mbin()?)),

            Some(AssemblyToken::Identifier("ADD")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Identifier("SUB")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Identifier("MUL")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Identifier("SHL")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Identifier("SHR")) => Ok(Some(self.parse_bin()?)),

            Some(AssemblyToken::Identifier("DIV")) => Ok(Some(self.parse_div()?)),

            Some(AssemblyToken::Identifier("EQ")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Identifier("NE")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Identifier("GT")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Identifier("GTE")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Identifier("LT")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Identifier("LTE")) => Ok(Some(self.parse_comparison()?)),

            Some(AssemblyToken::Identifier("SETTAG")) => Ok(Some(self.parse_settag()?)),
            Some(AssemblyToken::Identifier("GETTAG")) => Ok(Some(self.parse_gettag()?)),
            Some(AssemblyToken::Identifier("SETPAYLOAD")) => Ok(Some(self.parse_setpayload()?)),
            Some(AssemblyToken::Identifier("GETPAYLOAD")) => Ok(Some(self.parse_getpayload()?)),

            Some(AssemblyToken::Identifier("REQ")) => Ok(Some(self.parse_req()?)),
            Some(AssemblyToken::Identifier("CONS")) => Ok(Some(self.parse_cons()?)),
            Some(AssemblyToken::Identifier("UNCONS")) => Ok(Some(self.parse_uncons()?)),

            Some(AssemblyToken::Identifier("CAR")) => Ok(Some(self.parse_car()?)),
            Some(AssemblyToken::Identifier("CDR")) => Ok(Some(self.parse_cdr()?)),
            Some(AssemblyToken::Identifier("SETCAR")) => Ok(Some(self.parse_setcar()?)),
            Some(AssemblyToken::Identifier("SETCDR")) => Ok(Some(self.parse_setcdr()?)),

            Some(AssemblyToken::Identifier("MAKECLOSURE")) => Ok(Some(self.parse_makeclosure()?)),
            Some(AssemblyToken::Identifier("TYPEP")) => Ok(Some(self.parse_typep()?)),
            Some(AssemblyToken::Identifier("MEMCPY")) => Ok(Some(self.parse_memcpy()?)),
            Some(AssemblyToken::Identifier("COMPARISON")) => Ok(Some(self.parse_comparison()?)),

            Some(AssemblyToken::Identifier(instruction)) => {
                Err(ParserError::UnknownInstruction { instruction })
            }
            _ => Ok(None),
        }
    }

    fn try_line(&mut self) -> Result<Vec<AssemblyLine<'input>>, ParserError<'input>> {
        let mut parts = vec![];
        let label = self.try_label_line();
        let dir = self.try_directive_line()?;
        let instr = self.try_instruction_line()?;
        self.try_comment();
        let eol = self.try_end();
        self.expect(eol, "End of line")?;

        if let Some(label) = label {
            parts.push(label);
        }
        if let Some(dir) = dir {
            parts.push(dir);
        }
        if let Some(instr) = instr {
            parts.push(instr);
        }

        Ok(parts)
    }

    fn runparse(&mut self) -> Result<Vec<AssemblyLine<'input>>, ParserError<'input>> {
        let mut lines = vec![];
        while self.position < self.tokens.len() {
            lines.extend(self.try_line()?);
        }
        Ok(lines)
    }
}

pub fn parse<'input>(
    tokens: &[AssemblyToken<'input>],
) -> Result<Vec<AssemblyLine<'input>>, ParserError<'input>> {
    let mut p = NParser {
        tokens,
        position: 0,
    };

    let r = p.runparse()?;
    Ok(r)
}

#[cfg(test)]
mod test {
    use crate::cpu::assembler::{
        parser,
        test::{expected_lines, expected_tokens},
    };

    #[test]
    fn test_parse() {
        let tokens = expected_tokens();
        let ast = parser::parse(&tokens).unwrap();
        let expected = expected_lines();
        for i in 0..ast.len().min(expected.len()) {
            assert_eq!(ast[i], expected[i]);
        }
        assert_eq!(ast, expected);
    }
}
