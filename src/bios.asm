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
.equ builtin: 0xb0
.equ lambda: 0xb8
.equ let: 0xc0
.equ bad_string: 0xc8
.equ unknown_object: 0xd0
.equ not_found: 0xd8
.equ not_fn: 0xe0

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
.equ error_handler_tag: 12
.equ cons_free_start: 0x10000
.equ cons_free_end: 0x20000
.equ gen_free_start: 0x20000
.equ gen_free_end: 0x30000

.equ trap_interrupt: 0x00
.equ lisp_trap: 0x20
.equ video_interrupt: 0xf0

; Firmware constants
.org 0x00fffffffff00000
    boot_str:
        .str "Booting...\n"
    boot_program:
        ; .str "1"
        ; .str "\"123\""
        ; .str "t"
        ; .str "(123 123)"
        ; .str "+"
        .str "(s 1 2)"
        .str "(this is 'a (list \"of\" . things) 1112 21)"
    trap_str:
        .str "You broke the computer!\n"
    not_found_string:
        .str "Symbol not found."
    not_function_str:
        .str "Not a function."
    oom:
        .str "Out of memory!"
    
; Firmware lisp constants
    t_str: .str "t"
    quote_str: .str "quote"
    builtin_str: .str "builtin"
    if_str: .str "if"
    plus_str: .str "+"
    bad_string_str: .str "string-error"
    unknown_object_str: .str "unknown-type"
    not_found_str: .str "not-found"
    not_fn_str: .str "not-a-function"

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
    MOV V11, 'video_interrupt_handler
    MOV [VBR + 1920], V11
    MOV V11, 'keyboard_interrupt_handler
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
video_interrupt_handler:
    PUSH V0
    PUSH V1
    PUSH V2
    PUSH V3
    PUSH V4
    PUSH V5
    PUSH V10
    PUSH V11
    PUSH V12
    ; Read cursor position.
    MOV V2, ['cursorpos]

    ; Find Row(v4), Col(v3).
    DIV V4, V3, V2, #80

    ; If c == '\n', row++, col=0
    EQ V5, A0, #\Newline
    JUMPIF V5, 'newline ; If char == newline, goto newline.

    ; Else, print character and advance cursor.
    ; 0xb8000 + cursorpos(r2) * 2 = char
    ; 0xb8000 + cursorpos(r2) * 2 + 1 = attrib
    MUL V2, V2, #2
    ADD V2, V2, #'vgastart; ; VGA Start
    GETPAYLOAD V12, A0 ; Save CHAR
    MOV8 [!V2], V12
    GETPAYLOAD V12, A1 ; Save ATTR
    MOV8 [!V2 + 1], V12

    ; advance COLUMN
    ADD V3, V3, #1

    ; if col>=79, newline.
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

    MOV V10, 'vgastart
    MOV V11, 'vgastart
    AADD V11, V11, 160
    MEMCPY V10, V11, #3840 ; Slide everything up one.

    AADD V6, V10, 4000 ; 80 * 25 * 2
    AADD V10, V10, 3840
    MOV V11, 0x07
    MOV V12, \Space
  clearline_loop:
    MOV8 [V10], V12
    MOV8 [V10 + 1], V11
    AADD V10, V10, 2
    EQ V5, V10, V6
    JUMPIFNOT V5, 'clearline_loop ; Clear bottom line

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
    POP V1
    POP V0
    IRETURN

; On keypress, adds the key to the circular buffer.
keyboard_interrupt_handler:
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
    REQ V0, V5, #2 ; Allocate a string containting "nil"
    MOV V1, #!0 ; Temporary undefined symbol
    MOV V5, #!0 ; Allocate a symbol ["nil", nil]
    REQ V5, V5, #2
    GETPAYLOAD V10, V5
    MOV [V10 + 8], V5 ; Actually write NIL into the plist
    MOV NIL, V5 ; Define the real NIL

    MOV ['symboltable], NIL ; Special! Start an empty list
    CONS V4, NIL, NIL
    MOV ['symboltable], V4

    MOV EH, NIL ; Initial dumb error handler.

    ; Lisp-specific interrupts
    MOV V11, 'lisp_trap_handler
    MOV [VBR + 256], V11

    MOV A0, #$'t_str
    CALL 'intern_string
    MOV T, R0

    MOV A0, #$'quote_str
    CALL 'intern_string
    MOV ['quote], R0

    MOV A0, #$'if_str
    CALL 'intern_string
    MOV ['if], R0

    MOV A0, #$'builtin_str
    CALL 'intern_string
    MOV ['builtin], R0

    MOV A0, #$'not_fn_str
    CALL 'intern_string
    MOV ['not_fn], R0

    MOV A0, #$'bad_string_str
    CALL 'intern_string
    MOV ['bad_string], R0

    MOV A0, #$'unknown_object_str
    CALL 'intern_string
    MOV ['unknown_object], R0

    MOV A0, #$'not_found_str
    CALL 'intern_string
    MOV ['not_found], R0

    ; Create the root ENV as an AList
    MOV V20, NIL ; Empty list
    CONS V5, NIL, NIL ; (NIL . NIL)
    CONS V20, V5, V20 ; ((NIL . NIL))
    CONS V5, T, T ; (T . T)
    CONS V20, V5, V20 ; ((T . T) (NIL . NIL))

    ; Define builtin functions.

    MOV V0, 0x2B ; '+'
    PUSH V0
    MOV V0, #1 ; strlen
    PUSH V0
    MOV A0, #$0
    SETPAYLOAD A0, SP
    CALL 'intern_string
    POP V0
    POP V0

    MOV V0, ['builtin]
    MOV V1, 'add
    MOV V5, #?0
    REQ V5, V5, #2
    CONS V5, R0, V5     ; (+ . add)
    CONS V20, V5, V20

    MOV A0, V20
    PUSH V20
    CALL 'print
    POP V20

;;; Debug helper: Read, eval print a string.
  dumbloop:
    ; Create error handler
    MOV V0, 'repl_error_handler
    MOV V1, FP
    MOV V2, SP
    MOV V3, 'dumbloop
    MOV V4, V20
    MOV R0, #0
    SETTAG R0, 'error_handler_tag
    REQ R0, R0, #5
    CONS EH, R0, EH ; Set error handler

    MOV A0, #\Newline
    INT 'video_interrupt
    MOV A1, #0x07
    MOV A0, #\>
    INT 'video_interrupt
    MOV A0, #\Space
    INT 'video_interrupt
    MOV A0, #$'boot_program
    CALL 'format
    MOV A0, #\Newline
    INT 'video_interrupt

    PUSH V20
    MOV A0, #$'boot_program
    MOV A1, #0
    CALL 'read_string
    MOV A0, R0 ; Put the response into A0
    POP V20
    MOV A1, V20
    PUSH V20
    CALL 'eval
    MOV A0, R0 ; Put the response into A0
    CALL 'print
    POP V20
    MOV A0, #\Newline
    INT 'video_interrupt

    CDR EH, EH ; Remove error handler on success.
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
    INT 'video_interrupt
    JUMP 'repl

repl_error_handler:
    PUSH A0
    MOV A0, A1
    CALL 'print
    POP V20
    IRETURN

lisp_trap_handler:
    JUMPIFNOT EH, 'trap_handler_die ; If the handler list is empty, just die.

    UNCONS V1, EH, EH ; Pop the error handler.    
    TYPEP V3, V1, 'error_handler_tag
    JUMPIFNOT V3, 'bad_error_handler

    ; - Previous Frame Pointer
    MOV V0, [!V1 + 8]
    MOV [FP], V0
    ; - Stack pointer
    MOV V0, [!V1 + 16]
    MOV [FP+8], V0
    ; - Interruption value
    ; - Status flags
    ; - Previous Program Counter
    MOV V0, [!V1 + 24]
    MOV [FP + 32], V0
    ; Env
    MOV A0, [!V1 + 32]
    JUMP [!V1] ; Hand off to actual handler.

  trap_handler_die:
    HALT
    JUMP 'trap_handler_die ; literally nothing to do but die
  
  bad_error_handler:
    MOV A1, ['bad_error_handler]
    MOV A2, V1
    INT 'lisp_trap

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
    CALL 'read_string_next_char
    EQ R0, R0, \)
    JUMPIFNOT R0, 'read_list_error
    POP R0
    MOV RN, #1
    RETURN

  read_list_end:
    CALL 'read_string_next_char ; Skip )
    POP R0
    MOV RN, #1
    RETURN                ; Done: R0 contains the list.

  read_list_error:
    MOV A1, ['bad_string]
    MOV A2, R0
    INT 'lisp_trap


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
    GTE V3, V4, #256
    JUMPIF V3, 'read_string_error
    CALL 'read_string_next_char
    JUMP 'read_symbol_loop
  read_symbol_endstring:
    PUSH V4
    MOV V13, SP ; V13 points to length + data
    MOV V5, #$0 ; Build a temp string for intern.
    SETPAYLOAD V5, V13

    PUSH V2
    PUSH A1
    PUSH A0

    MOV A0, V5
    CALL 'intern_string

    POP A0
    POP A1
    POP V2
    MOV RN, #1
    RETURN
  read_symbol_error:
    MOV A1, ['bad_string]
    MOV A2, #256
    JUMP 'lisp_trap

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
    MOV A1, ['bad_string]
    MOV A2, V4
    INT 'lisp_trap

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

; Expects:
; A0: String.
; Returns:
; R0: A symbol
; Modifies:
; Allocates the string.
; Puts the symbol in the intern list or returns an existing one.
intern_string:
    MOV V2, ['symboltable]

  find_symbol_loop:
    JUMPIFNOT V2, 'symbol_not_found ; End of table

    UNCONS V3, V2, V2 ; V3 = symbol, V2 = next.
    MOV A1, [!V3] ; Take name string.

    PUSH A0
    PUSH A1
    PUSH V3
    CALL 'string_equal
    POP V3
    POP A1
    POP A0

    JUMPIFNOT R0, 'find_symbol_loop ; If not found, try again.
    MOV R0, V3                      ; If found, return it.
    MOV RN, #1
    RETURN

  symbol_not_found:                 ; If end of table, actually intern it.
    ; Interning
    MOV V8, A0 ; Request a string.
    MOV V4, [!A0] ; Of the proper size
    ADD V6, V4, #15 ; Round size
    DIV V6, V7, V6, #8
    REQ V0, V8, V6
    GETPAYLOAD V13, A0             ; Find str address
    GETPAYLOAD V10, V0             ; V0 points to stringobj
    MEMCPY V10, V13, V4 + #8       ; Put strdata into R0

    MOV V5, #!0 ; Request symbol
    MOV V1, NIL ; empty plist
    REQ V3, V5, #2

    MOV V1, ['symboltable]
    CONS V1, V3, V1
    MOV ['symboltable], V1 ; symbol table = cons(newsym, symbol table)

  symbol_found:
    MOV R0, V3
    MOV RN, #1
    RETURN

;;;
;;; Eval
;;;
; Expects:
; Object in A0.
; Environment in A1.
eval:
    ; Numbers and strings evaluate to themselves.
    TYPEP V1, A0, 'fixnumtag ; Tag == fixnum?
    JUMPIF V1, 'eval_self
    TYPEP V1, A0, 'stringtag ; Tag == string?
    JUMPIF V1, 'eval_self
    TYPEP V1, A0, 'symboltag ; Tag == symbol?
    JUMPIF V1, 'eval_symbol
    TYPEP V1, A0, 'constag ; Tag == cons?
    JUMPIF V1, 'eval_cons

    MOV A1, ['unknown_object]
    MOV A2, A0
    INT 'lisp_trap ; Evaluating an invalid object.

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
; Environment in A1.
eval_symbol:
    MOV V4, A1

  eval_symbol_find_loop:
    JUMPIFNOT V4, 'eval_symbol_not_found
    UNCONS V5, V4, V4 ; hd, tl
    UNCONS V5, V2, V5 ; sym, val
    EQ V3, V5, A0
    JUMPIFNOT V3, 'eval_symbol_find_loop ; Try again
    MOV R0, V2 ; Found!
    MOV RN, #1
    RETURN
  eval_symbol_not_found:
    MOV A1, ['not_found]
    MOV A2, A0
    INT 'lisp_trap ; Interrupt: not found.

; Expects:
; Object in A0.
; Environment in A1.
eval_cons:
    UNCONS A0, V1, A0 ; A0 = head, V1 = args

  ; Special forms: Check if A0 is any of the special forms.
    MOV V3, ['quote]
    EQ V2, A0, V3
    JUMPIF V2, 'eval_quote
    MOV V3, ['if] ; (if cond t f) => A0 = if, A0 = (cond t if)
    EQ V2, A0, V3
    JUMPIF V2, 'eval_cons_if
    JUMP 'eval_function_call


  eval_quote:
    CAR R0, V1
    MOV RN, #1
    RETURN

  eval_cons_if:
    UNCONS A0, V1, V1 ; Put condition in V0 ; V0 = cond, V1 = (t f)
    PUSH V1
    PUSH A1
    CALL 'eval                              ; Eval condition
    POP A1
    POP V1
    UNCONS A0, V1, V1                       ; V0 = t, V1 = (f)
    JUMPIF R0, 'eval_cons_if_body
    CAR A0, V1                              ; V0 = f
  eval_cons_if_body:
    JUMP 'eval                              ; Eval whatever is at A0

  eval_function_call:
    PUSH V1
    PUSH A1
    CALL 'eval ; Get the function in 'eval
    POP A1
    POP V1

    TYPEP V2, R0, 'functiontag ; Is function?
    JUMPIFNOT V2, 'eval_function_call_not_fn
    PUSH R0
    MOV A0, V1
    PUSH A1
    CALL 'eval_list ; Evaluate the list in R0
    MOV A1, R0
    POP A2
    POP A0
    JUMP 'apply
  
  eval_function_call_not_fn:
    MOV A1, ['not_fn]
    INT 'lisp_trap ; Not a function call.

; TODO:
; Expects:
; A0 a list of arguments
; Returns
; R0 a list of evaluated arguments
eval_list:
    MOV R0, A0
    RETURN

; Expects
; A function in A0
; A list of arguments in A1
; Environment in A2
apply:
    MOV V1, [!A0] ; Fetch function type.
    EQ V2, V1, ['builtin] ; Handle native functions.
    JUMPIF V2, 'apply_builtin
  apply_lambda:
    MOV V2, [!A0 + 8] ; Fetch parameter list
    MOV V3, [!A0 + 16] ; Fetch lambda's env
    ; TODO: Assign each param an argument.
    ; ...
    ; CONS V4, paramname, arg
    ; CONS V3, V4, V3 ; prepend assoc to list
    MOV V4, [!A0 + 24] ; Fetch body
    ; For each line in the body, evaluate with the new environment.
    MOV A1, A2
  apply_lambda_loop:
    JUMPIFNOT V4, 'apply_lambda_done
    UNCONS A0, V4, V4
    PUSH V3
    PUSH A1
    CALL 'eval
    POP A1
    POP V3
  apply_lambda_done:
    RETURN

  apply_builtin:
    MOV V20, [!A0 + 8] ; Fetch function location.
    PUSH V20
    MOV V1, A1 ; keep arglist
    MOV AN, #0 ; Arg count.
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1    ; For each of the 8 first args, if any, push them.
    UNCONS A0, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A1, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A2, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A3, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A4, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A5, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A6, V1, V1
    JUMPIFNOT V1, 'call

    ADD AN, AN, #1
    UNCONS A7, V1, V1
    JUMPIFNOT V1, 'call

    PUSH A0              ; After the first 8, allocate some space on the stack.
    MOV A0, V1
    CALL 'listlen
    POP A0
    MUL R0, R0, #8
    GETPAYLOAD R0, R0
    ASUB SP, SP, R0      ; Allocate n words
    MOV V2, SP
  argloop:               ; While there are more arguments, push them onto the stack
    JUMPIFNOT V1, 'call

    UNCONS V3, V1, V1
    MOV [V2], V3
    AADD V2, V2, 8
    ADD AN, AN, #1
    JUMP 'argloop
  call:
    POP V1
    JUMP V1

; Expects:
; A0: A list
; Returns:
; R0: The number of elements in that list.
listlen:
    MOV V2, A0
    MOV R0, #0
    MOV RN, #1
  lenloop:
    JUMPIFNOT V2, 'endlistlen
    ADD R0, R0, #1
    CDR V2, V2
    JUMP 'lenloop

  endlistlen:
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
    INT 'video_interrupt
    POP A0
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Get length
    AADD A1, V10, 8 ; Skip header
    CALL 'print_stringslice
    MOV A0, #\"
    MOV A1, #0x07
    INT 'video_interrupt
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
    INT 'video_interrupt
    MOV A1, #0x07
    MOV A0, #\<
    INT 'video_interrupt

    MOV A0, #0
    GETTAG V10, V4
    SETPAYLOAD A0, V10
    PUSH V4
    CALL 'print_number
    POP V4

    MOV A1, #0x07
    MOV A0, #\:
    INT 'video_interrupt

    MOV A0, #0
    GETPAYLOAD V10, V4
    SETPAYLOAD A0, V10
    CALL 'print_number

    MOV A1, #0x07
    MOV A0, #\>
    INT 'video_interrupt
    MOV RN, #0
    RETURN

print_cons:
    PUSH A0
    MOV A0, #\(
    MOV A1, #0x07
    INT 'video_interrupt
    POP A0
  print_cons_next:
    PUSH A0
    CAR A0, A0
    CALL 'print
    POP A0
    CDR A0, A0
    JUMPIFNOT A0, 'print_cons_end

    PUSH A0
    MOV A0, #\Space
    MOV A1, #0x07
    INT 'video_interrupt
    POP A0

    TYPEP A1, A0, 'constag ; if cons?
    JUMPIF A1, 'print_cons_next


    PUSH A0
    MOV A0, #\.
    MOV A1, #0x07
    INT 'video_interrupt
    MOV A0, #\Space
    MOV A1, #0x07
    INT 'video_interrupt
    POP A0
    CALL 'print
  print_cons_end:
    MOV A0, #\)
    MOV A1, #0x07
    INT 'video_interrupt
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
    INT 'video_interrupt
    POP A1
    POP A0
    SUB A0, A0, #1
    JUMP 'print_stringslice_loop


format:
    GETPAYLOAD V10, A0
    MOV A0, [V10] ; Put Length in A0
    AADD A1, V10, 8 ; Put pointer to data in A1
    JUMP 'print_stringslice

; Expects strings in A0, A1
; Returns 't if they're equal, 'f is not.
string_equal:
    MOV V5, [!A0]      ; STR1 len
    MOV V6, [!A1]      ; STR1 len
    EQ V3, V5, V6
    JUMPIFNOT V3, 'str_neq

    GETPAYLOAD A0, A0 ; Get pointer to str
    GETPAYLOAD A1, A1 ; Get pointer to str
    AADD A0, A0, 8
    AADD A1, A1, 8

  str_eq_loop:

    MOV8 V6, [A0]
    MOV8 V7, [A1]
    EQ V3, V6, V7
    JUMPIFNOT V3, 'str_neq

    AADD A0, A0, 1
    AADD A1, A1, 1
    SUB V5, V5, #1
    EQ V3, V5, #0 ; End
    JUMPIFNOT V4, 'str_eq_loop

    MOV R0, T
    MOV RN, #1
    RETURN

  str_neq:
    MOV R0, NIL
    MOV RN, #1
    RETURN

;;;
;;; Builtin functions
;;;

add:
    ADD R0, A0, A1
    MOV RN, #1
    RETURN

.org 0xffffffffffffffe0
    JUMP 'bootstrap