use crate::function::structure::ControlFlowGraph;
use crate::graph::{NodeId, Order};
use std::collections::{HashMap, HashSet};

impl ControlFlowGraph {
    pub fn find_2_way_conditionals(
        &self,
        ignored_headers: &HashSet<NodeId>,
    ) -> HashMap<NodeId, NodeId> {
        // Find immediate dominators
        let idom = self.immediate_dominators();

        // Nodes for which the follow node has not yet been found
        let mut unresolved = HashSet::new();
        // Maps header nodes to follow nodes where branches join back together
        let mut follow = HashMap::new();

        let post_order = self.depth_first(Order::PostOrder);
        for &m in &post_order.traversal {
            if self[m].out_degree() == 2 && !ignored_headers.contains(&m) {
                let n = self
                    .iter_id()
                    .filter(|&i| idom[&i] == m && self[i].in_degree() >= 2)
                    // TODO: add test for this comparison: do with nested if and while triggers
                    // Look for "lowest" node in graph
                    .max_by(|&a, &b| post_order.cmp(a, b).reverse());
                match n {
                    Some(n) => {
                        follow.insert(m, n);
                        for x in unresolved.drain() {
                            follow.insert(x, n);
                        }
                    }
                    None => {
                        unresolved.insert(m);
                    }
                }
            }
        }

        follow
    }
}
