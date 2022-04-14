use crate::graph::{Graph, NodeId, NodeMap, NodeOrder, Order};

/// Helper function for [`Graph::immediate_dominators`].
fn intersect(
    post_order: &NodeOrder,
    doms: &NodeMap<Option<NodeId>>,
    mut finger1: NodeId, // = b1
    mut finger2: NodeId, // = b2
) -> NodeId {
    while finger1 != finger2 {
        while post_order.cmp(finger1, /* < */ finger2).is_lt() {
            finger1 = doms[finger1].unwrap()
        }
        while post_order.cmp(finger2, /* < */ finger1).is_lt() {
            finger2 = doms[finger2].unwrap()
        }
    }
    finger1
}

impl<T> Graph<T> {
    /// Computes the immediate dominator for each node in the graph.
    ///
    /// This uses the *engineered algorithm* described in Figure 3 of ["Keith Cooper, Timothy
    /// Harvey, and Ken Kennedy. A simple, fast dominance algorithm. Rice University, CS Technical
    /// Report 06-33870, 01 2006."](https://www.cs.rice.edu/~keith/EMBED/dom.pdf#page=7).
    ///
    /// # Definitions
    ///
    /// - Node `a` dominates `b` if control from the start must pass through `a` before reaching `b`
    /// - Node `a` strictly dominates `b` if `a` dominates `b` and `a != b`
    /// - Node `a` is the unique immediate dominator of `b` if `a` strictly dominates `b`,
    ///   but does not dominate any other strict dominator of `b`
    ///
    /// We define the immediate dominator of the entrypoint as itself.
    ///
    /// # Panics
    ///
    /// Panics if the graph doesn't have an entrypoint.
    pub fn immediate_dominators(&self) -> NodeMap<NodeId> {
        // Dominance is defined based on control passing *from* a starting node
        let start = self.entry.expect("dominators needs entrypoint");

        // We perform post-order comparisons, but iterate in reverse-post-order
        let post_order = self.depth_first(Order::PostOrder);
        let mut reverse_post_order_traversal = post_order.traversal.clone();
        reverse_post_order_traversal.reverse();

        // All immediate dominators are initially undefined...
        let mut idom = NodeMap::with_capacity_for(self);
        for id in self.iter_id() {
            idom.insert(id, None);
        }
        // ...except the start which is defined as its own dominator
        idom.insert(start, Some(start));

        // While immediate dominators change...
        let mut changed = true;
        while changed {
            changed = false;
            for &b in &reverse_post_order_traversal {
                // Always skip start node
                if b == start {
                    continue;
                }
                // b is not the start node, so must have at least one incoming edge
                assert!(!self[b].predecessors.is_empty());

                // Find the first immediate predecessor of b with a defined immediate dominator
                let mut new_idom = *self[b]
                    .predecessors
                    .iter()
                    .find(|&&p| idom[p].is_some())
                    .unwrap();
                // Traverse the dominator tree to find the correct intersection point
                for &p in &self[b].predecessors {
                    if p != new_idom && idom[p].is_some() {
                        new_idom = intersect(&post_order, &idom, p, new_idom)
                    }
                }
                // If the immediate dominator changed, update and note changed
                if idom[b] != Some(new_idom) {
                    idom.insert(b, Some(new_idom));
                    changed = true;
                }
            }
        }

        // Once we reach a fixed point, all nodes have immediate dominators so unwrap and return
        idom.into_iter().map(|(k, v)| (k, v.unwrap())).collect()
    }

    /// Computes the immediate post-dominator for each node in the graph.
    ///
    /// # Definitions
    ///
    /// - Node `a` post-dominates `b` if control from `b` must pass through `a` before reaching the
    ///   exit
    /// - Node `a` strictly post-dominates `b` if `a` post-dominates `b` and `a != b`
    /// - Node `a` is the unique immediate post-dominator of `b` if `a` strictly post-dominates `b`,
    ///   but does not post-dominate any other strict post-dominator of `b`
    ///
    /// We define the immediate post-dominator of the exit as itself.
    ///
    /// Notably, the immediate post-dominators of a graph G are the immediate dominators of the
    /// reverse of G (flipping edge directions and setting entry to exit).
    ///
    /// # Panics
    ///
    /// Panics if this graph does not have exactly one "exit" node (with out degree 0).
    #[inline]
    pub fn immediate_post_dominators(&self) -> NodeMap<NodeId> {
        // Mapped values don't matter here so use unit, we're only interested in IDs/edges
        self.map_reversed(|_, _| ()).immediate_dominators()
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::tests::{fixture_1, fixture_2, fixture_cyclic};
    use crate::graph::Graph;

    #[test]
    fn immediate_dominators_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let idom = g.immediate_dominators();
        let expected_idom = hashmap! {
            n1 => n1,
            n2 => n1,
            n3 => n2,
            n4 => n3,
            n5 => n2,
            n6 => n5,
        };
        assert_eq!(idom, expected_idom.into());
    }

    #[test]
    fn immediate_dominators_2() {
        let (g, (n1, n2, n3, n4, n5, n6, n7, n8)) = fixture_2();
        let idom = g.immediate_dominators();
        let expected_idom = hashmap! {
            n1 => n1,
            n2 => n1,
            n3 => n2,
            n4 => n3,
            n5 => n3,
            n6 => n3,
            n7 => n2,
            n8 => n7,
        };
        assert_eq!(idom, expected_idom.into());
    }

    #[test]
    fn immediate_dominators_cyclic() {
        let (g, (n1, n2)) = fixture_cyclic();
        let idom = g.immediate_dominators();
        let expected_idom = hashmap! {
            n1 => n1,
            n2 => n1,
        };
        assert_eq!(idom, expected_idom.into());
    }

    #[test]
    fn immediate_post_dominators_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let ipdom = g.immediate_post_dominators();
        let expected_ipdom = hashmap! {
            n1 => n2,
            n2 => n5,
            n3 => n4,
            n4 => n2,
            n5 => n6,
            n6 => n6,
        };
        assert_eq!(ipdom, expected_ipdom.into());
    }

    #[test]
    #[should_panic = "reverse expects an exit node"]
    fn invalid_immediate_post_dominators() {
        // Check `immediate_post_dominators` panics if no exit nodes for new entrypoint
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        g.add_edge(n1, n2);
        g.add_edge(n2, n1);
        g.immediate_post_dominators();
    }
}
