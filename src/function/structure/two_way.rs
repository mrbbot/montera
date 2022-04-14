use crate::function::structure::ControlFlowGraph;
use crate::graph::{NodeId, NodeMap, NodeSet, Order};

impl ControlFlowGraph {
    /// Identifies all 2-way conditionals (`if`-statements) in the control flow graph, returning a
    /// map of header nodes to their corresponding follow nodes, using the algorithm described in
    /// Figure 6.31 of "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland
    /// University of Technology, 1994".
    ///
    /// Note multiple headers may share the same follow node if they are nested.
    ///
    /// This should be called after structuring compound short-circuit conditionals, as these might
    /// be used in header nodes (e.g. `if (a && b) { ... }`).
    ///
    /// # Overview
    ///
    /// The analysis uses [immediate dominators](crate::graph::Graph::immediate_dominators).
    /// The graph is traversed in depth-first post-order (reverse) so nested structures are handled
    /// first. An `unresolved` set records header nodes for which follow nodes haven't yet been
    /// found. The follow node is the maximum node with the header as its immediate dominator, and
    /// at least 2 predecessors (for 2 paths from the header). If this follow cannot be found, the
    /// header is added to `unresolved`. When a follow node is found, all `unresolved` header nodes
    /// are assigned that follow node.
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
