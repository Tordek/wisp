.equ cursorpos: 0x28000 ; TODO: Map to VGA (00fffffff000b8000)
.equ pressedkeyid: 0x28008 ; TODO: Map to KB DMA (00fffffff000c0000)
.equ freeptr: 0x28010
.equ kbstart: 0x28018
.equ kbend: 0x28020
.equ kblen: 0x28028
.equ kbbufferstart: 0x28030
.equ kbbufferend: 0x28080
.equ symboltable: 0x28090
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
    quote_str: .str "quote"
    nil_symbol:
        .w #$'nil_str
        .w #!'nil_symbol
    t_symbol:
        .w #$'t_str
        .w #!'nil_symbol
    quote_symbol:
        .w #$'quote_str
        .w #!'nil_symbol
    trap_str: .str "You broke the computer!"
    symbol_symbol:
        .w 'symbol_str
        .w #!'nil_symbol
    cons_symbol:
        .w 'cons_str
        .w #!'nil_symbol
    fixnum_symbol:
        .w 'fixnum_str
        .w #!'nil_symbol
    boot_str:
        .str "Booting...\n"
    boot_program:
        .str " a"

barecons:
    .w #!'t_symbol
    .w #!'t_symbol
nil_entry:
    .w #!'nil_symbol
    .w #['fixnum_entry

fixnum_entry:
    .w #!'fixnum_symbol
    .w #['symbol_entry

symbol_entry:
    .w #!'symbol_symbol
    .w #['t_entry

t_entry:
    .w #!'t_symbol
    .w #!'nil_symbol

numberstr:
    ; .str "1234"
    ; .str "'1234"
    ; .str "\"A string\" "
    .str "t"
otherstring:
    .str " (a list is (made up) of 12 . symbols)"

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
    MOV A0, #['nil_entry
    MOV ['symboltable], A0

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
    EI

    MOV R0, #$'boot_str
    CALL 'format

  dumbloop:
    MOV R0, #$'boot_program
    MOV R1, #0
    CALL 'read_string
    CALL 'eval
    CALL 'print
    MOV R0, #\Newline
    INT 0xf0
    HALT
    JUMP 'dumbloop


loop:
    CALL 'repl
    JUMP 'loop

video_interrupt:
    PUSH A0  ;
    PUSH A1  ;
    PUSH A2  ;
    PUSH R0  ;
    PUSH R1
    PUSH R2  ;
    PUSH R3  ;
    PUSH R4  ;
    PUSH R5  ;
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
    MOV A1, 0x07
    MOV A2, #\Space
  clearline_loop:
    MOV8 [A0], A1
    ADD A0, A0, 1
    MOV8 [A0], A2
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

; Expects
; R0: Prototype (Object with the expected tag set, but its payload is ignored)
; R1: Size of allocation
; Results: R0 is modified.
alloc:
    PUSH A0
    PUSH A1
    MOV A0, ['freeptr]
    SETPAYLOAD R0, A0
    GETPAYLOAD A1, R1    ; *freeptr  += len
    ADD A0, A0, A1 ; todo: Align
    MOV ['freeptr], A0
    POP A1
    POP A0
    IRETURN

;;;
;;; Print
;;;
print:
    TYPEP R1, R0, 'symboltag ; Tag == symbol?
    JUMPIFNOT R1, 'print_notsym
    JUMP 'print_symbol
print_notsym:
    TYPEP R1, R0, 'fixnumtag ; Tag == fixnum?
    JUMPIFNOT R1, 'print_notfixnum
    JUMP 'print_number
print_notfixnum:
    TYPEP R1, R0, 'stringtag ; Tag == string?
    JUMPIFNOT R1, 'print_notstring
    JUMP 'print_string
print_notstring:
    TYPEP R1, R0, 'constag ; Tag == cons?
    JUMPIFNOT R1, 'print_notcons
    JUMP 'print_cons
print_notcons:
    JUMP 'print_arbitrary

print_symbol:
    GETPAYLOAD A0, R0 ; Read pointer to symbol table
    MOV R0, [A0] ; Read pointer to name
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
    MOV R0, R4
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
    MOV R0, #$'trap_str
    CALL 'print
  trap_loop:
    HALT
    JUMP 'trap_loop

format:
    GETPAYLOAD A0, R0
    MOV R0, [A0]
    ADD A0, A0, 8
    CALL 'print_stringslice
    RETURN

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
; Takes offset to start from in R1, end of read symbol in R1
; As helpers: R2 contains remaining characters, A0 the char being read
read_string:
    TYPEP R2, R0, 'stringtag       ; is string?
    JUMPIF R2, 'read_is_string
    MOV R7, #0            ; if not, return nothing
    RETURN
  read_is_string:
    GETPAYLOAD A0, R0    
    MOV R2, [A0]
    SUB R2, R2, R1
    LTE R3, R2, #0 ; finished? (len <= 0)
    JUMPIFNOT R3, 'read_string_available
    RETURN

  read_string_available:
    ADD A0, A0, 8 ; Skip header
    GETPAYLOAD A3, R1 ; skip first n
    ADD A0, A0, A3
    ; peek
    MOV8 A1, [A0]

; Expects stringbuf in A0, Char in A1, start in R1, len in R2
  read_stringslice: 
    CALL 'skip_whitespace ;

    EQ R3, A1, \' ; Quote?
    JUMPIFNOT R3, 'read_notquote
    JUMP 'read_quote
  read_notquote:

    EQ R3, A1, \" ; String?
    JUMPIFNOT R3, 'read_notstring
    JUMP 'read_string_string
  read_notstring:

    EQ R3, A1, \( ; List?
    JUMPIFNOT R3, 'read_notlist
    JUMP 'read_list
  read_notlist:

    LT R3, A1, \0
    JUMPIF R3, 'read_notnumber
    GT R3, A1, \9
    JUMPIF R3, 'read_notnumber ; Number?
    JUMP 'read_number
  read_notnumber:

    JUMP 'read_symbol ; Symbol?


; Expects:
; A0: Pointer to string
; R1: Start
; R2: Length
; Returns:
; A0: pointer to string + 1
; A1: char at string[n]
; R1: Start + 1
; R2: Length - 1
; OK
read_string_next_char:
    ADD A0, A0, 1
    ADD R1, R1, #1
    SUB R2, R2, #1
    LTE R3, R2, #0
    JUMPIF R3, 'read_string_next_char_eof
    MOV8 A1, [A0]
    RETURN
read_string_next_char_eof:
    MOV A1, 0xffff
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
; OK
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
; OK
read_number:
    MOV R0, #0
  read_number_loop:
    LT R3, A1, \0
    JUMPIF R3, 'rnnotnumber
    GT R3, A1, \9
    JUMPIF R3, 'rnnotnumber ; Number?

    MUL R0, R0, #10
    MOV R4, #0
    SETPAYLOAD R4, A1
    SUB R4, R4, #0x30
    ADD R0, R0, R4
    CALL 'read_string_next_char
    JUMP 'read_number_loop
  rnnotnumber:
    RETURN

; OK
read_quote:
    CALL 'read_string_next_char ; Skip '
    CALL 'read_stringslice
    MOV R3, ['nil]
    CONS R3, R0, R3
    MOV R0, #!'quote_symbol
    CONS R0, R0, R3
    RETURN

read_list:
    CALL 'read_string_next_char ; Skip (
    CALL 'skip_whitespace ;
    EQ R3, A1, \)
    JUMPIFNOT R3, 'read_list_nonempty
    CALL 'read_string_next_char ; Skip )
    MOV R0, ['nil]
    RETURN

  read_list_nonempty:
    CALL 'read_stringslice ; Puts the new element in R0 and advances pointers.
    MOV R4, ['nil]
    CONS R4, R0, R4 ; (h)
    PUSH R4

  read_list_loop:
    CALL 'skip_whitespace ;
    EQ R3, A1, \)
    JUMPIFNOT R3, 'read_list_more_items
    CALL 'read_string_next_char ; Skip )
    POP R0
    RETURN ; Done: R0 has the list, R1 has start of next

  read_list_more_items:
    EQ R3, A1, \. ; Cons pair
    JUMPIFNOT R3, 'read_list_more_list
    PUSH R4
    CALL 'read_string_next_char ; Skip .
    CALL 'read_stringslice ; Puts the new element in R0 and advances pointers.
    CALL 'skip_whitespace ;
    CALL 'read_string_next_char ; Skip ) TODO: Die if not )
    POP R4
    SETCDR R4, R0
    POP R0
    RETURN

  read_list_more_list:
    PUSH R4
    CALL 'read_stringslice ; Puts the new element in R0 and advances pointers.
    MOV R5, ['nil]
    CONS R0, R0, R5 ; (h)
    POP R4
    SETCDR R4, R0
    MOV R4, R0
    JUMP 'read_list_loop

    

read_symbol:
    MOV A3, ['nil]
    SUB SP, SP, 256 ; Scratch buffer
    MOV A3, SP
    MOV R4, #0 ; Strlen
  read_symbol_loop:
    EQ R3, A1, \Space ; End of symbol
    JUMPIF R3, 'read_symbol_endstring
    EQ R3, A1, \) ; End of symbol
    JUMPIF R3, 'read_symbol_endstring
    EQ R3, A1, 0xFFFF ; End of string
    JUMPIF R3, 'read_symbol_endstring
    MOV8 [A3], A1
    ADD A3, A3, 1 ; Advance buffer
    ADD R4, R4, #1 ; Increase len
    ; TODO: Die if >256
    CALL 'read_string_next_char
    JUMP 'read_symbol_loop
  read_symbol_endstring:
    MOV A3, SP ; R4 + A3 is the stringslice

    PUSH R4
    PUSH A1
    PUSH A0 ; Keep strptr
    PUSH R2 ; Keep curlen
    PUSH R1 ; Keep next position

    ; Find symbol
    ; Traverse symbol table
    MOV R2, ['symboltable]

    PUSH R4 ; Keep the new string length
    PUSH A3
  find_symbol_loop: 
    MOV R5, ['nil]
    EQ R3, R2, R5 ; End of table
    JUMPIF R3, 'symbol_not_found

    UNCONS R0, R2, R2 ; R1 = symbol, R2 = next.
    
    GETPAYLOAD A0, R0 ; Get pointer to str
    MOV R6, [A0] ; Fetch string
    GETPAYLOAD A0, R6 ; Find string2 length (R4 has our string length already)
    MOV R5, [A0]
    EQ R3, R4, R5
    JUMPIFNOT R3, 'find_symbol_loop ; If lengths differ, end.

    MOV A3, [SP] ; Reset stringslice
    ADD A0, A0, 8 ; skip header
    ; compare loop:
  find_symbol_strcmp_loop:
    MOV8 A1, [A0] ; Read char from existing symbol
    MOV8 A2, [A3] ; Read char from new symbol
    EQ R3, A1, A2
    JUMPIFNOT R3, 'find_symbol_loop
    SUB R5, R5, #1 ; length--
    ADD A0, A0, 1
    ADD A3, A3, 1
    EQ R3, R5, #0 ; length = 0?
    JUMPIFNOT R3, 'find_symbol_strcmp_loop ; If there's still data, keep comparing.

    ; No more data left, strings are the same.
    POP A3
    POP R4
    JUMP 'read_symbol_end

  symbol_not_found:
    POP A3 
    POP R4 

  ; intern: Make room for header
    ADD A3, SP, 32

    ; Interning
    MOV R0, #$0 ; Request a string.
    ADD R1, R4, #8 ; Of <header+count>
    INT 0x02 ; Alloc
    GETPAYLOAD A1, R0             ; R0 points to stringobj
    MEMCPY A1, A3, R1             ; Put strdata into A1
    PUSH R0                       ; push str

    MOV R0, #!0 ; Request symbol
    MOV R1, #16 ; sizeof symbol
    INT 0x02
    GETPAYLOAD A1, R0 ; Address the symbol
    POP R1            ; Grab the symbol str
    MOV [A1], R1      ; name = str
    MOV R1, ['nil]    ;
    MOV [A1 + 8], R1      ; plist = nil

    MOV R1, ['symboltable]
    CONS R1, R0, R1
    MOV ['symboltable], R1 ; symbol table = cons(newsym, symbol table)

  read_symbol_end:  ; Ensure: A0, A1, R1, R2
    POP R1 ; End of symbol
    POP R2 ;
    POP A0
    POP A1
    POP R4
    ADD SP, SP, 256
    RETURN

read_string_string:
    SUB SP, SP, 256 ; Scratch buffer
    MOV A3, SP
    MOV [SP], SP
    SUB SP, SP, 8 ; string header
    MOV R4, #0 ; Strlen
    CALL 'read_string_next_char ; Skip "
  read_string_loop:
    EQ R3, A1, \" ; End of string
    JUMPIF R3, 'read_string_endstring
    EQ R3, A1, \\ ; Backslash: Ignore specialness.
    JUMPIFNOT R3, 'read_string_addchar
    CALL 'read_string_next_char ; Skip \
  read_string_addchar:
    MOV8 [A3], A1
    ADD A3, A3, 1 ; Advance buffer
    ADD R4, R4, #1 ; Increase len
    ; TODO: Die if >256
    CALL 'read_string_next_char
    JUMP 'read_string_loop
  read_string_endstring:
    CALL 'read_string_next_char ; Skip "
    MOV A3, SP
    PUSH R1
    MOV [A3], R4
    MOV R0, #$0 ; Request a string.
    ADD R1, R4, #8 ; Of <header+count>
    INT 0x02 ; Alloc
    GETPAYLOAD A1, R0
    MEMCPY A1, A3, R4 + #8
    POP R1
    ADD SP, SP, 264
    RETURN

eval:
    RETURN

.org 0xffffffffffffffe0
    .w 'bootstrap
    nil: .w #!'nil_symbol
    t: .w #!'t_symbol