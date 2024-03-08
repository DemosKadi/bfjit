#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    Right { count: u8 },
    Left { count: u8 },
    Inc { count: u8 },
    Dec { count: u8 },
    Output,
    Input,
    JumpIfZero { target: usize },
    JumpIfNotZero { target: usize },
    SetZero,
}

#[derive(Debug, Clone, Copy)]
pub struct BfCompiler<'a> {
    code: &'a [u8],
    index: usize,
}

impl<'a> BfCompiler<'a> {
    pub fn new(code: &'a [u8]) -> Self {
        Self { code, index: 0 }
    }

    fn skip_non_code(&mut self) {
        let to_skip = self.code[self.index..]
            .iter()
            .take_while(|t| !b"<>+-.,[]".contains(*t))
            .count();
        self.index += to_skip;
    }

    fn count_instances(&self, token: u8) -> usize {
        self.code[self.index..]
            .iter()
            .take_while(|t| **t == token)
            .count()
    }
}

impl<'a> Iterator for BfCompiler<'a> {
    type Item = OpCode;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_non_code();
        if self.index >= self.code.len() {
            return None;
        }

        let t = self.code[self.index];
        match t {
            b'>' => {
                let count = self.count_instances(b'>');
                self.index += count;
                Some(OpCode::Right { count: count as u8 })
            }
            b'<' => {
                let count = self.count_instances(b'<');
                self.index += count;
                Some(OpCode::Left { count: count as u8 })
            }
            b'+' => {
                let count = self.count_instances(b'+');
                self.index += count;
                Some(OpCode::Inc { count: count as u8 })
            }
            b'-' => {
                let count = self.count_instances(b'-');
                self.index += count;
                Some(OpCode::Dec { count: count as u8 })
            }
            b'.' => {
                self.index += 1;
                Some(OpCode::Output)
            }
            b',' => {
                self.index += 1;
                Some(OpCode::Input)
            }
            b'[' => {
                self.index += 1;
                Some(OpCode::JumpIfZero { target: 0 })
            }
            b']' => {
                self.index += 1;
                Some(OpCode::JumpIfNotZero { target: 0 })
            }
            _ => unreachable!(),
        }
    }
}
