// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use patronus::ir::{
    cone_of_influence, cone_of_influence_comb, cone_of_influence_init, Context, ExprRef, GetNode,
    SerializableIrNode, TransitionSystem, TypeCheck,
};
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

    // // show all inputs
    // let inputs = sys.get_signals(|s| s.is_input());
    // println!("INPUTS:");
    // for (expr_ref, input) in inputs.iter() {
    //     let expr = ctx.get(*expr_ref);
    //     println!("{} : {:?}", expr.serialize_to_str(ctx), expr.get_type(ctx));
    // }
    //
    // // show all states
    // println!("\nSTATES:");
    // for (state_ref, state) in sys.states() {
    //     let expr = ctx.get(state.symbol);
    //     println!("{} : {:?}", expr.serialize_to_str(ctx), expr.get_type(ctx));
    // }

    // show system
    println!("{}", sys.serialize_to_str(ctx));

    // analyze constraint dependencies
    let constraints = analyze_constraints(ctx, &sys, false);
    println!("{constraints:?}");

    // TODO

    RandomResult::None
}

struct Constraints {
    /// Single input constraints can be checked when we are sampling the input.
    /// The tuple is (constraint ref, input ref)
    single_input: Vec<(ExprRef, ExprRef)>,
    /// State only constraints which we essentially just need to check once and give up if they fail.
    state_only: Vec<ExprRef>,
    /// All other constraints we currently rejection sample after an input has been chosen.
    others: Vec<ExprRef>,
}

#[derive(Debug, Clone)]
struct Constraint {
    expr: ExprRef,
    inputs: Vec<ExprRef>,
    states: Vec<ExprRef>,
}

/// Check to see which constraints we can fulfill
fn analyze_constraints(ctx: &Context, sys: &TransitionSystem, init: bool) -> Vec<Constraint> {
    let states = sys.state_map();
    let mut out = Vec::new();
    // we want to see which constraints depends on which inputs
    for (expr_ref, info) in sys.constraints() {
        //
        let cone = if init {
            cone_of_influence_init(ctx, sys, expr_ref)
        } else {
            cone_of_influence_comb(ctx, sys, expr_ref)
        };
        let (states, inputs) = cone.iter().partition(|e| states.contains_key(e));
        let constraint = Constraint {
            expr: expr_ref,
            inputs,
            states,
        };
        out.push(constraint);
    }
    out
}
