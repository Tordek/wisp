# CPU

## Registers

- 16 GP Registers R0-R16 containing Lisp objects
- 8 Address objects A0-8 contain raw values
- Of of these, 3 are aliased:
  - SP, pointer to head of stack - alias for A5
  - PC, pointer to current instruction - alias for A6
  - ENV, pointer to environment - alias for A7

## Opcodes

### Control Flow

- NOP
- HALT
- JUMP {target} - With 4 variants for {address+imm}, {register}, {absolute}, {relative}
- JT reg, {target} - With 4 variants for {address+imm}, {register}, {absolute}, {relative}
- JF reg, {target} - With 4 variants for {address+imm}, {register}, {absolute}, {relative}
- CALL {reg} - With 4 variants for {address+imm}, {register}, {absolute}, {relative}
- RETURN
- MAKE_CLOSURE dst, code

### Arithmetic:

- ADD, to: Reg, op1: Reg, op2: Reg/Imm
- MUL, to: Reg, op1: Reg, op2: Reg/Imm
- SUB, to: Reg, op1: Reg, op2: Reg/Imm
- DIV, to: Reg, op1: Reg, op2: Reg/Imm
- IDIV, div: Reg, rem: Reg, op1: Reg, op2: Reg/Imm
- NEG, to: Reg, op: Reg

- AADD, to: Adr, op1: Adr, op2: Adr/Imm
- ASUB, to: Adr, op1: Adr, op2: Adr/Imm
- ANEG, to: Reg, op: Reg

### Bitwise
- OR
- AND
- XOR
- SHL
- SHR

- AOR
- AAND
- AXOR
- ASHL
- ASHR

### Data

- MOV reg, imm --- Must be a valid word.
- MOV reg1, reg2
- LOAD reg, adr, off -- reg = *(adr+off)
- STORE reg, adr, off -- *(adr+off) = reg

- AMOV adr, imm
- AMOV adr1, adr2
- ALOAD adr, src -- adr = *(src+off)
- ASTORE dst, adr -- *(dst+off) = adr

- LOADPAYLOAD adr, reg - Loads the PAYLOAD of a WORD into an ADDRESS token.
- SETPAYLOAD reg, adr
- LOADTAG adr, reg
- SETTAG reg, adr
- SETTAG reg, imm

- LOADLOCAL reg, depth, slot ?
- STORELOCAL depth, slot, reg ?
- LOADGLOBAL reg, symbol - scoped per-program
- LOADSGLOBAL reg, symbol - scoped for the machine

### Comparison

- EQ
- LT
- GT
- LEQ
- GEQ
- NE

- AEQ
- ALT
- AGT
- ALEQ
- AGEQ
- ANE

### Cons cells

- CONS to, car, cdr
- UNCONS car, cdr, cons
- CAR to, cons
- CDR to, cons
- SET_CAR cons, value
- SET_CDR cons, value

### Type ops

- TYPEP dst, obj, register
- TYPEP dst, obj, type
- TYPE dst, obj
- CHECK_TYPE obj, type

### Memory

- ALLOC dst, {type} [, {size}]

## Preloaded symbols

On reset, program execution starts at RESET_VECTOR.

The CPU requires certain Root symbols to be defined at the start of RAM:

Minimally, usually in ROM:

- NIL
- T

Afterwards, loaded from a Bootstrap program:

Type symbols:

- CONS
- FIXNUM
- POINTER
- STRING

Trap symbols:

- INVALID-INSTRUCTION
- INVALID-WORD
- ALLOCATION-ERROR

## Interrupts

0x00-0x7f are reserved; 0x80 are user-defined.

Interrupt pointers are set up at 0xf00. When ain interrupt is called, all registers
are saved to the stack. Traps MUST exit with IRETURN, which recovers all registers
(except A0).

```c
alloc() {
  // Stack contains:
  // storage_location
  // data_hi
  // data_lo
  // size
  // type <- sp
  // type is a prototype for the requested object, that is a Word with the tag set as needed but empty Payload.

  // allocate <size> WORDs into obj. {type} can be used for metadata or
  // type optimization as needed
  obj = allocate(size);

  obj[0] = data_lo;
  obj[1] = data_hi;

  A0 = obj
  R0 = Word(type.tag, A0)

  // before exiting, the four parameters must be discarded from the stack
  // Afterwards, all 
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
