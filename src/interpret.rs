use crate::{
    measure, printer_function, scanner::OpCode, scanner_function, Printer, Runner, Scanner,
};

pub struct Interpreter;

impl Runner for Interpreter {
    fn exec(ops: &mut [OpCode], cells: &mut [u8], printer: &mut Printer, scanner: &mut Scanner) {
        measure!("back patchin", back_patch(ops));
        let mut ip = 0usize;
        let mut cell = 0usize;

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
            }
        }
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
        let ops = compile(code);

        let mut print_buffer = Vec::new();
        let mut printer = Printer::new(move |value| print_buffer.push(value));
        let mut scanner = Scanner::new(|| 12);
        let mut cells = vec![0u8; 30000];

        Interpreter::exec(&ops, &mut cells, &mut printer, &mut scanner);
    }
}
