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

    let zero = builder.ins().iconst(pointer_type, 0);
    builder.def_var(cell_index, zero);

    let mem_flags = MemFlags::new();

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
                let index = builder.use_var(cell_index);
                let cell_index = builder.ins().iadd(cells, index);
                let current_cell = builder.ins().load(I8, mem_flags, cell_index, 0);

                let new_current_cell = builder.ins().iadd_imm(current_cell, *count as i64);

                builder
                    .ins()
                    .store(mem_flags, new_current_cell, cell_index, 0);
            }
            OpCode::Dec { count } => {
                let index = builder.use_var(cell_index);
                let cell_index = builder.ins().iadd(cells, index);
                let current_cell = builder.ins().load(I8, mem_flags, cell_index, 0);

                let new_current_cell = builder.ins().iadd_imm(current_cell, -(*count as i64));

                builder
                    .ins()
                    .store(mem_flags, new_current_cell, cell_index, 0);
            }
            OpCode::Output => todo!(),
            OpCode::Input => todo!(),
            OpCode::JumpIfZero { .. } => todo!(),
            OpCode::JumpIfNotZero { .. } => todo!(),
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
    ctx.verify(&*isa)?;
    let code = match ctx.compile(&*isa, &mut Default::default()) {
        Ok(x) => x,
        Err(_) => anyhow::bail!("error while compiling"),
    };

    Ok(code.code_buffer().to_vec())
}
