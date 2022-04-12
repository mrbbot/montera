use crate::graph::{Graph, NodeId, NodeMap, NodeSet};
use either::Either;
use std::cell::RefCell;
use std::cmp::Ordering;

/// Ordering of [`NodeId`]s from a [`Graph`] traversal.
///
/// See [`NodeOrder::cmp`] for how an [`Ordering`] corresponds to the visited order.
pub struct NodeOrder {
    pub traversal: Vec<NodeId>,
    mapping: RefCell<Option<NodeMap<usize>>>,
}

impl NodeOrder {
    /// Constructs a new [`NodeOrder`]ing from the result of a traversal, in visited order.
    pub fn from_traversal(traversal: Vec<NodeId>) -> Self {
        // Lazily compute mapping between node IDs and ordering on first `cmp` call
        let mapping = RefCell::new(None);
        Self { traversal, mapping }
    }

    /// Compute a mapping between [`NodeId`]s and `usize`s for comparison.
    ///
    /// This is computed lazily by [`NodeOrder::cmp`] to avoid unnecessary allocations.
    #[inline]
    fn ensure_mapping<'a>(&self, mapping: &'a mut Option<NodeMap<usize>>) -> &'a NodeMap<usize> {
        mapping.get_or_insert_with(|| {
            self.traversal
                .iter()
                .enumerate()
                .map(|(i, &node)| (node, i))
                .collect()
        })
    }

    /// Compares nodes `a` and `b` relative to the constructed `traversal` visited order.
    ///
    /// - `a < b` implies node `a` was visited *before* node `b`
    /// - `a = b` implies node `a` is the same as node `b`
    /// - `a > b` implies node `a` was visited *after* node `b`
    pub fn cmp(&self, a: NodeId, b: NodeId) -> Ordering {
        // Make sure we've computed the mapping between `NodeId`s and `usize` indices
        let mut mapping = self.mapping.borrow_mut();
        let mapping = self.ensure_mapping(&mut mapping);
        // Compare mapped indices
        let a_order = mapping[a];
        let b_order = mapping[b];
        a_order.cmp(&b_order)
    }

    /// Returns an iterator containing all [`NodeId`]s in the interval `[a, b)`.
    pub fn range(&self, a: NodeId, b: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let a_position = self.traversal.iter().position(|&x| x == a).unwrap();
        let b_position = self.traversal.iter().position(|&x| x == b).unwrap();
        (a_position..b_position).map(move |i| self.traversal[i])
    }
}

/// Possible orderings for [`Graph::depth_first`] traversals.
// allow(dead_code) here as not all variants are used outside tests, but it's simple to include them
// and they may be useful in the future.
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum Order {
    PreOrder,
    PostOrder,
    ReversePreOrder,
    ReversePostOrder,
}

impl<T> Graph<T> {
    /// Recursive helper function for [`Graph::depth_first`].
    fn depth_first_inner(
        &self,
        order: Order,
        traversal: &mut Vec<NodeId>,
        visited: &mut NodeSet,
        node: NodeId,
    ) {
        // Visit if this is a pre-order traversal
        if matches!(order, Order::PreOrder | Order::ReversePreOrder) {
            traversal.push(node);
        }
        // If this is a reverse-order traversal, visit successors in reverse
        let iter = self[node].successors.iter();
        // `iter.rev()` and `iter` have different types/sizes, so use a sum type (`Either`) to store
        // the correct iterator
        let iter = match order {
            Order::ReversePreOrder | Order::ReversePostOrder => Either::Left(iter.rev()),
            _ => Either::Right(iter),
        };
        // Note, `Either` implements `IntoIterator` if both `Left` and `Right` do.
        for &succ in iter {
            // Recurse if not yet visited this node
            if !visited.contains(succ) {
                visited.insert(succ);
                self.depth_first_inner(order, traversal, visited, succ);
            }
        }
        // Visit if this is a post-order traversal
        if matches!(order, Order::PostOrder | Order::ReversePostOrder) {
            traversal.push(node);
        }
    }

    /// Performs a depth-first traversal on this graph.
    ///
    /// Possible orderings are defined in the [`Order`] enum.
    ///
    /// # Panics
    ///
    /// Panics if the graph doesn't have an entrypoint to start the traversal at.
    pub fn depth_first(&self, order: Order) -> NodeOrder {
        // Preallocate traversal/visited as we know we'll visit each node once, assuming connected
        let len = self.len();
        let mut traversal = Vec::with_capacity(len);
        let mut visited = NodeSet::with_capacity_for(self);

        // Mark start as initially visited
        let start = self.entry.expect("traversal needs entrypoint");
        visited.insert(start);

        // Recursively traverse graph
        self.depth_first_inner(order, &mut traversal, &mut visited, start);

        NodeOrder::from_traversal(traversal)
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::tests::{fixture_1, fixture_cyclic};
    use crate::graph::Order;

    #[test]
    fn depth_first_pre_order_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let traversal = g.depth_first(Order::PreOrder).traversal;
        assert_eq!(traversal, vec![n1, n2, n3, n4, n5, n6]);
    }

    #[test]
    fn depth_first_post_order_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let traversal = g.depth_first(Order::PostOrder).traversal;
        assert_eq!(traversal, vec![n4, n3, n6, n5, n2, n1]);
    }

    #[test]
    fn depth_first_reverse_pre_order_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let traversal = g.depth_first(Order::ReversePreOrder).traversal;
        assert_eq!(traversal, vec![n1, n2, n5, n6, n3, n4]);
    }

    #[test]
    fn depth_first_reverse_post_order_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let traversal = g.depth_first(Order::ReversePostOrder).traversal;
        assert_eq!(traversal, vec![n6, n5, n4, n3, n2, n1]);
    }

    #[test]
    fn depth_first_post_order_cyclic() {
        let (g, (n1, n2)) = fixture_cyclic();
        let traversal = g.depth_first(Order::PostOrder).traversal;
        assert_eq!(traversal, vec![n2, n1]);
    }
}
