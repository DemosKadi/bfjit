use crate::{
    compile::OpCode, printer_function, scanner_function, Measured, Printer, Runner, Scanner,
};

pub struct Interpreter;

impl Interpreter {
    fn run(ops: &mut [OpCode], cells: &mut [u8], printer: &mut Printer, scanner: &mut Scanner) {
        let mut ip = 0usize;
        let mut cell = 0usize;

        while ip < ops.len() {
            match ops[ip] {
                OpCode::Right { count } => {
                    cell += count as usize;
                    ip += 1;
                }
                OpCode::Left { count } => {
                    cell -= count as usize;
                    ip += 1;
                }
                OpCode::Inc { count, offset } => {
                    let cell = (cell as i32 + offset) as usize;
                    cells[cell] = cells[cell].wrapping_add(count);
                    ip += 1;
                }
                OpCode::Dec { count, offset } => {
                    let cell = (cell as i32 + offset) as usize;
                    cells[cell] = cells[cell].wrapping_sub(count);
                    ip += 1;
                }
                OpCode::Output => {
                    printer_function(printer, cells[cell]);
                    ip += 1;
                }
                OpCode::Input => {
                    cells[cell] = scanner_function(scanner);
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
                OpCode::Mul { factor, offset } => {
                    let off_cell = (cell as i32 + offset) as usize;

                    cells[off_cell] += cells[cell].wrapping_mul(factor);
                    cells[cell] = 0;
                    ip += 1;
                }
            }
        }
    }
}

impl Runner for Interpreter {
    fn exec(ops: &mut [OpCode], cells: &mut [u8], printer: &mut Printer, scanner: &mut Scanner) {
        back_patch(ops);
        Interpreter::run(ops, cells, printer, scanner);
    }

    fn exec_bench(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut Printer,
        scanner: &mut Scanner,
        count: usize,
    ) -> Measured<()> {
        let mut m = Measured::new();
        m.measure("back patching", || back_patch(ops));
        for i in 0..count {
            m.measure(format!("interpret {i}"), || {
                Interpreter::run(ops, cells, printer, scanner)
            })
        }
        m
    }
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

#[cfg(test)]
mod tests {
    use crate::{compile, Printer, Runner, Scanner};

    use super::Interpreter;

    #[test]
    fn code_interpret() {
        let code = b",++++++++++.";
        let mut ops = compile::compile(code);

        let mut print_buffer = Vec::new();
        let mut printer = Printer::new(move |value| print_buffer.push(value));
        let mut scanner = Scanner::new(|| 12);
        let mut cells = vec![0u8; 30000];

        Interpreter::exec(&mut ops, &mut cells, &mut printer, &mut scanner);
    }
}
