// Copyright 2024 Cornell University
// released under MIT License
// author: Kevin Laeufer <laeufer@cornell.edu>
//
// constraint analysis

use patronus::ir::*;
use smallvec::{smallvec, SmallVec};
use std::collections::HashMap;

pub type ExprRefVec = SmallVec<[ExprRef; 4]>;

/// A number of constraints that are connected by common symbols.
#[derive(Debug, Clone, Default)]
pub struct ConstraintCluster {
    exprs: ExprRefVec,
    states: ExprRefVec,
    inputs: ExprRefVec,
}

impl ConstraintCluster {
    pub fn new(exprs: ExprRefVec, states: ExprRefVec, inputs: ExprRefVec) -> Self {
        let mut out = Self {
            exprs,
            states,
            inputs,
        };
        out.dedup();
        out
    }
    fn dedup(&mut self) {
        self.exprs.sort_unstable();
        self.exprs.dedup();
        self.states.sort_unstable();
        self.states.dedup();
        self.inputs.sort_unstable();
        self.inputs.dedup();
    }
    pub fn exprs(&self) -> &ExprRefVec {
        &self.exprs
    }
    pub fn inputs(&self) -> &ExprRefVec {
        &self.inputs
    }
}

/// Check to see which constraints we can fulfill
pub fn analyze_constraints(
    ctx: &mut Context,
    sys: &TransitionSystem,
    init: bool,
) -> Vec<ConstraintCluster> {
    use petgraph::visit::NodeIndexable;
    let graph = extract_constraint_graph(ctx, sys, init);

    // extract connected components from graph
    let groups = connected_components(&graph);

    // turn components into constraint clusters
    let state_map = sys.state_map();
    let mut out = Vec::with_capacity(groups.len());
    for group in groups.into_iter() {
        let mut symbols: ExprRefVec = smallvec![];
        let mut exprs: ExprRefVec = smallvec![];

        for node_index in group {
            let node = NodeIndexable::from_index(&graph, node_index);
            symbols.push(*graph.node_weight(node).unwrap());
            // add all edges
            for edge in graph.edges(node) {
                exprs.push(*edge.weight());
            }
        }
        let (states, inputs) = symbols.into_iter().partition(|s| state_map.contains_key(s));

        out.push(ConstraintCluster::new(exprs, states, inputs));
    }

    out
}

type ConstraintGraph = petgraph::Graph<ExprRef, ExprRef, petgraph::Undirected>;

fn extract_constraint_graph(
    ctx: &mut Context,
    sys: &TransitionSystem,
    init: bool,
) -> ConstraintGraph {
    let state_map = sys.state_map();
    let mut out = petgraph::Graph::new_undirected();
    let mut var_to_node = HashMap::new();

    // we want to see which constraints depends on which inputs
    for (expr_ref, _) in sys.constraints() {
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
                    let node = out.add_node(*leaf);
                    var_to_node.insert(*leaf, node);
                }
            }

            // generate constraint edges
            while let Some(leaf) = leaves.pop() {
                for other in leaves.iter() {
                    debug_assert_ne!(leaf, *other);
                    // constraint creates a connection
                    out.add_edge(var_to_node[&leaf], var_to_node[other], expr_ref);
                }
                // we always need a self edge (in case there is only one leaf)
                out.add_edge(var_to_node[&leaf], var_to_node[&leaf], expr_ref);
            }
        }
    }
    out
}

/// extracts connected components, based on petgraph::algo::connected_components
fn connected_components(g: &ConstraintGraph) -> Vec<SmallVec<[usize; 2]>> {
    use petgraph::prelude::EdgeRef;
    use petgraph::visit::NodeIndexable;

    let mut vertex_sets = petgraph::unionfind::UnionFind::new(g.node_bound());
    for edge in g.edge_references() {
        let (a, b) = (edge.source(), edge.target());
        // union the two vertices of the edge
        vertex_sets.union(
            NodeIndexable::to_index(&g, a),
            NodeIndexable::to_index(&g, b),
        );
    }

    let mut clusters: HashMap<usize, SmallVec<[usize; 2]>> = HashMap::new();

    for (index, label) in vertex_sets.into_labeling().into_iter().enumerate() {
        if let Some(cluster) = clusters.get_mut(&label) {
            cluster.push(index)
        } else {
            clusters.insert(label, smallvec![index]);
        }
    }

    let mut out = clusters.into_values().collect::<Vec<_>>();
    out.sort_unstable();
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
