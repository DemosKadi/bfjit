mod cljit;
mod compile;
mod interpret;
mod jit;
mod scanner;

use std::{
    io::{stdin, stdout, BufRead, Write},
    path::PathBuf,
};

use clap::{Parser, ValueEnum};
use scanner::OpCode;

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
    CraneLift,
}

#[cfg(test)]
#[macro_export]
macro_rules! measure {
    ($name:expr, $code: expr) => {
        $code
    };
}

#[cfg(not(test))]
#[macro_export]
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

    match args.run {
        RunKind::Interpret => measure!(
            "interpret",
            run::<interpret::Interpreter>(&code, args.cells)
        ),
        RunKind::Jit => run::<jit::Jit>(&code, args.cells),
        RunKind::CraneLift => run::<cljit::ClJit>(&code, args.cells),
    }

    Ok(())
}

fn run<T: Runner>(code: &[u8], cells: usize) {
    let mut ops = compile::compile(code);
    let mut cells = vec![0u8; cells];

    let mut printer = make_printer();
    let mut scanner = make_scanner();

    T::exec(&mut ops, &mut cells, &mut printer, &mut scanner);
}

fn make_printer() -> Printer {
    let out = stdout();
    let mut out = out.lock();
    let print = move |value| {
        _ = out.write(&[value]).unwrap();
    };
    Printer::new(print)
}

fn make_scanner() -> Scanner {
    let mut buffer = Vec::new();
    let input = stdin();
    let mut input = input.lock();
    let scan = move || {
        if buffer.is_empty() {
            _ = input.read_until(b'\n', &mut buffer).unwrap();
            buffer.push(b'\0');
        }

        let val = buffer[0];
        buffer.remove(0);
        val
    };
    Scanner::new(scan)
}

pub(crate) trait Runner {
    fn exec(ops: &mut [OpCode], cells: &mut [u8], printer: &mut Printer, scanner: &mut Scanner);
}

pub(crate) type JitFunc = fn(*mut u8, *mut Printer, PrinterFunc, *mut Scanner, ScannerFunc);

pub(crate) struct Printer {
    printer: Box<dyn FnMut(u8)>,
}
impl Printer {
    fn new(printer: impl FnMut(u8) + 'static) -> Self {
        Self {
            printer: Box::new(printer),
        }
    }
    fn print(&mut self, value: u8) {
        (self.printer)(value);
    }
}
pub(crate) extern "C" fn printer_function(printer: &mut Printer, value: u8) {
    printer.print(value);
}
pub(crate) type PrinterFunc = extern "C" fn(&mut Printer, u8);

pub(crate) struct Scanner {
    scanner: Box<dyn FnMut() -> u8>,
}
impl Scanner {
    fn new(scanner: impl FnMut() -> u8 + 'static) -> Self {
        Self {
            scanner: Box::new(scanner),
        }
    }
    fn scan(&mut self) -> u8 {
        (self.scanner)()
    }
}
pub(crate) extern "C" fn scanner_function(scanner: &mut Scanner) -> u8 {
    scanner.scan()
}
pub(crate) type ScannerFunc = extern "C" fn(&mut Scanner) -> u8;

#[cfg(test)]
mod tests {
    use crate::{compile, interpret_with_custom_io, run_jit_with_io};

    #[test]
    fn code_interpret() {
        let code = b",++++++++++.";
        let ops = compile(code);

        let mut print_buffer = Vec::new();
        interpret_with_custom_io(
            &ops,
            30000,
            &mut |value| print_buffer.push(value),
            &mut || 12,
        );
    }

    #[test]
    fn code_jit() {
        let code = b",++++++++++.";
        let ops = compile(code);

        let mut print_buffer = Vec::new();
        run_jit_with_io(
            &ops,
            30000,
            &mut |value| print_buffer.push(value),
            &mut || 12,
        );
    }
}
