use crate::cpu::{self, assembler::tokenizer::AssemblyToken};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RegSource<'input> {
    pub op1: cpu::Register,
    pub op2: Option<cpu::Register>,
    pub op3: Option<Reference<'input>>,
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
    Literal(Reference<'input>),
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
        op1: cpu::Register,
        op2: Option<cpu::Register>,
        op3: Option<Reference<'input>>,
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
    UnresolvedNeg(&'input str),
    UnresolvedPos(&'input str),
    Resolved(i64),
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Data<'input> {
    Symbol(Reference<'input>),
    Literal(Reference<'input>),
    Cons(Reference<'input>, Reference<'input>),
}
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AssemblyLine<'input> {
    Ord(usize),
    UnresolvedData(Data<'input>),
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

    fn try_number(&mut self) -> Option<i64> {
        self.consume_if(|t| match t {
            AssemblyToken::Number(n) => Some(*n),
            _ => None,
        })
    }

    fn try_instruction(&mut self) -> Option<&'input str> {
        self.consume_if(|t| match t {
            AssemblyToken::Instruction(l) => Some(*l),
            _ => None,
        })
    }

    fn try_directive(&mut self) -> Option<&'input str> {
        self.consume_if(|t| match t {
            AssemblyToken::Directive(l) => Some(*l),
            _ => None,
        })
    }
    fn try_register(&mut self) -> Option<cpu::Register> {
        self.consume_if(|t| match t {
            AssemblyToken::Register(l) => Some(*l),
            _ => None,
        })
    }

    fn try_mregister(&mut self) -> Option<cpu::MachineRegister> {
        self.consume_if(|t| match t {
            AssemblyToken::MachineRegister(l) => Some(*l),
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
                self.position += 1;
                Some(())
            }
            _ => None,
        }
    }

    fn expect_any_value(&mut self) -> Result<Reference<'input>, ParserError<'input>> {
        let next = self.peek();
        match next {
            Some(AssemblyToken::Number(_)) => self.parse_number(),
            Some(AssemblyToken::Reference(_)) => self.parse_number(),
            Some(AssemblyToken::LispLiteral) => self.parse_number(),
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
    fn parse_ord(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("ord")?;
        let position = self.try_number();
        let position = self.expect(position, "an address")?;
        Ok(AssemblyLine::Ord(position as usize))
    }
    fn parse_symbol(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("symbol");
        let pos = self.expect_any_value()?;
        Ok(match pos {
            Reference::Resolved(r) => {
                AssemblyLine::ResolvedData(cpu::LispWord::symbol(r as u64).0.to_le_bytes().to_vec())
            }
            unresolved => AssemblyLine::UnresolvedData(Data::Symbol(unresolved)),
        })
    }
    fn parse_string(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("string");
        let pos = self.expect(self.raw_string()?, "String contents")?;
        let lenblock = cpu::LispWord::fixnum(pos.len() as u64);

        let mut encoded = lenblock.0.to_le_bytes().to_vec();
        encoded.extend(pos.as_bytes().to_vec());
        Ok((AssemblyLine::ResolvedData(encoded)))
    }
    fn parse_w(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("w");
        let contents = self.expect_any_value()?;
        Ok(match contents {
            Reference::Resolved(r) => AssemblyLine::ResolvedData(r.to_le_bytes().to_vec()),
            unresolved => AssemblyLine::UnresolvedData(Data::Literal(unresolved)),
        })
    }
    fn parse_consdir(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_directive("consdir");
        let car = self.expect_any_value()?;
        self.expect_comma()?;
        let cdr = self.expect_any_value()?;
        Ok(match (&car, &cdr) {
            (Reference::Resolved(car), Reference::Resolved(cdr)) => {
                let mut vec = car.to_le_bytes().to_vec();
                vec.extend(cdr.to_be_bytes());

                AssemblyLine::ResolvedData(vec)
            }
            _ => AssemblyLine::UnresolvedData(Data::Cons(car, cdr)),
        })
    }
    fn try_directive_line(&mut self) -> Result<Option<AssemblyLine<'input>>, ParserError<'input>> {
        let directive = self.peek();

        match directive {
            Some(AssemblyToken::Directive("ord")) => Ok(Some(self.parse_ord()?)),
            Some(AssemblyToken::Directive("symbol")) => Ok(Some(self.parse_symbol()?)),
            Some(AssemblyToken::Directive("string")) => Ok(Some(self.parse_string()?)),
            Some(AssemblyToken::Directive("w")) => Ok(Some(self.parse_w()?)),
            Some(AssemblyToken::Directive("consdir")) => Ok(Some(self.parse_consdir()?)),
            Some(AssemblyToken::Directive(directive)) => {
                Err(ParserError::UnknownDirective { directive })
            }
            _ => return Ok(None),
        }
    }

    fn parse_lispword(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        // Read next symbol to see what needs to be converted.
        //match next { Charecter => LispWord::char
        todo!()
    }

    fn parse_nop(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("nop");
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Nop))
    }
    fn parse_halt(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("halt");
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Halt))
    }
    fn parse_return_op(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("return");
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::Return))
    }
    fn parse_ireturn_op(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("ireturn");
        Ok(AssemblyLine::ResolvedInstruction(cpu::Instruction::IReturn))
    }
    fn parse_interrupt(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("interrupt");

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
    fn parse_jump(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("jump");
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
    fn parse_call(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("call");
        todo!()
    }
    fn parse_pusha(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("pusha");
        todo!()
    }
    fn parse_popa(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("popa");
        todo!()
    }
    fn parse_pushr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("pushr");
        todo!()
    }
    fn parse_popr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("popr");
        todo!()
    }
    fn parse_mov(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("mov");
        todo!()
    }
    fn parse_mov8(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("mov8");
        todo!()
    }
    fn parse_mbin(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("mbin");
        todo!()
    }
    fn parse_settag(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("settag");
        todo!()
    }
    fn parse_gettag(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("gettag");
        todo!()
    }
    fn parse_setpayload(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("setpayload");
        todo!()
    }
    fn parse_getpayload(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("getpayload");
        todo!()
    }
    fn parse_cons(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("cons");
        todo!()
    }
    fn parse_uncons(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("uncons");
        todo!()
    }
    fn parse_car(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("car");
        todo!()
    }
    fn parse_cdr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("cdr");
        todo!()
    }
    fn parse_setcar(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("setcar");
        todo!()
    }
    fn parse_setcdr(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("setcdr");
        todo!()
    }
    fn parse_bin(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("bin");
        todo!()
    }
    fn parse_div(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("div");
        todo!()
    }
    fn parse_makeclosure(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("makeclosure");
        let dst = self.expect_register()?;
        self.expect_comma();
        let src = self.expect_machine_register()?;
        Ok((AssemblyLine::ResolvedInstruction(cpu::Instruction::MakeClosure { code: src, dst })))
    }
    fn parse_typep(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("typep");
        let dst = self.expect_register()?;
        self.expect_comma();
        let src = self.expect_register()?;
        self.expect_comma();
        let count = self.try_number()?;
        Ok((AsseblyLine::ResolvedInstruction(cpu::Instruction::Typep { dst, src, count })))
    }
    fn parse_memcpy(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("memcpy");
        let dst = self.expect_machine_register()?;
        self.expect_comma();
        let src = self.expect_machine_register()?;
        self.expect_comma();
        let count = self.try_number()?;
        Ok((AsseblyLine::ResolvedInstruction(cpu::Instruction::MemCpy { dst, src, count })))
    }
    fn parse_comparison(&mut self) -> Result<AssemblyLine<'input>, ParserError<'input>> {
        self.expect_instruction("comparison");
        todo!()
    }

    fn try_instruction_line(
        &mut self,
    ) -> Result<Option<AssemblyLine<'input>>, ParserError<'input>> {
        let instruction = self.peek();

        match instruction {
            Some(AssemblyToken::Instruction("nop")) => Ok(Some(self.parse_nop()?)),
            Some(AssemblyToken::Instruction("halt")) => Ok(Some(self.parse_halt()?)),
            Some(AssemblyToken::Instruction("mov")) => Ok(Some(self.parse_mov()?)),
            Some(AssemblyToken::Instruction("return_op")) => Ok(Some(self.parse_return_op()?)),
            Some(AssemblyToken::Instruction("ireturn_op")) => Ok(Some(self.parse_ireturn_op()?)),
            Some(AssemblyToken::Instruction("interrupt")) => Ok(Some(self.parse_interrupt()?)),
            Some(AssemblyToken::Instruction("jump")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Instruction("jumpif")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Instruction("jumpifnot")) => Ok(Some(self.parse_jump()?)),
            Some(AssemblyToken::Instruction("call")) => Ok(Some(self.parse_call()?)),
            Some(AssemblyToken::Instruction("pusha")) => Ok(Some(self.parse_pusha()?)),
            Some(AssemblyToken::Instruction("popa")) => Ok(Some(self.parse_popa()?)),
            Some(AssemblyToken::Instruction("pushr")) => Ok(Some(self.parse_pushr()?)),
            Some(AssemblyToken::Instruction("popr")) => Ok(Some(self.parse_popr()?)),
            Some(AssemblyToken::Instruction("mov8")) => Ok(Some(self.parse_mov8()?)),
            Some(AssemblyToken::Instruction("mbin")) => Ok(Some(self.parse_mbin()?)),
            Some(AssemblyToken::Instruction("settag")) => Ok(Some(self.parse_settag()?)),
            Some(AssemblyToken::Instruction("gettag")) => Ok(Some(self.parse_gettag()?)),
            Some(AssemblyToken::Instruction("setpayload")) => Ok(Some(self.parse_setpayload()?)),
            Some(AssemblyToken::Instruction("getpayload")) => Ok(Some(self.parse_getpayload()?)),
            Some(AssemblyToken::Instruction("cons")) => Ok(Some(self.parse_cons()?)),
            Some(AssemblyToken::Instruction("uncons")) => Ok(Some(self.parse_uncons()?)),
            Some(AssemblyToken::Instruction("car")) => Ok(Some(self.parse_car()?)),
            Some(AssemblyToken::Instruction("cdr")) => Ok(Some(self.parse_cdr()?)),
            Some(AssemblyToken::Instruction("setcar")) => Ok(Some(self.parse_setcar()?)),
            Some(AssemblyToken::Instruction("setcdr")) => Ok(Some(self.parse_setcdr()?)),
            Some(AssemblyToken::Instruction("bin")) => Ok(Some(self.parse_bin()?)),
            Some(AssemblyToken::Instruction("div")) => Ok(Some(self.parse_div()?)),
            Some(AssemblyToken::Instruction("makeclosure")) => Ok(Some(self.parse_makeclosure()?)),
            Some(AssemblyToken::Instruction("typep")) => Ok(Some(self.parse_typep()?)),
            Some(AssemblyToken::Instruction("memcpy")) => Ok(Some(self.parse_memcpy()?)),
            Some(AssemblyToken::Instruction("comparison")) => Ok(Some(self.parse_comparison()?)),
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
    use crate::{
        cpu::{
            self,
            assembler::{self, parse},
        },
        memory,
    };

    fn scaffold() -> (Vec<assembler::AssemblyLine>, Vec<assembler::AssemblyLine>) {
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
            assembler::AssemblyLine::UnresolvedData(assembler::Data::Symbol(
                assembler::Reference::UnresolvedPos("loop".to_string()),
            )),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Halt),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Nop),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Return),
            assembler::AssemblyLine::Label("loop".to_string()),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(42)),
            assembler::AssemblyLine::UnresolvedInstruction(assembler::UnresolvedInstruction::Int(
                assembler::Reference::UnresolvedPos("loop".to_string()),
            )),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::IReturn),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::Always,
                target: cpu::JumpTarget::Absolute(memory::Address(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::Always,
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::Always,
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::Always,
                target: cpu::JumpTarget::Register(cpu::Register(1)),
            }),
            assembler::AssemblyLine::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Jump {
                    condition: cpu::Condition::Always,
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: cpu::JumpTarget::Absolute(memory::Address(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::True(cpu::Register(5)),
                target: cpu::JumpTarget::Register(cpu::Register(1)),
            }),
            assembler::AssemblyLine::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Jump {
                    condition: cpu::Condition::True(cpu::Register(5)),
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: cpu::JumpTarget::Absolute(memory::Address(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: cpu::Condition::False(cpu::Register(5)),
                target: cpu::JumpTarget::Register(cpu::Register(1)),
            }),
            assembler::AssemblyLine::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Jump {
                    condition: cpu::Condition::False(cpu::Register(5)),
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::Absolute(memory::Address(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::Machine(cpu::MachineRegister(1), memory::Offset(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::Register(cpu::Register(1)),
            }),
            assembler::AssemblyLine::UnresolvedInstruction(
                assembler::UnresolvedInstruction::Call {
                    target: assembler::JumpTarget::Absolute(assembler::Reference::UnresolvedPos(
                        "loop".to_string(),
                    )),
                },
            ),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::PushA {
                src: cpu::MachineRegister(1),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::PopA {
                dst: cpu::MachineRegister(2),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::PushR {
                src: cpu::Register(3),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::PopR {
                dst: cpu::Register(4),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Register(cpu::Register(1)),
                src: cpu::Location::Literal(cpu::Native(cpu::LispWord::fixnum(1234).0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Machine(cpu::MachineRegister(3)),
                src: cpu::Location::Literal(cpu::Native(0x7fffffff)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Register(cpu::Register(5)),
                src: cpu::Location::Register(cpu::Register(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Register(cpu::Register(5)),
                src: cpu::Location::IndirectRegister(cpu::Register(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::IndirectRegister(cpu::Register(6)),
                src: cpu::Location::Register(cpu::Register(7)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Machine(cpu::MachineRegister(4)),
                src: cpu::Location::Machine(cpu::MachineRegister(5)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Machine(cpu::MachineRegister(1)),
                src: cpu::Location::IndirectMachine(cpu::MachineRegister(2), cpu::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Machine(cpu::MachineRegister(1)),
                src: cpu::Location::IndirectMachine(cpu::MachineRegister(2), cpu::Offset(16)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Machine(cpu::MachineRegister(1)),
                src: cpu::Location::Absolute(cpu::Address(64)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::IndirectMachine(cpu::MachineRegister(2), memory::Offset(0)),
                src: cpu::Location::Machine(cpu::MachineRegister(3)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::IndirectMachine(cpu::MachineRegister(2), memory::Offset(8)),
                src: cpu::Location::Machine(cpu::MachineRegister(3)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Absolute(memory::Address(24)),
                src: cpu::Location::Machine(cpu::MachineRegister(4)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Machine(cpu::MachineRegister(5)),
                src: cpu::Location::Register(cpu::Register(8)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Register(cpu::Register(9)),
                src: cpu::Location::Machine(cpu::MachineRegister(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::IndirectMachine(cpu::MachineRegister(5), memory::Offset(0)),
                src: cpu::Location::Register(cpu::Register(8)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::IndirectMachine(cpu::MachineRegister(5), memory::Offset(8)),
                src: cpu::Location::Register(cpu::Register(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Register(cpu::Register(5)),
                src: cpu::Location::IndirectMachine(cpu::MachineRegister(6), memory::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                dst: cpu::Location::Register(cpu::Register(5)),
                src: cpu::Location::IndirectMachine(cpu::MachineRegister(6), memory::Offset(8)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                dst: cpu::Location::Machine(cpu::MachineRegister(3)),
                src: cpu::Location::IndirectMachine(cpu::MachineRegister(4), memory::Offset(0)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                dst: cpu::Location::Machine(cpu::MachineRegister(3)),
                src: cpu::Location::IndirectMachine(cpu::MachineRegister(4), memory::Offset(5)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                dst: cpu::Location::Machine(cpu::MachineRegister(3)),
                src: cpu::Location::Absolute(memory::Address(5)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                dst: cpu::Location::IndirectMachine(cpu::MachineRegister(4), memory::Offset(0)),
                src: cpu::Location::Machine(cpu::MachineRegister(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                dst: cpu::Location::IndirectMachine(cpu::MachineRegister(4), memory::Offset(5)),
                src: cpu::Location::Machine(cpu::MachineRegister(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                dst: cpu::Location::Absolute(memory::Address(5)),
                src: cpu::Location::Machine(cpu::MachineRegister(6)),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: cpu::MBinaryOp::Add,

                dst: cpu::MachineRegister(1),
                operands: cpu::MachSource {
                    op1: cpu::MachineRegister(2),
                    op2: Some(cpu::MachineRegister(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: cpu::MBinaryOp::Add,

                dst: cpu::MachineRegister(1),
                operands: cpu::MachSource {
                    op1: cpu::MachineRegister(2),
                    op2: Some(cpu::MachineRegister(3)),
                    op3: Some(cpu::Native(4)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: cpu::MBinaryOp::Add,

                dst: cpu::MachineRegister(1),
                operands: cpu::MachSource {
                    op1: cpu::MachineRegister(2),
                    op2: None,
                    op3: Some(cpu::Native(8)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,

                dst: cpu::MachineRegister(4),
                operands: cpu::MachSource {
                    op1: cpu::MachineRegister(5),
                    op2: Some(cpu::MachineRegister(6)),
                    op3: None,
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,

                dst: cpu::MachineRegister(4),
                operands: cpu::MachSource {
                    op1: cpu::MachineRegister(5),
                    op2: Some(cpu::MachineRegister(6)),
                    op3: Some(cpu::Native(2)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: cpu::MBinaryOp::Sub,

                dst: cpu::MachineRegister(4),
                operands: cpu::MachSource {
                    op1: cpu::MachineRegister(5),
                    op2: None,
                    op3: Some(cpu::Native(12)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetTag {
                dst: cpu::Register(2),
                src: cpu::MachineRegister(1),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::GetTag {
                dst: cpu::MachineRegister(3),
                src: cpu::Register(2),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetPayload {
                dst: cpu::Register(4),
                src: cpu::MachineRegister(3),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::GetPayload {
                dst: cpu::MachineRegister(5),
                src: cpu::Register(4),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(0x03)),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Uncons {
                car: cpu::Register(1),
                cdr: cpu::Register(2),
                src: cpu::Register(3),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Car(cpu::TwoRegs {
                dst: cpu::Register(4),
                src: cpu::Register(5),
            })),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Cdr(cpu::TwoRegs {
                dst: cpu::Register(6),
                src: cpu::Register(7),
            })),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCar(cpu::TwoRegs {
                dst: cpu::Register(8),
                src: cpu::Register(9),
            })),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::SetCdr(cpu::TwoRegs {
                dst: cpu::Register(10),
                src: cpu::Register(11),
            })),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Add,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Add,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Add,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Sub,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Sub,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Sub,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Mul,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Mul,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: cpu::BinaryOp::Mul,

                dst: cpu::Register(1),
                operands: cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Eq,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Eq,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Eq,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Ne,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Ne,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Ne,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Lt,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Lt,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Lt,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Lte,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Lte,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Lte,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Gt,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Gt,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Gt,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Gte,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: None,
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Gte,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: Some(cpu::Register(3)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: cpu::Comparison::Gte,

                dst: cpu::Register(1),
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: cpu::Register(2),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(10)),
                }),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                div: cpu::Register(9),
                rem: cpu::Register(2),
                operands: cpu::RegSource {
                    op1: cpu::Register(3),
                    op2: Some(cpu::Register(4)),
                    op3: None,
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                div: cpu::Register(9),
                rem: cpu::Register(2),
                operands: cpu::RegSource {
                    op1: cpu::Register(3),
                    op2: Some(cpu::Register(4)),
                    op3: Some(cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                div: cpu::Register(9),
                rem: cpu::Register(2),
                operands: cpu::RegSource {
                    op1: cpu::Register(3),
                    op2: None,
                    op3: Some(cpu::LispWord::fixnum(5)),
                },
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MakeClosure {
                dst: cpu::Register(5),
                code: cpu::MachineRegister(6),
            }),
            assembler::AssemblyLine::ResolvedInstruction(cpu::Instruction::MemCpy {
                dst: cpu::MachineRegister(1),
                src: cpu::MachineRegister(2),
                count: cpu::Count(1),
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
            if let assembler::AssemblyLine::UnresolvedInstruction(i) = line {
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
            match &expected[i] {
                assembler::AssemblyLine::ResolvedInstruction(inst) => {
                    let (lo, hi) = inst.encode();
                    assert_eq!(
                        (
                            i,
                            &cpu::Instruction::decode(lo, hi).map_err(|t| format!(
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
            match &expected[i] {
                assembler::AssemblyLine::ResolvedInstruction(inst) => {
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
                            &cpu::Instruction::decode(lo, hi).map_err(|t| format!(
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
