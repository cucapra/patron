// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>

mod constraints;
mod random;

use clap::{arg, Parser};
use patronus::ir::{replace_anonymous_inputs_with_zero, simplify_expressions, Word};
use patronus::*;
use random::*;
use std::fmt::{Debug, Formatter};

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
    large_k: 1_000,
    large_k_prob: 0.01,
};

fn main() {
    let args = Args::parse();

    // load system
    let (mut ctx, mut sys) = btor2::parse_file(&args.filename).expect("Failed to load btor2 file!");

    // simplify system
    replace_anonymous_inputs_with_zero(&mut ctx, &mut sys);
    simplify_expressions(&mut ctx, &mut sys);

    // try random testing
    match random_testing(ctx, sys, RANDOM_OPTS) {
        ModelCheckResult::Unknown => {
            // print nothing
        }
        ModelCheckResult::UnSat => {
            println!("unsat");
        }
        ModelCheckResult::Sat(wit) => {
            println!("sat");
            println!("TODO: serialize witness correctly!");
            println!("{:?}", wit);
        }
    }
}

pub enum ModelCheckResult {
    Unknown,
    UnSat,
    Sat(Witness),
}

pub type StepInt = u64;

#[derive(Clone)]
pub struct Witness {
    pub input_data: Vec<Word>,
    pub k: StepInt,
    pub bad_states: Vec<usize>,
}

impl Debug for Witness {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Witness(k={}, {:?})", self.k, self.bad_states)
    }
}
