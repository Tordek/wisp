.equ cursorpos: 0x28000 ; TODO: Map to VGA (00fffffff000b8000)
.equ pressedkeyid: 0x28008 ; TODO: Map to KB DMA (00fffffff000c0000)
.equ freeptr: 0x28010
.equ kbstart: 0x28018
.equ kbend: 0x28020
.equ kblen: 0x28028
.equ kbbufferstart: 0x28030
.equ kbbufferend: 0x28080
.equ fixnumtag: 1
.equ symboltag: 2
.equ constag: 3
.equ chartag: 6
.equ stringtag: 7

.org 0x00ffffff00020000
    nil_str: .str "nil"
    t_str: .str "t"
    symbol_str: .str "symbol"
    cons_str: .str "cons"
    fixnum_str: .str "fixnum"
    nil_symbol:
        .w #$'nil_str
        .w 'nil
    t_symbol:
        .w #$'t_str
        .w 'nil
    trap_str: .str "You broke the computer!"
;     symbol_symbol:
;         .w 'symbol_str
;         .w 'nil
;     cons_symbol:
;         .w 'cons_str
;         .w 'nil
;     fixnum_symbol:
;         .w 'fixnum_str
;         .w 'nil 
barecons:
    .w #!'t_symbol
    .w #!'t_symbol
list:
    .w #!'nil_symbol
    .w #['fixnum_entry

fixnum_entry:
    .w #$'fixnum_str
    .w #['symbol_entry

symbol_entry:
    .w #$'symbol_str
    .w #['t_entry

t_entry:
    .w #!'t_symbol
    .w #!'nil_symbol

numberstr:
    .str "1234"

.org 0x00fffffffff19000
bootstrap:
    ; Initial setup
    ; Set starting values
    MOV ['pressedkeyid], A1
    MOV A1, 0x40000
    MOV ['freeptr], A1
    MOV A1, 'kbbufferstart
    MOV ['kbstart], A1
    MOV ['kbend], A1
    MOV A1, #10
    MOV ['kblen], A1
    MOV A0, #0
    MOV ['cursorpos], A0
    ; Set stack position
    MOV SP, ['freeptr]

    ; Set interrupt handlers
    MOV VBR, 0x7f00
    MOV A1, 'trap
    MOV [VBR], A1
    MOV A1, 'alloc
    MOV [VBR + 16], A1
    MOV [VBR + 24], A1
    MOV A1, 'video_interrupt
    MOV [VBR + 1920], A1
    MOV A1, 'keyboard_interrupt
    MOV [VBR + 1928], A1

    ; Hello world!
    MOV R0, #\H
    MOV R1, #0x07
    INT 0xf0
    MOV R0, #\e
    INT 0xf0
    MOV R0, #\l
    INT 0xf0
    MOV R0, #\l
    INT 0xf0
    MOV R0, #\o
    INT 0xf0
    MOV R0, #\,
    INT 0xf0
    MOV R0, #\Space
    INT 0xf0
    MOV R0, #\n
    INT 0xf0
    MOV R0, #\o
    INT 0xf0
    MOV R0, #\w
    INT 0xf0
    MOV R0, #\Space
    INT 0xf0
    MOV R0, #\w
    INT 0xf0
    MOV R0, #\i
    INT 0xf0
    MOV R0, #\t
    INT 0xf0
    MOV R0, #\h
    INT 0xf0
    MOV R0, #\Newline
    INT 0xf0
    MOV R1, #0x27
    MOV R0, #\V
    INT 0xf0
    MOV R1, #0x43
    MOV R0, #\G
    INT 0xf0
    MOV R1, #0x10
    MOV R0, #\A
    INT 0xf0
    MOV R1, #0x85
    MOV R0, #\!
    INT 0xf0
    MOV R0, #\Newline
    INT 0xf0

    ; Print objects
    MOV R0, ['nil]
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    MOV R0, ['t]
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    MOV R0, #0 ;number zero
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    MOV R0, #123 ; number 123
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    MOV R0, #$'fixnum_str ; string fixnum
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    MOV R0, #['barecons ; a simple cons
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    MOV R0, #['list ; list
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
; --- Error
    ; MOV R0, #0
    ; ADD R0, R0, #!0
; --- String-reader
    ; MOV R0, #$'numberstr
    ; CALL 'read_string
    ; CALL 'print

loop:
    CALL 'repl
    JUMP 'loop

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
    EQ R5, R0, #\Newline
    JUMPIF R5, 'newline; ; GOTO: newline.

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
    newline:
    ; Newline
    ADD R4, R4, #1
    MOV R3, #0
    no_newline:

    ; If row == 25, scroll (row--)
    GTE R5, R4, #25
    JUMPIFNOT R5, 'savecursor
    MOV A0, 0xb8000
    MOV A1, 0xb80a0
    MEMCPY A0, A1, 3840 ; Slide everything up one.
    MOV A0, 0xb8f00
    clearline_loop:
    MOV A1, 0x07
    MOV8 [A0], A1
    ADD A0, A0, 1
    MOV A1, #\Space
    MOV8 [A0], A1
    ADD A0, A0, 1
    EQ R0, A0, 0xb8fa0
    JUMPIFNOT R0, 'clearline_loop
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

; On keypress, adds the key to the circular buffer.
keyboard_interrupt:
    PUSH A0
    PUSH R0
    MOV A0, ['pressedkeyid]
    CALL 'kbpush
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
    TYPEP R1, R0, 'symboltag ; Tag == symbol?
    JUMPIFNOT R1, 'print_notsym
    CALL 'print_symbol
    JUMP 'print_end
print_notsym:
    TYPEP R1, R0, 'fixnumtag ; Tag == fixnum?
    JUMPIFNOT R1, 'print_notfixnum
    CALL 'print_number
    JUMP 'print_end
print_notfixnum:
    TYPEP R1, R0, 'stringtag ; Tag == string?
    JUMPIFNOT R1, 'print_notstring
    CALL 'print_string
    JUMP 'print_end
print_notstring:
    TYPEP R1, R0, 'constag ; Tag == cons?
    JUMPIFNOT R1, 'print_notcons
    CALL 'print_cons
    JUMP 'print_end
print_notcons:
    CALL 'print_arbitrary
print_end:
    RETURN

print_symbol:
    GETPAYLOAD A0, R0 ; Read payload (pointer to symbol table)
    MOV R0, [A0] ; Read first element of symbol (pointer to name)
    GETPAYLOAD A0, R0
    MOV R0, [A0] ; Take string length
    ADD A0, A0, 8
    CALL 'print_stringslice
    RETURN

print_string: ; Prints the string at R0
    PUSH R0
    MOV R0, #\"
    MOV R1, 0x07
    INT 0xf0
    POP R0
    GETPAYLOAD A0, R0
    MOV R0, [A0] ; Get length
    ADD A0, A0, 8 ; Skip header
    CALL 'print_stringslice
    MOV R0, #\"
    INT 0xf0
    RETURN

; Takes a fixnum in r0.
print_number:
    ; A0: points to characters
    ; R0: points to length
    MOV A0, SP ; Pointer to string
    SUB SP, SP, 24
    MOV R4, #0 ; String slice length
  print_number_loop:
    SUB A0, A0, 1
    ADD R4, R4, #1
    DIV R0, R3, R0, #10 ; Take R0 /= 10, R3 = R0 % 10
    GETPAYLOAD A3, R3
    ADD A3, A3, \0      ; char + '0'
    MOV8 [A0], A3       ; Put char
    EQ R1, R0, #0 ; zerop
    JUMPIFNOT R1, 'print_number_loop
  print_number_loop_end:
    MOV R0, R4
    EQ R1, R0, #0 ; len = 0?
    JUMPIFNOT R1, 'print_number_nonzero
    MOV R1, 0x07
    MOV R0, #\0
    INT 0xf0
    RETURN
  print_number_nonzero:
    CALL 'print_stringslice
    ADD SP, SP, 24
    RETURN

; takes an unknown word in r0
print_arbitrary:
    MOV R4, R0
    MOV R1, #0x07
    MOV R0, #\#
    INT 0xf0
    MOV R1, #0x07
    MOV R0, #\<
    INT 0xf0
    MOV R0, #0
    GETTAG A0, R4
    SETPAYLOAD R0, A0
    PUSH R4
    CALL 'print
    POP R4
    MOV R1, #0x07
    MOV R0, #\:
    INT 0xf0
    MOV R0, #0
    GETPAYLOAD A0, R4
    SETPAYLOAD R0, A0
    CALL 'print
    MOV R1, #0x07
    MOV R0, #\>
    INT 0xf0
    RETURN

print_cons:
    PUSH R0
    MOV R0, #\(
    MOV R1, 0x07
    INT 0xf0
    POP R0
  print_cons_next:
    PUSH R0
    CAR R0, R0
    CALL 'print
    POP R0
    CDR R0, R0
    EQ R1, R0, #!'nil_symbol
    JUMPIF R1, 'print_cons_end

    PUSH R0
    MOV R0, #\Space
    MOV R1, 0x07
    INT 0xf0
    POP R0

    TYPEP R1, R0, 'constag ; if cons?
    JUMPIF R1, 'print_cons_next
    ; 

    PUSH R0
    MOV R0, #\.
    MOV R1, 0x07
    INT 0xf0
    MOV R0, #\Space
    MOV R1, 0x07
    INT 0xf0
    POP R0
    CALL 'print
  print_cons_end:
    MOV R0, #\)
    MOV R1, 0x07
    INT 0xf0
    RETURN

; Expects Length in R0, Data in [A0]
print_stringslice:
  print_stringslice_loop:
    LTE R4, R0, #0
    JUMPIFNOT R4, 'print_stringslice_loop_continue
    RETURN
  print_stringslice_loop_continue:
    PUSH R0
    MOV8 A1, [A0]
    MOV R0, #\0
    SETPAYLOAD R0, A1
    ADD A0, A0, 1
    MOV R1, 0x07
    INT 0xf0
    POP R0
    SUB R0, R0, #1
    JUMP 'print_stringslice_loop
  
trap:
    MOV R1, 0x70
    ; MOV R0, #$'trap_str
    CALL 'print
  trap_loop:
    HALT
    ; JUMP 'trap_loop

; Puts A0 into the keyboard circular buffer.
; Traps if full.
kbpush:
    PUSH A1
    PUSH A2
    PUSH A3
    PUSH R0
    MOV A1, ['kbend]          ; A1 = *end
    ADD A2, A1, 8             ; A2 = *end + 1
    MOV A3, ['kbbufferend]
    GTE R0, A2, A3 ; If a2 > limit, 
    JUMPIFNOT R0, PC + 32
    SUB A2, A2, 80            ; a2 -= size
    MOV A3, ['kbstart]
    EQ R0, A2, A3             ; if *end+1 == *start
    JUMPIFNOT R0, PC + 32
    INT 0                     ; trap
    MOV [A1], A0              ; *end = A0
    MOV ['kbend], A2          ; end = (end+1%10)
    POP R0
    POP A3
    POP A2
    POP A1
    RETURN



; Puts the first char in the circular buffer into A0
kbpop:
    CALL 'kbpending
    JUMPIF R0, PC + 32
    RETURN

    PUSH R1
    PUSH A2
    MOV A2, ['kbstart]
    MOV A0, [A2]
    ADD A2, A2, 8             ; A2 = *start + 1
    MOV A3, ['kbbufferend]
    GTE R1, A2, A3 ; If a2 > limit, 
    JUMPIFNOT R1, PC + 32
    SUB A2, A2, 80            ; a2 -= size
    MOV ['kbstart], A2
    POP A2
    POP R1
    RETURN

; Returns whether there are available characters into R0
kbpending:
    PUSH A0
    PUSH A1
    MOV A0, ['kbstart]
    MOV A1, ['kbend]
    NE R0, A0, A1
    POP A1
    POP A0
    RETURN

repl:
    ; Loop until there's something to look at in the KB buffer.
    CALL 'kbpop
    JUMPIF R0, PC + 48
    HALT
    JUMP 'repl
    MOV R0, #\0
    SETPAYLOAD R0, A0
    MOV R1, 0x07
    INT 0xf0
    JUMP 'repl

; (read-string "string" 0)
; Receives a String in R0, returns some lisp object in R0.
; Returns offset to start from in R1, end of read symbol in R1
; As helpers: R2 contains remaining characters, A0 the char being read
read_string:
    TYPEP R2, R0, 2       ; is string?
    JUMPIF R2, 'read_is_string
    MOV R7, #0            ; if not, return nothing
    RETURN
  read_is_string:
    GETPAYLOAD A0, R0    
    MOV R2, [A0]
    SUB R2, R2, R1
    LTE R3, R2, #0 ; finished? (len <= 0)
    JUMPIF R3, 'read_end


    ADD A0, A0, 8 ; Skip header
    GETPAYLOAD A4, R1 ; skip first n
    ADD A0, A0, A4

    ; peek
    MOV8 A2, [A0]

    EQ R3, A2, \' ; Quote?
    JUMPIFNOT R3, 'read_notquote
    CALL 'read_quote
    RETURN

  read_notquote:
    EQ R2, A2, \( ; List?
    CALL 'read_list
    RETURN

  read_notlist:
    LT R3, A2, \0
    JUMPIF R3, 'read_notnumber
    GT R3, A2, \9
    JUMPIF R3, 'read_notnumber ; Number?
    CALL 'read_number
    RETURN

  read_notnumber:
    CALL 'read_symbol ; Symbol?
    RETURN


read_end:
    RETURN

; Expects:
; A0: Pointer to string
; R1: Start
; R2: Length
; Returns:
; A0: pointer to string + 1
; A1: char at string[n]
; R1: Start + 1
; R2: Length - 1
read_string_next_char:
    ADD A0, A0, 1
    ADD R1, R1, #1
    SUB R2, R2, #1
    LTE R3, R2, 0
    JUMPIFNOT R3, 'more
    MOV A1, 0xffff
    RETURN
more:
    MOV8 A1, [A0]
    RETURN

; Expects:
; A0: Pointer to string
; A1: char at string
; R1: Start
; R2: Length
; Returns:
; A0: pointer to string + n
; A1: char at string[n]
; R1: Start + n
; R2: Length - n
; where n depends on the number of whitespace characters
skip_whitespace:
    EQ R3, A1, \Space
    JUMPIFNOT R3, 'done
    EQ R3, A1, 0xffff
    JUMPIF R3, 'done
    CALL 'read_string_next_char
    JUMP 'skip_whitespace
done:
    RETURN


; Expects:
; A0: Pointer to string
; A1: char at string
; R1: Start
; R2: Length
; Returns:
; A0: pointer to string + n
; R0: a number
; A1: char at string[n]
; R1: Start + n
; R2: Length - n
read_number:
    MOV R0, #0
  read_number_loop:
    LT R3, A2, \0
    JUMPIF R3, 'rnnotnumber
    GT R3, A2, \9
    JUMPIF R3, 'rnnotnumber ; Number?

    MOV R4, #0
    SETPAYLOAD R4, A1
    SUB R4, R4, #0
    JUMP 'read_number_loop
  rnnotnumber:
    RETURN

read_quote:
    RETURN

read_list:
    RETURN

read_symbol:
    RETURN


.org 0xffffffffffffffe0
    .w 'bootstrap
    nil: .w #!'nil_symbol
    t: .w #!'t_symbol