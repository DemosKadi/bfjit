use cranelift::{
    codegen::{
        ir::{types::I8, Function, UserFuncName},
        Context,
    },
    prelude::*,
};

use crate::scanner::OpCode;

pub fn compile(ops: &[OpCode]) -> anyhow::Result<Vec<u8>> {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "false").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();

    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
        panic!("Host machine is not supported: {}", msg);
    });

    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .unwrap();

    let mut sig = Signature::new(isa::CallConv::SystemV);
    let pointer_type = isa.pointer_type();
    let ptr_arg = AbiParam::new(pointer_type);
    sig.params.extend([
        ptr_arg, // array
        ptr_arg, // print object
        ptr_arg, // print trait
        ptr_arg, // scan object
        ptr_arg, // scan trait
    ]);

    let mut func = Function::with_name_signature(UserFuncName::user(0, 0), sig);

    let mut func_ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut func, &mut func_ctx);

    let cell_index = Variable::new(0);
    builder.declare_var(cell_index, pointer_type);

    let block = builder.create_block();
    builder.seal_block(block);

    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);

    let cells = builder.block_params(block)[0];

    let (print_obj, print_func, print_func_ref) = print_function(&mut builder, block, ptr_arg);
    let (scan_obj, scan_func, scan_func_ref) = scan_function(&mut builder, block, ptr_arg);

    let zero = builder.ins().iconst(pointer_type, 0);
    builder.def_var(cell_index, zero);

    let mem_flags = MemFlags::new();

    let mut stack = Vec::new();

    let zero = builder.ins().iconst(I8, 0);

    for op in ops {
        match op {
            OpCode::Right { count } => {
                let var = builder.use_var(cell_index);
                let value = builder.ins().iadd_imm(var, *count as i64);
                builder.def_var(cell_index, value);
            }
            OpCode::Left { count } => {
                let var = builder.use_var(cell_index);
                let value = builder.ins().iadd_imm(var, -(*count as i64));
                builder.def_var(cell_index, value);
            }
            OpCode::Inc { count } => {
                let (cell_index, current_cell) =
                    get_current_cell(&mut builder, cell_index, cells, mem_flags);
                let current_cell = builder.ins().iadd_imm(current_cell, *count as i64);

                builder.ins().store(mem_flags, current_cell, cell_index, 0);
            }
            OpCode::Dec { count } => {
                let (cell_index, current_cell) =
                    get_current_cell(&mut builder, cell_index, cells, mem_flags);
                let current_cell = builder.ins().iadd_imm(current_cell, -(*count as i64));

                builder.ins().store(mem_flags, current_cell, cell_index, 0);
            }
            OpCode::Output => {
                let (_, current_cell) =
                    get_current_cell(&mut builder, cell_index, cells, mem_flags);

                builder
                    .ins()
                    .call_indirect(print_func_ref, print_func, &[print_obj, current_cell]);
            }
            OpCode::Input => {
                let rets = builder
                    .ins()
                    .call_indirect(scan_func_ref, scan_func, &[scan_obj]);
                let ret = builder.inst_results(rets)[0];

                let (cell_index, _) = get_current_cell(&mut builder, cell_index, cells, mem_flags);

                builder.ins().store(mem_flags, ret, cell_index, 0);
            }
            OpCode::JumpIfZero { .. } => {
                let block_if_not_zero = builder.create_block();
                let block_if_zero = builder.create_block();

                let (_, current_cell) =
                    get_current_cell(&mut builder, cell_index, cells, mem_flags);

                builder
                    .ins()
                    .brif(current_cell, block_if_not_zero, &[], block_if_zero, &[]);

                builder.switch_to_block(block_if_not_zero);

                stack.push((block_if_not_zero, block_if_zero));
            }
            OpCode::JumpIfNotZero { .. } => {
                let (block_if_not_zero, block_if_zero) = stack.pop().unwrap();

                let (_, current_cell) =
                    get_current_cell(&mut builder, cell_index, cells, mem_flags);
                builder
                    .ins()
                    .brif(current_cell, block_if_not_zero, &[], block_if_zero, &[]);

                builder.seal_block(block_if_zero);
                builder.seal_block(block_if_not_zero);

                builder.switch_to_block(block_if_zero);
            }
            OpCode::SetZero => {
                let index = builder.use_var(cell_index);
                let cell_index = builder.ins().iadd(cells, index);
                builder.ins().store(mem_flags, zero, cell_index, 0);
            }
        }
    }

    builder.ins().return_(&[]);

    builder.finalize();

    let mut ctx = Context::for_function(func);
    ctx.set_disasm(true);
    dbg!(&ctx.func);
    ctx.verify(&*isa)?;
    let code = match ctx.compile(&*isa, &mut Default::default()) {
        Ok(x) => x,
        Err(_) => anyhow::bail!("error while compiling"),
    };

    Ok(code.code_buffer().to_vec())
}

fn scan_function(
    builder: &mut FunctionBuilder<'_>,
    block: Block,
    ptr_arg: AbiParam,
) -> (Value, Value, codegen::ir::SigRef) {
    let scan_obj = builder.block_params(block)[3];
    let scan_trait = builder.block_params(block)[4];
    let scan_func = builder.ins().iadd_imm(scan_trait, 32);
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
    let print_trait = builder.block_params(block)[2];
    let print_func = builder.ins().iadd_imm(print_trait, 32);
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
