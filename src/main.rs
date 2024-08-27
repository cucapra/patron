// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>

mod constraints;
mod random;

use clap::{arg, Parser};
use patronus::ir::{replace_anonymous_inputs_with_zero, simplify_expressions};
use patronus::*;
use random::*;

#[derive(Parser, Debug)]
#[command(name = "patron")]
#[command(author = "Kevin Laeufer <laeufer@cornell.edu>")]
#[command(version)]
#[command(about = "Tries to find a witness that shows how to get to a bad state.", long_about = None)]
struct Args {
    #[arg(short, long)]
    verbose: bool,
    #[arg(value_name = "BTOR2", index = 1)]
    filename: String,
}

static RANDOM_OPTS: RandomOptions = RandomOptions {
    small_k: 50,
    large_k: 10_000,
};

fn main() {
    let args = Args::parse();

    // load system
    let (mut ctx, mut sys) = btor2::parse_file(&args.filename).expect("Failed to load btor2 file!");

    // simplify system
    replace_anonymous_inputs_with_zero(&mut ctx, &mut sys);
    simplify_expressions(&mut ctx, &mut sys);

    // try random testing
    match random_testing(&mut ctx, sys, RANDOM_OPTS) {
        RandomResult::None => {
            println!("None")
        }
        RandomResult::Sat(bad_states) => {
            println!("Failed assertion: {:?}", bad_states);
        }
    }
}
