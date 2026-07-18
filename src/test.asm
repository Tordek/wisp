; CONSTANT POOL
; C0 = '(1 2 3 poop)


; ----------------------------------------
; run(n)
;
; R0 = n
; ----------------------------------------

run:
    EQ           R1, R0, NIL
    JT           R1, run_nil

    ; Need cdr(n) and fib result to survive calls.
    PUSH         R12
    PUSH         R13

    UNCONS       R0, R12, R0      ; R0 = car(n), R12 = cdr(n)

    ; fib(car(n), 0)
    LOAD_FIXNUM  R1, 0
    CALL         fib

    MOV          R13, R0          ; save fib result

    ; run(cdr(n))
    MOV          R0, R12
    CALL         run

    ; cons(fib-result, recursive-result)
    CONS         R0, R13, R0


    POP          R13
    POP          R12
    MOV          R7, 1
    RETURN


run_nil:
    LOAD_SPECIAL R0, NIL
    MOV          R7, 1
    RETURN



; ----------------------------------------
; fib(n, a)
;
; R0 = n
; R1 = a
;
; fib(n,a):
;   if n == 0 return a
;   else fib(n-1, n+a)
;
; tail recursive
; ----------------------------------------

fib:
    EQ           R2, R0, ZERO
    JT           R2, fib_done

    MOV          R3, R0          ; save n

    SUB          R0, R0, ONE     ; n - 1
    ADD          R1, R3, R1      ; n + a

    JUMP         fib             ; tail call

fib_done:
    MOV          R0, R1
    MOV          R7, 1
    RETURN



; ----------------------------------------
; main
; ----------------------------------------

main:
    LOAD_CONST   R0, C0          ; '(1 2 3 poop)
    CALL         run

    HALT