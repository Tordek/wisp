#[macro_export]
macro_rules! parse_asm {
    {} => {{ vec![] }};
    { HALT; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Halt];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { NOP; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Nop];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { RETURN; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Return];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { INT $int:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Int($int)];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { IRETURN; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::IReturn
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMP [$off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Jump(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister(0),
                    adr: None,
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMP [A $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Jump(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister(0),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: 0,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMP [A $base:expr => $off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Jump(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister(0),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMP [R $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Jump(
                $crate::cpu::JumpAddressing::Register {
                    condition: $crate::cpu::Register(0),
                    adr: $crate::cpu::Register($base),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIF A $cond:expr, [$off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIf(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister($cond),
                    adr: None,
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIF A $cond:expr, [A $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIf(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister($cond),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: 0,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIF A $cond:expr, [A $base:expr => $off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIf(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister($cond),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIF R $cond:expr, [R $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIf(
                $crate::cpu::JumpAddressing::Register {
                    condition: $crate::cpu::Register($cond),
                    adr: $crate::cpu::Register($base),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIFNOT A $cond:expr, [$off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIfNot(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister($cond),
                    adr: None,
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIFNOT A $cond:expr, [A $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIfNot(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister($cond),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: 0,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIFNOT A $cond:expr, [A $base:expr => $off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIfNot(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister($cond),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { JUMPIFNOT R $cond:expr, [R $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::JumpIfNot(
                $crate::cpu::JumpAddressing::Register {
                    condition: $crate::cpu::Register($cond),
                    adr: $crate::cpu::Register($base),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CALL [$off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Call(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister(0),
                    adr: None,
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CALL [A $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Call(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister(0),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: 0,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CALL [A $base:expr => $off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Call(
                $crate::cpu::JumpAddressing::AddressRegister {
                    condition: $crate::cpu::AddressRegister(0),
                    adr: Some($crate::cpu::AddressRegister($base)),
                    offset: $off,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CALL [R $base:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Call(
                $crate::cpu::JumpAddressing::Register {
                    condition: $crate::cpu::Register(0),
                    adr: $crate::cpu::Register($base),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { PUSH A $adr:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::PushA {
            src: $crate::cpu::AddressRegister($adr),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { POP A $adr:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::PopA {
            dst: $crate::cpu::AddressRegister($adr),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { PUSH R $adr:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::PushR {
            src: $crate::cpu::Register($adr),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { POP R $adr:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::PopR {
            dst: $crate::cpu::Register($adr),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};

    // Mov
    { MOV R $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
                dst: $crate::cpu::Location::Register(
                    $crate::cpu::Register($p1)
                ),
                src: $crate::cpu::Location::Register(
                    $crate::cpu::Register($p2)
                ),
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV R $p1:expr, [R $p2:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::Register(
                $crate::cpu::Register($p1)
            ),
            src: $crate::cpu::Location::IndirectRegister(
                $crate::cpu::Register($p2)
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV [R $p1:expr], R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::IndirectRegister(
                $crate::cpu::Register($p1)
            ),
            src: $crate::cpu::Location::Register(
                $crate::cpu::Register($p2)
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV A $p1:expr, A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p1)
            ),
            src: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p2)
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV A $p1:expr, [A $p2:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p1)
            ),
            src: $crate::cpu::Location::IndirectAddress(
                $crate::cpu::AddressRegister($p2),
                0
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV A $p1:expr, [A $p2:expr => $off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p1)
            ),
            src: $crate::cpu::Location::IndirectAddress(
                $crate::cpu::AddressRegister($p2),
                $off,
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV A $p1:expr, [$off:expr]; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p1)
            ),
            src: $crate::cpu::Location::Absolute(
                $off
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV [A $p1:expr => $off:expr], A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::IndirectAddress(
                $crate::cpu::AddressRegister($p1),
                $off
            ),
            src: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p2)
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV [A $p1:expr], A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::IndirectAddress(
                $crate::cpu::AddressRegister($p1),
                0
            ),
            src: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p2)
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV [$p1:expr], A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
            dst: $crate::cpu::Location::Absolute(
                $p1,
            ),
            src: $crate::cpu::Location::Address(
                $crate::cpu::AddressRegister($p2)
            ),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV R $p1:expr, A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
                dst: $crate::cpu::Location::Register(
                    $crate::cpu::Register($p1)
                ),
                src: $crate::cpu::Location::Address(
                    $crate::cpu::AddressRegister($p2)
                ),
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV A $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mov {
                dst: $crate::cpu::Location::Address(
                    $crate::cpu::AddressRegister($p1)
                ),
                src: $crate::cpu::Location::Register(
                    $crate::cpu::Register($p2)
                ),
            }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV R $reg:expr, $lit:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::LoadLiteral {
            dst: $crate::cpu::Register($reg),
            val: $lit,
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MOV A $reg:expr, $lit:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::LoadAddress {
            dst: $crate::cpu::AddressRegister($reg),
            val: $lit,
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};

    // Word manipulation
    { SETTAG R $p1:expr, A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::SetTag {
                dst: $crate::cpu::Register($p1),
                src: $crate::cpu::AddressRegister($p2),
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GETTAG A $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::GetTag {
                dst: $crate::cpu::AddressRegister($p1),
                src: $crate::cpu::Register($p2)
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SETPAYLOAD R $p1:expr, A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::SetPayload {
                dst: $crate::cpu::Register($p1),
                src: $crate::cpu::AddressRegister($p2),
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GETPAYLOAD A $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::GetPayload {
            dst: $crate::cpu::AddressRegister($p1),
            src: $crate::cpu::Register($p2),
        }];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { ADD A $p1:expr, A $p2:expr, A $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::AAdd(
                $crate::cpu::ThreeAddrs {
                    dst: $crate::cpu::AddressRegister($p1),
                    op1: $crate::cpu::AddressRegister($p2),
                    op2: Some($crate::cpu::AddressRegister($p3)),
                    imm: $imm
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { ADD A $p1:expr, A $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::AAdd(
                $crate::cpu::ThreeAddrs {
                    dst: $crate::cpu::AddressRegister($p1),
                    op1: $crate::cpu::AddressRegister($p2),
                    op2: None,
                    imm: $imm
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SUB A $p1:expr, A $p2:expr, A $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::ASub(
                $crate::cpu::ThreeAddrs {
                    dst: $crate::cpu::AddressRegister($p1),
                    op1: $crate::cpu::AddressRegister($p2),
                    op2: Some($crate::cpu::AddressRegister($p3)),
                    imm: 0,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SUB A $p1:expr, A $p2:expr, A $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::ASub(
                $crate::cpu::ThreeAddrs {
                    dst: $crate::cpu::AddressRegister($p1),
                    op1: $crate::cpu::AddressRegister($p2),
                    op2: Some($crate::cpu::AddressRegister($p3)),
                    imm: $imm
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SUB A $p1:expr, A $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::ASub(
                $crate::cpu::ThreeAddrs {
                    dst: $crate::cpu::AddressRegister($p1),
                    op1: $crate::cpu::AddressRegister($p2),
                    op2: None,
                    imm: $imm
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { ADD A $p1:expr, A $p2:expr, A $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::AAdd(
                $crate::cpu::ThreeAddrs {
                    dst: $crate::cpu::AddressRegister($p1),
                    op1: $crate::cpu::AddressRegister($p2),
                    op2: Some($crate::cpu::AddressRegister($p3)),
                    imm: 0,
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CONS R $dst:expr, R $car:expr, R $cdr:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::PushR(Register(0));
            $crate::cpu::Instruction::PushR(Register(1));
            $crate::cpu::Instruction::Mov {
                dst: $crate::cpu::Location::Register(
                    $crate::cpu::Register(0)
                ),
                src: $crate::cpu::Location::Register(
                    $crate::cpu::Register($car)
                ),
            }
            $crate::cpu::Instruction::Mov {
                dst: $crate::cpu::Location::Register(
                    $crate::cpu::Register(1)
                ),
                src: $crate::cpu::Location::Register(
                    $crate::cpu::Register($cdr)
                ),
            }
            $crate::cpu::Instruction::Int(3),
            $crate::cpu::Instruction::Mov {
                dst: $crate::cpu::Location::Register(
                    $crate::cpu::Register(0)
                ),
                src: $crate::cpu::Location::Register(
                    $crate::cpu::Register($dst)
                ),
            }
            $crate::cpu::Instruction::PopR(Register(1));
            $crate::cpu::Instruction::PopR(Register(0));
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CONS; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Int(3),
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { UNCONS R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Uncons {
                car: $crate::cpu::Register($p1),
                cdr: $crate::cpu::Register($p2),
                src: $crate::cpu::Register($p3)
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CAR R $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Car(
                $crate::cpu::TwoRegs {
                    dst: $crate::cpu::Register($p1),
                    src: $crate::cpu::Register($p2),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { CDR R $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Cdr(
                $crate::cpu::TwoRegs {
                    dst: $crate::cpu::Register($p1),
                    src: $crate::cpu::Register($p2),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SETCAR R $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::SetCar(
                $crate::cpu::TwoRegs {
                    dst: $crate::cpu::Register($p1),
                    src: $crate::cpu::Register($p2),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SETCDR R $p1:expr, R $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::SetCdr(
                $crate::cpu::TwoRegs {
                    dst: $crate::cpu::Register($p1),
                    src: $crate::cpu::Register($p2),
                }
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { ADD R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Add(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { ADD R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Add(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { ADD R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Add(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SUB R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Sub(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SUB R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Sub(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { SUB R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Sub(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MUL R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mul(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MUL R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mul(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MUL R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Mul(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { EQ R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Eq(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { EQ R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Eq(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { EQ R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Eq(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { NE R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Ne(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { NE R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Ne(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { NE R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Ne(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { LT R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Lt(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { LT R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Lt(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { LT R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Lt(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { LTE R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Lte(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { LTE R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Lte(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { LTE R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Lte(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GT R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Gt(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GT R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Gt(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GT R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Gt(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GTE R $p1:expr, R $p2:expr, R $p3:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Gte(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $crate::cpu::Word::fixnum(0),
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GTE R $p1:expr, R $p2:expr, R $p3:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Gte(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: Some($crate::cpu::Register($p3)),
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { GTE R $p1:expr, R $p2:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::Gte(
                $crate::cpu::ThreeRegs {
                    dst: $crate::cpu::Register($p1),
                    op1: $crate::cpu::Register($p2),
                    op2: None,
                    imm: $imm,
                }  
            )
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { DIV R $p1:expr, R $p2:expr, R $p3:expr, R $p4:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::IDiv {
                div: $crate::cpu::Register($p1),
                rem: $crate::cpu::Register($p2),
                op1: $crate::cpu::Register($p3),
                op2: Some($crate::cpu::Register($p4)),
                imm: $crate::cpu::Word::fixnum(0)
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { DIV R $p1:expr, R $p2:expr, R $p3:expr, R $p4:expr => $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::IDiv {
                div: $crate::cpu::Register($p1),
                rem: $crate::cpu::Register($p2),
                op1: $crate::cpu::Register($p3),
                op2: Some($crate::cpu::Register($p4)),
                imm: $imm
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { DIV R $p1:expr, R $p2:expr, R $p3:expr, $imm:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::IDiv {
                div: $crate::cpu::Register($p1),
                rem: $crate::cpu::Register($p2),
                op1: $crate::cpu::Register($p3),
                op2: None,
                imm: $imm,
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
    { MAKECLOSURE R $p1:expr, A $p2:expr; $($rest:tt)* } => {{
        let mut insts = vec![
            $crate::cpu::Instruction::MakeClosure {
                dst: $crate::cpu::Register($p1),
                code: $crate::cpu::AddressRegister($p2),
            }
        ];
        insts.extend(parse_asm!($($rest)*));
        insts
    }};
}

mod test {
    #[test]
    fn test_assembler_macro() {
        let asm = parse_asm! {
            HALT;
            NOP;
            RETURN;
            INT 42;
            IRETURN;

            // Jump variants
            JUMP [16];
            JUMP [A 1];
            JUMP [A 1 => 16];
            JUMP [R 1];
            JUMPIF A 5, [16];
            JUMPIF A 5, [A 2];
            JUMPIF A 5, [A 2 => 32];
            JUMPIF R 2, [R 3];
            JUMPIFNOT A 5, [16];
            JUMPIFNOT A 5, [A 2];
            JUMPIFNOT A 5, [A 2 => -16];
            JUMPIFNOT R 4, [R 5];
            CALL [16];
            CALL [A 3];
            CALL [A 3 => 8];
            CALL [R 7];

            // Stack variants
            PUSH A 1;
            POP A 2;
            PUSH R 3;
            POP R 4;

            // Loading & Memory
            MOV R 1, crate::cpu::Word::fixnum(1234);
            MOV A 3, 0x7FFF_FFFF;
            MOV R 5, R 6;
            MOV R 5, [R 6];
            MOV [R 6], R 7;
            MOV A 4, A 5;
            MOV A 1, [A 2];
            MOV A 1, [A 2 => -16];
            MOV A 1, [64];
            MOV [A 2], A 3;
            MOV [A 2 => 8], A 3;
            MOV [24], A 4;
            MOV A 5, R 8;
            MOV R 9, A 6;

            // Address Arithmetic
            ADD A 1, A 2, A 3;
            ADD A 1, A 2, A 3 => 4;
            ADD A 1, A 2, 8;
            SUB A 4, A 5, A 6;
            SUB A 4, A 5, A 6 => 2;
            SUB A 4, A 5, 12;

            // Tag Operations
            SETTAG R 2, A 1;
            GETTAG A 3, R 2;
            SETPAYLOAD R 4, A 3;
            GETPAYLOAD A 5, R 4;

            // Lisp Destructuring Primitives
            CONS;
            UNCONS R 1, R 2, R 3;
            CAR R 4, R 5;
            CDR R 6, R 7;
            SETCAR R 8, R 9;
            SETCDR R 10, R 11;

            // Arithmetic (crate::cpu::ThreeRegs layouts using Option types, Word Immediates)
            ADD R 1, R 2, R 3;
            ADD R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            ADD R 1, R 2, crate::cpu::Word::fixnum(10);
            SUB R 1, R 2, R 3;
            SUB R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            SUB R 1, R 2, crate::cpu::Word::fixnum(10);
            MUL R 1, R 2, R 3;
            MUL R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            MUL R 1, R 2, crate::cpu::Word::fixnum(10);
            EQ R 1, R 2, R 3;
            EQ R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            EQ R 1, R 2, crate::cpu::Word::fixnum(10);
            NE R 1, R 2, R 3;
            NE R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            NE R 1, R 2, crate::cpu::Word::fixnum(10);
            LT R 1, R 2, R 3;
            LT R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            LT R 1, R 2, crate::cpu::Word::fixnum(10);
            LTE R 1, R 2, R 3;
            LTE R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            LTE R 1, R 2, crate::cpu::Word::fixnum(10);
            GT R 1, R 2, R 3;
            GT R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            GT R 1, R 2, crate::cpu::Word::fixnum(10);
            GTE R 1, R 2, R 3;
            GTE R 1, R 2, R 3 => crate::cpu::Word::fixnum(5);
            GTE R 1, R 2, crate::cpu::Word::fixnum(10);
            DIV R 1, R 2, R 3, R 4;
            DIV R 1, R 2, R 3, R 4 => crate::cpu::Word::fixnum(5);
            DIV R 1, R 2, R 3, crate::cpu::Word::fixnum(5);

            // Advanced Operations
            MAKECLOSURE R 5, A 6;
        };

        let expected = [
            crate::cpu::Instruction::Halt,
            crate::cpu::Instruction::Nop,
            crate::cpu::Instruction::Return,
            crate::cpu::Instruction::Int(42),
            crate::cpu::Instruction::IReturn,
            crate::cpu::Instruction::Jump(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(0),
                adr: None,
                offset: 16,
            }),
            crate::cpu::Instruction::Jump(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(0),
                adr: Some(crate::cpu::AddressRegister(1)),
                offset: 0,
            }),
            crate::cpu::Instruction::Jump(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(0),
                adr: Some(crate::cpu::AddressRegister(1)),
                offset: 16,
            }),
            crate::cpu::Instruction::Jump(crate::cpu::JumpAddressing::Register {
                condition: crate::cpu::Register(0),
                adr: crate::cpu::Register(1),
            }),
            crate::cpu::Instruction::JumpIf(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(5),
                adr: None,
                offset: 16,
            }),
            crate::cpu::Instruction::JumpIf(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(5),
                adr: Some(crate::cpu::AddressRegister(2)),
                offset: 0,
            }),
            crate::cpu::Instruction::JumpIf(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(5),
                adr: Some(crate::cpu::AddressRegister(2)),
                offset: 32,
            }),
            crate::cpu::Instruction::JumpIf(crate::cpu::JumpAddressing::Register {
                condition: crate::cpu::Register(2),
                adr: crate::cpu::Register(3),
            }),
            crate::cpu::Instruction::JumpIfNot(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(5),
                adr: None,
                offset: 16,
            }),
            crate::cpu::Instruction::JumpIfNot(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(5),
                adr: Some(crate::cpu::AddressRegister(2)),
                offset: 0,
            }),
            crate::cpu::Instruction::JumpIfNot(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(5),
                adr: Some(crate::cpu::AddressRegister(2)),
                offset: -16,
            }),
            crate::cpu::Instruction::JumpIfNot(crate::cpu::JumpAddressing::Register {
                condition: crate::cpu::Register(4),
                adr: crate::cpu::Register(5),
            }),
            crate::cpu::Instruction::Call(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(0),
                adr: None,
                offset: 16,
            }),
            crate::cpu::Instruction::Call(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(0),
                adr: Some(crate::cpu::AddressRegister(3)),
                offset: 0,
            }),
            crate::cpu::Instruction::Call(crate::cpu::JumpAddressing::AddressRegister {
                condition: crate::cpu::AddressRegister(0),
                adr: Some(crate::cpu::AddressRegister(3)),
                offset: 8,
            }),
            crate::cpu::Instruction::Call(crate::cpu::JumpAddressing::Register {
                condition: crate::cpu::Register(0),
                adr: crate::cpu::Register(7),
            }),
            crate::cpu::Instruction::PushA {
                src: crate::cpu::AddressRegister(1),
            },
            crate::cpu::Instruction::PopA {
                dst: crate::cpu::AddressRegister(2),
            },
            crate::cpu::Instruction::PushR {
                src: crate::cpu::Register(3),
            },
            crate::cpu::Instruction::PopR {
                dst: crate::cpu::Register(4),
            },
            crate::cpu::Instruction::LoadLiteral {
                dst: crate::cpu::Register(1),
                val: crate::cpu::Word::fixnum(1234),
            },
            crate::cpu::Instruction::LoadAddress {
                dst: crate::cpu::AddressRegister(3),
                val: 0x7FFF_FFFF,
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(5)),
                src: crate::cpu::Location::Register(crate::cpu::Register(6)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(5)),
                src: crate::cpu::Location::IndirectRegister(crate::cpu::Register(6)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectRegister(crate::cpu::Register(6)),
                src: crate::cpu::Location::Register(crate::cpu::Register(7)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Address(crate::cpu::AddressRegister(4)),
                src: crate::cpu::Location::Address(crate::cpu::AddressRegister(5)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Address(crate::cpu::AddressRegister(1)),
                src: crate::cpu::Location::IndirectAddress(crate::cpu::AddressRegister(2), 0),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Address(crate::cpu::AddressRegister(1)),
                src: crate::cpu::Location::IndirectAddress(crate::cpu::AddressRegister(2), -16),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Address(crate::cpu::AddressRegister(1)),
                src: crate::cpu::Location::Absolute(64),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectAddress(crate::cpu::AddressRegister(2), 0),
                src: crate::cpu::Location::Address(crate::cpu::AddressRegister(3)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::IndirectAddress(crate::cpu::AddressRegister(2), 8),
                src: crate::cpu::Location::Address(crate::cpu::AddressRegister(3)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Absolute(24),
                src: crate::cpu::Location::Address(crate::cpu::AddressRegister(4)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Address(crate::cpu::AddressRegister(5)),
                src: crate::cpu::Location::Register(crate::cpu::Register(8)),
            },
            crate::cpu::Instruction::Mov {
                dst: crate::cpu::Location::Register(crate::cpu::Register(9)),
                src: crate::cpu::Location::Address(crate::cpu::AddressRegister(6)),
            },
            crate::cpu::Instruction::AAdd(crate::cpu::ThreeAddrs {
                dst: crate::cpu::AddressRegister(1),
                op1: crate::cpu::AddressRegister(2),
                op2: Some(crate::cpu::AddressRegister(3)),
                imm: (0),
            }),
            crate::cpu::Instruction::AAdd(crate::cpu::ThreeAddrs {
                dst: crate::cpu::AddressRegister(1),
                op1: crate::cpu::AddressRegister(2),
                op2: Some(crate::cpu::AddressRegister(3)),
                imm: (4),
            }),
            crate::cpu::Instruction::AAdd(crate::cpu::ThreeAddrs {
                dst: crate::cpu::AddressRegister(1),
                op1: crate::cpu::AddressRegister(2),
                op2: None,
                imm: (8),
            }),
            crate::cpu::Instruction::ASub(crate::cpu::ThreeAddrs {
                dst: crate::cpu::AddressRegister(4),
                op1: crate::cpu::AddressRegister(5),
                op2: Some(crate::cpu::AddressRegister(6)),
                imm: (0),
            }),
            crate::cpu::Instruction::ASub(crate::cpu::ThreeAddrs {
                dst: crate::cpu::AddressRegister(4),
                op1: crate::cpu::AddressRegister(5),
                op2: Some(crate::cpu::AddressRegister(6)),
                imm: (2),
            }),
            crate::cpu::Instruction::ASub(crate::cpu::ThreeAddrs {
                dst: crate::cpu::AddressRegister(4),
                op1: crate::cpu::AddressRegister(5),
                op2: None,
                imm: (12),
            }),
            crate::cpu::Instruction::SetTag {
                dst: crate::cpu::Register(2),
                src: crate::cpu::AddressRegister(1),
            },
            crate::cpu::Instruction::GetTag {
                dst: crate::cpu::AddressRegister(3),
                src: crate::cpu::Register(2),
            },
            crate::cpu::Instruction::SetPayload {
                dst: crate::cpu::Register(4),
                src: crate::cpu::AddressRegister(3),
            },
            crate::cpu::Instruction::GetPayload {
                dst: crate::cpu::AddressRegister(5),
                src: crate::cpu::Register(4),
            },
            crate::cpu::Instruction::Int(0x03),
            crate::cpu::Instruction::Uncons {
                car: crate::cpu::Register(1),
                cdr: crate::cpu::Register(2),
                src: crate::cpu::Register(3),
            },
            crate::cpu::Instruction::Car(crate::cpu::TwoRegs {
                dst: crate::cpu::Register(4),
                src: crate::cpu::Register(5),
            }),
            crate::cpu::Instruction::Cdr(crate::cpu::TwoRegs {
                dst: crate::cpu::Register(6),
                src: crate::cpu::Register(7),
            }),
            crate::cpu::Instruction::SetCar(crate::cpu::TwoRegs {
                dst: crate::cpu::Register(8),
                src: crate::cpu::Register(9),
            }),
            crate::cpu::Instruction::SetCdr(crate::cpu::TwoRegs {
                dst: crate::cpu::Register(10),
                src: crate::cpu::Register(11),
            }),
            crate::cpu::Instruction::Add(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Add(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Add(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Sub(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Sub(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Sub(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Mul(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Mul(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Mul(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Eq(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Eq(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Eq(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Ne(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Ne(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Ne(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Lt(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Lt(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Lt(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Lte(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Lte(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Lte(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Gt(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Gt(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Gt(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::Gte(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(0),
            }),
            crate::cpu::Instruction::Gte(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: Some(crate::cpu::Register(3)),
                imm: crate::cpu::Word::fixnum(5),
            }),
            crate::cpu::Instruction::Gte(crate::cpu::ThreeRegs {
                dst: crate::cpu::Register(1),
                op1: crate::cpu::Register(2),
                op2: None,
                imm: crate::cpu::Word::fixnum(10),
            }),
            crate::cpu::Instruction::IDiv {
                div: crate::cpu::Register(1),
                rem: crate::cpu::Register(2),
                op1: crate::cpu::Register(3),
                op2: Some(crate::cpu::Register(4)),
                imm: crate::cpu::Word::fixnum(0),
            },
            crate::cpu::Instruction::IDiv {
                div: crate::cpu::Register(1),
                rem: crate::cpu::Register(2),
                op1: crate::cpu::Register(3),
                op2: Some(crate::cpu::Register(4)),
                imm: crate::cpu::Word::fixnum(5),
            },
            crate::cpu::Instruction::IDiv {
                div: crate::cpu::Register(1),
                rem: crate::cpu::Register(2),
                op1: crate::cpu::Register(3),
                op2: None,
                imm: crate::cpu::Word::fixnum(5),
            },
            crate::cpu::Instruction::MakeClosure {
                dst: crate::cpu::Register(5),
                code: crate::cpu::AddressRegister(6),
            },
        ];
        for i in 0..asm.len() {
            assert_eq!((i, &asm[i]), (i, &expected[i]));
        }
    }
}
