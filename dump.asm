;format ELF64

extern _start
_start:
    ret

%define CELL byte [rdi + rbx]
%define cell_off(offset) byte [rdi + rbx + offset]

op_add:
    add cell_off(-0x12345678), byte 0x12

op_sub:
    sub cell_off(0x12345678), byte 0x12

op_move_left:
    sub ebx, dword 0x12345678

op_move_right:
    add ebx, dword 0x12345678

op_init:
    push rbx
    xor rbx, rbx

op_finish:
    pop rbx
    ret

op_jump_if_zero:
    mov al, CELL
    cmp al, byte 0
    je $+0xdeadbeef

op_jump_if_not_zero:
    mov al, CELL
    cmp al, byte 0
    jne $-0xdeadbeef

op_write:
    mov CELL, byte 0x12

op_mul:
    movzx rax, CELL
    imul rax, 0x69
    add cell_off(0xdeadbeef), al
    mov CELL, 0

;; rdi: CELLS
;; rsi: pointer to printer object
;; rdx: pointer to printer function
;; rcx: pointer to scanner object
;; r8: pointer to scanner function
;; rbx: CELLS_INDEX
op_print:
    push rdi
    push rsi
    push rdx
    push rcx
    push r8

    ; rdi cannot be overritten directly becaus it is used to index CELL
    mov rax, rsi
    movzx rsi, CELL
    mov rdi, rax
    call rdx


    pop r8
    pop rcx
    pop rdx
    pop rsi
    pop rdi

op_read:
    push rdi
    push rsi
    push rdx
    push rcx
    push r8

    ; rdi cannot be overritten directly becaus it is used to index CELL
    ;mov rax, rcx
    ;movzx rsi, CELL
    mov rdi, rcx
    call r8


    pop r8
    pop rcx
    pop rdx
    pop rsi
    pop rdi

    mov CELL, al
