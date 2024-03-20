use crate::{
    scanner::{BfCompiler, OpCode},
    Measured,
};

pub fn compile(code: &[u8]) -> Measured<Vec<OpCode>> {
    let mut m = Measured::new();
    let mut ops = m.measure("compiling", || {
        BfCompiler::new(trim(code)).collect::<Vec<_>>()
    });
    m.measure("optimizing", || optimize(&mut ops));
    m.set(ops);
    m
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
