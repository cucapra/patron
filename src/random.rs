// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use crate::constraints::{analyze_constraints, ConstraintCluster};
use patronus::ir::*;
use patronus::mc::Simulator;
use patronus::sim::interpreter::{InitKind, Interpreter};
use std::collections::HashSet;

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
    // collect constraints for input randomization
    let constraints = analyze_constraints(ctx, &sys, false);

    // find out which inputs are unconstrained
    let constrained_inputs = constraints
        .iter()
        .map(|c| c.inputs().to_vec())
        .flatten()
        .collect::<HashSet<_>>();
    let unconstrained_inputs = sys
        .get_signals(|s| s.is_input())
        .iter()
        .map(|(s, _)| *s)
        .filter(|s| !constrained_inputs.contains(s))
        .collect::<Vec<_>>();

    // create simulator
    let sim_ctx = ctx.clone();
    let mut sim = Interpreter::new(&sim_ctx, &sys);

    // we initialize all states to zero, since most bugs are not reset initialization bugs
    sim.init(InitKind::Zero);

    // randomize our first inputs
    randomize_inputs(ctx, &constraints, &unconstrained_inputs, &sys, &mut sim);

    RandomResult::None
}

fn randomize_inputs(
    ctx: &Context,
    constraints: &[ConstraintCluster],
    unconstrained_inputs: &[ExprRef],
    sys: &TransitionSystem,
    sim: &mut Interpreter,
) {
    // randomize constrained inputs
    for cluster in constraints.iter() {
        loop {
            // randomize all inputs in cluster
            for input in cluster.inputs().iter() {
                randomize_symbol(ctx, *input, sim);
            }

            // check to see if constraints are fulfilled
            let ok = cluster
                .exprs()
                .iter()
                .all(|expr| sim.get(*expr).unwrap().to_u64().unwrap() == 1);
            // if they are, we are done here
            if ok {
                break;
            }
        }
    }

    // randomize other inputs
    for input in unconstrained_inputs.iter() {
        randomize_symbol(ctx, *input, sim);
    }
}

fn randomize_symbol(ctx: &Context, symbol: ExprRef, sim: &mut Interpreter) {
    match ctx.get(symbol).get_bv_type(ctx) {
        Some(1) => {
            todo!("generate 1-bit values");
        }
        Some(width) => {
            if width <= 64 {
                todo!("generate 64-bit values");
            } else {
                todo!("generate value wider than 64-bit");
            }
        }
        None => {
            todo!("support array type inputs");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_size() {}
}
