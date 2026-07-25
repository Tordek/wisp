# CPU

## Registers

- 16 GP Registers R0-R16 containing Lisp objects
- 8 Address objects A0-8 contain raw values
- Of of these, 3 are aliased:
  - SP, pointer to head of stack - alias for A5
  - PC, pointer to current instruction - alias for A6
  - ENV, pointer to environment - alias for A7

## Opcodes

### Lisp instructions

The generally higher-level calls that compiled Lisp code should use.

Reg refers to any Word register; Adr refers to any Address register, including
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

- GETPAYLOAD adr, reg - Loads the PAYLOAD of a WORD into an ADDRESS register.
- SETPAYLOAD reg, adr
- GETTAG adr, reg
- SETTAG reg, adr
- SETTAG reg, imm

#### Interrupt

- INT {int}
- IRETURN

## Preloaded symbols

On reset, program execution starts at RESET_VECTOR.

Lisp instructions require some symbols to be defined starting at 0x00.

- NIL
- T
- CONS
- FIXNUM
- POINTER
- STRING
- INVALID-INSTRUCTION
- INVALID-WORD
- ALLOCATION-ERROR

## Interrupts

0x00-0x7f are reserved; 0x80 are user-defined.

Interrupt pointers are set up at 0xf00. When an interrupt is called, all registers
are saved to the stack. Traps MUST exit with IRETURN n, which recovers all
registers below n.

Alloc (int 0x01) and Cons (int 0x02) requires a hook to exist with semantics
similar to this:

```rust
// Params in R0, R1
fn alloc(size: Word, typ: Word) -> (Word, Address) {
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

The first 12 registers are caller-saved, as needed; their values can be modified
freely by the function call.

The first 8 are used for parameter-passing and for returning values.

When more than 5 values are needed (either for passing arguments or receiving
results), R6 points to a contiguous, mixed ARRAYDATA of params.

The last 4 registers may be used as temporary storage, but the function MUST
restore their values before exiting.

On return, R7 indicates how many of the parameters contain values.

- R0-7 hold arguments to calls. Caller-saved.
- R0-5 hold results from calls.
- R6 may contain a pointer to extra values returned by the call.
- R7 contains the count of Values returned by the call. Only the first R{R7-1} values are valid after a call.
- R8-11 are temp variables, may be clobbered. Caller-saved.
- R12-15 are temp variables, may not be clobbered. Callee-saved.

- A0-3 are Address registers, able to hold raw data, usually pointers.
