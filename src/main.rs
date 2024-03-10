mod scanner;

use std::{
    collections::HashMap,
    io::{stdin, stdout, Write},
    path::PathBuf,
};

use clap::{Parser, ValueEnum};
use scanner::{BfCompiler, OpCode};

#[derive(Debug, Parser)]
struct Args {
    #[arg(value_enum, long, short, default_value_t = RunKind::Interpret)]
    run: RunKind,
    #[arg(long, short, default_value_t = 30_000)]
    cells: usize,
    path: PathBuf,
}

#[derive(Debug, ValueEnum, Clone, Copy)]
enum RunKind {
    Interpret,
    Jit,
}

macro_rules! measure {
    ($name:expr, $code: expr) => {{
        let now = std::time::Instant::now();
        let ret = $code;

        let duration = std::time::Instant::now().duration_since(now);
        println!("{}: {duration:?}", $name);

        ret
    }};
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let code = std::fs::read(args.path)?;

    let mut ops = measure!(
        "compiling",
        BfCompiler::new(trim(&code)).collect::<Vec<_>>()
    );
    measure!("optimizing", optimize(&mut ops));
    measure!("back patching", back_patch(&mut ops));
    //measure!("running", interpret(&ops, args.cells));
    /*
    let code = jit(&ops);
    for (i, c) in code.iter().enumerate() {
        println!("{:06}: {:#04x}", i, c);
    }
    */
    let runner = Runner::new(jit(&ops));
    runner.exec();

    Ok(())
}

fn trim(input: &[u8]) -> &[u8] {
    let start = input.iter().position(|c| b"<>+-.,[]".contains(c)).unwrap();
    let end = input
        .iter()
        .rev()
        .position(|c| b"<>+-.,[]".contains(c))
        .unwrap();
    let end = input.len() - end;
    &input[start..end]
}

fn back_patch(ops: &mut [OpCode]) {
    let mut open: Vec<usize> = Vec::new();
    let mut current = 0usize;

    loop {
        if current >= ops.len() {
            return;
        }
        match &mut ops[current] {
            OpCode::JumpIfZero { .. } => {
                open.push(current);
                current += 1;
            }
            OpCode::JumpIfNotZero { target } => {
                let Some(top) = open.pop() else {
                    eprintln!("No open bracket available");
                    return;
                };

                *target = top + 1;
                if let OpCode::JumpIfZero { target } = &mut ops[top] {
                    *target = current + 1;
                } else {
                    eprintln!("{top} has to be jump if zero");
                }

                current += 1;
            }
            _ => {
                current += 1;
                continue;
            }
        }
    }
}

fn optimize(ops: &mut Vec<OpCode>) {
    let mut current = 0usize;
    while current < ops.len() {
        match &ops[current..] {
            [OpCode::JumpIfZero { .. }, OpCode::Dec { .. } | OpCode::Inc { .. }, OpCode::JumpIfNotZero { .. }, ..] =>
            {
                ops[current] = OpCode::SetZero;
                ops.remove(current + 1);
                ops.remove(current + 1);
                current += 3;
            }
            _ => {
                current += 1;
            }
        }
    }
}

fn interpret(ops: &[OpCode], cell_count: usize) {
    let mut ip = 0usize;
    let mut cell = 0usize;
    let mut cells = vec![0u8; cell_count];
    let mut input = String::new();
    let mut out = stdout();

    while ip < ops.len() {
        let op = ops[ip];
        match op {
            OpCode::Right { count } => {
                cell += count as usize;
                ip += 1;
            }
            OpCode::Left { count } => {
                cell -= count as usize;
                ip += 1;
            }
            OpCode::Inc { count } => {
                cells[cell] = cells[cell].wrapping_add(count);
                ip += 1;
            }
            OpCode::Dec { count } => {
                cells[cell] = cells[cell].wrapping_sub(count);
                ip += 1;
            }
            OpCode::Output => {
                _ = out.write(&[cells[cell]]).unwrap();
                ip += 1;
            }
            OpCode::Input => {
                if input.is_empty() {
                    stdin().read_line(&mut input).unwrap();
                    input.push('\0');
                }

                cells[cell] = input.as_bytes()[0];
                input = String::from(&input[1..]);

                ip += 1;
            }
            OpCode::JumpIfZero { target } => {
                ip = if cells[cell] == 0 { target } else { ip + 1 };
            }
            OpCode::JumpIfNotZero { target } => {
                ip = if cells[cell] != 0 { target } else { ip + 1 };
            }
            OpCode::SetZero => {
                cells[cell] = 0;
                ip += 1;
            }
        }
    }
    out.flush().unwrap();
}

//type Runner = fn(cells: *mut u8, len: usize);

struct Runner {
    code: *const u8,
    len: usize,
}

type JitFunc = fn(*mut u8, usize);

impl Runner {
    fn new(code: Vec<u8>) -> Self {
        /*
                let mut map = memmap2::MmapMut::map_anon(code.len()).unwrap();
                map.copy_from_slice(&code);
                let map = map.make_exec().unwrap();
        */
        /*
        let code_box = code.into_boxed_slice();
        let len = code_box.len();
        let leaked_code = Box::into_raw(code_box) as *mut u8;
        */
        let code = [ret()];
        let len = code.len();
        let code_ptr = code.as_ptr();
        let code = unsafe {
            let page = libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_WRITE | libc::PROT_EXEC,
                libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
                -1,
                0,
            );

            /*
            let mut page: *mut libc::c_void = std::ptr::null_mut();
            libc::posix_memalign(&mut page, 4096, len);
            libc::mprotect(
                page,
                len,
                libc::PROT_EXEC | libc::PROT_READ | libc::PROT_WRITE,
            );
            */
            libc::memcpy(page, code_ptr as *const libc::c_void, len);

            page as *mut u8
            /*
            libc::mprotect(
                leaked_code as *mut libc::c_void,
                len,
                libc::PROT_EXEC | libc::PROT_READ | libc::PROT_WRITE,
            );
            */
        };

        Self { code, len }
    }

    fn as_func(&self) -> JitFunc {
        unsafe { std::mem::transmute(self.code) }
    }

    fn exec(&self) {
        let mut cells = vec![0u8; 30000];
        let func = self.as_func();
        let raw_cells = cells.as_mut_ptr();
        let cells_len = cells.len();
        func(raw_cells, cells_len);
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        unsafe {
            let slice: &mut [u8] = std::slice::from_raw_parts_mut(self.code as *mut u8, self.len);
            _ = Box::from_raw(slice);
        }
    }
}

/// Registers:
/// rsi: cells array
/// rdi: length of cells array
/// rbx: current cell

const fn move_cell_right(count: u8) -> [u8; 4] {
    let mut operation = [0u8; 4];
    // 64bit operation
    operation[0] = 0x48;
    // add operation
    operation[1] = 0x83;
    // rbx register
    operation[2] = 0xc3;
    operation[3] = count;

    operation
}

const fn move_cell_left(count: u8) -> [u8; 4] {
    let mut operation = [0u8; 4];
    // 64bit operation
    operation[0] = 0x48;
    // sub operation
    operation[1] = 0x83;
    // rbx register
    operation[2] = 0xeb;
    operation[3] = count;

    operation
}

const fn add_current_cell(count: u8) -> [u8; 4] {
    let mut operation = [0u8; 4];
    // add operation
    operation[0] = 0x80;
    // sib register
    operation[1] = 0x04;
    // rbx + rsi
    operation[2] = 0x1e;
    operation[3] = count;

    operation
}

const fn sub_current_cell(count: u8) -> [u8; 4] {
    let mut operation = [0u8; 4];
    // add operation
    operation[0] = 0x80;
    // sib register
    operation[1] = 0x2c;
    // rbx + rsi
    operation[2] = 0x1e;
    operation[3] = count;

    operation
}

const fn init_rbx() -> [u8; 7] {
    let mut operation = [0u8; 7];
    //\x48\xc7\xc3

    // 64bit operation
    operation[0] = 0x48;
    // mov operation
    operation[1] = 0xc7;
    // rbx register
    operation[2] = 0xc3;
    // data
    operation[3] = 0x0;
    operation[4] = 0x0;
    operation[5] = 0x0;
    operation[6] = 0x0;

    operation
}

/// this is only the opcode, back patching is needed
const fn jump_if_zero() -> [u8; 7] {
    [
        0x8a, 0x04, 0x1e, // move current cell into al
        0x84, 0xc0, // test al
        0x74, 0x00, // jump if al is zero
    ]
}

/// this is only the opcode, back patching is needed
const fn jump_if_not_zero() -> [u8; 7] {
    [
        0x8a, 0x04, 0x1e, // move current cell into al
        0x84, 0xc0, // test al
        0x75, 0x00, // jump if al is zero
    ]
}

const fn write_to_current_cell(value: u8) -> [u8; 4] {
    [
        0xc6, // mov op
        0x04, // indicates sib register, seems to mean a combinations register
        0x1e, //sib register of rbx + rsi (in this case, index into array)
        value,
    ]
}

const fn ret() -> u8 {
    0xc3
}

const fn print_current_cell() -> [u8; 13] {
    /*
    53                      push   %rbx
    b2 01                   mov    $0x1,%dl
    8a 0c 37                mov    (%rdi,%rsi,1),%cl
    b3 01                   mov    $0x1,%bl
    b0 04                   mov    $0x4,%al
    cd 80                   int    $0x80
    5b                      pop    %rbx
    */

    [
        0x53, // push current rbx to stack
        0xb2, 0x01, // set bl(rdx) to 1 (length of data to be printed)
        0x8a, 0x0c, 0x37, // mov current cell to cl (rcx)
        0xb3, 0x01, // set bl (rbx) to 1 (stdout file descriptor)
        0xb0, 0x04, // set al (rax) to 4 (write syscall)
        0xcd, 0x80, // make a system call
        0x5b, // pop rbx from the stack
    ]
}

fn jit(ops: &[OpCode]) -> Vec<u8> {
    let mut back_patch_stack: Vec<usize> = Vec::new();
    let mut code: Vec<u8> = Vec::new();
    code.extend(init_rbx());
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
                // TODO: implement
                code.push(0xFE);
            }
            OpCode::JumpIfZero { .. } => {
                code.extend(jump_if_zero());
                // push the location of the jump target on the back patch stack
                back_patch_stack.push(code.len() - 1);
            }
            OpCode::JumpIfNotZero { .. } => {
                code.extend(jump_if_not_zero());
                let target = back_patch_stack.pop().expect("Closing ] without [");
                // set the jez target after the jz target
                *code.last_mut().unwrap() = (code.len() - target) as u8 + 1;
                // set jz target to byte after jez
                code[target] = (code.len() - target) as u8;
            }
            OpCode::SetZero => {
                code.extend(write_to_current_cell(0x0));
            }
        }
    }

    code.push(ret());
    code
}
