use crate::function::structure::ControlFlowGraph;
use crate::graph::{NodeId, NodeMap, NodeSet, Order};

impl ControlFlowGraph {
    pub fn find_2_way_conditionals(&self, ignored_headers: &NodeSet) -> NodeMap<NodeId> {
        // Find immediate dominators
        let idom = self.immediate_dominators();

        // Nodes for which the follow node has not yet been found
        let mut unresolved = NodeSet::new();
        // Maps header nodes to follow nodes where branches join back together
        let mut follow = NodeMap::with_capacity_for(self);

        let post_order = self.depth_first(Order::PostOrder);
        for &m in &post_order.traversal {
            if self[m].out_degree() == 2 && !ignored_headers.contains(m) {
                let n = self
                    .iter_id()
                    .filter(|&i| idom[i] == m && self[i].in_degree() >= 2)
                    // TODO: add test for this comparison: do with nested if and while triggers
                    // Look for "lowest" node in graph
                    .max_by(|&a, &b| post_order.cmp(a, b).reverse());
                match n {
                    Some(n) => {
                        follow.insert(m, n);
                        for x in unresolved.iter() {
                            follow.insert(x, n);
                        }
                        unresolved.clear();
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
