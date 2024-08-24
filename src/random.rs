// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// Random testing strategy to finding counter examples.

use patronus::ir::*;
use patronus::mc::Simulator;
use patronus::sim::interpreter::{InitKind, Interpreter};
use smallvec::{smallvec, SmallVec};
use std::collections::{HashMap, HashSet};

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

    // analyze constraint dependencies
    let constraints = analyze_constraints(ctx, &sys, true);
    let constraints = merge_clusters(&constraints);
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

/// A number of constraints that are connected by common symbols.
#[derive(Debug, Clone)]
struct ConstraintCluster {
    exprs: ExprRefVec,
    leaves: ExprRefVec,
}

fn extract_constraint_graph(ctx: &mut Context, sys: &TransitionSystem, init: bool) {
    let state_map = sys.state_map();
    let mut out = petgraph::Graph::new_undirected();
    let mut var_to_node = HashMap::new();

    // we want to see which constraints depends on which inputs
    for (expr_ref, info) in sys.constraints() {
        // split constraints if they are conjunctions, i.e., we split and(a, b) into two constraints
        let sub_constraints = split_conjunction(ctx, expr_ref);

        for expr_ref in sub_constraints.into_iter() {
            // analyze the constraint
            let mut leaves: ExprRefVec = if init {
                // if we are initializing, then we need to choose states and inputs
                cone_of_influence_init(ctx, sys, expr_ref).into()
            } else {
                // if we are in a different cycle, then we do not actually care about states, since
                // we cannot change them without going back in time
                cone_of_influence_comb(ctx, sys, expr_ref)
                    .into_iter()
                    .filter(|e| !state_map.contains_key(e))
                    .collect()
            };

            // constraints connect all their leaves together
            leaves.sort();
            leaves.dedup();

            // make sure all leaves are represented as nodes
            for leaf in leaves.iter() {
                if !var_to_node.contains_key(leaf) {
                    let node = out.add_node(leaf);
                    var_to_node.insert(leaf, node);
                }
            }

            while let Some(leaf) = leaves.pop() {
                for other in leaves.iter() {
                    debug_assert_ne!(leaf, *other);
                    // constraint creates a connection
                    out.add_edge(var_to_node[&leaf], var_to_node[other], expr_ref);
                }
            }
        }
    }
}

/// Check to see which constraints we can fulfill
fn analyze_constraints(
    ctx: &mut Context,
    sys: &TransitionSystem,
    init: bool,
) -> Vec<ConstraintCluster> {
    let state_map = sys.state_map();

    let mut out = Vec::new();
    // we want to see which constraints depends on which inputs
    for (expr_ref, info) in sys.constraints() {
        // split constraints if they are conjunctions, i.e., we split and(a, b) into two constraints
        let sub_constraints = split_conjunction(ctx, expr_ref);

        for expr_ref in sub_constraints.into_iter() {
            // analyze the constraint
            let leaves = if init {
                // if we are initializing, then we need to choose states and inputs
                cone_of_influence_init(ctx, sys, expr_ref).into()
            } else {
                // if we are in a different cycle, then we do not actually care about states, since
                // we cannot change them without going back in time
                cone_of_influence_comb(ctx, sys, expr_ref)
                    .into_iter()
                    .filter(|e| !state_map.contains_key(e))
                    .collect()
            };

            out.push(ConstraintCluster {
                exprs: smallvec![expr_ref],
                leaves,
            });
        }
    }

    out
}

fn merge_clusters(in_clusters: &[ConstraintCluster]) -> Vec<ConstraintCluster> {
    let mut m = HashMap::new();
    let mut clusters = Vec::new();
    for cluster in in_clusters.iter() {
        // collect all other constraints that share at least one leaf
        let others: Vec<_> = cluster.leaves.iter().flat_map(|l| m.get(l)).collect();

        if others.is_empty() {
            let id = clusters.len();
            clusters.push(cluster.clone());
            for leaf in cluster.leaves.iter() {
                m.insert(leaf, id);
            }
        } else {
            todo!("deal with overlapping cluster");
        }
    }

    // collect output
    let mut indices = m.iter().map(|(_, idx)| *idx).collect::<Vec<_>>();
    indices.sort();
    indices.dedup();
    indices
        .into_iter()
        .map(|ii| clusters[ii].clone())
        .collect::<Vec<_>>()
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
