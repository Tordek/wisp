mod parser;
mod tokenizer;

use std::collections::HashMap;

use crate::{
    cpu::{self},
    memory::{self},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RegSource {
    pub op1: cpu::Register,
    pub op2: Option<cpu::Register>,
    pub op3: Option<Reference>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MachSource {
    pub op1: cpu::MachineRegister,
    pub op2: Option<cpu::MachineRegister>,
    pub op3: Option<Reference>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EitherSource {
    Mach(MachSource),
    Reg(RegSource),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Location {
    Literal(Reference),
    Absolute(Reference),
    Register(cpu::Register),
    Machine(cpu::MachineRegister),
    IndirectRegister(cpu::Register),
    IndirectMachine(cpu::MachineRegister, Reference),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum JumpTarget {
    Absolute(Reference),
    Register(cpu::Register),
    Machine(cpu::MachineRegister, Reference),
    IndirectRegister(cpu::Register),
    IndirectMachine(cpu::MachineRegister, Reference),
}

impl Location {
    fn try_resolve(&self) -> Option<cpu::Location> {
        match *self {
            Location::Literal(Reference::Resolved(r)) => {
                Some(cpu::Location::Literal(cpu::Native(r as u64)))
            }
            Location::Absolute(Reference::Resolved(r)) => {
                Some(cpu::Location::Absolute(cpu::Address(r as u64)))
            }
            Location::Machine(reg) => Some(cpu::Location::Machine(reg)),
            Location::Register(reg) => Some(cpu::Location::Register(reg)),
            Location::IndirectMachine(reg, Reference::Resolved(off)) => {
                Some(cpu::Location::IndirectMachine(reg, cpu::Offset(off)))
            }
            Location::IndirectRegister(r) => Some(cpu::Location::IndirectRegister(r)),
            _ => None,
        }
    }
}
impl JumpTarget {
    fn try_resolve(&self) -> Option<cpu::JumpTarget> {
        match *self {
            JumpTarget::Absolute(Reference::Resolved(r)) => {
                Some(cpu::JumpTarget::Absolute(cpu::Address(r as u64)))
            }
            JumpTarget::Machine(reg, Reference::Resolved(off)) => {
                Some(cpu::JumpTarget::Machine(reg, cpu::Offset(off)))
            }
            JumpTarget::Register(reg) => Some(cpu::JumpTarget::Register(reg)),
            JumpTarget::IndirectMachine(reg, Reference::Resolved(off)) => {
                Some(cpu::JumpTarget::IndirectMachine(reg, cpu::Offset(off)))
            }
            JumpTarget::IndirectRegister(r) => Some(cpu::JumpTarget::IndirectRegister(r)),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum UnresolvedInstruction {
    Jump {
        condition: cpu::Condition,
        target: JumpTarget,
    },
    Int(Reference),
    Mov {
        dst: Location,
        src: Location,
    },
    Mov8 {
        dst: Location,
        src: Location,
    },
    MBinary {
        op: cpu::MBinaryOp,
        dst: cpu::MachineRegister,
        operands: MachSource,
    },
    IDiv {
        div: cpu::Register,
        rem: cpu::Register,
        op1: cpu::Register,
        op2: Option<cpu::Register>,
        op3: Option<Reference>,
    },
    Binary {
        op: cpu::BinaryOp,
        dst: cpu::Register,
        operands: RegSource,
    },
    MComparison {
        op: cpu::Comparison,
        dst: cpu::Register,
        operands: EitherSource,
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
        target: JumpTarget,
    },
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Reference {
    UnresolvedNeg(String),
    UnresolvedPos(String),
    Resolved(i64),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Data {
    Symbol(Reference),
    Literal(Reference),
    Cons(Reference, Reference),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AssemblyLine {
    Ord(usize),
    UnresolvedData(Data),
    ResolvedData(Vec<u8>),
    ResolvedInstruction(cpu::Instruction),
    UnresolvedInstruction(UnresolvedInstruction),
    Label(String),
}

pub fn parse(input: &str) -> Result<Vec<AssemblyLine>, String> {
    let (rest, assembly_lines) =
        parser::asm_lines(input).map_err(|e| format!("parse error: {}", e))?;

    if !rest.is_empty() {
        return Err(rest.to_string());
    }

    Ok(assembly_lines)
}

pub fn layout(assembly_lines: &[AssemblyLine]) -> HashMap<String, usize> {
    let mut position: usize = 0;
    let mut labels = HashMap::new();
    for line in assembly_lines {
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
                labels.insert(label.clone(), position.next_multiple_of(8));
            }
            AssemblyLine::Ord(p) => position = *p,
        }
    }
    labels
}

pub fn resolve_opt_reference(
    reference: &Option<Reference>,
    labels: &HashMap<String, usize>,
) -> Result<Option<usize>, String> {
    reference
        .clone()
        .map(|r| resolve_reference(&r, labels))
        .transpose()
}

pub fn resolve_reference(
    reference: &Reference,
    labels: &HashMap<String, usize>,
) -> Result<usize, String> {
    match reference {
        Reference::Resolved(_) => todo!("This shouldn't happen"),
        Reference::UnresolvedNeg(r) => {
            Ok(-(*labels.get(r).ok_or(format!("Symbol {} not found", r))? as isize) as usize)
        }
        Reference::UnresolvedPos(r) => {
            Ok(*labels.get(r).ok_or(format!("Symbol {} not found", r))? as usize)
        }
    }
}

pub fn resolve_location(
    location: &Location,
    labels: &HashMap<String, usize>,
) -> Result<cpu::Location, String> {
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

pub fn resolve(
    assembly_lines: &[AssemblyLine],
    labels: &HashMap<String, usize>,
) -> Result<Vec<AssemblyLine>, String> {
    let mut result = vec![];
    for line in assembly_lines {
        result.push(match line {
            AssemblyLine::Ord(o) => AssemblyLine::Ord(*o),
            AssemblyLine::Label(l) => AssemblyLine::Label(l.clone()),
            AssemblyLine::UnresolvedData(Data::Cons(car, cdr)) => {
                AssemblyLine::ResolvedData(todo!())
            }
            AssemblyLine::UnresolvedData(Data::Symbol(refr)) => AssemblyLine::ResolvedData(
                cpu::LispWord::symbol(resolve_reference(refr, labels)? as u64)
                    .0
                    .to_le_bytes()
                    .to_vec(),
            ),
            AssemblyLine::UnresolvedData(Data::Literal(refr)) => AssemblyLine::ResolvedData(
                (resolve_reference(refr, labels)? as u64)
                    .to_le_bytes()
                    .to_vec(),
            ),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Binary {
                op,
                dst,
                operands: RegSource { op1, op2, op3 },
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Binary {
                op: *op,
                dst: *dst,
                operands: cpu::RegSource {
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(op3, labels)?.map(|w| cpu::LispWord(w as u64)),
                },
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Absolute(refr),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::Absolute(cpu::Address(
                    resolve_reference(refr, labels)? as u64
                )),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Machine(m, refr),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::Machine(
                    *m,
                    cpu::Offset(resolve_reference(refr, labels)? as i64),
                ),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::IndirectMachine(m, refr),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Call {
                target: cpu::JumpTarget::IndirectMachine(
                    *m,
                    cpu::Offset(resolve_reference(refr, labels)? as i64),
                ),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::Register(_),
            }) => return Err("can't happen".to_string()),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Call {
                target: JumpTarget::IndirectRegister(_),
            }) => return Err("can't happen".to_string()),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::IDiv {
                div,
                rem,
                op1,
                op2,
                op3,
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::IDiv {
                div: *div,
                rem: *rem,
                operands: cpu::RegSource {
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(op3, labels)?.map(|v| cpu::LispWord(v as u64)),
                },
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Int(i)) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Int(resolve_reference(
                    i, labels,
                )? as u64))
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::Absolute(refr),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: *condition,
                target: cpu::JumpTarget::Absolute(cpu::Address(
                    resolve_reference(refr, labels)? as u64
                )),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::Machine(m, refr),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: *condition,
                target: cpu::JumpTarget::Machine(
                    *m,
                    cpu::Offset(resolve_reference(refr, labels)? as i64),
                ),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition,
                target: JumpTarget::IndirectMachine(m, refr),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::Jump {
                condition: *condition,
                target: cpu::JumpTarget::IndirectMachine(
                    *m,
                    cpu::Offset(resolve_reference(refr, labels)? as i64),
                ),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition: _,
                target: JumpTarget::Register(_),
            }) => return Err("can't happen".to_string()),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Jump {
                condition: _,
                target: JumpTarget::IndirectRegister(_),
            }) => return Err("can't happen".to_string()),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MBinary {
                op,
                dst,
                operands: MachSource { op1, op2, op3 },
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MBinary {
                op: *op,
                dst: *dst,
                operands: cpu::MachSource {
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(op3, labels)?.map(|v| cpu::Native(v as u64)),
                },
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                op,
                dst,
                operands: EitherSource::Reg(RegSource { op1, op2, op3 }),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: *op,
                dst: *dst,
                operands: cpu::EitherSource::Reg(cpu::RegSource {
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(op3, labels)?.map(|v| cpu::LispWord(v as u64)),
                }),
            }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::MComparison {
                op,
                dst,
                operands: EitherSource::Mach(MachSource { op1, op2, op3 }),
            }) => AssemblyLine::ResolvedInstruction(cpu::Instruction::MComparison {
                op: *op,
                dst: *dst,
                operands: cpu::EitherSource::Mach(cpu::MachSource {
                    op1: *op1,
                    op2: *op2,
                    op3: resolve_opt_reference(op3, labels)?.map(|v| cpu::Native(v as u64)),
                }),
            }),
            // AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MemCpy {
            //     dst,
            //     src,
            //     count,
            // }) => AssemblyToken::ResolvedInstruction(cpu::Instruction::MemCpy {
            //     dst: *dst,
            //     src: *src,
            //     count: cpu::Count(resolve_reference(&count, labels)? as u64),
            // }),
            // AssemblyToken::UnresolvedInstruction(UnresolvedInstruction::MemSet {
            //     dst,
            //     src,
            //     count,
            // }) => AssemblyToken::ResolvedInstruction(cpu::Instruction::MemSet {
            //     dst: *dst,
            //     src: *src,
            //     count: cpu::Count(resolve_reference(count, labels)? as u64),
            // }),
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov { dst, src }) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov {
                    dst: resolve_location(dst, labels)?,
                    src: resolve_location(src, labels)?,
                })
            }
            AssemblyLine::UnresolvedInstruction(UnresolvedInstruction::Mov8 { dst, src }) => {
                AssemblyLine::ResolvedInstruction(cpu::Instruction::Mov8 {
                    dst: resolve_location(dst, labels)?,
                    src: resolve_location(src, labels)?,
                })
            }

            AssemblyLine::ResolvedInstruction(r) => AssemblyLine::ResolvedInstruction(*r),
            AssemblyLine::ResolvedData(d) => AssemblyLine::ResolvedData(d.clone()),
        })
    }
    Ok(result)
}

pub fn assemble(assembly_lines: &[AssemblyLine]) -> Result<Vec<u8>, String> {
    let mut result: Vec<u8> = vec![];
    let mut position: usize = 0;

    for line in assembly_lines {
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
            AssemblyLine::UnresolvedData(_) => return Err("Forgot to resolve".to_string()),
            AssemblyLine::UnresolvedInstruction(_) => return Err("Forgot to resolve".to_string()),
        }
    }

    Ok(result)
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
