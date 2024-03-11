format ELF64

; function call rdi, rsi
section '.text' executable

public _start
; rdi -> the cells
; rsi -> out callback object (FnMut(u8))
; rdx + 32 -> out callback function pointer
; rcx -> in callbak object (FnMut() -> u8)
; r8 + 32 -> in callback function pointer
; rbx will be current cell
; 48 means 64bit operation
_start:

    ;mov ebx, 0
    ;mov eax, 1
    ;int 0x80

    mov rdi, array
    mov rsi, 10
    call impl

impl:
    ;push rax
    ;push rbx
    xor rbx, rbx
    ;pop rbx
    ;pop rax
    ;add rdx, 32
    ;add r8, 32

    ; print
    push rax
    push rsi
    push rdx
    push rcx
    push r8
    push rdx

    push rbx
    push rdi

    mov rdi, rcx
    call qword[r8 + 32]

    pop rdi
    pop rbx

    mov [rdi + rbx], al

    pop rdx
    pop r8
    pop rcx
    pop rdx
    pop rsi
    pop rax


    ; closure call:
    ; rdi -> closure object
    ; rsi -> parameter

    ;mov al, [rdi + rbx]
    ;test al, al
    ;mov rbx, 3
    ;add byte[rdi + rbx], 4
;    push rbx

    ;mov dl, 1 ; message len
    ;mov cl, byte[rdi + rbx] ; message
    ;mov bl, 1 ; stdout file descriptor
    ;mov al, 4 ; sys_write syscall

    ;int 0x80

    ;pop rbx

    ret


section '.data'
    array db 0x63, 0x3, 0x3, 0x7
