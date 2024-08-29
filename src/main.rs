// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>

mod constraints;
mod random;

use clap::{arg, Parser};
use patronus::btor2::DEFAULT_INPUT_PREFIX;
use patronus::ir::*;
use patronus::*;
use random::*;
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};

#[derive(Parser, Debug)]
#[command(name = "patron")]
#[command(author = "Kevin Laeufer <laeufer@cornell.edu>")]
#[command(version)]
#[command(about = "Tries to find a witness that shows how to get to a bad state.", long_about = None)]
struct Args {
    #[arg(short, long)]
    verbose: bool,
    #[arg(long)]
    single_thread: bool,
    #[arg(long)]
    max_cycles: Option<u64>,
    #[arg(value_name = "BTOR2", index = 1)]
    filename: String,
}

static RANDOM_OPTS: RandomOptions = RandomOptions {
    small_k: 50,
    large_k: 1_000,
    large_k_prob: 0.0,
    max_cycles: None,
};

fn main() {
    let args = Args::parse();

    // load system
    let (mut ctx, mut sys) = btor2::parse_file(&args.filename).expect("Failed to load btor2 file!");

    let orig_sys = sys.clone();
    let orig_ctx = ctx.clone();

    // simplify system
    replace_anonymous_inputs_with_zero(&mut ctx, &mut sys);
    simplify_expressions(&mut ctx, &mut sys);

    // run testing on multiple cores
    let num_threads = if args.single_thread {
        1
    } else {
        std::thread::available_parallelism().unwrap().get() as u64
    };
    let result = Arc::new(RwLock::new(None));
    for seed in 0..num_threads {
        let result = result.clone();
        let sys = sys.clone();
        let ctx = ctx.clone();
        let mut options = RANDOM_OPTS.clone();
        options.max_cycles = args.max_cycles.map(|c| c.div_ceil(num_threads));
        std::thread::spawn(move || {
            let res = random_testing(ctx.clone(), sys.clone(), options, seed);
            let mut shared_result = result.write().unwrap();
            *shared_result = Some(res);
        });
    }

    loop {
        let shared_result = (*result.read().unwrap()).clone();
        if let Some(res) = shared_result {
            match res {
                ModelCheckResult::Unknown => {
                    // print nothing
                }
                ModelCheckResult::UnSat => {
                    println!("unsat");
                }
                ModelCheckResult::Sat(wit) => {
                    println!("sat");
                    wit.print(&orig_ctx, &orig_sys, &mut std::io::stdout())
                        .unwrap()
                }
            }
            std::process::exit(0);
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModelCheckResult {
    Unknown,
    UnSat,
    Sat(Witness),
}

pub type StepInt = u64;

/// In-memory representation of a witness.
/// We currently assume that all states start at zero.
#[derive(Clone)]
pub struct Witness {
    pub input_data: Vec<Word>,
    pub state_init: Vec<Word>,
    pub k: StepInt,
    pub failed_safety: Vec<usize>,
}

impl Debug for Witness {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Witness(k={}, {:?})", self.k, self.failed_safety)
    }
}

/// Based on the upstream `patronus` implementation, however, using a much more lightweight
/// data format.
/// https://github.com/ekiwi/patronus/blob/a0ced099581d7a02079059eb96ac459e3133e70b/src/btor2/witness.rs#L224C22-L224C42
impl Witness {
    pub fn print(
        &self,
        ctx: &Context,
        sys: &TransitionSystem,
        out: &mut impl std::io::Write,
    ) -> std::io::Result<()> {
        // declare failed properties
        for (ii, bad_id) in self.failed_safety.iter().enumerate() {
            let is_last = ii + 1 == self.failed_safety.len();
            write!(out, "b{bad_id}")?;
            if is_last {
                writeln!(out)?;
            } else {
                write!(out, " ")?;
            }
        }

        // print starting state (always zero!)
        let mut offset = 0;
        if sys.states().count() > 0 {
            writeln!(out, "#0")?;
            for (ii, (_, state)) in sys.states().enumerate() {
                if state.init.is_some() {
                    // the state has a computed init value
                    continue;
                }
                let name = state
                    .symbol
                    .get_symbol_name(ctx)
                    .map(Cow::from)
                    .unwrap_or(Cow::from(format!("state_{}", ii)));

                match state.symbol.get_type(ctx) {
                    Type::BV(width) => {
                        let words = width.div_ceil(Word::BITS) as usize;
                        let value = ValueRef::new(&self.state_init[offset..offset + words], width);
                        offset += words;
                        writeln!(out, "{ii} {} {name}#0", value.to_bit_string())?;
                    }
                    Type::Array(_) => {
                        todo!("print array values!")
                    }
                }
            }
        }

        // filter out anonymous inputs which were removed from the system we were testing!
        let inputs = sys
            .get_signals(|s| s.is_input())
            .iter()
            .map(|(expr, _)| *expr)
            .enumerate()
            .collect::<Vec<_>>();

        // print inputs
        let mut offset = 0;
        for k in 0..=self.k {
            writeln!(out, "@{k}")?;
            for (ii, input) in inputs.iter() {
                let name = input.get_symbol_name(ctx).unwrap();
                let is_removed = name.starts_with(DEFAULT_INPUT_PREFIX);
                let width = input.get_bv_type(ctx).unwrap();
                let words = width.div_ceil(Word::BITS) as usize;
                let value = if is_removed {
                    "0".repeat(width as usize)
                } else {
                    let value = ValueRef::new(&self.input_data[offset..offset + words], width);
                    offset += words;
                    value.to_bit_string()
                };
                writeln!(out, "{ii} {} {name}@{k}", value)?;
            }
        }
        debug_assert_eq!(offset, self.input_data.len());
        writeln!(out, ".")?;
        Ok(())
    }
}
