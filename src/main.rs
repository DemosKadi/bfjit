mod cljit;
mod compile;
mod interpret;
mod jit;
mod scanner;

use std::{
    io::{stdin, stdout, BufRead, Write},
    path::PathBuf,
    time::Duration,
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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let code = std::fs::read(args.path)?;

    let measurements = match args.run {
        RunKind::Interpret => run::<interpret::Interpreter>(&code, args.cells),
        RunKind::Jit => run::<jit::Jit>(&code, args.cells),
        RunKind::CraneLift => run::<cljit::ClJit>(&code, args.cells),
    };

    for (name, duration) in &measurements.measurements {
        println!("{name}: {duration:?}");
    }
    println!(
        "time: {:?}",
        measurements
            .measurements
            .into_iter()
            .map(|(_, d)| d)
            .sum::<Duration>()
    );

    Ok(())
}

fn run<T: Runner>(code: &[u8], cells: usize) -> Measured<()> {
    let mut measured_ops = compile::compile(code);
    let mut ops = measured_ops.data();
    let mut cells = vec![0u8; cells];

    let mut printer = make_printer();
    let mut scanner = make_scanner();

    measured_ops.append(T::exec(&mut ops, &mut cells, &mut printer, &mut scanner))
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
    fn exec(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut Printer,
        scanner: &mut Scanner,
    ) -> Measured<()>;
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

pub(crate) struct Measured<T> {
    data: Option<T>,
    measurements: Vec<(&'static str, std::time::Duration)>,
}

impl<T> Measured<T> {
    pub fn new() -> Self {
        Self {
            data: None,
            measurements: Vec::new(),
        }
    }

    pub fn set(&mut self, data: T) {
        self.data = Some(data);
    }

    pub fn measure<Ret>(&mut self, name: &'static str, func: impl FnOnce() -> Ret) -> Ret {
        let now = std::time::Instant::now();
        let ret = func();

        self.measurements
            .push((name, std::time::Instant::now().duration_since(now)));

        ret
    }

    fn data(&mut self) -> T {
        self.data.take().unwrap()
    }

    fn append<D>(mut self, other: Measured<D>) -> Measured<D> {
        self.measurements.extend(other.measurements);
        Measured {
            data: other.data,
            measurements: self.measurements,
        }
    }
}
