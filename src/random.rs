// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use crate::constraints::{analyze_constraints, ConstraintCluster};
use patronus::ir::value::mask;
use patronus::ir::*;
use patronus::mc::Simulator;
use patronus::sim::interpreter::{InitKind, Interpreter};
use rand::{Rng, SeedableRng};
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
    Sat(Vec<usize>),
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

    // collect bad states
    let bad_states = sys
        .bad_states()
        .into_iter()
        .map(|(e, _)| e)
        .collect::<Vec<_>>();

    // create simulator
    let sim_ctx = ctx.clone();
    let mut sim = Interpreter::new(&sim_ctx, &sys);

    // we initialize all states to zero, since most bugs are not reset initialization bugs
    sim.init(InitKind::Zero);

    let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(1);

    // randomize inputs to the system
    randomize_inputs(ctx, &mut rng, &constraints, &unconstrained_inputs, &mut sim);
    sim.update(); // FIXME: support partial re-evaluation!

    // check if we are in a bad state
    let bads = check_for_bad_states(ctx, &bad_states, &mut sim);
    if !bads.is_empty() {
        return RandomResult::Sat(bads);
    }

    RandomResult::None
}

fn check_for_bad_states(
    ctx: &Context,
    bad_states: &[ExprRef],
    sim: &mut Interpreter,
) -> Vec<usize> {
    let mut out = Vec::with_capacity(0);

    for (index, expr) in bad_states.iter().enumerate() {
        let is_bad = sim.get(*expr).unwrap().to_u64().unwrap() == 1;
        if is_bad {
            out.push(index);
        }
    }

    out
}

fn randomize_inputs(
    ctx: &Context,
    rng: &mut impl Rng,
    constraints: &[ConstraintCluster],
    unconstrained_inputs: &[ExprRef],
    sim: &mut Interpreter,
) {
    // randomize constrained inputs
    for cluster in constraints.iter() {
        loop {
            // randomize all inputs in cluster
            for input in cluster.inputs().iter() {
                randomize_symbol(ctx, rng, *input, sim);
            }

            // recalculate values
            sim.update(); // FIXME: support partial re-evaluation!

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
        randomize_symbol(ctx, rng, *input, sim);
    }
}

fn randomize_symbol(ctx: &Context, rng: &mut impl Rng, symbol: ExprRef, sim: &mut Interpreter) {
    match ctx.get(symbol).get_bv_type(ctx) {
        Some(width) => {
            if width <= 64 {
                let mask = mask(width);
                debug_assert_eq!(Word::BITS, 64);
                let value = (rng.next_u64() as Word) & mask;
                let words = [value];
                sim.set(symbol, ValueRef::new(&words, width));
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
