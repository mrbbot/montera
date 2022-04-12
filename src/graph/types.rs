use std::mem::take;
use std::{fmt, ops};

/// Removes the first instance of `value` in `vec`.
///
/// # Panics
///
/// Panics if `value` does not exist in `vec`.
#[inline]
pub fn remove_element<T: PartialEq>(vec: &mut Vec<T>, value: &T) {
    let index = vec.iter().position(|x| x == value).expect("Not found");
    vec.remove(index);
}

/// Opaque identifier for [`Node`]s.
///
/// This makes it explicit where a node is expected and allows us to change the internal
/// representation in the future.
///
/// The internal `usize` has `pub(super)` visibility so it can be used by [`NodeSet`] and
/// [`NodeMap`] for efficient set and map data structures where `NodeId`s are the key.
///
/// [`NodeSet`]: super::collections::NodeSet
/// [`NodeMap`]: super::collections::NodeMap
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct NodeId(pub(super) usize);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Node in a [`Graph`], connected to other [`Node`]s.
///
/// Contains a `value` and the [`NodeId`]s of predecessors (incoming edges) and successors (outgoing
/// edges). For control flow graphs, each node will have up to 2 successors (unconditional or
/// conditional branch) so most graphs will be sparse. Therefore, storing directed edges twice has
/// little memory cost and simplifies the implementation of structuring algorithms.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Node<T> {
    pub id: NodeId,
    pub value: T,
    pub predecessors: Vec<NodeId>, // Incoming
    pub successors: Vec<NodeId>,   // Outgoing
}

impl<T> Node<T> {
    /// Returns the number of predecessors **in** to this node.
    #[inline]
    pub fn in_degree(&self) -> usize {
        self.predecessors.len()
    }

    /// Returns the number of successors **out** of this node.
    #[inline]
    pub fn out_degree(&self) -> usize {
        self.successors.len()
    }
}

/// Directed single-entry cyclic graph, used for control flow graphs and trees
///
/// The requirements for this data structure are:
/// - Must permit cycles (for loop back edges)
/// - Must be mutable (add/remove nodes & edges), although node removal is rare
/// - Must be able to read/write node by ID in constant time
/// - Must be able to iterate all nodes in linear time
/// - Graphs will be sparse
///
/// An arena-style structure is used, where [`Node`]s are stored once in a [`Vec`]. [`NodeId`]s are
/// simply indices into this [`Vec`]. Each [`Node`] stores the [`NodeId`]s of successors and
/// predecessors. When removing a [`Node`] from the graph, the value at that index is replaced with
/// [`Option::None`] tombstone. This ensures [`NodeId`]s remain direct indices permitting constant
/// time access. Removals are infrequent, so this doesn't waste much memory.
///
/// `entry` is automatically assigned to the [`NodeId`] of the first inserted [`Node`].
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Graph<T> {
    nodes: Vec<Option<Node<T>>>,
    pub entry: Option<NodeId>,
}

impl<T> Graph<T> {
    /// Constructs a new, empty `Graph<T>`.
    ///
    /// The graph will not allocate until nodes are added to it.
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            entry: None,
        }
    }

    /// Adds a node to the graph, returning its ID.
    ///
    /// The node will not be connected to any other nodes. If this is the first added node, and
    /// `entry` is [`Option::None`], it will become the `entry`point.
    pub fn add_node(&mut self, value: T) -> NodeId {
        // The node's ID will be the index into `nodes` once it's inserted
        let id = NodeId(self.nodes.len());
        let node = Node {
            id,
            value,
            // Initially unconnected
            predecessors: vec![],
            successors: vec![],
        };
        self.nodes.push(Some(node));

        // Set as entrypoint if this is the first inserted node
        self.entry.get_or_insert(id);

        id
    }

    /// Adds a directed edge between two nodes in the graph.
    ///
    /// # Panics
    ///
    /// Panics if either the `source` or `target` nodes do not exist in the graph.
    pub fn add_edge(&mut self, source: NodeId, target: NodeId) {
        self[source].successors.push(target);
        self[target].predecessors.push(source);
    }

    /// Removes a node and all its edges from the graph.
    ///
    /// If the node is the `entry`point, the `entry`point is reset to [`Option::None`].
    ///
    /// # Panics
    ///
    /// Panics if the node does not exist in the graph.
    pub fn remove_node(&mut self, id: NodeId) {
        // `take()` node leaving `None` as tombstone
        let node = self.nodes[id.0].take().expect("Not found");
        // Remove node as successor from all predecessors
        for pred in node.predecessors {
            // Only remove edge if it's not to the node we're removing
            if pred != id {
                remove_element(&mut self[pred].successors, &id);
            }
        }
        // Remove node as predecessor from all successors
        for succ in node.successors {
            // Only remove edge if it's not to the node we're removing
            if succ != id {
                remove_element(&mut self[succ].predecessors, &id);
            }
        }
        // Reset entrypoint if removed
        if self.entry == Some(id) {
            self.entry = None;
        }
    }

    /// Removes a directed edge between two nodes in the graph.
    ///
    /// # Panics
    ///
    /// - Panics if either the `source` or the `target` nodes do not exist in the graph
    /// - Panics if there is no edge between the `source` and `target` in the graph
    pub fn remove_edge(&mut self, source: NodeId, target: NodeId) {
        // Remove target as successor of source
        remove_element(&mut self[source].successors, &target);
        // Remove source as predecessor of target
        remove_element(&mut self[target].predecessors, &source);
    }

    /// Replaces the directed edge from `source` -> `from_target` to `source` -> `to_target`.
    ///
    /// Importantly, the new edge will have the same index in the `source`'s `successors` list.
    /// This preserves edge order for conditional branches using edge 0/1 for false/true.
    ///
    /// # Panics
    ///
    /// - Panics if either the `source`, `from_target` or `to_target` nodes don't exist in the graph
    /// - Panics if there is no edge between the `source` and `from_target` in the graph
    pub fn swap_edge(&mut self, source: NodeId, from_target: NodeId, to_target: NodeId) {
        // Update target in successors of source (preserving order for conditional branches)
        let successor = self[source]
            .successors
            .iter_mut()
            .find(|x| **x == from_target)
            .expect("Not found");
        *successor = to_target;
        // Remove source of predecessor of previous target...
        remove_element(&mut self[from_target].predecessors, &source);
        // ...and add it as a predecessor of the new target
        self[to_target].predecessors.push(source);
    }

    /// Removes all outgoing edges from `source`.
    ///
    /// # Panics
    ///
    /// Panics if `source` node does not exist in the graph.
    pub fn remove_all_successors(&mut self, source: NodeId) {
        for succ in take(&mut self[source].successors) {
            remove_element(&mut self[succ].predecessors, &source);
        }
    }

    /// Returns an iterator over `Node`s in the graph.
    ///
    /// Nodes are iterated in insertion order and deleted nodes are not yielded.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Node<T>> {
        // Filter out deleted nodes
        self.nodes.iter().filter_map(Option::as_ref)
    }

    /// Returns an iterator over `NodeId`s of `Node`s in the graph.
    ///
    /// Nodes are iterated in insertion order and deleted nodes are not yielded.
    #[inline]
    pub fn iter_id(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.iter().map(|x| x.id)
    }

    /// Returns the number of nodes in the graph, *excluding* deleted nodes.
    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns the number of nodes inserted in the graph, *including* deleted nodes.
    ///
    /// This is equal to the number of times [`add_node`] has been called.
    ///
    /// [`add_node`]: Graph::add_node
    #[inline]
    pub(super) fn capacity(&self) -> usize {
        self.nodes.len()
    }

    /// Creates a new graph with the same topology, applying a closure to each [`Node`]'s `value`.
    ///
    /// Note node IDs, edges and their order will be the same in the new graph.
    pub fn map<U, F: Fn(NodeId, &T) -> U>(&self, f: F) -> Graph<U> {
        let nodes = self
            .nodes
            .iter()
            .map(|maybe_node| {
                maybe_node.as_ref().map(|node| Node {
                    id: node.id,
                    value: f(node.id, &node.value),
                    predecessors: node.predecessors.clone(),
                    successors: node.successors.clone(),
                })
            })
            .collect();
        Graph {
            nodes,
            entry: self.entry,
        }
    }

    /// Creates a new graph, with all edges flipped, the entrypoint as the exit point of this graph,
    /// and applying a closure to each [`Node`]'s `value`.
    ///
    /// Note node IDs and edge order will be the same in the new graph. Only the direction of edges
    /// will be swapped.
    ///
    /// # Panics
    ///
    /// If this graph does not have exactly one "exit" node (with out degree 0) for the new graphs
    /// entrypoint.
    pub fn map_reversed<U, F: Fn(NodeId, &T) -> U>(&self, f: F) -> Graph<U> {
        // Find single exit node in graph, this will become the new entry
        let mut exits = self.iter().filter(|node| node.out_degree() == 0);
        let exit = exits.next().expect("reverse expects an exit node");
        assert!(
            exits.next().is_none(),
            "reverse expects exactly one exit node"
        );

        let nodes = self
            .nodes
            .iter()
            .map(|maybe_node| {
                maybe_node.as_ref().map(|node| Node {
                    id: node.id,
                    value: f(node.id, &node.value),
                    // Swap direction of edges
                    predecessors: node.successors.clone(),
                    successors: node.predecessors.clone(),
                })
            })
            .collect();
        Graph {
            nodes,
            entry: Some(exit.id),
        }
    }
}

// Permit array-style subscripting of `Graph`s by `NodeId`s (e.g. `graph[n]`)
impl<T> ops::Index<NodeId> for Graph<T> {
    type Output = Node<T>;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.nodes[index.0].as_ref().expect("Not found")
    }
}

impl<T> ops::IndexMut<NodeId> for Graph<T> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        self.nodes[index.0].as_mut().expect("Not found")
    }
}

impl<T> IntoIterator for Graph<T> {
    type Item = Node<T>;
    type IntoIter = std::iter::FilterMap<
        std::vec::IntoIter<Option<Node<T>>>,
        fn(Option<Node<T>>) -> Option<Node<T>>,
    >;

    /// Returns an iterator over `Node`s in the graph.
    ///
    /// Nodes are iterated in insertion order and deleted nodes are not yielded.
    fn into_iter(self) -> Self::IntoIter {
        // Filter out deleted nodes
        self.nodes.into_iter().filter_map(|x| x)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::graph::Order;
    use itertools::Itertools;

    /// Constructs a graph based on Figure 6.9 (Page 133) from "Cristina Cifuentes. Reverse
    /// Compilation Techniques. PhD thesis, Queensland University of Technology, 1994".
    ///
    /// ```text
    ///     ↓
    /// ┌──→1
    /// │   ↓
    /// │ ┌→2─┐
    /// │ │ ↓ │
    /// │ │ 3 │
    /// │ │ ↓ │
    /// │ └─4 │
    /// │     │
    /// └───5←┘
    ///     ↓
    ///     6
    /// ```
    pub fn fixture_1() -> (
        Graph<usize>,
        (NodeId, NodeId, NodeId, NodeId, NodeId, NodeId),
    ) {
        let mut g = Graph::new();

        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);
        let n4 = g.add_node(4);
        let n5 = g.add_node(5);
        let n6 = g.add_node(6);

        g.add_edge(n1, n2);
        g.add_edge(n2, n3);
        g.add_edge(n3, n4);
        g.add_edge(n4, n2);
        g.add_edge(n2, n5);
        g.add_edge(n5, n6);
        g.add_edge(n5, n1);

        (g, (n1, n2, n3, n4, n5, n6))
    }

    /// Constructs a graph based on Figure 2 (Page 7) from "Frances E. Allen. 1970. Control flow
    /// analysis. SIGPLAN Not. 5, 7 (July 1970), 1–19".
    ///
    /// ```text
    ///     ↓
    ///     1
    ///     ↓
    /// ┌──→2───┐
    /// │   ↓   │
    /// │ ┌→3   │
    /// │ │↙ ↘  │
    /// │ 4   5 │
    /// │  ↘ ↙  │
    /// │   6   │
    /// │   ↓   │
    /// └───7←──┘
    ///     ↓
    ///     8
    /// ```
    pub fn fixture_2() -> (
        Graph<usize>,
        (
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
            NodeId,
        ),
    ) {
        let mut g = Graph::new();

        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);
        let n4 = g.add_node(4);
        let n5 = g.add_node(5);
        let n6 = g.add_node(6);
        let n7 = g.add_node(7);
        let n8 = g.add_node(8);

        g.add_edge(n1, n2);
        g.add_edge(n2, n3);
        g.add_edge(n3, n4);
        g.add_edge(n4, n3);
        g.add_edge(n3, n5);
        g.add_edge(n4, n6);
        g.add_edge(n5, n6);
        g.add_edge(n6, n7);
        g.add_edge(n2, n7);
        g.add_edge(n7, n2);
        g.add_edge(n7, n8);

        (g, (n1, n2, n3, n4, n5, n6, n7, n8))
    }

    /// Constructs a simple graph consisting of 3 nodes joined together in a line.
    ///
    /// ```text
    /// →1→2→3
    /// ```
    pub fn fixture_3() -> (Graph<usize>, (NodeId, NodeId, NodeId)) {
        let mut g = Graph::new();

        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);

        g.add_edge(n1, n2);
        g.add_edge(n2, n3);

        (g, (n1, n2, n3))
    }

    /// Constructs a graph containing cycles and an edge with the same source/target node.
    ///
    /// ```text
    /// ┌─↘┌─↘
    /// │ →1  2
    /// └──┘↖─┘
    /// ```
    pub fn fixture_cyclic() -> (Graph<usize>, (NodeId, NodeId)) {
        let mut g = Graph::new();

        let n1 = g.add_node(1);
        let n2 = g.add_node(2);

        g.add_edge(n1, n1);
        g.add_edge(n1, n2);
        g.add_edge(n2, n1);

        (g, (n1, n2))
    }

    #[test]
    fn remove_elements() {
        let mut v = vec![1, 2, 3, 2];
        // Check only removes first instance
        remove_element(&mut v, &2);
        assert_eq!(v, [1, 3, 2]);
        // Check removes first element
        remove_element(&mut v, &1);
        assert_eq!(v, [3, 2]);
        // Check removes last element
        remove_element(&mut v, &2);
        assert_eq!(v, [3]);
        // Check removes all elements
        remove_element(&mut v, &3);
        assert_eq!(v, []);
    }

    #[test]
    #[should_panic = "Not found"]
    fn invalid_remove_elements() {
        // Check `remove_element` panics if value not found in vec
        let mut v = vec![1, 2, 3];
        remove_element(&mut v, &4);
    }

    #[test]
    fn node_id_format() {
        let n = NodeId(3);
        assert_eq!(format!("{n}"), "3");
        assert_eq!(format!("{n:?}"), "#3");
    }

    #[test]
    fn node_degrees() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        g.add_edge(n1, n2);
        assert_eq!(g[n1].in_degree(), 0);
        assert_eq!(g[n2].in_degree(), 1);
        assert_eq!(g[n1].out_degree(), 1);
        assert_eq!(g[n2].out_degree(), 0);
    }

    #[test]
    fn add_node() {
        // Create empty graph and check no allocations
        let mut g = Graph::new();
        assert_eq!(g.nodes.capacity(), 0);

        // Check empty node created
        let n1 = g.add_node(1);
        assert_eq!(g.nodes.len(), 1);
        assert_eq!(g[n1].id, n1);
        assert_eq!(g[n1].value, 1);
        assert_eq!(g[n1].predecessors, []);
        assert_eq!(g[n1].successors, []);
        // Check entrypoint assigned to new node
        assert_eq!(g.entry, Some(n1));

        // Check entrypoint unchanged if this isn't the first `add_node` call
        g.add_node(2);
        assert_eq!(g.nodes.len(), 2);
        assert_eq!(g.entry, Some(n1));
    }

    #[test]
    fn add_edge() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        // Check directed edge added and both nodes updated appropriately
        g.add_edge(n1, n2);
        assert_eq!(g[n1].predecessors, []);
        assert_eq!(g[n1].successors, [n2]);
        assert_eq!(g[n2].predecessors, [n1]);
        assert_eq!(g[n2].successors, []);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 1 but the index is 2"]
    fn invalid_add_edge_source() {
        // Check `add_edge` panics if source node does not exist in graph
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        // Try to use source node created outside of `add_node` call
        g.add_edge(NodeId(2), n1);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 1 but the index is 2"]
    fn invalid_add_edge_target() {
        // Check `add_edge` panics if target node does not exist in graph
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        // Try to use target node created outside of `add_node` call
        g.add_edge(n1, NodeId(2));
    }

    #[test]
    fn remove_node() {
        let (mut g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        g.remove_node(n2);
        assert_eq!(g.len(), 5);
        assert_eq!(g[n1].value, 1);
        assert_eq!(g[n1].successors, []);
        assert_eq!(g[n3].value, 3);
        assert_eq!(g[n3].predecessors, []);
        assert_eq!(g[n4].successors, []);
        assert_eq!(g[n5].predecessors, []);
        g.remove_node(n6);
        assert_eq!(g.len(), 4);
        assert_eq!(g[n5].successors, [n1]);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), [1, 3, 4, 5]);
    }

    #[test]
    fn remove_node_cyclic() {
        let (mut g, (n1, n2)) = fixture_cyclic();
        g.remove_node(n1);
        assert_eq!(g.len(), 1);
        assert_eq!(g[n2].value, 2);
        assert_eq!(g[n2].predecessors, []);
        assert_eq!(g[n2].successors, []);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), [2]);
    }

    #[test]
    fn remove_entry() {
        // Check removing entrypoint resets entrypoint
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        assert_eq!(g.entry, Some(n1));
        g.remove_node(n1);
        assert_eq!(g.entry, None);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 0 but the index is 0"]
    fn invalid_remove_node() {
        // Check `remove_node` panics if node does not exist in graph
        let mut g = Graph::<()>::new();
        g.remove_node(NodeId(0));
    }

    #[test]
    fn remove_edge() {
        let (mut g, (n1, n2, n3, n4, n5, _n6)) = fixture_1();
        g.remove_edge(n4, n2);
        assert_eq!(g.len(), 6);
        assert_eq!(g[n2].predecessors, [n1]);
        assert_eq!(g[n2].successors, [n3, n5]);
        assert_eq!(g[n4].predecessors, [n3]);
        assert_eq!(g[n4].successors, []);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn remove_edge_cyclic() {
        let (mut g, (n1, n2)) = fixture_cyclic();
        assert_eq!(g[n1].predecessors, [n1, n2]);
        assert_eq!(g[n1].successors, [n1, n2]);
        g.remove_edge(n1, n1);
        assert_eq!(g.len(), 2);
        assert_eq!(g[n1].predecessors, [n2]);
        assert_eq!(g[n1].successors, [n2]);
        assert_eq!(g[n2].predecessors, [n1]);
        assert_eq!(g[n2].successors, [n1]);
        g.remove_edge(n1, n2);
        assert_eq!(g[n1].predecessors, [n2]);
        assert_eq!(g[n1].successors, []);
        assert_eq!(g[n2].predecessors, []);
        assert_eq!(g[n2].successors, [n1]);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), [1, 2]);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 3 but the index is 4"]
    fn invalid_remove_edge_source() {
        // Check `remove_edge` panics if source node does not exist in graph
        let (mut g, (n1, _n2, _n3)) = fixture_3();
        g.remove_edge(NodeId(4), n1);
    }

    #[test]
    #[should_panic = "Not found"]
    fn invalid_remove_edge_target() {
        // Check `remove_edge` panics if target node does not exist in graph
        let (mut g, (n1, _n2, _n3)) = fixture_3();
        g.remove_edge(n1, NodeId(4));
    }

    #[test]
    #[should_panic = "Not found"]
    fn invalid_remove_edge() {
        // Check `remove_edge` panics if edge from source to target does not exist in graph
        let (mut g, (n1, _n2, n3)) = fixture_3();
        g.remove_edge(n1, n3);
    }

    #[test]
    fn swap_edge() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);
        let n4 = g.add_node(4);
        g.add_edge(n1, n2);
        g.add_edge(n1, n3);
        // Check edge order preserved
        g.swap_edge(n1, n2, n4);
        assert_eq!(g[n1].successors, [n4, n3]);
        // Check other edge invariants preserved
        assert_eq!(g[n2].predecessors, []);
        assert_eq!(g[n3].predecessors, [n1]);
        assert_eq!(g[n4].predecessors, [n1]);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 3 but the index is 4"]
    fn invalid_swap_edge_source() {
        // Check `swap_edge` panics if `source` node does not exist in graph
        let (mut g, (_n1, n2, n3)) = fixture_3();
        g.swap_edge(NodeId(4), n2, n3);
    }

    #[test]
    #[should_panic = "Not found"]
    fn invalid_swap_edge_from_target() {
        // Check `swap_edge` panics if `from_target` node does not exist in graph
        let (mut g, (n1, _n2, n3)) = fixture_3();
        g.swap_edge(n1, NodeId(4), n3);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 3 but the index is 4"]
    fn invalid_swap_edge_to_target() {
        // Check `swap_edge` panics if `to_target` node does not exist in graph
        let (mut g, (n1, n2, _n3)) = fixture_3();
        g.swap_edge(n1, n2, NodeId(4));
    }

    #[test]
    #[should_panic = "Not found"]
    fn invalid_swap_edge() {
        // Check `swap_edge` panics if edge from `source` to `from_target` does not exist in graph
        let (mut g, (n1, _n2, n3)) = fixture_3();
        g.swap_edge(n1, n3, n1);
    }

    #[test]
    fn remove_all_successors() {
        let (mut g, (_n1, _n2, n3, n4, _n5, _n6, _n7, _n8)) = fixture_2();
        g.remove_all_successors(n4);
        assert_eq!(g[n4].predecessors, [n3]);
        assert_eq!(g[n4].successors, []);
    }

    #[test]
    fn remove_all_successors_cyclic() {
        let (mut g, (n1, n2)) = fixture_cyclic();
        g.remove_all_successors(n1);
        assert_eq!(g[n1].predecessors, [n2]);
        assert_eq!(g[n1].successors, []);
        assert_eq!(g[n2].predecessors, []);
        assert_eq!(g[n2].successors, [n1]);
    }

    #[test]
    #[should_panic = "index out of bounds: the len is 0 but the index is 0"]
    fn invalid_remove_all_successors() {
        // Check `remove_all_successors` panics if `source` node does not exist in graph
        let mut g = Graph::<()>::new();
        g.remove_all_successors(NodeId(0));
    }

    #[test]
    fn iter() {
        let (mut g, (n1, n2, n3)) = fixture_3();
        g.remove_node(n2);
        // Check all nodes excluding deleted yielded
        assert_eq!(g.iter().map(|n| n.value).collect_vec(), [1, 3]);
        assert_eq!(g.iter_id().collect_vec(), [n1, n3]);
    }

    #[test]
    fn len_capacity() {
        let (mut g, (_n1, n2, _n3)) = fixture_3();
        g.remove_node(n2);
        // Check len() excludes deleted nodes, whereas capacity() includes them
        assert_eq!(g.len(), 2);
        assert_eq!(g.capacity(), 3);
    }

    #[test]
    fn map() {
        let (g, (n1, n2)) = fixture_cyclic();
        // Double each node's value (and include NodeId for assertions)
        let m = g.map(|id, value| (id, value * 2));
        // Check closure applied to each value correctly
        assert_eq!(m[n1].value, (n1, 2));
        assert_eq!(m[n2].value, (n2, 4));
        // Check edges and their orders preserved
        assert_eq!(m[n1].predecessors, g[n1].predecessors);
        assert_eq!(m[n1].successors, g[n1].successors);
        assert_eq!(m[n2].predecessors, g[n2].predecessors);
        assert_eq!(m[n2].successors, g[n2].successors);
    }

    #[test]
    fn map_reversed() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        // Double each node's value (and include NodeId for assertions)
        let m = g.map_reversed(|id, value| (id, value * 2));
        // Check closure applied to values correctly
        assert_eq!(m[n1].value, (n1, 2));
        assert_eq!(m[n3].value, (n3, 6));
        assert_eq!(m[n6].value, (n6, 12));
        // Check entrypoint of new graph is exit of previous
        assert_eq!(m.entry, Some(n6));
        // Check edges are reversed
        assert_eq!(m[n6].predecessors, []);
        assert_eq!(m[n6].successors, [n5]);
        assert_eq!(m[n5].predecessors, [n6, n1]);
        assert_eq!(m[n5].successors, [n2]);
        assert_eq!(m[n2].predecessors, [n3, n5]);
        assert_eq!(m[n2].successors, [n1, n4]);
        assert_eq!(m[n1].predecessors, [n2]);
        assert_eq!(m[n1].successors, [n5]);
        // Check again with depth-first pre-order traversal
        assert_eq!(
            m.depth_first(Order::PreOrder).traversal,
            [n6, n5, n2, n1, n4, n3]
        );
    }

    #[test]
    #[should_panic = "reverse expects an exit node"]
    fn invalid_map_reversed_no_exit() {
        // Check `map_reversed` panics if no possible exit nodes for new entrypoint
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        g.add_edge(n1, n2);
        g.add_edge(n2, n1);
        // n1 and n2 both have out-degree == 1, so neither is a suitable exit
        g.map_reversed(|_, _| ());
    }

    #[test]
    #[should_panic = "reverse expects exactly one exit node"]
    fn invalid_map_reversed_multiple_exit() {
        // Check `map_reversed` panics if multiple possible exit nodes for new entrypoint
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);
        g.add_edge(n1, n2);
        g.add_edge(n1, n3);
        // n3 and n3 both have out-degree == 0, so both are suitable exits
        g.map_reversed(|_, _| ());
    }
}
