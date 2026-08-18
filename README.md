# CPU

## Registers

256 general purpose registers. A few are reserved by ISA:

- ENV: Pointer to current environment.
- NIL: Cache for 'nil - on reset, equal to 0x00
- T: Cache for 't - on reset, equal to 0x01
- CONS/GEN_FREE/END: Pointers to start/end of current arena
- SP: Stack pointer
- FP: Pointer to start of current frame, for locals access.
- PC: Program Counter
- VBR: Interrupt table location

And a few are aliased by the [calling convention](^calling_convention).

## Opcodes

### Lisp instructions

The generally higher-level calls that compiled Lisp code should use.

Reg refers to any Word register; Adr refers to any Machine register, including
SP, PC and ENV.

In the 4-operand versions of instructions, the evaluated expression is generally

to <- op1 <op> (op2 + imm)

#### Arithmetic

- ADD to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- MUL to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- SUB to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- DIV div: Reg, rem: Reg, op1: Reg, op2: Reg, imm: WordLiteral
- NEG to: Reg, op: Reg

### Bitwise

- OR to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- AND to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- XOR to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- SHL to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- SHR to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral

#### Comparison

- EQ to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- LT to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- GT to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- LEQ to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- GEQ to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral
- NE to: Reg, op1: Reg, op2?: Reg, imm: WordLiteral

#### Control Flow

- RETURN
- MAKE_CLOSURE dst, code
- CALL to: Reg
- JUMP to: Reg
- JT cond: Reg, to: Reg
- JF cond: Reg, to: Reg

#### Cons cells

- UNCONS car, cdr, cons
- CAR to, cons
- CDR to, cons
- SET_CAR cons, value
- SET_CDR cons, value

#### Data

- MOV reg, imm --- Must be a valid word.
- MOV reg1, reg2
- LOAD reg, adr, off -- reg = \*(adr+off)
- STORE reg, adr, off -- \*(adr+off) = reg

- LOADGLOBAL reg, symbol - scoped per-program
- LOADSGLOBAL reg, symbol - scoped for the machine

#### Type ops

- TYPEP dst, obj, register
- TYPEP dst, obj, type
- TYPE dst, obj
- CHECK_TYPE obj, type

### Low-level instructions

#### Control Flow

- NOP
- HALT
- JUMP to: Adr?+Imm
- JT cond: Adr, to: Adr?+Imm
- JF cond: Adr, to: Adr?+Imm
- CALL to: Adr?+Imm
- RETURN

#### Arithmetic:

- AADD to: Adr, op1: Adr, op2?: Adr, imm: Imm
- ASUB to: Adr, op1: Adr, op2?: Adr, imm: Imm
- ANEG to: Reg, op: Reg

#### Bitwise

- AOR to: Adr, op1: Adr, op2?: Adr, imm: Imm
- AAND to: Adr, op1: Adr, op2?: Adr, imm: Imm
- AXOR to: Adr, op1: Adr, op2?: Adr, imm: Imm
- ASHL to: Adr, op: Adr
- ASHR to: Adr, op: Adr

#### Comparison

- AEQ to: Adr, op1: Adr, op2?: Adr, imm: Imm
- ALT to: Adr, op1: Adr, op2?: Adr, imm: Imm
- AGT to: Adr, op1: Adr, op2?: Adr, imm: Imm
- ALEQ to: Adr, op1: Adr, op2?: Adr, imm: Imm
- AGEQ to: Adr, op1: Adr, op2?: Adr, imm: Imm
- ANE to: Adr, op1: Adr, op2?: Adr, imm: Imm

#### Data

- AMOV adr1, adr2?, imm -- adr1 <- adr2 + imm
- ALOAD adr, src -- adr <- \*(src+off)
- ASTORE dst, adr -- \*(dst+off) <- adr

- MOVAR adr, reg
- MOVRA reg, adr

- GETPAYLOAD adr, reg - Loads the PAYLOAD of a WORD into an MACHINE register.
- SETPAYLOAD reg, adr
- GETTAG adr, reg
- SETTAG reg, adr
- SETTAG reg, imm

#### Interrupt

- INT {int}
- IRETURN

## Preloaded symbols

On reset, program execution starts at RESET_VECTOR.

Lisp instructions require some symbols to be defined and stored in dedicated registers:

- NIL
- T

Conditionals use these for comparison, and they're initialized to 0 and 1 on reset, meaning they will
work correctly, but they won't be LISP objects until they've been setup.

## Interrupts

0x00-0x7f are reserved; 0x80 are user-defined.

Interrupt pointers are set up at 0xf00. When an interrupt is called, all registers
are saved to the stack. Traps MUST exit with IRETURN n, which recovers all
registers below n.

Alloc (int 0x01) and Cons (int 0x02) requires a hook to exist with semantics
similar to this:

```rust
// Params in R0, R1
fn alloc(size: Word, typ: Word) -> (Word, Machine) {
  let address = allocate(size);
  // Returns in R0, A0
  ireturn(Word::pointer(address), address);
}
```

alloc_cons is a specialized version that will always have type=CONS and size=2.

They MUST return the pointer at A0 and restore ALL other registers, even ones defined
as callee-saved. On error, they MUST jump to TRAP-HAMDLER.

When the CPU detects an error,

If R0 contains a Fixnum,

## Calling Convention

Registers A0..7 are used to pass the first few positional parameters. AN holds the count of passed parameters. Any further parameters are passed in the stack.

Return values are placed in R0..3, then RX points to additional storage. RN holds the count of return values.
