mod scanner;

use std::{
    io::{stdin, stdout, Write},
    path::PathBuf,
};

use clap::{Parser, ValueEnum};
use memmap2::Mmap;
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

    match args.run {
        RunKind::Interpret => measure!("interpret", interpret(&ops, args.cells)),
        RunKind::Jit => {
            let code = measure!("jit compile", jit(&ops));
            measure!("jit run", Runner::new(code).exec(args.cells))
        }
    }

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
    map: Mmap,
}

type JitFunc = fn(*mut u8);

impl Runner {
    fn new(code: Vec<u8>) -> Self {
        let mut map = memmap2::MmapMut::map_anon(code.len()).unwrap();
        map.copy_from_slice(&code);
        let map = map.make_exec().unwrap();
        Self { map }
    }

    fn as_func(&self) -> JitFunc {
        unsafe { std::mem::transmute(self.map.as_ptr()) }
    }

    fn exec(&self, cell_count: usize) {
        let mut cells = vec![0u8; cell_count];
        {
            let func = self.as_func();
            let raw_cells = cells.as_mut_ptr();
            //println!("{:?}", raw_cells);
            func(raw_cells);
        }
        for c in cells {
            if c != 0 {
                print!("{}", c as char);
            }
        }
        println!();
    }
}

/// Registers:
/// rdi: cells array
/// rsi: length of cells array
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

const fn init_rbx() -> [u8; 3] {
    // sets rbx to 0
    [
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

const fn ret() -> u8 {
    0xc3
}

const fn print_current_cell() -> [u8; 13] {
    [
        0x53, // push current rbx to stack
        0xb2, 0x01, // set bl(rdx) to 1 (length of data to be printed)
        0x8a, 0x0c, 0x1f, // mov current cell to cl (rcx)
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

    code.push(ret());
    code
}
