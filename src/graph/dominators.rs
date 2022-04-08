use crate::graph::{Graph, NodeId, NodeOrder, Order};
use std::collections::HashMap;

fn intersect(
    post_order: &NodeOrder,
    doms: &HashMap<NodeId, Option<NodeId>>,
    b1: NodeId,
    b2: NodeId,
) -> NodeId {
    let mut finger1 = b1;
    let mut finger2 = b2;
    while finger1 != finger2 {
        while post_order.cmp(finger1, finger2).is_lt() {
            finger1 = doms[&finger1].unwrap()
        }
        while post_order.cmp(finger2, finger1).is_lt() {
            finger2 = doms[&finger2].unwrap()
        }
    }
    finger1
}

impl<T> Graph<T> {
    pub fn immediate_dominators(&self) -> HashMap<NodeId, NodeId> {
        let start = self.entry.expect("dominators needs entrypoint");

        let post_order = self.depth_first(Order::PostOrder);
        let mut reverse_post_order_traversal = post_order.traversal.clone();
        reverse_post_order_traversal.reverse();

        // https://www.cs.rice.edu/~keith/EMBED/dom.pdf#page=7
        let mut doms = HashMap::<NodeId, Option<NodeId>>::new();
        for id in self.iter_id() {
            doms.insert(id, None);
        }
        doms.insert(start, Some(start));

        let mut changed = true;
        while changed {
            changed = false;
            for &b in &reverse_post_order_traversal {
                // Skip start node
                if b == start {
                    continue;
                }
                // b is not the start node, so must have at least one incoming edge
                assert!(!self[b].predecessors.is_empty());

                let mut new_idom = *self[b]
                    .predecessors
                    .iter()
                    .find(|p| doms[p].is_some())
                    .unwrap();
                for &p in &self[b].predecessors {
                    if p != new_idom && doms[&p].is_some() {
                        new_idom = intersect(&post_order, &doms, p, new_idom)
                    }
                }
                if doms[&b] != Some(new_idom) {
                    doms.insert(b, Some(new_idom));
                    changed = true;
                }
            }
        }

        doms.into_iter().map(|(k, v)| (k, v.unwrap())).collect()
    }

    #[inline]
    pub fn immediate_post_dominators(&self) -> HashMap<NodeId, NodeId> {
        // Mapped values don't matter here, so use unit, we're only interested in IDs/edges
        self.map_reversed(|_, _| ()).immediate_dominators()
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::tests::fixture_2;

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
        assert_eq!(idom, expected_idom);
    }
}
