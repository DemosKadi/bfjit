use crate::{printer_function, scanner::OpCode, scanner_function, JitFunc, Measured, Runner};

pub struct Jit;

impl Runner for Jit {
    fn exec(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut crate::Printer,
        scanner: &mut crate::Scanner,
    ) -> Measured<()> {
        let mut m = Measured::new();

        let code = m.measure("jit compile", || jit(ops));
        let mut map = memmap2::MmapMut::map_anon(code.len()).unwrap();
        map.copy_from_slice(&code);
        let map = map.make_exec().unwrap();
        m.measure("jit run", || {
            let printer = printer as *mut crate::Printer;
            let scanner = scanner as *mut crate::Scanner;
            let func: JitFunc = unsafe { std::mem::transmute((&map).as_ptr()) };
            let cells = cells.as_mut_ptr();

            func(cells, printer, printer_function, scanner, scanner_function);
        });

        m
    }
}

/// Registers:
/// rdi: cells array
/// rbx: current cell

const fn move_cell_right(count: u8) -> [u8; 4] {
    [
        0x48, // 64bit operation
        0x83, // add operation
        0xc3, // rbx register
        count,
    ]
}

const fn move_cell_left(count: u8) -> [u8; 4] {
    [
        0x48, // 64bit operation
        0x83, // sub operation
        0xeb, // rbx register
        count,
    ]
}

const fn add_current_cell(count: u8) -> [u8; 4] {
    [
        0x80, // add operation
        0x04, // sib register
        0x1f, // rbx + rdi
        count,
    ]
}

const fn sub_current_cell(count: u8) -> [u8; 4] {
    [
        0x80, // add operation
        0x2c, // sib register
        0x1f, // rbx + rdi
        count,
    ]
}

const fn init() -> [u8; 5] {
    // sets rbx to 0
    [
        0x50, // push rax
        0x53, // push rbx
        0x48, // 64bit op
        0x31, // xor
        0xdb, // rbx
    ]
}

/// this is only the opcode, back patching is needed
const fn jump_if_zero() -> [u8; 11] {
    [
        0x8a, 0x04, 0x1f, // move current cell into al
        0x84, 0xc0, // test al
        0x0f, 0x84, 0x00, 0x00, 0x00, 0x00, // jump if al is zero
    ]
}

/// this is only the opcode, back patching is needed
const fn jump_if_not_zero() -> [u8; 11] {
    [
        0x8a, 0x04, 0x1f, // move current cell into al
        0x84, 0xc0, // test al
        0x0f, 0x85, 0x00, 0x00, 0x00, 0x00, // jump if al is zero
    ]
}

const fn write_to_current_cell(value: u8) -> [u8; 4] {
    [
        0xc6, // mov op
        0x04, // indicates sib register, seems to mean a combinations register
        0x1f, //sib register of rbx + rdi (in this case, index into array)
        value,
    ]
}

const fn finish() -> [u8; 3] {
    [
        0x5b, // pop rbx
        0x58, // pop rax
        0xc3, // ret
    ]
}

const fn print_current_cell() -> [u8; 28] {
    /*
    50                      push   %rax
    53                      push   %rbx
    57                      push   %rdi
    56                      push   %rsi
    52                      push   %rdx
    51                      push   %rcx
    41 50                   push   %r8

    48 89 f0                mov    %rsi,%rax
    0f b6 34 1f             movzbl (%rdi,%rbx,1),%esi
    48 89 c7                mov    %rax,%rdi
    ; ff 52 20                call   *0x20(%rdx)
    ff d2                   call   *%rdx

    41 58                   pop    %r8
    59                      pop    %rcx
    5a                      pop    %rdx
    5e                      pop    %rsi
    5f                      pop    %rdi
    5b                      pop    %rbx
    58                      pop    %rax
    */

    [
        0x50, 0x53, 0x57, 0x56, 0x52, 0x51, // push rax, rbx, rdi, rsi, rdx, rcx
        0x41, 0x50, // push r8
        //
        //
        0x48, 0x89, 0xf0, // move rsi to rax
        0x0f, 0xb6, 0x34, 0x1f, // move current cell to esi(rsi)
        0x48, 0x89, 0xc7, // move rax to rdi
        // 0xff, 0x52, 0x20, // call rdx + 32 ( function )
        0xff, 0xd2, // call rdx ( print function )
        //
        //
        0x41, 0x58, // pop r8
        0x59, 0x5a, 0x5e, 0x5f, 0x5b, 0x58, // pop rcx, rdx, rsi, rdi, rbx, rax
    ]
}

const fn scan_current_cell() -> [u8; 27] {
    /*
    50                      push   %rax
    56                      push   %rsi
    52                      push   %rdx
    51                      push   %rcx
    41 50                   push   %r8
    52                      push   %rdx
    53                      push   %rbx
    57                      push   %rdi
    48 89 cf                mov    %rcx,%rdi
    ; 41 ff 50 20             call   *0x20(%r8)
    41 ff d0                call   *%r8
    5f                      pop    %rdi
    5b                      pop    %rbx
    88 04 1f                mov    %al,(%rdi,%rbx,1)
    5a                      pop    %rdx
    41 58                   pop    %r8
    59                      pop    %rcx
    5a                      pop    %rdx
    5e                      pop    %rsi
    58                      pop    %rax
    */
    [
        0x50, 0x56, 0x52, 0x51, // push rax, rsi, rdx, rcx
        0x41, 0x50, // push r8
        0x52, // push rdx
        0x53, 0x57, // push rbx, rdi
        0x48, 0x89, 0xcf, // move rcx (function object pointer) to rdi
        // 0x41, 0xff, 0x50, 0x20, // call the scan function
        0x41, 0xff, 0xd0, // call the scan function
        0x5f, 0x5b, // pop rdi, rbx
        0x88, 0x04, 0x1f, // move al (rax/return value) into current cell
        0x5a, // pop rdx
        0x41, 0x58, // pop r8
        0x59, 0x5a, 0x5e, 0x58, // pop rcx, rdx, rsi, rax
    ]
}

fn jit(ops: &[OpCode]) -> Vec<u8> {
    let mut back_patch_stack: Vec<usize> = Vec::new();
    let mut code: Vec<u8> = Vec::new();
    code.extend(init());
    for op in ops {
        match op {
            OpCode::Right { count } => {
                code.extend(move_cell_right(*count));
            }
            OpCode::Left { count } => {
                code.extend(move_cell_left(*count));
            }
            OpCode::Inc { count } => {
                code.extend(add_current_cell(*count));
            }
            OpCode::Dec { count } => {
                code.extend(sub_current_cell(*count));
            }
            OpCode::Output => {
                code.extend(print_current_cell());
            }
            OpCode::Input => {
                code.extend(scan_current_cell());
            }
            OpCode::JumpIfZero { .. } => {
                code.extend(jump_if_zero());
                // push the location of the jump target on the back patch stack
                back_patch_stack.push(code.len());
            }
            OpCode::JumpIfNotZero { .. } => {
                code.extend(jump_if_not_zero());
                let target = back_patch_stack.pop().expect("Closing ] without [");
                let offset = code.len() - target;

                // setup jump to after the jz target
                let jump = u32::MAX - (offset as u32) + 1;
                let bytes = jump.to_ne_bytes();
                let len = code.len();
                code[len - 4..].copy_from_slice(&bytes);

                // set jz target to byte after jez
                let bytes = ((code.len() - target) as u32).to_ne_bytes();
                code[target - 4..target].copy_from_slice(&bytes);
            }
            OpCode::SetZero => {
                code.extend(write_to_current_cell(0x0));
            }
        }
    }

    code.extend(finish());
    code
}

#[cfg(test)]
mod tests {
    use crate::{compile, Printer, Runner, Scanner};

    use super::Jit;

    #[test]
    fn code_jit() {
        let code = b",++++++++++.";
        let ops = compile(code);

        let mut print_buffer = Vec::new();
        let mut printer = Printer::new(move |value| print_buffer.push(value));
        let mut scanner = Scanner::new(|| 12);
        let mut cells = vec![0u8; 30000];

        Jit::exec(&ops, &mut cells, &mut printer, &mut scanner);
    }
}
