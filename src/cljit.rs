use cranelift::{codegen::ir::types::I8, prelude::*};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

use crate::{printer_function, scanner::OpCode, scanner_function, JitFunc, Measured, Runner};

pub struct ClJit;

impl Runner for ClJit {
    fn exec(
        ops: &mut [OpCode],
        cells: &mut [u8],
        printer: &mut crate::Printer,
        scanner: &mut crate::Scanner,
    ) -> Measured<()> {
        let mut m = Measured::new();

        let mut jit = Jit::new().unwrap();

        let code = m.measure("compile cranelift", || jit.compile(ops).unwrap());

        let func: JitFunc = unsafe { std::mem::transmute(code) };
        let printer = printer as *mut crate::Printer;
        let scanner = scanner as *mut crate::Scanner;

        m.measure("run cranelift", || {
            func(
                cells.as_mut_ptr(),
                printer,
                printer_function,
                scanner,
                scanner_function,
            )
        });

        m
    }
}

struct Jit {
    builder_context: FunctionBuilderContext,
    ctx: codegen::Context,
    module: JITModule,
}

impl Jit {
    fn new() -> anyhow::Result<Self> {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false")?;
        flag_builder.set("is_pic", "false")?;
        let isa_builder = match cranelift_native::builder() {
            Ok(ok) => ok,
            Err(e) => anyhow::bail!("host maschine is not supported: {e}"),
        };
        let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        let module = JITModule::new(builder);

        Ok(Self {
            builder_context: FunctionBuilderContext::new(),
            ctx: module.make_context(),
            module,
        })
    }

    fn compile(&mut self, ops: &[OpCode]) -> anyhow::Result<*const u8> {
        self.translate(ops);

        let id =
            self.module
                .declare_function("brainfuck", Linkage::Export, &self.ctx.func.signature)?;

        self.module.define_function(id, &mut self.ctx)?;
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions()?;
        Ok(self.module.get_finalized_function(id))
    }

    fn translate(&mut self, ops: &[OpCode]) {
        let pointer_type = self.module.target_config().pointer_type();
        let ptr_arg = AbiParam::new(pointer_type);
        self.ctx.func.signature.params.extend([
            ptr_arg, // cells
            ptr_arg, // print object
            ptr_arg, // print trait
            ptr_arg, // scan object
            ptr_arg, // scan trait
        ]);

        let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.builder_context);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let cells = builder.block_params(entry_block)[0];
        let mut trans = OpTranslator::new(pointer_type, builder, cells, entry_block);
        for op in ops {
            trans.translate(*op);
        }

        trans.builder.ins().return_(&[]);
        trans.builder.finalize();
    }
}

struct OpTranslator<'a> {
    ptr: types::Type,
    builder: FunctionBuilder<'a>,
    cell_index: Variable,
    cells: Value,
    mem_flags: MemFlags,
    stack: Vec<(Block, Block)>,
    block: Block,
}

impl<'a> OpTranslator<'a> {
    fn new(ptr: types::Type, mut builder: FunctionBuilder<'a>, cells: Value, block: Block) -> Self {
        let cell_index = Variable::new(0);
        builder.declare_var(cell_index, ptr);
        Self {
            ptr,
            builder,
            cell_index,
            cells,
            mem_flags: MemFlags::new(),
            stack: Vec::new(),
            block,
        }
    }

    fn translate(&mut self, op: OpCode) {
        match op {
            OpCode::Right { count } => {
                let var = self.builder.use_var(self.cell_index);
                let value = self.builder.ins().iadd_imm(var, count as i64);
                self.builder.def_var(self.cell_index, value);
            }
            OpCode::Left { count } => {
                let var = self.builder.use_var(self.cell_index);
                let value = self.builder.ins().iadd_imm(var, -(count as i64));
                self.builder.def_var(self.cell_index, value);
            }
            OpCode::Inc { count } => {
                let (cell_index, current_cell) = self.get_current_cell();
                let current_cell = self.builder.ins().iadd_imm(current_cell, count as i64);

                self.builder
                    .ins()
                    .store(self.mem_flags, current_cell, cell_index, 0);
            }
            OpCode::Dec { count } => {
                let (cell_index, current_cell) = self.get_current_cell();
                let current_cell = self.builder.ins().iadd_imm(current_cell, -(count as i64));

                self.builder
                    .ins()
                    .store(self.mem_flags, current_cell, cell_index, 0);
            }
            OpCode::Output => {
                let (_, current_cell) = self.get_current_cell();
                let (print_obj, print_func, print_func_ref) =
                    print_function(&mut self.builder, self.block, AbiParam::new(self.ptr));

                self.builder.ins().call_indirect(
                    print_func_ref,
                    print_func,
                    &[print_obj, current_cell],
                );
            }
            OpCode::Input => {
                let (scan_obj, scan_func, scan_func_ref) =
                    scan_function(&mut self.builder, self.block, AbiParam::new(self.ptr));
                let rets = self
                    .builder
                    .ins()
                    .call_indirect(scan_func_ref, scan_func, &[scan_obj]);
                let ret = self.builder.inst_results(rets)[0];

                let (cell_index, _) = self.get_current_cell();

                self.builder.ins().store(self.mem_flags, ret, cell_index, 0);
            }
            OpCode::JumpIfZero { .. } => {
                let block_if_not_zero = self.builder.create_block();
                let block_if_zero = self.builder.create_block();

                let (_, current_cell) = self.get_current_cell();

                self.builder
                    .ins()
                    .brif(current_cell, block_if_not_zero, &[], block_if_zero, &[]);

                self.builder.switch_to_block(block_if_not_zero);

                self.stack.push((block_if_not_zero, block_if_zero));
            }
            OpCode::JumpIfNotZero { .. } => {
                let (block_if_not_zero, block_if_zero) = self.stack.pop().unwrap();

                let (_, current_cell) = self.get_current_cell();
                self.builder
                    .ins()
                    .brif(current_cell, block_if_not_zero, &[], block_if_zero, &[]);

                self.builder.seal_block(block_if_zero);
                self.builder.seal_block(block_if_not_zero);

                self.builder.switch_to_block(block_if_zero);
            }
            OpCode::SetZero => {
                let index = self.builder.use_var(self.cell_index);
                let cell_index = self.builder.ins().iadd(self.cells, index);
                let zero = self.builder.ins().iconst(I8, 0);
                self.builder
                    .ins()
                    .store(self.mem_flags, zero, cell_index, 0);
            }
        }
    }

    fn get_current_cell(&mut self) -> (Value, Value) {
        get_current_cell(
            &mut self.builder,
            self.cell_index,
            self.cells,
            self.mem_flags,
        )
    }
}

fn scan_function(
    builder: &mut FunctionBuilder<'_>,
    block: Block,
    ptr_arg: AbiParam,
) -> (Value, Value, codegen::ir::SigRef) {
    let scan_obj = builder.block_params(block)[3];
    let scan_func = builder.block_params(block)[4];
    let mut scan_signature = Signature::new(isa::CallConv::SystemV);
    scan_signature.params.push(ptr_arg);
    scan_signature.returns.push(AbiParam::new(I8));
    let scan_func_ref = builder.import_signature(scan_signature);
    (scan_obj, scan_func, scan_func_ref)
}

fn print_function(
    builder: &mut FunctionBuilder<'_>,
    block: Block,
    ptr_arg: AbiParam,
) -> (Value, Value, codegen::ir::SigRef) {
    let print_obj = builder.block_params(block)[1];
    let print_func = builder.block_params(block)[2];
    let mut print_signature = Signature::new(isa::CallConv::SystemV);
    print_signature.params.extend([ptr_arg, AbiParam::new(I8)]);
    let print_func_ref = builder.import_signature(print_signature);
    (print_obj, print_func, print_func_ref)
}

/// returns (cell_index, current_cell)
fn get_current_cell(
    builder: &mut FunctionBuilder<'_>,
    cell_index: Variable,
    cells: Value,
    mem_flags: MemFlags,
) -> (Value, Value) {
    let index = builder.use_var(cell_index);
    let cell_index = builder.ins().iadd(cells, index);
    let current_cell = builder.ins().load(I8, mem_flags, cell_index, 0);
    (cell_index, current_cell)
}
