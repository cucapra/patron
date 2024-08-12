// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use patronus::ir::{Context, GetNode, SerializableIrNode, TransitionSystem, TypeCheck};
use patronus::sim;

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
    let mut sim = sim::interpreter::Interpreter::new(&sim_ctx, &sys);

    // show all inputs
    let inputs = sys.get_signals(|s| s.is_input());
    println!("INPUTS:");
    for (expr_ref, input) in inputs.iter() {
        let expr = ctx.get(*expr_ref);
        println!("{} : {:?}", expr.serialize_to_str(ctx), expr.get_type(ctx));
    }

    // show all states
    println!("\nSTATES:");
    for (state_ref, state) in sys.states() {
        let expr = ctx.get(state.symbol);
        println!("{} : {:?}", expr.serialize_to_str(ctx), expr.get_type(ctx));
    }

    // TODO

    RandomResult::None
}
