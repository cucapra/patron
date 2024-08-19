// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use patronus::ir::*;
use patronus::mc::Simulator;
use patronus::sim::interpreter::{InitKind, Interpreter};
use smallvec::{smallvec, SmallVec};

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

/// randomize all inputs

struct Constraints {
    /// Single input constraints can be checked when we are sampling the input.
    /// The tuple is (constraint ref, input ref)
    single_input: Vec<(ExprRef, ExprRef)>,
    /// State only constraints which we essentially just need to check once and give up if they fail.
    state_only: Vec<ExprRef>,
    /// All other constraints we currently rejection sample after an input has been chosen.
    others: Vec<ExprRef>,
}

type ExprRefVec = SmallVec<[ExprRef; 4]>;

/// A number of constraints that connect several inputs/states together.
#[derive(Debug, Clone)]
struct ConstraintCluster {
    exprs: ExprRefVec,
    inputs: ExprRefVec,
    states: ExprRefVec,
}

/// Check to see which constraints we can fulfill
fn analyze_constraints(
    ctx: &Context,
    sys: &TransitionSystem,
    init: bool,
) -> Vec<ConstraintCluster> {
    let state_map = sys.state_map();
    let mut out = Vec::new();
    // we want to see which constraints depends on which inputs
    for (expr_ref, info) in sys.constraints() {
        //
        let cone = if init {
            cone_of_influence_init(ctx, sys, expr_ref)
        } else {
            cone_of_influence_comb(ctx, sys, expr_ref)
        };
        let (states, inputs) = cone.into_iter().partition(|e| state_map.contains_key(e));
        let constraint = ConstraintCluster {
            exprs: smallvec![expr_ref],
            inputs,
            states,
        };
        out.push(constraint);
    }
    out
}

fn split_conjunction(ctx: &mut Context, e: ExprRef) -> ExprRefVec {
    let mut out = smallvec![];
    let mut todo: ExprRefVec = smallvec![e];
    while let Some(e) = todo.pop() {
        match ctx.get(e) {
            Expr::BVAnd(a, b, 1) => {
                todo.push(*b);
                todo.push(*a);
            }
            Expr::BVNot(e2, 1) => match *ctx.get(*e2) {
                Expr::BVOr(a, b, 1) => {
                    todo.push(ctx.not(b));
                    todo.push(ctx.not(a));
                }
                _ => out.push(e),
            },
            _ => out.push(e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_size() {
        // one ExprRef is 1/2 a pointer, thus 4 ExprRef fit into two pointer sized memory slots
        assert_eq!(std::mem::size_of::<ExprRef>(), 4);
        // we scale the ExprRefVec to be the same size on stack as a Vec<ExprRef>
        assert_eq!(
            std::mem::size_of::<ExprRefVec>(),
            std::mem::size_of::<Vec<ExprRef>>()
        );
    }

    #[test]
    fn test_split_conjunction() {
        let mut ctx = Context::default();
        let a = ctx.bv_symbol("a", 1);
        let b = ctx.bv_symbol("b", 1);
        let c = ctx.bv_symbol("b", 1);
        assert_eq!(split_conjunction(&mut ctx, a), [a].into());
        let a_and_b = ctx.and(a, b);
        assert_eq!(split_conjunction(&mut ctx, a_and_b), [a, b].into());
        let a_and_b_and_c_1 = ctx.and(a_and_b, c);
        assert_eq!(
            split_conjunction(&mut ctx, a_and_b_and_c_1),
            [a, b, c].into()
        );
        let b_and_c = ctx.and(b, c);
        let a_and_b_and_c_2 = ctx.and(a, b_and_c);
        assert_eq!(
            split_conjunction(&mut ctx, a_and_b_and_c_2),
            [a, b, c].into()
        );
        let a_or_b = ctx.or(a, b);
        assert_eq!(split_conjunction(&mut ctx, a_or_b), [a_or_b].into());
        let not_a_or_b = ctx.not(a_or_b);
        let not_a = ctx.not(a);
        let not_b = ctx.not(b);
        assert_eq!(
            split_conjunction(&mut ctx, not_a_or_b),
            [not_a, not_b].into()
        );
    }
}
