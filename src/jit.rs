use memmap2::Mmap;

use crate::{compile::OpCode, printer_function, scanner_function, JitFunc, Measured, Runner};

pub struct Jit {
    program: Mmap,
}

impl Jit {
    fn compile(ops: &[OpCode]) -> Self {
        let code = jit(ops);
        let mut map = memmap2::MmapMut::map_anon(code.len()).unwrap();
        map.copy_from_slice(&code);
        Self {
            program: map.make_exec().unwrap(),
        }
    }

    fn get_func(&self) -> JitFunc {
        unsafe { std::mem::transmute(self.program.as_ptr()) }
    }

    fn run(&self, cells: &mut [u8], printer: &mut crate::Printer, scanner: &mut crate::Scanner) {
        let printer = printer as *mut crate::Printer;
        let scanner = scanner as *mut crate::Scanner;
        let cells = cells.as_mut_ptr();

        let func = self.get_func();
        func(cells, printer, printer_function, scanner, scanner_function);
    }
}

impl Runner for Jit {
    fn exec(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut crate::Printer,
        scanner: &mut crate::Scanner,
    ) {
        Jit::compile(ops).run(cells, printer, scanner);
    }

    fn exec_bench(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut crate::Printer,
        scanner: &mut crate::Scanner,
        count: usize,
    ) -> Measured<()> {
        let mut m = Measured::new();

        let j = Jit::compile(ops);

        for i in 0..count {
            m.measure(format!("run {i}"), || j.run(cells, printer, scanner));
        }

        m
    }
}

/// Registers:
/// rdi: cells array
/// rbx: current cell
const fn move_cell_right(count: u32) -> [u8; 6] {
    let count = count.to_ne_bytes();
    // add ebx, dword <count>
    [0x81, 0xc3, count[0], count[1], count[2], count[3]]
}

const fn move_cell_left(count: u32) -> [u8; 6] {
    let count = count.to_ne_bytes();
    // sub ebx, dword <count>
    [0x81, 0xeb, count[0], count[1], count[2], count[3]]
}

const fn add_current_cell(count: u8, offset: i32) -> [u8; 8] {
    // add byte [rdi + rbx], count
    //[0x80, 0x04, 0x1f, count]
    let off = (offset as u32).to_ne_bytes();
    // add byte [rdi + rbx + offset], count
    [0x80, 0x84, 0x1f, off[0], off[1], off[2], off[3], count]
}

const fn sub_current_cell(count: u8, offset: i32) -> [u8; 8] {
    // sub byte [rdi + rbx], count
    //[0x80, 0x2c, 0x1f, count]

    let off = (offset as u32).to_ne_bytes();
    // sub byte [rdi + rbx + offset], count
    [0x80, 0xac, 0x1f, off[0], off[1], off[2], off[3], count]
}

const fn init() -> [u8; 4] {
    [
        0x53, // push rbx
        0x48, 0x31, 0xdb, // xor rbx, rbx
    ]
}

/// this is only the opcode, back patching is needed
const fn jump_if_zero() -> [u8; 11] {
    [
        0x8a, 0x04, 0x1f, // mov al, [rdi + rbx]
        0x3c, 0x0, // cmp al, byte 0
        0x0f, 0x84, 0x00, 0x00, 0x00, 0x00, // jump if al is zero
    ]
}

/// this is only the opcode, back patching is needed
const fn jump_if_not_zero() -> [u8; 11] {
    [
        0x8a, 0x04, 0x1f, // mov al, [rdi + rbx]
        0x3c, 0x0, // cmp al, byte 0
        0x0f, 0x85, 0x00, 0x00, 0x00, 0x00, // jump if al is zero
    ]
}

const fn write_to_current_cell(value: u8) -> [u8; 4] {
    [
        // mov byte [rdi + rbx], byte <value>
        0xc6, 0x04, 0x1f, value,
    ]
}

const fn finish() -> [u8; 2] {
    [
        0x5b, // pop rbx
        0xc3, // ret
    ]
}

const fn print_current_cell() -> [u8; 25] {
    [
        0x57, // push   rdi
        0x56, // push   rsi
        0x52, // push   rdx
        0x51, // push   rcx
        0x41, 0x50, // push   r8
        0x48, 0x89, 0xf0, // mov    rax,rsi
        0x48, 0x0f, 0xb6, 0x34, 0x1f, // movzx  rsi,BYTE PTR [rdi+rbx*1]
        0x48, 0x89, 0xc7, // mov rdi, rax
        0xff, 0xd2, // call  rdx
        0x41, 0x58, // pop    r8
        0x59, // pop    rcx
        0x5a, // pop    rdx
        0x5e, // pop    rsi
        0x5f, // pop    rdi
    ]
}

const fn scan_current_cell() -> [u8; 21] {
    [
        0x57, // push   rdi
        0x56, // push   rsi
        0x52, // push   rdx
        0x51, // push   rcx
        0x41, 0x50, // push   r8
        0x48, 0x89, 0xcf, // mov    rdi,rcx
        0x41, 0xff, 0xd0, // call   r8
        0x41, 0x58, // pop    r8
        0x59, // pop    rcx
        0x5a, // pop    rdx
        0x5e, // pop    rsi
        0x5f, // pop    rdi
        0x88, 0x04, 0x1f, // mov byte [rdi+rbx],al
    ]
}

const fn mul(factor: u8, offset: i32) -> [u8; 20] {
    let offset = offset.to_ne_bytes();
    [
        0x48, 0x0f, 0xb6, 0x04, 0x1f, // movzx  rax,BYTE PTR [rdi+rbx]
        0x48, 0x6b, 0xc0, factor, // imul   rax,rax,factor
        0x00, 0x84, 0x1f, offset[0], offset[1], offset[2],
        offset[3], // add    BYTE PTR [rdi+rbx+offset],al
        0xc6, 0x04, 0x1f, 0x00, // mov    BYTE PTR [rdi+rbx],0x0
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
            OpCode::Inc { count, offset } => {
                code.extend(add_current_cell(*count, *offset));
            }
            OpCode::Dec { count, offset } => {
                code.extend(sub_current_cell(*count, *offset));
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
            OpCode::Mul { factor, offset } => {
                code.extend(mul(*factor, *offset));
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
        let mut ops = compile::compile(code);

        let mut print_buffer = Vec::new();
        let mut printer = Printer::new(move |value| print_buffer.push(value));
        let mut scanner = Scanner::new(|| 12);
        let mut cells = vec![0u8; 30000];

        Jit::exec(&mut ops, &mut cells, &mut printer, &mut scanner);
    }
}
