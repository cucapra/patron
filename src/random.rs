// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use crate::constraints::{analyze_constraints, ConstraintCluster};
use crate::{ModelCheckResult, StepInt, Witness};
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
    /// probability of sampling a large instead of a small k
    pub large_k_prob: f64,
    /// maximum number of cycles to execute
    pub max_cycles: Option<u64>,
}

pub fn random_testing(
    mut ctx: Context,
    sys: TransitionSystem,
    opts: RandomOptions,
    seed: u64,
) -> ModelCheckResult {
    // println!("{}", sys.serialize_to_str(&ctx));

    // collect constraints for input randomization
    let constraints = analyze_constraints(&mut ctx, &sys, false);

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

    // take a snapshot so that we can go back to the initial state
    let start_state = sim.take_snapshot();

    // create random number generator
    let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(seed);

    // main loop
    let mut cycle_count = 0;
    loop {
        let k_max = sample_k_max(&mut rng, &opts);

        // restore starting state
        sim.restore_snapshot(start_state);

        // save state of random number generator
        let rng_start = rng.clone();

        for k in 0..=k_max {
            // randomize inputs to the system
            randomize_inputs(
                &mut ctx,
                &mut rng,
                &constraints,
                &unconstrained_inputs,
                &mut sim,
            );
            sim.update(); // FIXME: support partial re-evaluation!

            // check if we are in a bad state
            let bads = check_for_bad_states(&mut ctx, &bad_states, &mut sim);
            if !bads.is_empty() {
                sim.restore_snapshot(start_state);
                let wit = record_witness(
                    &mut ctx,
                    &sys,
                    &constraints,
                    &unconstrained_inputs,
                    &bad_states,
                    &mut sim,
                    rng_start,
                    k,
                    bads,
                );
                return ModelCheckResult::Sat(wit);
            }

            // advance the system
            sim.step();
            cycle_count += 1;
            if let Some(max_cycles) = opts.max_cycles {
                if max_cycles <= cycle_count {
                    println!("Exciting after executing {} cycles.", cycle_count);
                    return ModelCheckResult::Unknown;
                }
            }
        }
    }
}

fn find_inputs() {}

/// replays random execution in order to record the witness
fn record_witness(
    ctx: &Context,
    sys: &TransitionSystem,
    constraints: &[ConstraintCluster],
    unconstrained_inputs: &[ExprRef],
    bad_states: &[ExprRef],
    sim: &mut Interpreter,
    mut rng: rand_xoshiro::Xoshiro256PlusPlus,
    k_bad: StepInt,
    bads: Vec<usize>,
) -> Witness {
    let mut state_init = Vec::new();
    for (_, state) in sys.states() {
        let value = sim.get(state.symbol).unwrap();
        state_init.extend_from_slice(value.words());
    }

    let mut input_data = Vec::new();
    for k in 0..=k_bad {
        // randomize inputs to the system
        randomize_inputs(ctx, &mut rng, constraints, unconstrained_inputs, sim);

        // TODO: implement this without tunneling through the sim!
        for (expr, info) in sys.get_signals(|s| s.is_input()) {
            if let Some(value) = sim.get(expr) {
                input_data.extend_from_slice(value.words());
            } else {
                let width = ctx.get(expr).get_bv_type(ctx).unwrap();
                if width > Word::BITS {
                    println!(
                        "TODO: deal with missing input {} of width: {}",
                        ctx.get(info.name.unwrap()),
                        width
                    );
                } else {
                    input_data.push(0);
                }
            }
        }

        // TODO: remove
        sim.update();

        // sanity check constraints
        for cluster in constraints.iter() {
            for expr in cluster.exprs() {
                let is_ok = sim.get(*expr).unwrap().to_u64().unwrap() == 1;
                debug_assert!(
                    is_ok,
                    "{k}: failed {} in {:?}",
                    ctx.get(*expr).serialize_to_str(ctx),
                    cluster
                );
            }
        }

        if k == k_bad {
            // sanity check bad
            let bads = check_for_bad_states(ctx, bad_states, sim);
            debug_assert!(!bads.is_empty());
        }
        sim.step();
    }

    Witness {
        input_data,
        state_init,
        k: k_bad,
        failed_safety: bads,
    }
}

fn sample_k_max(rng: &mut impl Rng, opts: &RandomOptions) -> StepInt {
    let pick_large_k = rng.gen_bool(opts.large_k_prob);
    if pick_large_k {
        rng.gen_range(opts.small_k..(opts.large_k + 1))
    } else {
        rng.gen_range(1..(opts.small_k + 1))
    }
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
