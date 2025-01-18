pub mod cljit;
pub mod compile;
pub mod interpret;
pub mod jit;
pub mod meassure;
use compile::OpCode;
use meassure::Measured;
use std::io::{stdin, stdout, BufRead, Write};

pub fn run<T: Runner>(code: &[u8], cells: usize) {
    let mut ops = compile::compile(code);
    let mut cells = vec![0u8; cells];

    let mut printer = make_printer();
    let mut scanner = make_scanner();

    T::exec(&mut ops, &mut cells, &mut printer, &mut scanner)
}

pub fn make_printer() -> Printer {
    let out = stdout();
    let mut out = out.lock();
    let print = move |value| {
        _ = out.write(&[value]).unwrap();
    };
    Printer::new(print)
}

pub fn make_scanner() -> Scanner {
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

pub trait Runner {
    fn exec(ops: &mut [OpCode], cells: &mut [u8], printer: &mut Printer, scanner: &mut Scanner);

    fn exec_bench(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut Printer,
        scanner: &mut Scanner,
        count: usize,
    ) -> Measured<()>;
}

pub type JitFunc = fn(*mut u8, *mut Printer, PrinterFunc, *mut Scanner, ScannerFunc);

pub struct Printer {
    printer: Box<dyn FnMut(u8)>,
}
impl Printer {
    pub fn new(printer: impl FnMut(u8) + 'static) -> Self {
        Self {
            printer: Box::new(printer),
        }
    }
    pub fn print(&mut self, value: u8) {
        (self.printer)(value);
    }
}
pub extern "C" fn printer_function(printer: &mut Printer, value: u8) {
    printer.print(value);
}
pub type PrinterFunc = extern "C" fn(&mut Printer, u8);

pub struct Scanner {
    scanner: Box<dyn FnMut() -> u8>,
}
impl Scanner {
    pub fn new(scanner: impl FnMut() -> u8 + 'static) -> Self {
        Self {
            scanner: Box::new(scanner),
        }
    }
    pub fn scan(&mut self) -> u8 {
        (self.scanner)()
    }
}
pub extern "C" fn scanner_function(scanner: &mut Scanner) -> u8 {
    scanner.scan()
}

pub type ScannerFunc = extern "C" fn(&mut Scanner) -> u8;
