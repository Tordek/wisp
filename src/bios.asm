.ord 0
    nil: .symbol 'nil_symbol
    t: .symbol 't_symbol
    symbol: .symbol 'symbol_symbol
    cons: .symbol 'cons_symbol
    fixnum: .symbol 'fixnum_symbol

.ord 0x3000
    nil_str: .str "nil"
    t_str: .str "t"
    symbol_str: .str "symbol"
    cons_str: .str "cons"
    fixnum_str: .str "fixnum"
    nil_symbol:
        .w 'nil_str
        .w 'nil
    t_symbol:
        .w 't_str
        .w 'nil
    symbol_symbol:
        .w 'symbol_str
        .w 'nil
    cons_symbol:
        .w 'cons_str
        .w 'nil
    fixnum_symbol:
        .w 'fixnum_str
        .w 'nil

.ord 0x7ef0
    .w 'bootstrap

.ord 0x7f00
    .w 'trap
    .w 'alloc
    .w 'alloc
    .w 'alloc

.ord 0x8680
    video_interrupt_hook_address: .w 'video_interrupt
    keyboard_interrupt_hook_address: .w 'keyboard_interrupt

.ord 0x18000
cursorpos: .w #0
pressedkeyid: .w #0
freeptr: .w 0x30000

.ord 0x19000
bootstrap:
    MOV SP, 0x30000 ; TODO: pass initial symbols for resolution.
    MOV R0, #0
    MOV A0, R0
    MOV ['cursorpos], A0
    MOV R0, \#H
    MOV R1, #0x07
    INT 0xf0
    MOV R0, \#e
    INT 0xf0
    MOV R0, \#l
    INT 0xf0
    MOV R0, \#l
    INT 0xf0
    MOV R0, \#o
    INT 0xf0
    MOV R0, \#,
    INT 0xf0
    MOV R0, \# 
    INT 0xf0
    MOV R0, \#n
    INT 0xf0
    MOV R0, \#o
    INT 0xf0
    MOV R0, \#w
    INT 0xf0
    MOV R0, \# 
    INT 0xf0
    MOV R0, \#w
    INT 0xf0
    MOV R0, \#i
    INT 0xf0
    MOV R0, \#t
    INT 0xf0
    MOV R0, \#h
    INT 0xf0
    MOV R0, \#Newline
    INT 0xf0
    MOV R1, #0x27
    MOV R0, \#V
    INT 0xf0
    MOV R1, #0x43
    MOV R0, \#G
    INT 0xf0
    MOV R1, #0x10
    MOV R0, \#A
    INT 0xf0
    MOV R1, #0x85
    MOV R0, \#!
    INT 0xf0
    MOV R0, \#Newline
    INT 0xf0
    MOV R0, \#A
    MOV R1, #0x61
    MOV R0, ['nil]
    CALL 'print
    MOV R0, \#Newline
    INT 0xf0
    MOV R0, ['t]
    CALL 'print
    MOV R0, \#Newline
    INT 0xf0
    MOV R0, #0
    CALL 'print
    MOV R0, \#Newline
    MOV R0, #123
    CALL 'print
    MOV R0, \#Newline
    INT 0xf0
    MOV R1, #0x10
    MOV R0, \#A
    INT 0xf0
    MOV R1, #0x10
    MOV R0, \#A
    INT 0xf0
    MOV R1, #0x10
    MOV R0, \#A
    INT 0xf0
repl:
    HALT
    JUMP 'repl

video_interrupt:
    PUSH A0
    PUSH A1
    PUSH A2
    PUSH R0
    PUSH R1
    PUSH R2
    PUSH R3
    PUSH R4
    PUSH R5
    ; Read cursor position.
    MOV R2, ['cursorpos]

    ; Find Row(r4), Col(r3).
    DIV R4, R3, R2, #80

    ; If c == '\n', row++, col=0
    EQ R5, R0, \#Newline
    JUMPIF R5, [PC + 176]; ; GOTO: newline.

    ; Else, print character and advance cursor.
    ; 0xb8000 + cursorpos(r2) * 2 = attrib
    ; 0xb8000 + cursorpos(r2) * 2 + 1 = char
    MUL R2, R2, #2
    GETPAYLOAD A0, R2
    ADD A0, A0, 0xB8000; ; VGA Start
    ; Save ATTR
    GETPAYLOAD A2, R1
    MOV8 [A0], A2
    ; Save CHAR
    GETPAYLOAD A2, R0
    MOV8 [A0 + 1], A2
    ; advance COLUMN
    ADD R3, R3, #1

    ; if col>79, newline.
    GT R5, R3, #79
    JUMPIFNOT R5, 'no_newline
    ; Newline
    ADD R4, R4, #1
    MOV R3, #0
    no_newline:

    ; If row == 25, scroll (row--)
    GTE R5, R4, #25
    JUMPIFNOT R5, 'savecursor
    MOV A0, 0xb8000
    MOV A1, 0xb80a0
    MEMCPY A0, A1, 3920 ; Slide everything up one.
    MOV R3, #0
    MOV R4, #24
savecursor:
    ; Save cursor position.
    MUL R4, R4, #80
    ADD R2, R4, R3
    MOV ['cursorpos], R2


    POP R5
    POP R4
    POP R3
    POP R2
    POP R1
    POP R0
    POP A2
    POP A1
    POP A0
    IRETURN

keyboard_interrupt:
    PUSH A0
    PUSH R0
    PUSH R1
    MOV R0, \#0
    MOV A0, ['pressedkeyid]
    SETPAYLOAD R0, A0
    MOV R1, #0x07
    INT 0xf0
    POP R1
    POP R0
    POP A0
    IRETURN

alloc:
    PUSH A1
    PUSH A2
    PUSH A3
    PUSH R1
    MOV A0, ['freeptr]
    GETPAYLOAD A2, R1    ; *freeptr  += len
    ADD A3, A0, A2
    MOV ['freeptr], A3
    POP R1
    POP A3
    POP A2
    POP A1
    IRETURN

print:
    TYPEP R1, R0, 2 ; Tag == symbol?
    JUMPIFNOT R1, '_aftersym
    GETPAYLOAD A0, R0 ; Read payload (pointer to symbol table)
    MOV A0, [A0] ; Read first element of symbol (pointer to name)
    CALL 'printstring
    JUMP '_endprint
_aftersym:
    TYPEP R1, R0, 1 ; Tag == fixnum?
    JUMPIFNOT R1, '_afterfixnum
    EQ R1, R0, #0 ; zerop
    JUMPIFNOT R1, '_nonzero
    MOV A0, 0x30
    PUSH A0
    MOV R2, #1
    PUSH R2
    MOV A0, SP
    CALL 'printstring
    ADD SP, SP, 16
    JUMP '_endprint
    ; TODO: Handle negatives
    _nonzero:   

_afterfixnum:
_endprint:
    RETURN

printstring: ; Prints the string at A0
    MOV R2, [A0] ; Read length
    MOV R1, #0x07
    ADD A0, A0, 8 ; 
  printstringloop:
    EQ R4, R2, #0
    JUMPIF R4, 'endprintloop
    SUB R2, R2, #1
    MOV A3, 6
    SETTAG R0, A3 ; Make char
    MOV8 A1, [A0]
    ADD A0, A0, 1
    SETPAYLOAD R0, A1
    MOV R1, 0x07
    INT 0xf0
    JUMP 'printstringloop
    endprintloop:
    RETURN

trap:
    HALT
