use std::{path::PathBuf, time::Duration};

use bfjit::cljit::ClJit;
use bfjit::interpret::Interpreter;
use bfjit::jit::Jit;
use bfjit::{compile, make_printer, make_scanner, run};
use bfjit::{meassure::Measured, Runner};
use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
struct Args {
    #[arg(value_enum, long, short, default_value_t = RunKind::Interpret)]
    run: RunKind,
    #[arg(long, short, default_value_t = 30_000)]
    cells: usize,
    #[arg(long, short, num_args = 0..=1, default_missing_value = "10")]
    meassure: Option<usize>,
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

    if let Some(measure_count) = args.meassure {
        let measurements = match args.run {
            RunKind::Interpret => run_meassured::<Interpreter>,
            RunKind::Jit => run_meassured::<Jit>,
            RunKind::CraneLift => run_meassured::<ClJit>,
        }(&code, args.cells, measure_count);

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
    } else {
        match args.run {
            RunKind::Interpret => run::<Interpreter>(&code, args.cells),
            RunKind::Jit => run::<Jit>(&code, args.cells),
            RunKind::CraneLift => run::<ClJit>(&code, args.cells),
        };
    }

    Ok(())
}

fn run_meassured<T: Runner>(code: &[u8], cells: usize, meassure: usize) -> Measured<()> {
    let mut measured_ops = compile::compile_meassured(code);
    let mut ops = measured_ops.data();
    let mut cells = vec![0u8; cells];

    let mut printer = make_printer();
    let mut scanner = make_scanner();

    measured_ops.append(T::exec_bench(
        &mut ops,
        &mut cells,
        &mut printer,
        &mut scanner,
        meassure,
    ))
}
