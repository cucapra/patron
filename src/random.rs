// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use crate::constraints::analyze_constraints;
use patronus::ir::*;
use patronus::mc::Simulator;
use patronus::sim::interpreter::{InitKind, Interpreter};

#[derive(Debug, Copy, Clone)]
pub struct RandomOptions {
    /// bound for searching for a small counter examples
    pub small_k: u64,
    /// maximum length to try
    pub large_k: u64,
}

#[derive(Debug)]
pub enum RandomResult {
    None,
}

pub fn random_testing(
    ctx: &mut Context,
    sys: TransitionSystem,
    opts: RandomOptions,
) -> RandomResult {
    let sim_ctx = ctx.clone();
    let mut sim = Interpreter::new(&sim_ctx, &sys);

    // we initialize all states to zero, since most bugs are not reset initialization bugs
    sim.init(InitKind::Zero);

    // show system
    println!("{}", sys.serialize_to_str(ctx));
    let constraints = analyze_constraints(ctx, &sys, true);
    println!("{:?}", constraints);

    // TODO

    RandomResult::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_size() {}
}
