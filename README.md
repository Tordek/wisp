# CPU

## Registers

- 16 GP Registers R0-R16 containing Lisp objects
- 4 Address objects A0-3 contain raw values
- n Special registers containing raw data:
  - SP, pointer to head of stack
  - PC, pointer to current instruction
  - ENV, pointer to environment

## Opcodes

### Control Flow

- NOP
- HALT
- JUMP {target} - With 4 variants for {address}, {register}, {absolute}, {relative}
- JT reg, {target} - With 4 variants for {address}, {register}, {absolute}, {relative}
- JF reg, {target} - With 4 variants for {address}, {register}, {absolute}, {relative}
- CALL {reg} - With 4 variants for {address}, {register}, {absolute}, {relative}
- RETURN
- MAKE_CLOSURE dst, code

### Arithmetic:

- ADD, to: 4, op1: 4, op2: 4
- MUL, to: 4, op1: 4, op2: 4
- SUB, to: 4, op1: 4, op2: 4
- DIV, to: 4, op1: 4, op2: 4
- IDIV - returns 2 values

### Data

- LOAD reg, {literal}, Where a Literal is either NIL, T, an INTEGRAL or an ADDRESS
- LOAD reg, {constant} Where constant is an relative position of the CONSTANTS area of the program
- MOV toReg, fromReg
- LOADLOCAL reg, depth, slot ?
- STORE depth, slot, reg ?
- LOADGLOBAL reg, symbol - scoped per-program
- LOADSGLOBAL reg, symbol - scoped for the machine
- LDPAYLOAD {Ax}, reg - Loads the PAYLOAD of a WORD into an ADDRESS token.

### Comparison

- EQ
- LT
- GT
- LEQ
- GEQ
- NE

### Cons cells

- CONS to, car, cdr
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

Additionally, at ALLOC_VECTOR, and ALLOC_CONS_VECTOR two functions need to be
defined with special semantics:

```c
alloc() {
  // Stack contains:
  // storage_location
  // data_hi
  // data_lo
  // size
  // type <- sp

  // allocate <size> WORDs into obj. {type} can be used for metadata or
  // type optimization as needed
  obj = allocate(size);

  obj[0] = data_lo;
  obj[1] = data_hi;

  A0 = obj
  R0 = Pointer(A0)

  // before exiting, the four parameters must be discarded from the stack
  // Afterwards, all 
}
```

alloc_cons is a specialized version that will always have type=CONS and size=2.
Instad of Pointer(A0) it stores Cons(A0).

They MUST return the pointer at A0 and restore ALL other registers, even ones defined
as callee-saved. On error, they MUST jump to TRAP-HAMDLER.

When the CPU detects an error,

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
