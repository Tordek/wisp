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
        ; .str "1"
        ; .str "\"123\""
        ; .str "t"
        ; .str "(123 123)"
        .str "(this is 'a (list \"of\" . things) 1112 21)"
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

    ; Initial frame
    MOV FP, SP

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

    JUMP 'fallback

;;;
;;; Interrupts
;;;

;;; Expects character in A0, attribtue in A1
video_interrupt:
    PUSH A0
    PUSH A1
    PUSH V2
    PUSH V3
    PUSH V4
    PUSH V5
    PUSH V10
    PUSH V11
    PUSH V12
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
    MOV8 [V10 + 1], V11
    AADD V10, V10, 2
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
    PUSH A0
    PUSH AN
    PUSH RN
    MOV A0, ['pressedkeyid]
    MOV AN, #1
    CALL 'kbpush
    POP RN
    POP AN
    POP A0
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

trap:
    MOV A0, #$'trap_str
    CALL 'format
  trap_loop:
    HALT
    JUMP 'trap_loop


; Puts A0 into the keyboard circular buffer if there is room.
kbpush:
    PUSH V10
    PUSH V11
    PUSH V12
    PUSH V13
    MOV V11, ['kbend]            ; V11 = *end
    AADD V12, V11, 1             ; Advance endbuffer
    MOV V13, 'kbbufferend
    GTE V10, V12, V13             ; Wrap around
    JUMPIFNOT V10, PC + 32
    ASUB V12, V12, 80
    MOV V13, ['kbstart]
    EQ V10, V12, V13              ; If full (start=end)
    JUMPIF V10, PC + 48           ; Skip
    MOV8 [V11], A0               ; Else: Write char
    MOV ['kbend], V12            ; And save
    POP V13
    POP V12
    POP V11
    POP V10
    MOV RN, #0
    RETURN

; Puts the first char in the circular buffer into V10
; Returns if any character was read in R0 and the character in R1.
kbpop:
    CALL 'kbpending
    JUMPIF R0, PC + 32
    MOV RN, #1
    RETURN

    PUSH V11
    PUSH V12
    PUSH V13
    MOV V12, ['kbstart]
    MOV R1, [V12]                ; Read the character
    AADD V12, V12, 1             ; Advance startbuffer
    MOV V13, 'kbbufferend
    GTE V11, V12, V13             ; Wrap around.
    JUMPIFNOT V11, PC + 32
    ASUB V12, V12, 80
    MOV ['kbstart], V12
    POP V13
    POP V12
    POP V11
    MOV RN, #2
    RETURN

; Returns 'T/'NIL if there are any characters available in the buffer.
kbpending:
    PUSH V10
    PUSH V11
    MOV V10, ['kbstart]
    MOV V11, ['kbend]
    NE R0, V10, V11
    POP V11
    POP V10
    RETURN

;;;
;;; Fall back to defining a LISP
;;;
; Need to start setting up symbols and stuff.
  fallback:
    ; Create a return value area
    ASUB SP, SP, 256
    MOV RX, SP ; Up to 32 values.

    ; Free memory
    MOV V11, 'cons_free_start
    MOV CONS_FREE, V11
    MOV V11, 'cons_free_end
    MOV CONS_END, V11
    MOV V11, 'gen_free_start
    MOV GEN_FREE, V11
    MOV V11, 'gen_free_end
    MOV GEN_END, V11

    ; Initial symbols: NIL, T
    ; Store them in the symbol "table" for finding.
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

    ; Evaluator helpers: quote, if...
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

    ; Create the root ENV as an AList
    MOV ENV, NIL ; Empty list
    CONS V5, NIL, NIL ; (NIL . NIL)
    CONS ENV, V5, ENV ; ((NIL . NIL))
    CONS V5, T, T ; (T . T)
    CONS ENV, V5, ENV ; ((T . T) (NIL . NIL))

    ; MOV A0, ENV
    ; CALL 'print

    ; Lisp-specific interrupts
    MOV V11, 'repl_notfound
    MOV [VBR + 64], V11
    MOV V11, 'repl_notfunction
    MOV [VBR + 80], V11

;;; Debug helper: Read, eval print a string.
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
    ; MOV A0, R0 ; Put the response into A0
    ; CALL 'eval
    MOV A0, R0 ; Put the response into A0
    CALL 'print
    MOV A0, #\Newline
    INT 0xf0
    HALT
    JUMP 'dumbloop

  loop:
    CALL 'repl
    JUMP 'loop

repl:
    ; Loop until there's something to look at in the KB buffer.
    CALL 'kbpop
    JUMPIF R0, PC + 48
    HALT
    JUMP 'repl
    MOV A0, #\0
    SETPAYLOAD A0, R1
    MOV A1, #0x07
    INT 0xf0
    JUMP 'repl

repl_notfound:
    MOV A0, #$'not_found
    CALL 'format
    IRETURN

repl_notfunction:
    MOV A0, #$'not_function
    CALL 'format
    IRETURN

; (read-string "string" 0)
; Receives a String in A0, returns some lisp object in R0.
; Takes offset to start from in A1, end of read symbol in R1
; During execution keeps available length in V2 and current character in R0
read_string:
    TYPEP V2, A0, 'stringtag       ; is string?
    JUMPIF V2, 'read_is_string
    MOV RN, #0                     ; if not, return nothing
    RETURN
  read_is_string:


  ; Prepare string slice: Put (length - start) in V2
    ; MOV V2, [A0]
    MOV V2, [!A0]
    GETPAYLOAD A0, A0
    SUB V2, V2, A1
    LTE V3, V2, #0 ; finished? (len <= 0)
    JUMPIFNOT V3, 'read_string_available
    MOV RN, #0            ; if not, return nothing
    RETURN
  read_string_available:

    GETPAYLOAD V13, A1 ; skip first n + header
    AADD A0, A0, V13 + 8
    ; peek
    CALL 'read_string_peek

; Expects stringbuf in A0, Char in R0, start in A1, len in V2
  read_stringslice:
    CALL 'skip_whitespace

    EQ V3, R0, \' ; Quote?
    JUMPIF V3, 'read_quote

    EQ V3, R0, \" ; String?
    JUMPIF V3, 'read_string_string

    EQ V3, R0, \( ; List?
    JUMPIF V3, 'read_list

    LT V3, R0, \0
    JUMPIF V3, 'read_notnumber
    GT V3, R0, \9
    JUMPIF V3, 'read_notnumber ; Number?
    JUMP 'read_number
  read_notnumber:

    JUMP 'read_symbol ; Symbol?



; Expects:
; A0: Pointer to string
; A1: Start
; R0: char at string
; V2: Length
; Returns:
; R0: a number
; Modifies:
; A0: pointer to string + n
; A1: Start + n
; V2: Length - n
read_number:
    MOV V18, #0
  read_number_loop:
    EQ V3, RN, #0
    JUMPIF V3, 'rnnotnumber ; EOF
    LT V3, R0, \0
    JUMPIF V3, 'rnnotnumber
    GT V3, R0, \9
    JUMPIF V3, 'rnnotnumber ; Number?

    MUL V18, V18, #10
    MOV V4, #0
    SETPAYLOAD V4, R0
    SUB V4, V4, #0x30
    ADD V18, V18, V4 ; while (isnumber(n)) total = total * 10 + n - '0';
    CALL 'read_string_next_char
    JUMP 'read_number_loop
  rnnotnumber: ; End
    MOV R0, V18
    MOV RN, #1
    RETURN

; Expects:
; A0: Pointer to string
; A1: Start
; R0: char at string
; V2: Length
; Returns:
; R0: (quote <whatever>)
; Modifies:
; A0: pointer to string + n
; A1: Start + n
; V2: Length - n
read_quote:
    CALL 'read_string_next_char ; Skip '
    CALL 'skip_whitespace
    CALL 'read_stringslice
    CONS R0, R0, NIL
    MOV V1, ['quote]
    CONS R0, V1, R0
    MOV RN, #1
    RETURN

; Expects:
; A0: Pointer to string
; A1: Start
; R0: char at string
; V2: Length
; Returns:
; R0: a list
; Modifies:
; A0: pointer to string + n
; A1: Start + n
; V2: Length - n
read_list:
    CALL 'read_string_next_char ; Skip (
    CALL 'skip_whitespace
    EQ V3, R0, \)               ; If (), return NIL
    JUMPIF V3, 'read_list_empty

    
    CALL 'read_stringslice
    MOV V4, NIL
    CONS V4, R0, V4 ; (h) ; Start the list and keep a reference to the head.
    PUSH V4

  read_list_loop:
    CALL 'skip_whitespace
    EQ V3, R0, \)
    JUMPIF V3, 'read_list_end
    EQ V3, R0, \. ; Cons pair
    JUMPIF V3, 'read_list_pair_end
    PUSH V4
    CALL 'read_stringslice ; Grab another component
    CONS R0, R0, NIL ; (h)
    POP V4
    SETCDR V4, R0
    MOV V4, R0
    JUMP 'read_list_loop

  read_list_empty:
    CALL 'read_string_next_char ; Skip )
    MOV R0, NIL
    MOV R1, #1
    RETURN

  read_list_pair_end:
    PUSH V4
    CALL 'read_string_next_char ; Skip .
    CALL 'read_stringslice ; Puts the new element in A0 and advances pointers.
    POP V4
    SETCDR V4, R0
    CALL 'skip_whitespace
    CALL 'read_string_next_char ; Skip ) TODO: Die if not )
    POP R0
    MOV RN, #1
    RETURN

  read_list_end:
    CALL 'read_string_next_char ; Skip )
    POP R0
    MOV RN, #1
    RETURN                ; Done: R0 contains the list.

  read_list_more_items:

; Expects:
; A0: Pointer to string
; R0: char at string
; V1: Start
; V2: Length
; Returns:
; R0: A symbol
; Modifies:
; A0: pointer to string + n
; V1: Start + n
; V2: Length - n
; Interns the symbol
read_symbol:
    MOV V13, NIL
    ASUB SP, SP, 256 ; Scratch buffer
    MOV V13, SP
    MOV V4, #0 ; Strlen
  read_symbol_loop:
    EQ V3, RN, #0     ; EOF
    JUMPIF V3, 'read_symbol_endstring
    EQ V3, R0, \Space ; End of symbol
    JUMPIF V3, 'read_symbol_endstring
    EQ V3, R0, \)     ; End of symbol
    JUMPIF V3, 'read_symbol_endstring
    MOV8 [V13], R0
    AADD V13, V13, 1 ; Advance buffer
    ADD V4, V4, #1 ; Increase len
    ; TODO: Die if >256
    CALL 'read_string_next_char
    JUMP 'read_symbol_loop
  read_symbol_endstring:
    PUSH V4 ; The stack points to the data, so now it's got the whole string.

    MOV V5, #$0
    SETPAYLOAD V5, SP  ; Make a virtual string.

    PUSH V2
    PUSH A1
    PUSH A0

    ; Find symbol
    ; Traverse symbol table
    MOV V2, ['symboltable]


  find_symbol_loop:
    EQ V3, V2, NIL ; End of table?
    JUMPIF V3, 'symbol_not_found

    UNCONS A0, V2, V2 ; A1 = symbol, V2 = next.

    MOV A1, V5
    PUSH A0
    PUSH V5
    CALL 'symbol_equal_name
    POP V5
    POP V7

    JUMPIF R0, 'symbol_found

  symbol_not_found:
  ; intern: Make room for header
    MOV V4, [SP + 24]
    MOV V13, SP + 24

    ; Interning
    MOV V8, #$0 ; Request a string.
    ADD V6, V4, #15 ; Round size
    DIV V6, V7, V6, #8
    REQ V0, V8, V6
    GETPAYLOAD V10, V0             ; V0 points to stringobj
    MEMCPY V10, V13, V4 + #8       ; Put strdata into R0

    MOV V5, #!0 ; Request symbol
    MOV V6, #2
    MOV V1, NIL ; empty plist
    REQ V7, V5, V6

    MOV V1, ['symboltable]
    CONS V1, V7, V1
    MOV ['symboltable], V1 ; symbol table = cons(newsym, symbol table)

  symbol_found:
    POP A0
    POP A1
    POP V2
    MOV R0, V7
    MOV RN, #1
    RETURN

; Expects:
; A0: Pointer to string
; R0: char at string
; V1: Start
; V2: Length
; Returns:
; R0: A string
; Modifies:
; A0: pointer to string + n
; V1: Start + n
; V2: Length - n
read_string_string:
    ASUB SP, SP, 256 ; Scratch buffer
    MOV V13, SP
    MOV V4, #0 ; Strlen
    CALL 'read_string_next_char ; Skip "
  read_string_loop:
    ; EQ V3, RN, #0
    ; JUMPIF V3, 'read_string_error ; EOF before close
    EQ V3, R0, \" ; End of string ?
    JUMPIF V3, 'read_string_endstring
    EQ V3, R0, \\ ; Backslash: Ignore specialness.
    JUMPIFNOT V3, 'read_string_addchar
    CALL 'read_string_next_char ; Skip \
  read_string_addchar:

    MOV8 [V13], R0
    AADD V13, V13, 1 ; Advance buffer
    ADD V4, V4, #1 ; Increase len
    ; GT V3, V4, #256
    ; JUMPIF V3, 'read_string_error ; String larger than scratch buffer.
    CALL 'read_string_next_char
    JUMP 'read_string_loop
  read_string_endstring:
    CALL 'read_string_next_char ; Skip "
    PUSH V4 ; Since the stack points at the data, pushing the length builds the string
    MOV V5, #$0 ; Request a string.
    ADD V6, V4, #15 ; Round to next word
    DIV V6, V7, V6, #8
    REQ R0, V5, V6
    MOV V13, SP
    GETPAYLOAD V11, R0 ; Put data into string
    MEMCPY V11, V13, V4 + #8
    AADD SP, SP, 264
    MOV RN, #1
    RETURN
  read_string_error:
    INT 0 ; Pick a better interrupt.

; Expects:
; A0: Pointer to string
; A1: Start
; V2: Length
; Returns:
; R0: char at string[n]
read_string_peek:
    MOV8 R0, [A0]
    MOV RN, #1
    RETURN

; Expects:
; A0: Pointer to string
; A1: Start
; V2: Length
; Returns:
; R0: char at string[n]
; Modifies
; A0: pointer to string + 1
; A1: Start + 1
; V2: Length - 1
read_string_next_char:
    AADD A0, A0, 1
    ADD A1, A1, #1
    SUB V2, V2, #1
    LTE V3, V2, #0
    JUMPIF V3, 'read_string_next_char_eof
    MOV8 R0, [A0]
    MOV RN, #1
    RETURN
read_string_next_char_eof:
    MOV RN, #0
    RETURN

; Expects:
; A0: Pointer to string
; A1: Start
; V2: Length
; Returns:
; R0: char at string[n]
; Modifies:
; A0: pointer to string + n
; A1: Start + n
; V2: Length - n
; where n depends on the number of whitespace characters
; OK
skip_whitespace:
    CALL 'read_string_peek
  skip_whitespace_loop:
    EQ V3, RN, #0
    JUMPIF V3, 'skip_whitespace_done ; Nothing was read - EOF
    EQ V3, R0, \Space
    JUMPIFNOT V3, 'skip_whitespace_done ; Found non-whitespace
    CALL 'read_string_next_char ; Advance
    JUMP 'skip_whitespace_loop
skip_whitespace_done:
    RETURN



;;;
;;; Eval
;;;
; Expects:
; Object in A0.
; Environment in ENV.
eval:
    ; Numbers and strings evaluate to themselves.
    TYPEP A1, A0, 'fixnumtag ; Tag == fixnum?
    JUMPIF A1, 'eval_self
    TYPEP A1, A0, 'stringtag ; Tag == string?
    JUMPIF A1, 'eval_self

    ; Symbols require a lookup in ENV.
    TYPEP A1, A0, 'symboltag ; Tag == symbol?
    JUMPIF A1, 'eval_symbol
    TYPEP A1, A0, 'constag ; Tag == cons?
    JUMPIF A1, 'eval_cons

    INT 0x08 ; Evaluating an invalid object.

; Expects:
; An object in A0
; Returns:
; The same object in R0
eval_self:
    MOV R0, A0
    MOV RN, #1
    RETURN

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
    EQ V3, A1, A0
    JUMPIFNOT V3, 'eval_symbol_find_loop ; Try again
    MOV R0, V2 ; Found!
    MOV RN, #1
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
    MOV RN, #1
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

;;;
;;; Prints something in A0
;;;
print:
    TYPEP V1, A0, 'symboltag ; Tag == symbol?
    JUMPIF V1, 'print_symbol
    TYPEP V1, A0, 'fixnumtag ; Tag == fixnum?
    JUMPIF V1, 'print_number
    TYPEP V1, A0, 'stringtag ; Tag == string?
    JUMPIF V1, 'print_string
    TYPEP V1, A0, 'constag ; Tag == cons?
    JUMPIF V1, 'print_cons
    JUMP 'print_arbitrary

print_symbol:
    MOV A0, [!A0] ; Name pointer
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Take string length
    AADD A1, V10, 8
    JUMP 'print_stringslice

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


format:
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Put Length in A0
    AADD A1, V10, 8 ; Put pointer to data in A1
    JUMP 'print_stringslice

; Expects symbols in A0, A1
; Returns 't if they have the same name
symbol_equal_name:
    MOV A0, [!A0]      ; SYM1 name
    MOV A1, [!A1]      ; SYM2 name
    JUMP 'string_equal

; Expects strings in A0, A1
; Returns 't if they're equal, 'f is not.
string_equal:
    GETPAYLOAD A0, A0 ; Get pointer to str
    MOV V5, [A0]      ; STR1 len
    GETPAYLOAD A1, A1 ; Get pointer to str
    MOV V6, [A1]      ; STR1 len
    EQ V3, V5, V6
    JUMPIF V3, 'str_eq_same_length
    MOV R0, NIL
    MOV RN, #1
    RETURN
  str_eq_same_length:
    AADD A0, A0, 8
    AADD A1, A1, 8

  str_eq_loop:

    MOV8 V6, [A0]
    MOV8 V7, [A1]
    EQ V3, V5, V6
    JUMPIF V3, 'str_eq_continue

    MOV R0, NIL
    MOV RN, #1
    RETURN

  str_eq_continue:
    AADD A0, A0, 1
    AADD A1, A1, 1
    SUB V5, V5, #1
    EQ V3, V5, #0 ; End
    JUMP 'str_eq_loop


.org 0xffffffffffffffe0
    JUMP 'bootstrap