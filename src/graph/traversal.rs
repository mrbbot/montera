use crate::graph::{Graph, NodeId};
use either::Either;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

pub struct NodeOrder {
    pub traversal: Vec<NodeId>,
    mapping: RefCell<Option<HashMap<NodeId, usize>>>,
}

impl NodeOrder {
    pub fn from_traversal(traversal: Vec<NodeId>) -> Self {
        // Lazily compute mapping between node IDs and ordering on first `cmp` call
        let mapping = RefCell::new(None);
        Self { traversal, mapping }
    }

    #[inline]
    fn ensure_mapping<'a>(
        &self,
        mapping: &'a mut Option<HashMap<NodeId, usize>>,
    ) -> &'a HashMap<NodeId, usize> {
        mapping.get_or_insert_with(|| {
            self.traversal
                .iter()
                .enumerate()
                .map(|(i, &node)| (node, i))
                .collect()
        })
    }

    pub fn cmp(&self, a: NodeId, b: NodeId) -> Ordering {
        let mut mapping = self.mapping.borrow_mut();
        let mapping = self.ensure_mapping(&mut mapping);
        let a_order = mapping[&a];
        let b_order = mapping[&b];
        a_order.cmp(&b_order)
    }

    pub fn range(&self, a: NodeId, b: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let a_position = self.traversal.iter().position(|&x| x == a).unwrap();
        let b_position = self.traversal.iter().position(|&x| x == b).unwrap();
        (a_position..b_position).map(move |i| self.traversal[i])
    }
}

#[derive(Copy, Clone)]
pub enum Order {
    PreOrder,
    PostOrder,
    ReversePreOrder,
    ReversePostOrder,
}

impl<T> Graph<T> {
    fn depth_first_inner(
        &self,
        order: Order,
        traversal: &mut Vec<NodeId>,
        visited: &mut HashSet<NodeId>,
        node: NodeId,
    ) {
        if matches!(order, Order::PreOrder | Order::ReversePreOrder) {
            traversal.push(node);
        }
        let iter = self[node].successors.iter();
        let iter = match order {
            Order::ReversePreOrder | Order::ReversePostOrder => Either::Left(iter.rev()),
            _ => Either::Right(iter),
        };
        for &succ in iter {
            if !visited.contains(&succ) {
                visited.insert(succ);
                self.depth_first_inner(order, traversal, visited, succ);
            }
        }
        if matches!(order, Order::PostOrder | Order::ReversePostOrder) {
            traversal.push(node);
        }
    }

    pub fn depth_first(&self, order: Order) -> NodeOrder {
        // Preallocate traversal/visited as we know we'll visit each node once, assuming connected
        let len = self.len();
        let mut traversal = Vec::with_capacity(len);
        let mut visited = HashSet::with_capacity(len);

        let start = self.entry_id().expect("traversal needs entrypoint");
        visited.insert(start);

        self.depth_first_inner(order, &mut traversal, &mut visited, start);

        NodeOrder::from_traversal(traversal)
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::tests::{fixture_1, fixture_cyclic};
    use crate::graph::Order;

    #[test]
    fn depth_first_post_order_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let traversal = g.depth_first(Order::PostOrder).traversal;
        assert_eq!(traversal, vec![n4, n3, n6, n5, n2, n1]);
    }

    #[test]
    fn depth_first_post_order_cyclic() {
        let (g, (n1, n2)) = fixture_cyclic();
        let traversal = g.depth_first(Order::PostOrder).traversal;
        assert_eq!(traversal, vec![n2, n1]);
    }
}
