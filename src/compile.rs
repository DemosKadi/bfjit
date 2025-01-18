use crate::Measured;

#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    /// Moves the cursor `count` to the right
    Right { count: u32 },
    /// Moves the cursor `count` to the left
    Left { count: u32 },
    /// Increases the current cell + `offset` by `count`
    Inc { count: u8, offset: i32 },
    /// Decreases the current cell + `offset` by `count`
    Dec { count: u8, offset: i32 },
    /// Prints the byte in the current cell
    Output,
    /// Reads one byte from the input
    Input,
    /// If the current cell is 0, jumps to the closing op, else executes the next op
    JumpIfZero { target: usize },
    /// If the current cell is 0, continues with the next op, else jumps to the opening op
    JumpIfNotZero { target: usize },
    /// Sets the value of the current cell to 0
    SetZero,
    /// Multiplies the current cell by `factor` and writes the value to the `offset` of the current cell, after that sets the current cell to 0
    Mul { factor: u8, offset: i32 },
}

fn compile_impl(code: &[u8]) -> Vec<OpCode> {
    let mut ret = Vec::with_capacity(code.len());
    let mut index = 0usize;

    while index < code.len() {
        match code[index] {
            b'+' => {
                ret.push(OpCode::Inc {
                    count: 1,
                    offset: 0,
                });
                index += 1;
            }
            b'-' => {
                ret.push(OpCode::Dec {
                    count: 1,
                    offset: 0,
                });
                index += 1;
            }
            b'<' => {
                ret.push(OpCode::Left { count: 1 });
                index += 1;
            }
            b'>' => {
                ret.push(OpCode::Right { count: 1 });
                index += 1;
            }
            b'[' => {
                ret.push(OpCode::JumpIfZero { target: 0 });
                index += 1;
            }
            b']' => {
                ret.push(OpCode::JumpIfNotZero { target: 0 });
                index += 1;
            }
            b'.' => {
                ret.push(OpCode::Output);
                index += 1;
            }
            b',' => {
                ret.push(OpCode::Input);
                index += 1;
            }
            _ => index += 1,
        }
    }
    ret
}

pub fn compile(code: &[u8]) -> Vec<OpCode> {
    let mut ops = compile_impl(code);
    optimize(&mut ops);
    ops
}

pub fn compile_meassured(code: &[u8]) -> Measured<Vec<OpCode>> {
    let mut m = Measured::new();
    let mut ops = m.measure("compiling", || compile_impl(code));
    m.measure("optimizing", || optimize(&mut ops));
    m.set(ops);
    m
}

fn optimize(ops: &mut Vec<OpCode>) {
    use OpCode as Op;
    let mut read = 0usize;
    let mut write = 0usize;

    macro_rules! count {
        ($op:pat) => {{
            ops[read..]
                .iter()
                .take_while(|op| matches!(op, $op))
                .count()
        }};
    }

    while read < ops.len() {
        match &ops[read] {
            Op::Inc { .. } => {
                let count = count!(Op::Inc { .. });
                ops[write] = Op::Inc {
                    count: count as u8,
                    offset: 0,
                };
                write += 1;
                read += count;
            }
            Op::Dec { .. } => {
                let count = count!(Op::Dec { .. });
                ops[write] = Op::Dec {
                    count: count as u8,
                    offset: 0,
                };
                write += 1;
                read += count;
            }
            Op::Left { .. } => {
                let count = count!(Op::Left { .. });
                ops[write] = Op::Left {
                    count: count as u32,
                };
                write += 1;
                read += count;
            }
            Op::Right { .. } => {
                let count = count!(Op::Right { .. });
                ops[write] = Op::Right {
                    count: count as u32,
                };
                write += 1;
                read += count;
            }
            _ => {
                ops[write] = ops[read];
                write += 1;
                read += 1;
            }
        }
    }

    ops.truncate(write);
    read = 0;
    write = 0;

    while read < ops.len() {
        match &ops[read..] {
            // Clean the current cell
            // [-] or [+]
            [Op::JumpIfZero { .. }, Op::Dec { .. } | Op::Inc { .. }, Op::JumpIfNotZero { .. }, ..] =>
            {
                ops[write] = Op::SetZero;
                write += 1;
                read += 3;
            }
            // Add/Sub with offset
            // >>>+<<<
            [Op::Right { count: r_count }, change @ Op::Dec { .. } | change @ Op::Inc { .. }, Op::Left { count: l_count }, ..]
                if *r_count == *l_count =>
            {
                ops[write] = match *change {
                    Op::Dec { count, .. } => Op::Dec {
                        count,
                        offset: *r_count as i32,
                    },
                    Op::Inc { count, .. } => Op::Inc {
                        count,
                        offset: *r_count as i32,
                    },
                    _ => unreachable!(),
                };
                write += 1;
                read += 3;
            }
            // Add/Sub with offset
            // <<<+>>>
            [Op::Left { count: l_count }, change @ Op::Dec { .. } | change @ Op::Inc { .. }, Op::Right { count: r_count }, ..]
                if *r_count == *l_count =>
            {
                ops[write] = match *change {
                    Op::Dec { count, .. } => Op::Dec {
                        count,
                        offset: -(*r_count as i32),
                    },
                    Op::Inc { count, .. } => Op::Inc {
                        count,
                        offset: -(*r_count as i32),
                    },
                    _ => unreachable!(),
                };
                write += 1;
                read += 3;
            }
            _ => {
                ops[write] = ops[read];
                write += 1;
                read += 1;
            }
        }
    }
    ops.truncate(write);

    read = 0;
    write = 0;

    // new loop, because it uses opcodes which are only created in the previous optimization loop
    while read < ops.len() {
        match &ops[read..] {
            [Op::JumpIfZero { .. }, Op::Inc { count, offset }, Op::Dec {
                count: 1,
                offset: 0,
            }, Op::JumpIfNotZero { .. }, ..] => {
                ops[write] = Op::Mul {
                    factor: *count,
                    offset: *offset,
                };
                read += 4;
                write += 1;
            }
            _ => {
                ops[write] = ops[read];
                write += 1;
                read += 1;
            }
        }
    }

    ops.truncate(write);
    ops.shrink_to(write);
}
