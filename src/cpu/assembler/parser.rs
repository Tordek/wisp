use nom::Err;
use sdl2::libc::sleep;

use crate::cpu::{
    self, TwoRegs,
    assembler::{
        parser,
        tokenizer::AssemblyToken::{self, Instruction},
    },
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RegSource<'input> {
    pub op1: cpu::Register,
    pub op2: Option<cpu::Register>,
    pub op3: Option<Native<'input>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MachSource<'input> {
    pub op1: cpu::MachineRegister,
    pub op2: Option<cpu::MachineRegister>,
    pub op3: Option<Reference<'input>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EitherSource<'input> {
    Mach(MachSource<'input>),
    Reg(RegSource<'input>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Location<'input> {
    Literal(Native<'input>),
    Absolute(Reference<'input>),
    Register(cpu::Register),
    Machine(cpu::MachineRegister),
    IndirectRegister(cpu::Register),
    IndirectMachine(cpu::MachineRegister, Reference<'input>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum JumpTarget<'input> {
    Absolute(Reference<'input>),
    Register(cpu::Register),
    Machine(cpu::MachineRegister, Reference<'input>),
    IndirectRegister(cpu::Register),
    IndirectMachine(cpu::MachineRegister, Reference<'input>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UnresolvedInstruction<'input> {
    Jump {
        condition: cpu::Condition,
        target: JumpTarget<'input>,
    },
    Int(Reference<'input>),
    Mov {
        dst: Location<'input>,
        src: Location<'input>,
    },
    Mov8 {
        dst: Location<'input>,
        src: Location<'input>,
    },
    MBinary {
        op: cpu::MBinaryOp,
        dst: cpu::MachineRegister,
        operands: MachSource<'input>,
    },
    IDiv {
        div: cpu::Register,
        rem: cpu::Register,
        operands: RegSource<'input>,
    },
    Binary {
        op: cpu::BinaryOp,
        dst: cpu::Register,
        operands: RegSource<'input>,
    },
    MComparison {
        op: cpu::Comparison,
        dst: cpu::Register,
        operands: EitherSource<'input>,
    },
    // MemCpy {
    //     dst: cpu::MachineRegister,
    //     src: cpu::MachineRegister,
    //     count: Reference,
    // },
    // MemSet {
    //     dst: cpu::MachineRegister,
    //     src: cpu::MachineRegister,
    //     count: Reference,
    // },
    Call {
        target: JumpTarget<'input>,
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
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AssemblyLine<'input> {
    Org(usize),
    UnresolvedData(Native<'input>),
    ResolvedData(Vec<u8>),
    ResolvedInstruction(cpu::Instruction),
    UnresolvedInstruction(UnresolvedInstruction<'input>),
    Label(&'input str),
}

#[derive(Debug)]
pub enum ParserError<'input> {
    Expected {
        expected: &'static str,
        found: Option<AssemblyToken<'input>>,
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
                found: self.peek().cloned(),
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

    fn try_label(&mut self) -> Option<AssemblyLine<'input>> {
        self.consume_if(|t| match t {
            AssemblyToken::Label(l) => Some(AssemblyLine::Label(*l)),
            _ => None,
        })
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

    fn try_reference_token(&mut self) -> Option<&'input str> {
        self.consume_if(|reference| match reference {
            AssemblyToken::Reference(r) => Some(*r),
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
    fn try_bang(&mut self) -> Option<()> {
        self.consume_if(|reference| match reference {
            AssemblyToken::Bang => Some(()),
            _ => None,
        })
    }

    fn try_reference(&mut self) -> Option<Reference<'input>> {
        let name = self.try_reference_token()?;
        Some(Reference::Unresolved(name))
    }

    fn try_any_machine_value(&mut self) -> Option<Reference<'input>> {
        let next = self.peek();
        match next {
            Some(AssemblyToken::Number(_)) => self.try_number().map(Reference::Resolved),
            Some(AssemblyToken::Reference(_)) => self.try_reference(),
            _ => None,
        }
    }

    fn expect_lisp_value(&mut self) -> Result<Native<'input>, ParserError<'input>> {
        self.consume_if(|t| match t {
            AssemblyToken::LispLiteral => Some(()),
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
                Ok(Native::Fixnum(Reference::Resolved(v as i64)))
            }
            Some(AssemblyToken::Reference(c)) => Ok(Native::Fixnum(Reference::Unresolved(c))),
            v => Err(ParserError::Expected {
                expected: "A value to convert",
                found: v.cloned(),
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
            Some(AssemblyToken::Reference(_)) => {
                let r = self.try_reference();
                let r = self.expect(r, "A reference")?;
                Ok(Native::Raw(r))
            }
            Some(AssemblyToken::LispLiteral) => self.expect_lisp_value(),
            v => Err(ParserError::Expected {
                expected: "A value",
                found: v.cloned(),
            }),
        }
    }

    fn expect_directive(&mut self, name: &'static str) -> Result<(), ParserError<'input>> {
        match self.next() {
            Some(AssemblyToken::Directive(actual)) if *actual == name => Ok(()),
            found => Err(ParserError::ExpectedDirective {
                expected: name,
                found: found.cloned(),
            }),
        }
    }
    fn expect_instruction(&mut self, name: &'static str) -> Result<(), ParserError<'input>> {
        match self.next() {
            Some(AssemblyToken::Instruction(actual)) if *actual == name => Ok(()),
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
        let lenblock = cpu::LispWord::fixnum(str.len() as u64);

        let mut encoded = lenblock.0.to_le_bytes().to_vec();
        encoded.extend(str.as_bytes().to_vec());
        Ok(AssemblyLine::ResolvedData(encoded))
    }
    fn parse_w(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("w")?;
        let contents = self.expect_any_value()?;
        Ok(AssemblyLine::UnresolvedData(contents))
    }
    fn try_directive_line(&mut self) -> Result<Option<AssemblyLine<'input>>, ParserError<'input>> {
        let directive = self.peek();

        match directive {
            Some(AssemblyToken::Directive("org")) => Ok(Some(self.parse_org()?)),
            Some(AssemblyToken::Directive("str")) => Ok(Some(self.parse_string()?)),
            Some(AssemblyToken::Directive("w")) => Ok(Some(self.parse_w()?)),
            Some(AssemblyToken::Directive(directive)) => {
                Err(ParserError::UnknownDirective { directive })
            }
            _ => return Ok(None),
        }
    }

    fn parse_nop(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("NOP")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Nop))
    }
    fn parse_halt(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("HALT")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Halt))
    }
    fn parse_return_op(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("RETURN")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Return))
    }
    fn parse_ireturn_op(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("IRETURN")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::IReturn))
    }
    fn parse_interrupt(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("INT")?;

        let target = self.try_any_machine_value();
        let target = self.expect(target, "The number of the interruption")?;

        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Int(target),
        ))
    }

    fn expect_jump_condition(&mut self) -> Result<cpu::Condition, ParserError<'input>> {
        let condition = self.next();
        match condition {
            Some(AssemblyToken::Instruction("JUMP")) => Ok(cpu::Condition::Always),
            Some(AssemblyToken::Instruction("JUMPIF")) => {
                let conditional = self.try_register();
                let conditional = self.expect(conditional, "A register to check")?;
                self.expect_comma()?;
                Ok(cpu::Condition::True(conditional))
            }
            Some(AssemblyToken::Instruction("JUMPIFNOT")) => {
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

    fn expect_r_andor_offset(
        &mut self,
    ) -> Result<(Option<cpu::Register>, Option<Native<'input>>), ParserError<'input>> {
        let mreg = self.try_register();
        match mreg {
            Some(reg) => {
                let plus = self.try_plus();
                let extra = if plus.is_some() {
                    Some(self.expect_any_value()?)
                } else {
                    None
                };
                Ok((Some(reg), extra))
            }
            None => {
                let off = self.expect_any_value()?;
                Ok((None, Some(off)))
            }
        }
    }
    fn expect_mr_andor_offset(
        &mut self,
    ) -> Result<(Option<cpu::MachineRegister>, Option<Reference<'input>>), ParserError<'input>>
    {
        let mreg = self.try_machine_register();
        match mreg {
            Some(reg) => {
                let plus = self.try_plus();
                let extra = if plus.is_some() {
                    let r = self.try_any_machine_value();
                    Some(self.expect(r, "an offset")?)
                } else {
                    None
                };
                Ok((Some(reg), extra))
            }
            None => {
                let off = self.try_any_machine_value();
                let off = self.expect(off, "An address")?;
                Ok((None, Some(off)))
            }
        }
    }
    fn expect_jump_target(&mut self) -> Result<JumpTarget<'input>, ParserError<'input>> {
        let register = self.try_register();
        if let Some(reg) = register {
            return Ok(JumpTarget::Register(reg));
        }

        let target = self.expect_mr_andor_offset()?;
        match target {
            (Some(r), Some(off)) => Ok(JumpTarget::IndirectMachine(r, off)),
            (Some(r), None) => Ok(JumpTarget::Machine(r, Reference::Resolved(0))),
            (None, Some(adr)) => Ok(JumpTarget::Absolute(adr)),
            (None, None) => Err(ParserError::Expected {
                expected: "a target to jump to",
                found: None,
            }),
        }
    }
    fn parse_jump(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let condition = self.expect_jump_condition()?;
        let jumptarget = self.expect_jump_target()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Jump {
                condition,
                target: jumptarget,
            },
        ))
    }
    fn parse_call(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("CALL")?;
        let target = self.expect_jump_target()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Call { target },
        ))
    }
    fn parse_push(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("PUSH")?;
        let reg = self.try_register();
        if let Some(r) = reg {
            return Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::PushR {
                src: r,
            }));
        }
        let mreg = self.try_machine_register();
        if let Some(r) = mreg {
            return Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::PushA {
                src: r,
            }));
        }
        let next = self.peek();
        Err(ParserError::Expected {
            expected: "A location to push from",
            found: next.cloned(),
        })
    }
    fn parse_pop(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("POP")?;
        let reg = self.try_register();
        if let Some(r) = reg {
            return Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::PopR {
                dst: r,
            }));
        }
        let mreg = self.try_machine_register();
        if let Some(r) = mreg {
            return Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::PopA {
                dst: r,
            }));
        }
        let next = self.peek();
        Err(ParserError::Expected {
            expected: "A location to pop to",
            found: next.cloned(),
        })
    }

    fn expect_location(&mut self) -> Result<Location<'input>, ParserError<'input>> {
        let register = self.try_register();
        if let Some(l) = register {
            return Ok(Location::Register(l));
        }

        let machinereg = self.try_machine_register();
        if let Some(m) = machinereg {
            return Ok(Location::Machine(m));
        }

        let relative = self.try_open_bracket();
        if let Some(()) = relative {
            let register = self.try_register();
            self.try_close_bracket();
            if let Some(l) = register {
                return Ok(Location::IndirectRegister(l));
            }

            let machinereg = self.try_machine_register();
            if let Some(m) = machinereg {
                let plus = self.try_plus();
                let extra = if plus.is_some() {
                    let r = self.try_any_machine_value();
                    self.expect(r, "an offset")?
                } else {
                    Reference::Resolved(0)
                };
                self.try_close_bracket();

                return Ok(Location::IndirectMachine(m, extra));
            }

            let literal = self.try_any_machine_value();
            let literal = self.expect(literal, "An address")?;
            self.try_close_bracket();
            return Ok(Location::Absolute(literal));
        }
        let literal = self.expect_any_value()?;
        return Ok(Location::Literal(literal));
    }

    fn parse_mov(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("MOV")?;
        let dst = self.expect_location()?;
        self.expect_comma()?;
        let src = self.expect_location()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Mov { dst, src },
        ))
    }
    fn parse_mov8(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("MOV8")?;
        let dst = self.expect_location()?;
        self.expect_comma()?;
        let src = self.expect_location()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Mov8 { dst, src },
        ))
    }

    fn parse_either_source(&mut self) -> Result<EitherSource<'input>, ParserError<'input>> {
        let source = self.peek();
        match source {
            Some(AssemblyToken::Register(_)) => self.parse_reg_source().map(EitherSource::Reg),
            Some(AssemblyToken::MachineRegister(_)) => {
                self.parse_mach_source().map(EitherSource::Mach)
            }

            other => Err(ParserError::Expected {
                expected: "A source",
                found: other.cloned(),
            }),
        }
    }

    fn parse_mach_source(&mut self) -> Result<MachSource<'input>, ParserError<'input>> {
        let op1 = self.try_machine_register();
        let op1 = self.expect(op1, "an operand")?;
        self.expect_comma()?;
        let (op2, op3) = self.expect_mr_andor_offset()?;
        Ok(MachSource { op1, op2, op3 })
    }
    fn parse_reg_source(&mut self) -> Result<RegSource<'input>, ParserError<'input>> {
        let op1 = self.try_register();
        let op1 = self.expect(op1, "an operand")?;
        self.expect_comma()?;
        let (op2, op3) = self.expect_r_andor_offset()?;
        Ok(RegSource { op1, op2, op3 })
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
                    Some(AssemblyToken::Instruction("ADD")) => cpu::BinaryOp::Add,
                    Some(AssemblyToken::Instruction("SUB")) => cpu::BinaryOp::Sub,
                    other => {
                        return Err(ParserError::Expected {
                            expected: "ADD or SUB",
                            found: other,
                        });
                    }
                };
                let operands = self.parse_reg_source()?;
                Ok(AssemblyLine::UnresolvedInstruction(
                    UnresolvedInstruction::Binary { op, dst, operands },
                ))
            }
            Some(AssemblyToken::MachineRegister(dst)) => {
                self.next();
                self.expect_comma()?;
                let op = match instr {
                    Some(AssemblyToken::Instruction("ADD")) => cpu::MBinaryOp::Add,
                    Some(AssemblyToken::Instruction("SUB")) => cpu::MBinaryOp::Sub,
                    other => {
                        return Err(ParserError::Expected {
                            expected: "ADD or SUB",
                            found: other,
                        });
                    }
                };
                let operands = self.parse_mach_source()?;
                Ok(AssemblyLine::UnresolvedInstruction(
                    UnresolvedInstruction::MBinary { op, dst, operands },
                ))
            }
            s => Err(ParserError::Expected {
                expected: "ADD or SUB",
                found: s,
            }),
        }
    }
    fn parse_settag(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("SETTAG")?;
        let dst = self.try_register();
        self.expect_comma()?;
        let dst = self.expect(dst, "A register")?;
        let src = self.try_machine_register();
        let src = self.expect(src, "A machine register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::SetTag { dst, src },
        ))
    }
    fn parse_gettag(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("GETTAG")?;
        let dst = self.try_machine_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::GetTag { dst, src },
        ))
    }
    fn parse_setpayload(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("SETPAYLOAD")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let src = self.try_machine_register();
        let src = self.expect(src, "A machine register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::SetPayload { dst, src },
        ))
    }
    fn parse_getpayload(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("GETPAYLOAD")?;
        let dst = self.try_machine_register();
        let dst = self.expect(dst, "A machine register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::GetPayload { dst, src },
        ))
    }
    fn parse_cons(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("CONS")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(
            0x03,
        )))
    }
    fn parse_uncons(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("UNCONS")?;
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
    fn parse_tworegs(&mut self) -> Result<cpu::TwoRegs, ParserError<'input>> {
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A register")?;
        Ok(TwoRegs { dst, src })
    }
    fn parse_car(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("CAR")?;
        let tworegs = self.parse_tworegs()?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Car(
            tworegs,
        )))
    }
    fn parse_cdr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("CDR")?;
        let tworegs = self.parse_tworegs()?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Cdr(
            tworegs,
        )))
    }
    fn parse_setcar(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("SETCAR")?;
        let tworegs = self.parse_tworegs()?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCar(
            tworegs,
        )))
    }
    fn parse_setcdr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("SETCDR")?;
        let tworegs = self.parse_tworegs()?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCdr(
            tworegs,
        )))
    }
    fn parse_bin(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let instr = self.peek().cloned();
        self.next();
        let dst = self.try_register();
        let dst = self.expect(dst, "A destination register")?;
        self.expect_comma()?;
        let op = match instr {
            Some(AssemblyToken::Instruction("MUL")) => cpu::BinaryOp::Mul,
            Some(AssemblyToken::Instruction("SHL")) => cpu::BinaryOp::Shl,
            Some(AssemblyToken::Instruction("SHR")) => cpu::BinaryOp::Shr,
            other => {
                return Err(ParserError::Expected {
                    expected: "A binary op",
                    found: other,
                });
            }
        };
        let operands = self.parse_reg_source()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::Binary { op, dst, operands },
        ))
    }
    fn parse_div(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("DIV")?;
        let div = self.try_register();
        let div = self.expect(div, "A destination register")?;
        self.expect_comma()?;
        let rem = self.try_register();
        let rem = self.expect(rem, "A destination register")?;
        self.expect_comma()?;
        let operands = self.parse_reg_source()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::IDiv { div, rem, operands },
        ))
    }
    fn parse_makeclosure(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("MAKECLOSURE")?;
        let dst = self.try_register();
        self.expect_comma()?;
        let dst = self.expect(dst, "A register")?;
        let code = self.try_machine_register();
        let code = self.expect(code, "A machine register")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::MakeClosure { dst, code },
        ))
    }
    fn parse_typep(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("TYPEP")?;
        let dst = self.try_register();
        let dst = self.expect(dst, "A place to store result")?;
        self.expect_comma()?;
        let src = self.try_register();
        let src = self.expect(src, "A source")?;
        self.expect_comma()?;
        let compare = self.try_number().map(|n| cpu::Native(n as u64));
        let compare = self.expect(compare, "A type")?;
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Typep {
            dst,
            src,
            compare,
        }))
    }

    fn try_machine_register(&mut self) -> Option<cpu::MachineRegister> {
        self.consume_if(|token| match token {
            AssemblyToken::MachineRegister(r) => Some(*r),
            _ => None,
        })
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
        self.expect_instruction("MEMCPY")?;
        let dst = self.try_machine_register();
        let dst = self.expect(dst, "Register for dst")?;
        self.expect_comma()?;
        let src = self.try_machine_register();
        let src = self.expect(src, "Register for src")?;
        self.expect_comma()?;
        let count = self.try_number();
        let count = self.expect(count, "Count")?;
        Ok(AssemblyLine::ResolvedInstruction(
            cpu::Instruction::MemCpy {
                dst,
                src,
                count: cpu::Count(count as u64),
            },
        ))
    }
    fn parse_comparison(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        let instr = self.peek().cloned();
        self.next();
        let dst = self.try_register();
        let dst = self.expect(dst, "A register")?;
        self.expect_comma()?;
        let op = match instr {
            Some(AssemblyToken::Instruction("EQ")) => cpu::Comparison::Eq,
            Some(AssemblyToken::Instruction("NE")) => cpu::Comparison::Ne,
            Some(AssemblyToken::Instruction("GT")) => cpu::Comparison::Gt,
            Some(AssemblyToken::Instruction("GTE")) => cpu::Comparison::Gte,
            Some(AssemblyToken::Instruction("LT")) => cpu::Comparison::Lt,
            Some(AssemblyToken::Instruction("LTE")) => cpu::Comparison::Lte,
            other => {
                return Err(ParserError::Expected {
                    expected: "ADD or SUB",
                    found: other,
                });
            }
        };
        let operands = self.parse_either_source()?;
        Ok(AssemblyLine::UnresolvedInstruction(
            UnresolvedInstruction::MComparison { op, dst, operands },
        ))
    }

    fn try_instruction_line(
        &mut self,
    ) -> Result<Option<AssemblyLine<'input>>, ParserError<'input>> {
        let instruction = self.peek();

        match instruction {
            Some(AssemblyToken::Instruction("NOP")) => Ok(Some(self.parse_nop()?)),
            Some(AssemblyToken::Instruction("HALT")) => Ok(Some(self.parse_halt()?)),
            Some(AssemblyToken::Instruction("RETURN")) => Ok(Some(self.parse_return_op()?)),
            Some(AssemblyToken::Instruction("IRETURN")) => Ok(Some(self.parse_ireturn_op()?)),

            Some(AssemblyToken::Instruction("INT")) => Ok(Some(self.parse_interrupt()?)),
            Some(AssemblyToken::Instruction("CALL")) => Ok(Some(self.parse_call()?)),
            Some(AssemblyToken::Instruction("JUMP")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Instruction("JUMPIF")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Instruction("JUMPIFNOT")) => Ok(Some(self.parse_jump()?)),

            Some(AssemblyToken::Instruction("PUSH")) => Ok(Some(self.parse_push()?)),
            Some(AssemblyToken::Instruction("POP")) => Ok(Some(self.parse_pop()?)),

            Some(AssemblyToken::Instruction("MOV")) => Ok(Some(self.parse_mov()?)),
            Some(AssemblyToken::Instruction("MOV8")) => Ok(Some(self.parse_mov8()?)),

            Some(AssemblyToken::Instruction("ADD")) => Ok(Some(self.parse_mbin()?)),
            Some(AssemblyToken::Instruction("SUB")) => Ok(Some(self.parse_mbin()?)),

            Some(AssemblyToken::Instruction("MUL")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Instruction("SHL")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Instruction("SHR")) => Ok(Some(self.parse_bin()?)),

            Some(AssemblyToken::Instruction("DIV")) => Ok(Some(self.parse_div()?)),

            Some(AssemblyToken::Instruction("EQ")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Instruction("NE")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Instruction("GT")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Instruction("GTE")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Instruction("LT")) => Ok(Some(self.parse_comparison()?)),
            Some(AssemblyToken::Instruction("LTE")) => Ok(Some(self.parse_comparison()?)),

            Some(AssemblyToken::Instruction("SETTAG")) => Ok(Some(self.parse_settag()?)),
            Some(AssemblyToken::Instruction("GETTAG")) => Ok(Some(self.parse_gettag()?)),
            Some(AssemblyToken::Instruction("SETPAYLOAD")) => Ok(Some(self.parse_setpayload()?)),
            Some(AssemblyToken::Instruction("GETPAYLOAD")) => Ok(Some(self.parse_getpayload()?)),

            Some(AssemblyToken::Instruction("CONS")) => Ok(Some(self.parse_cons()?)),
            Some(AssemblyToken::Instruction("UNCONS")) => Ok(Some(self.parse_uncons()?)),

            Some(AssemblyToken::Instruction("CAR")) => Ok(Some(self.parse_car()?)),
            Some(AssemblyToken::Instruction("CDR")) => Ok(Some(self.parse_cdr()?)),
            Some(AssemblyToken::Instruction("SETCAR")) => Ok(Some(self.parse_setcar()?)),
            Some(AssemblyToken::Instruction("SETCDR")) => Ok(Some(self.parse_setcdr()?)),

            Some(AssemblyToken::Instruction("MAKECLOSURE")) => Ok(Some(self.parse_makeclosure()?)),
            Some(AssemblyToken::Instruction("TYPEP")) => Ok(Some(self.parse_typep()?)),
            Some(AssemblyToken::Instruction("MEMCPY")) => Ok(Some(self.parse_memcpy()?)),
            Some(AssemblyToken::Instruction("COMPARISON")) => Ok(Some(self.parse_comparison()?)),

            Some(AssemblyToken::Instruction(instruction)) => {
                Err(ParserError::UnknownInstruction { instruction })
            }
            _ => Ok(None),
        }
    }

    fn try_line(&mut self) -> Result<Vec<AssemblyLine<'input>>, ParserError<'input>> {
        let mut parts = vec![];
        let label = self.try_label();
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

pub fn parser_new<'input>(
    tokens: &[AssemblyToken<'input>],
) -> Result<Vec<AssemblyLine<'input>>, ParserError<'input>> {
    let mut p = NParser {
        tokens,
        position: 0,
    };

    let r = p.runparse()?;
    Ok(r)
}

mod test {
    use crate::cpu::assembler::{parser, test::expected_tokens};

    #[test]
    fn test_parse() {
        let tokens = expected_tokens();
        parser::parser_new(&tokens).unwrap();
    }
}
