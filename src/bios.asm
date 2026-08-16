; Firmware reserved memory: 0x00 to 0x10000
; This holds any firmware-specific variables like the default keyboard
; handler's circular buffer.
.equ freeptr: 0x10
.equ kbstart: 0x18
.equ kbend: 0x20
.equ kblen: 0x28
.equ kbbufferstart: 0x30
.equ kbbufferend: 0x80
.equ symboltable: 0x90

; Special symbols
.equ quote: 0xa0
.equ if: 0xa8

; Known memory mapped devices.
; 0x00ff_ffff_00000000 forwards contains devices.
.equ ramdevice: 0x00ffffff00000000 ; The very first MMAPPED device is Ram
.equ vgastart: 0x00ffffff000b8000
.equ cursorpos: 0x00ffffff000ba000
.equ pressedkeyid: 0x00ffffff000c0010

.equ fixnumtag: 0
.equ symboltag: 2
.equ constag: 3
.equ functiontag: 4
.equ chartag: 6
.equ stringtag: 7
.equ cons_free_start: 0x10000
.equ cons_free_end: 0x20000
.equ gen_free_start: 0x20000
.equ gen_free_end: 0x30000

; Firmware constants
.org 0x00fffffffff00000
    boot_str:
        .str "Booting...\n"
    boot_program:
        .str "\"juan\""
    trap_str:
        .str "You broke the computer!\n"
    not_found:
        .str "Symbol not found."
    not_function:
        .str "Not a function."
    oom:
        .str "Out of memory!"

.org 0x00fffffffff01000
bootstrap:
    ; Initial setup

    ; Set stack position
    MOV V10, 'ramdevice
    MOV SP, [V10 + 0x10]

    ; Set up base BIOS interrupts variables
    MOV V11, 'kbbufferstart
    MOV ['kbstart], V11
    MOV ['kbend], V11
    MOV V11, #10
    MOV ['kblen], V11
    MOV V10, #0
    MOV ['cursorpos], V10

    ; Set interrupt handlers
    MOV VBR, 0x7f00
    MOV V11, 'trap
    MOV [VBR], V11
    MOV V11, 'alloc
    MOV [VBR + 16], V11
    MOV [VBR + 24], V11
    MOV V11, 'video_interrupt
    MOV [VBR + 1920], V11
    MOV V11, 'keyboard_interrupt
    MOV [VBR + 1928], V11
    EI

    MOV A0, #$'boot_str
    CALL 'format


; TODO: Attempt to load LISP image

; Fall back to defining a LISP
; Need to start setting up symbols and stuff.
  fallback:
    MOV V11, 'cons_free_start
    MOV CONS_FREE, V11
    MOV V11, 'cons_free_end
    MOV CONS_END, V11
    MOV V11, 'gen_free_start
    MOV GEN_FREE, V11
    MOV V11, 'gen_free_end
    MOV GEN_END, V11

    MOV V0, #3
    MOV V1, 0x6C696E ; 'nil'
    MOV V5, #$0
    MOV V6, #2
    REQ V0, V5, V6 ; Allocate a string containting "nil"
    MOV V1, #!0 ; Temporary undefined symbol
    MOV V5, #!0 ; Allocate a symbol ["nil", nil]
    MOV V6, #2
    REQ V5, V5, V6
    GETPAYLOAD V10, V5
    MOV [V10 + 8], V5 ; Actually write NIL into the plist
    MOV NIL, V5 ; Define the real NIL

    MOV ['symboltable], NIL ; Special! Start an empty list
    CONS V4, NIL, NIL
    MOV ['symboltable], V4

    MOV V0, #1
    MOV V1, 0x74 ; 't'
    MOV V5, #$0
    MOV V6, #2
    REQ V0, V5, V6 ; Allocate a string containting "t"
    MOV V1, NIL 
    MOV V5, #!0 ; Allocate a symbol ["t", nil]
    MOV V6, #2
    REQ V5, V5, V6
    MOV T, V5
    MOV V6, ['symboltable] ; prepend
    CONS V0, V5, V6
    MOV ['symboltable], V0

    MOV V0, #5
    MOV V1, 0x65746F7571 ; 'quote'
    MOV V5, #$0
    MOV V6, #2
    REQ V0, V5, V6 ; Allocate a string containting "quote"
    MOV V1, NIL 
    MOV V5, #!0 ; Allocate a symbol ["quote", nil]
    MOV V6, #2
    REQ V5, V5, V6
    MOV ['quote], V5
    MOV V6, ['symboltable] ; prepend
    CONS V0, V5, V6
    MOV ['symboltable], V0

    MOV V0, #2
    MOV V1, 0x6669 ; 'if'
    MOV V5, #$0
    MOV V6, #2
    REQ V0, V5, V6 ; Allocate a string containting "if"
    MOV V1, NIL 
    MOV V5, #!0 ; Allocate a symbol ["if", nil]
    MOV V6, #2
    REQ V5, V5, V6
    MOV ['if], V5
    MOV V6, ['symboltable] ; prepend
    CONS V0, V5, V6
    MOV ['symboltable], V0

    ; Create the root ENV
    MOV ENV, NIL ; Empty list
    CONS V5, NIL, NIL ; (NIL . NIL)
    CONS ENV, V5, ENV ; ((NIL . NIL))
    CONS V5, T, T ; (T . T)
    CONS ENV, V5, ENV ; ((T . T) (NIL . NIL))
    MOV V4, ['if]
    MOV V5, #123
    CONS V5, V4, V5
    CONS ENV, V5, ENV ; (IF . 123)

    MOV A0, ENV
    CALL 'print

    MOV V11, 'repl_notfound
    MOV [VBR + 64], V11
    MOV V11, 'repl_notfunction
    MOV [VBR + 80], V11

  dumbloop:
    MOV A0, #\Newline
    INT 0xf0
    MOV A1, #0x07
    MOV A0, #\>
    INT 0xf0
    MOV A0, #\Space
    INT 0xf0
    MOV A0, #$'boot_program
    CALL 'format
    MOV A0, #\Newline
    INT 0xf0
    MOV A0, #$'boot_program
    MOV A1, #0
    CALL 'read_string
    MOV A0, R0 ; Put the response into A0
    CALL 'eval
    MOV A0, R0 ; Put the response into A0
    CALL 'print
    MOV A0, #\Newline
    INT 0xf0
    ; HALT
    ; JUMP 'dumbloop

  loop:
    CALL 'repl
    JUMP 'loop

video_interrupt:
    PUSH A0  ;
    PUSH A1
    PUSH V2  ;
    PUSH V3  ;
    PUSH V4  ;
    PUSH V5  ;
    PUSH V10  ;
    PUSH V11  ;
    PUSH V12  ;
    ; Read cursor position.
    MOV V2, ['cursorpos]

    ; Find Row(r4), Col(r3).
    DIV V4, V3, V2, #80

    ; If c == '\n', row++, col=0
    EQ V5, A0, #\Newline
    JUMPIF V5, 'newline; ; GOTO: newline.

    ; Else, print character and advance cursor.
    ; 0xb8000 + cursorpos(r2) * 2 = char
    ; 0xb8000 + cursorpos(r2) * 2 + 1 = attrib
    MUL V2, V2, #2
    ADD V2, V2, #'vgastart; ; VGA Start
    GETPAYLOAD V10, V2
    ; Save CHAR
    GETPAYLOAD V12, A0
    MOV8 [V10], V12
    ; Save ATTR
    GETPAYLOAD V12, A1
    MOV8 [V10 + 1], V12
    ; advance COLUMN
    ADD V3, V3, #1

    ; if col>79, newline.
    GT V5, V3, #79
    JUMPIFNOT V5, 'no_newline
    newline:
    ; Newline
    ADD V4, V4, #1
    MOV V3, #0
    no_newline:

    ; If row == 25, scroll (row--)
    GTE V5, V4, #25
    JUMPIFNOT V5, 'savecursor
    MOV V10, 0xb8000
    MOV V11, 0xb80a0
    MEMCPY V10, V11, 3840 ; Slide everything up one.
    MOV V10, 0xb8f00
    MOV V11, 0x07
    MOV V12, \Space
  clearline_loop:
    MOV8 [V10], V12
    AADD V10, V10, 1
    MOV8 [V10], V11
    AADD V10, V10, 1
    EQ V5, V10, 0xb8fa0
    JUMPIFNOT V5, 'clearline_loop
    MOV V3, #0
    MOV V4, #24
savecursor:
    ; Save cursor position.
    MUL V4, V4, #80
    ADD V2, V4, V3
    MOV ['cursorpos], V2

    POP V12
    POP V11
    POP V10
    POP V5
    POP V4
    POP V3
    POP V2
    POP A1
    POP A0
    IRETURN

; On keypress, adds the key to the circular buffer.
keyboard_interrupt:
    PUSH V10
    PUSH A0
    MOV V10, ['pressedkeyid]
    CALL 'kbpush
    POP A0
    POP V10
    IRETURN

; Expects
; A0: Prototype (Object with the expected tag set, but its payload is ignored)
; V1: Size of allocation
; Results: A0 is modified.
alloc:
    ; PUSH V10
    ; PUSH V11
    ; MOV V10, ['freeptr]
    ; SETPAYLOAD A0, V10
    ; GETPAYLOAD V11, A1 ; *freeptr  += len
    ; AADD V10, V10, V11 ; todo: Align
    ; MOV ['freeptr], V10
    ; POP V11
    ; POP V10
    ; IRETURN
    MOV A0, #$'oom
    CALL 'format
    HALT
    JUMP 'alloc

;;;
;;; Prints something in A0
;;;
print:
    TYPEP V1, A0, 'symboltag ; Tag == symbol?
    JUMPIFNOT V1, 'print_notsym
    JUMP 'print_symbol
print_notsym:
    TYPEP V1, A0, 'fixnumtag ; Tag == fixnum?
    JUMPIFNOT V1, 'print_notfixnum
    JUMP 'print_number
print_notfixnum:
    TYPEP V1, A0, 'stringtag ; Tag == string?
    JUMPIFNOT V1, 'print_notstring
    JUMP 'print_string
print_notstring:
    TYPEP V1, A0, 'constag ; Tag == cons?
    JUMPIFNOT V1, 'print_notcons
    JUMP 'print_cons
print_notcons:
    JUMP 'print_arbitrary

print_symbol:
    GETPAYLOAD V10, A0 ; Read pointer to symbol table
    MOV A0, [V10] ; Read pointer to name
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Take string length
    AADD A1, V10, 8
    CALL 'print_stringslice
    RETURN

print_string: ; Prints the string at A0
    PUSH A0
    MOV A0, #\"
    MOV A1, #0x07
    INT 0xf0
    POP A0
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Get length
    AADD A1, V10, 8 ; Skip header
    CALL 'print_stringslice
    MOV A0, #\"
    MOV A1, #0x07
    INT 0xf0
    MOV RN, #0
    RETURN

; Takes a fixnum in r0.
print_number:
    ; V10: points to characters
    ; V4: points to length
    MOV V10, SP ; Pointer to string
    ASUB SP, SP, 24
    MOV V4, #0 ; String slice length
  print_number_loop:
    ASUB V10, V10, 1
    ADD V4, V4, #1
    DIV A0, V3, A0, #10 ; Take A0 /= 10, V3 = A0 % 10
    GETPAYLOAD V13, V3
    AADD V13, V13, \0      ; char + '0'
    MOV8 [V10], V13       ; Put char
    EQ A1, A0, #0 ; zerop
    JUMPIFNOT A1, 'print_number_loop
    MOV A0, V4
    MOV A1, V10
    CALL 'print_stringslice
    AADD SP, SP, 24
    MOV RN, #0
    RETURN

; takes an unknown word in r0
print_arbitrary:
    MOV V4, A0
    MOV A1, #0x07
    MOV A0, #\#
    INT 0xf0
    MOV A1, #0x07
    MOV A0, #\<
    INT 0xf0

    MOV A0, #0
    GETTAG V10, V4
    SETPAYLOAD A0, V10
    PUSH V4
    CALL 'print
    POP V4

    MOV A1, #0x07
    MOV A0, #\:
    INT 0xf0

    MOV A0, #0
    GETPAYLOAD V10, V4
    SETPAYLOAD A0, V10
    CALL 'print

    MOV A1, #0x07
    MOV A0, #\>
    INT 0xf0
    MOV RN, #0
    RETURN

print_cons:
    PUSH A0
    MOV A0, #\(
    MOV A1, #0x07
    INT 0xf0
    POP A0
  print_cons_next:
    PUSH A0
    CAR A0, A0
    CALL 'print
    POP A0
    CDR A0, A0
    EQ A1, A0, NIL
    JUMPIF A1, 'print_cons_end

    PUSH A0
    MOV A0, #\Space
    MOV A1, #0x07
    INT 0xf0
    POP A0

    TYPEP A1, A0, 'constag ; if cons?
    JUMPIF A1, 'print_cons_next
    ; 

    PUSH A0
    MOV A0, #\.
    MOV A1, #0x07
    INT 0xf0
    MOV A0, #\Space
    MOV A1, #0x07
    INT 0xf0
    POP A0
    CALL 'print
  print_cons_end:
    MOV A0, #\)
    MOV A1, #0x07
    INT 0xf0
    MOV RN, #0
    RETURN

; Expects Length in A0, Data in A1
print_stringslice:
  print_stringslice_loop:
    LTE V4, A0, #0
    JUMPIFNOT V4, 'print_stringslice_loop_continue
    MOV RN, #0 ; No return values.
    RETURN
  print_stringslice_loop_continue:
    PUSH A0
    MOV8 V11, [A1]
    MOV A0, #\0
    SETPAYLOAD A0, V11
    AADD A1, A1, 1
    PUSH A1
    MOV A1, #0x07
    INT 0xf0
    POP A1
    POP A0
    SUB A0, A0, #1
    JUMP 'print_stringslice_loop
  
trap:
    MOV A0, #$'trap_str
    CALL 'format
  trap_loop:
    HALT
    JUMP 'trap_loop

repl_notfound:
    MOV A0, #$'not_found
    CALL 'format
    IRETURN

repl_notfunction:
    MOV A0, #$'not_function
    CALL 'format
    IRETURN

format:
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Put Length in A0
    AADD A1, V10, 8 ; Put pointer to data in A1
    JUMP 'print_stringslice

; Puts V10 into the keyboard circular buffer.
; Traps if full.
kbpush:
    PUSH V11
    PUSH V12
    PUSH V13
    PUSH A0
    MOV V11, ['kbend]          ; V11 = *end
    AADD V12, V11, 1             ; V12 = *end + 1
    MOV V13, 'kbbufferend
    GTE A0, V12, V13 ; If a2 > limit, 
    JUMPIFNOT A0, PC + 32
    ASUB V12, V12, 80            ; a2 -= size
    MOV V13, ['kbstart]
    EQ A0, V12, V13             ; if *end+1 == *start
    JUMPIFNOT A0, PC + 32
    INT 0                     ; trap
    MOV8 [V11], V10              ; *end = V10
    MOV ['kbend], V12          ; end = (end+1%10)
    POP A0
    POP V13
    POP V12
    POP V11
    RETURN



; Puts the first char in the circular buffer into V10
kbpop:
    CALL 'kbpending
    JUMPIF A0, PC + 32
    RETURN

    PUSH A1
    PUSH V12
    PUSH V13
    MOV V12, ['kbstart]
    MOV V10, [V12]
    AADD V12, V12, 1             ; V12 = *start + 1
    MOV V13, 'kbbufferend
    GTE A1, V12, V13 ; If a2 > limit, 
    JUMPIFNOT A1, PC + 32
    ASUB V12, V12, 80            ; a2 -= size
    MOV ['kbstart], V12
    POP V13
    POP V12
    POP A1
    RETURN

; Returns whether there are available characters into A0
kbpending:
    PUSH V10
    PUSH V11
    MOV V10, ['kbstart]
    MOV V11, ['kbend]
    NE A0, V10, V11
    POP V11
    POP V10
    RETURN

repl:
    ; Loop until there's something to look at in the KB buffer.
    CALL 'kbpop
    JUMPIF A0, PC + 48
    HALT
    JUMP 'repl
    MOV A0, #\0
    SETPAYLOAD A0, V10
    MOV A1, #0x07
    INT 0xf0
    JUMP 'repl

; (read-string "string" 0)
; Receives a String in A0, returns some lisp object in R0.
; Takes offset to start from in A1, end of read symbol in R1
; As helpers: V2 contains remaining characters, V10 the char being read
read_string:
    TYPEP V2, A0, 'stringtag       ; is string?
    JUMPIF V2, 'read_is_string
    MOV R0, NIL
    MOV RN, #0            ; if not, return nothing
    RETURN
  read_is_string:

    GETPAYLOAD V10, A0
    MOV V2, [V10]
    SUB V2, V2, A1
    LTE V3, V2, #0 ; finished? (len <= 0)
    JUMPIFNOT V3, 'read_string_available
    MOV R0, NIL
    MOV RN, #0            ; if not, return nothing
    RETURN
  read_string_available:
    GETPAYLOAD V13, A1 ; skip first n + header
    AADD V10, V10, V13 + 8
    ; peek
    MOV8 V11, [V10]

; Expects stringbuf in V10, Char in V11, start in A1, len in V2
  read_stringslice: 
    CALL 'skip_whitespace ;

    EQ V3, V11, \' ; Quote?
    JUMPIFNOT V3, 'read_notquote
    JUMP 'read_quote
  read_notquote:

    EQ V3, V11, \" ; String?
    JUMPIFNOT V3, 'read_notstring
    JUMP 'read_string_string
  read_notstring:

    EQ V3, V11, \( ; List?
    JUMPIFNOT V3, 'read_notlist
    JUMP 'read_list
  read_notlist:

    LT V3, V11, \0
    JUMPIF V3, 'read_notnumber
    GT V3, V11, \9
    JUMPIF V3, 'read_notnumber ; Number?
    JUMP 'read_number
  read_notnumber:

    JUMP 'read_symbol ; Symbol?


; Expects:
; V10: Pointer to string
; A1: Start
; V2: Length
; Returns:
; V10: pointer to string + 1
; V11: char at string[n]
; A1: Start + 1
; V2: Length - 1
; OK
read_string_next_char:
    AADD V10, V10, 1
    ADD A1, A1, #1
    SUB V2, V2, #1
    LTE V3, V2, #0
    JUMPIF V3, 'read_string_next_char_eof
    MOV8 V11, [V10]
    RETURN
read_string_next_char_eof:
    MOV V11, 0xffff
    RETURN

; Expects:
; V10: Pointer to string
; V11: char at string
; A1: Start
; V2: Length
; Returns:
; V10: pointer to string + n
; V11: char at string[n]
; A1: Start + n
; V2: Length - n
; where n depends on the number of whitespace characters
; OK
skip_whitespace:
    EQ V3, V11, \Space
    JUMPIFNOT V3, 'done
    EQ V3, V11, 0xffff
    JUMPIF V3, 'done
    CALL 'read_string_next_char
    JUMP 'skip_whitespace
done:
    RETURN


; Expects:
; V10: Pointer to string
; V11: char at string
; V1: Start
; V2: Length
; Returns:
; V10: pointer to string + n
; A0: a number
; V11: char at string[n]
; V1: Start + n
; V2: Length - n
; OK
read_number:
    MOV A0, #0
  read_number_loop:
    LT V3, V11, \0
    JUMPIF V3, 'rnnotnumber
    GT V3, V11, \9
    JUMPIF V3, 'rnnotnumber ; Number?

    MUL A0, A0, #10
    MOV V4, #0
    SETPAYLOAD V4, V11
    SUB V4, V4, #0x30
    AADD A0, A0, V4
    CALL 'read_string_next_char
    JUMP 'read_number_loop
  rnnotnumber:
    MOV R0, A0
    RETURN

; OK
read_quote:
    CALL 'read_string_next_char ; Skip '
    CALL 'read_stringslice
    MOV V3, NIL
    CONS V3, R0, V3
    MOV A0, ['quote]
    CONS R0, A0, V3
    MOV R1, #1
    RETURN

read_list:
    CALL 'read_string_next_char ; Skip (
    CALL 'skip_whitespace ;
    EQ V3, V11, \)
    JUMPIFNOT V3, 'read_list_nonempty
    CALL 'read_string_next_char ; Skip )
    MOV R0, NIL
    MOV R1, #1
    RETURN

  read_list_nonempty:
    CALL 'read_stringslice ; Puts the new element in A0 and advances pointers.
    MOV V4, NIL
    CONS V4, R0, V4 ; (h)
    PUSH V4

  read_list_loop:
    CALL 'skip_whitespace ;
    EQ V3, V11, \)
    JUMPIFNOT V3, 'read_list_more_items
    CALL 'read_string_next_char ; Skip )
    POP R0
    MOV R1, #1
    RETURN ; Done: A0 has the list, A1 has start of next

  read_list_more_items:
    EQ V3, V11, \. ; Cons pair
    JUMPIFNOT V3, 'read_list_more_list
    PUSH V4
    CALL 'read_string_next_char ; Skip .
    CALL 'read_stringslice ; Puts the new element in A0 and advances pointers.
    PUSH R0
    CALL 'skip_whitespace ;
    CALL 'read_string_next_char ; Skip ) TODO: Die if not )
    POP R0
    POP V4
    SETCDR V4, R0
    POP R0
    MOV R1, #1
    RETURN

  read_list_more_list:
    PUSH V4
    CALL 'read_stringslice ; Puts the new element in A0 and advances pointers.
    MOV V5, NIL
    CONS A0, R0, V5 ; (h)
    POP V4
    SETCDR V4, A0
    MOV V4, A0
    JUMP 'read_list_loop

    

read_symbol:
    MOV V13, NIL
    ASUB SP, SP, 256 ; Scratch buffer
    MOV V13, SP
    MOV V4, #0 ; Strlen
  read_symbol_loop:
    EQ V3, V11, \Space ; End of symbol
    JUMPIF V3, 'read_symbol_endstring
    EQ V3, V11, \) ; End of symbol
    JUMPIF V3, 'read_symbol_endstring
    EQ V3, V11, 0xFFFF ; End of string
    JUMPIF V3, 'read_symbol_endstring
    MOV8 [V13], V11
    AADD V13, V13, 1 ; Advance buffer
    ADD V4, V4, #1 ; Increase len
    ; TODO: Die if >256
    CALL 'read_string_next_char
    JUMP 'read_symbol_loop
  read_symbol_endstring:
    MOV V13, SP ; V4 + V13 is the stringslice

    PUSH V4
    PUSH V11
    PUSH V10 ; Keep strptr
    PUSH V2 ; Keep curlen
    PUSH A1 ; Keep next position

    ; Find symbol
    ; Traverse symbol table
    MOV V2, ['symboltable]

    PUSH V4 ; Keep the new string length
    PUSH V13
  find_symbol_loop: 
    EQ V3, V2, NIL ; End of table
    JUMPIF V3, 'symbol_not_found

    UNCONS A0, V2, V2 ; A1 = symbol, V2 = next.
    
    GETPAYLOAD V10, A0 ; Get pointer to str
    MOV V6, [V10] ; Fetch string
    GETPAYLOAD V10, V6 ; Find string2 length (V4 has our string length already)
    MOV V5, [V10]
    EQ V3, V4, V5
    JUMPIFNOT V3, 'find_symbol_loop ; If lengths differ, end.

    MOV V13, [SP] ; Reset stringslice
    AADD V10, V10, 8 ; skip header
    ; compare loop:
  find_symbol_strcmp_loop:
    MOV8 V11, [V10] ; Read char from existing symbol
    MOV8 V12, [V13] ; Read char from new symbol
    EQ V3, V11, V12
    JUMPIFNOT V3, 'find_symbol_loop
    SUB V5, V5, #1 ; length--
    AADD V10, V10, 1
    AADD V13, V13, 1
    EQ V3, V5, #0 ; length = 0?
    JUMPIFNOT V3, 'find_symbol_strcmp_loop ; If there's still data, keep comparing.

    ; No more data left, strings are the same.
    POP V13
    POP V4
    JUMP 'read_symbol_end

  symbol_not_found:
    POP V13 
    POP V4 

  ; intern: Make room for header
    AADD V13, SP, 32

    ; Interning
    MOV V8, #$0 ; Request a string.
    ADD V6, V4, #15 ; Round size
    DIV V6, V7, V6, #8
    REQ V0, V8, V6
    GETPAYLOAD V11, V0             ; V8 points to stringobj
    MEMCPY V11, V13, V4 + #8        ; Put strdata into V11

    MOV V5, #!0 ; Request symbol
    MOV V6, #2
    MOV A1, NIL ; empty plist
    REQ A0, V5, V6

    MOV A1, ['symboltable]
    CONS A1, A0, A1
    MOV ['symboltable], A1 ; symbol table = cons(newsym, symbol table)

  read_symbol_end:  ; Ensure: V10, V11, A1, V2
    POP A1 ; End of symbol
    POP V2 ;
    POP V10
    POP V11
    POP V4
    AADD SP, SP, 256
    MOV R0, A0
    RETURN

read_string_string:
    ASUB SP, SP, 256 ; Scratch buffer
    MOV V13, SP
    MOV V4, #0 ; Strlen
    CALL 'read_string_next_char ; Skip "
  read_string_loop:
    EQ V3, V11, \" ; End of string ?
    JUMPIF V3, 'read_string_endstring
    EQ V3, V11, \\ ; Backslash: Ignore specialness.
    JUMPIFNOT V3, 'read_string_addchar
    CALL 'read_string_next_char ; Skip \
  read_string_addchar:
    MOV8 [V13], V11
    AADD V13, V13, 1 ; Advance buffer
    ADD V4, V4, #1 ; Increase len
    ; TODO: Die if >256
    CALL 'read_string_next_char
    JUMP 'read_string_loop
  read_string_endstring:
    CALL 'read_string_next_char ; Skip "
    PUSH V4
    MOV V13, SP
    MOV V5, #$0 ; Request a string.
    ADD V6, V4, #15 ; Round to next word
    DIV V6, V7, V6, #8
    REQ A0, V5, V6
    GETPAYLOAD V11, A0 ; Put data into string
    MEMCPY V11, V13, V4 + #8
    AADD SP, SP, 264
    MOV R0, A0
    RETURN

; Expects:
; Object in A0.
; Environment in ENV.
eval:
    ; Numbers and strings evaluate to themselves.
    TYPEP A1, A0, 'fixnumtag ; Tag == fixnum?
    JUMPIFNOT A1, 'eval_notfixnum
    MOV R0, A0
    MOV RN, #1
    RETURN
eval_notfixnum:

    TYPEP A1, A0, 'stringtag ; Tag == string?
    JUMPIFNOT A1, 'eval_notstring
    MOV R0, A0
    MOV RN, #1
    RETURN
eval_notstring:

    ; Symbols require a lookup in ENV.
    TYPEP A1, A0, 'symboltag ; Tag == symbol?
    JUMPIFNOT A1, 'eval_notsym
    JUMP 'eval_symbol
eval_notsym:

    TYPEP A1, A0, 'constag ; Tag == cons?
    JUMPIFNOT A1, 'eval_notcons
    JUMP 'eval_cons
eval_notcons:

    INT 0x08 ; Evaluating an invalid object.

; Expects:
; Object in A0.
; Environment in ENV.
eval_symbol:
    MOV V4, ENV

  eval_symbol_find_loop:
    EQ V2, V4, NIL
    JUMPIFNOT V2, 'eval_symbol_next
    INT 0x0a ; Interrupt: not found.
  eval_symbol_next:
    UNCONS A1, V4, V4 ; hd, tl
    UNCONS A1, V2, A1 ; sym, val
    EQ V3, A1, A0 ;
    JUMPIFNOT V3, 'eval_symbol_find_loop ; Try again
    MOV R0, V2 ; Found!
    RETURN

; Expects:
; Object in A0.
; Environment in ENV.
eval_cons:
    UNCONS A0, A1, A0

    MOV V3, ['quote]
    EQ V2, A0, V3
    JUMPIFNOT V2, 'eval_cons_notquote
    CAR R0, A1
    RETURN
  eval_cons_notquote:

    MOV V3, ['if] ; (if cond t f) => A0 = if, A0 = (cond t if)
    EQ V2, A0, V3
    JUMPIFNOT V2, 'eval_cons_notif
    UNCONS A0, A1, A1 ; Put condition in A0 ; A0 = cond, A1 = (t if)
    PUSH A1
    CALL 'eval ; Eval it
    POP A1
    EQ V2, A0, NIL ; true?
    UNCONS A0, A1, A1 ; A0 = t, A1 = (f)
    JUMPIFNOT V2, 'eval_cons_true
    CAR A0, A1 ; A0 = f
  eval_cons_true:
    CALL 'eval ; Eval whatever is at A0
    RETURN

  eval_cons_notif:
    ; CALL 'eval ; Get the function in 'eval
    TYPEP V2, A0, 'functiontag ; Is function?
    JUMPIF V2, 'eval_function_call
    INT 0x0a ; Not a function call.

  eval_function_call:
    ; TODO: Eval all params, then call apply


    RETURN

.org 0xffffffffffffffe0
    JUMP 'bootstrap