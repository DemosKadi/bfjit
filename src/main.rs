mod scanner;

use std::{
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
    measure!("running", interpret(&ops, args.cells));

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
