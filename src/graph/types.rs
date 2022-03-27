use std::mem::take;
use std::{fmt, ops};

#[inline]
pub fn remove_element<T: PartialEq + Copy>(vec: &mut Vec<T>, value: T) {
    let index = vec.iter().position(|&x| x == value).expect("Not found");
    vec.remove(index);
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct NodeId(usize);

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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Node<T> {
    pub id: NodeId,
    pub value: T,
    pub predecessors: Vec<NodeId>, // Incoming
    pub successors: Vec<NodeId>,   // Outgoing
}

impl<T> Node<T> {
    #[inline]
    pub fn in_degree(&self) -> usize {
        self.predecessors.len()
    }

    #[inline]
    pub fn out_degree(&self) -> usize {
        self.successors.len()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Graph<T> {
    // Nodes are deleted infrequently, so store deletions as `None` tombstones.
    // This gives us constant time lookup by NodeId.
    nodes: Vec<Option<Node<T>>>,
    pub entry: Option<NodeId>,
}

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

    fn into_iter(self) -> Self::IntoIter {
        // Filter out deleted nodes
        self.nodes.into_iter().filter_map(|x| x)
    }
}

impl<T> Graph<T> {
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            entry: None,
        }
    }

    pub fn add_node(&mut self, value: T) -> NodeId {
        let id = NodeId(self.nodes.len());
        let node = Node {
            id,
            value,
            predecessors: vec![],
            successors: vec![],
        };
        self.nodes.push(Some(node));

        // Set as entrypoint if this is the first inserted node
        self.entry.get_or_insert(id);

        id
    }

    pub fn add_edge(&mut self, source: NodeId, target: NodeId) {
        self[source].successors.push(target);
        self[target].predecessors.push(source);
    }

    pub fn remove_node(&mut self, id: NodeId) {
        // take() node leaving None as tombstone
        let node = self.nodes[id.0].take().expect("Not found");
        // Remove node as successor from all predecessors
        for pred in node.predecessors {
            if pred != id {
                remove_element(&mut self[pred].successors, id);
            }
        }
        // Remove node as predecessor from all successors
        for succ in node.successors {
            if succ != id {
                remove_element(&mut self[succ].predecessors, id);
            }
        }
    }

    pub fn remove_edge(&mut self, source: NodeId, target: NodeId) {
        // Remove target as successor of source
        remove_element(&mut self[source].successors, target);
        // Remove source as predecessor of target
        remove_element(&mut self[target].predecessors, source);
    }

    pub fn swap_edge(&mut self, source: NodeId, from_target: NodeId, to_target: NodeId) {
        // Update target in successors of source (preserving order for conditional branches)
        let successor = self[source]
            .successors
            .iter_mut()
            .find(|x| **x == from_target)
            .expect("Not found");
        *successor = to_target;
        // Remove source of predecessor of previous target...
        remove_element(&mut self[from_target].predecessors, source);
        // ...and add it as a predecessor of the new target
        self[to_target].predecessors.push(source);
    }

    pub fn remove_all_successors(&mut self, source: NodeId) {
        for succ in take(&mut self[source].successors) {
            remove_element(&mut self[succ].predecessors, source);
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Node<T>> {
        // Filter out deleted nodes
        self.nodes.iter().filter_map(Option::as_ref)
    }

    #[inline]
    pub fn iter_id(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.iter().map(|x| &x.id).copied()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

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
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use itertools::Itertools;

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
        // https://www.cs.columbia.edu/~suman/secure_sw_devel/p1-allen.pdf#page=7
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

    pub fn fixture_cyclic() -> (Graph<usize>, (NodeId, NodeId)) {
        let mut g = Graph::new();

        let n1 = g.add_node(1);
        let n2 = g.add_node(2);

        g.add_edge(n1, n1);
        g.add_edge(n1, n2);
        g.add_edge(n2, n1);

        (g, (n1, n2))
    }

    // TODO: assert_edge!() macro for graph testing, may need intrinsic function for creating/
    //  comparing NodeIds

    #[test]
    fn test_add_nodes_edges() {
        let (g, (n1, n2, n3, n4, n5, _n6)) = fixture_1();
        assert_eq!(g.len(), 6);
        assert_eq!(g[n2].value, 2);
        assert_eq!(g[n2].predecessors, vec![n1, n4]);
        assert_eq!(g[n2].successors, vec![n3, n5]);
        assert_eq!(
            g.iter().map(|x| x.value).collect_vec(),
            vec![1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn test_remove_node() {
        let (mut g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        g.remove_node(n2);
        assert_eq!(g.len(), 5);
        assert_eq!(g[n1].value, 1);
        assert_eq!(g[n1].successors, vec![]);
        assert_eq!(g[n3].value, 3);
        assert_eq!(g[n3].predecessors, vec![]);
        assert_eq!(g[n4].successors, vec![]);
        assert_eq!(g[n5].predecessors, vec![]);
        g.remove_node(n6);
        assert_eq!(g.len(), 4);
        assert_eq!(g[n5].successors, vec![n1]);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), vec![1, 3, 4, 5]);
    }

    #[test]
    fn test_remove_node_cyclic() {
        let (mut g, (n1, n2)) = fixture_cyclic();
        g.remove_node(n1);
        assert_eq!(g.len(), 1);
        assert_eq!(g[n2].value, 2);
        assert_eq!(g[n2].predecessors, vec![]);
        assert_eq!(g[n2].successors, vec![]);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), vec![2]);
    }

    #[test]
    fn test_remove_edge() {
        let (mut g, (n1, n2, n3, n4, n5, _n6)) = fixture_1();
        g.remove_edge(n4, n2);
        assert_eq!(g.len(), 6);
        assert_eq!(g[n2].predecessors, vec![n1]);
        assert_eq!(g[n2].successors, vec![n3, n5]);
        assert_eq!(g[n4].predecessors, vec![n3]);
        assert_eq!(g[n4].successors, vec![]);
        assert_eq!(
            g.iter().map(|x| x.value).collect_vec(),
            vec![1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn test_remove_edge_cyclic() {
        let (mut g, (n1, n2)) = fixture_cyclic();
        assert_eq!(g[n1].predecessors, vec![n1, n2]);
        assert_eq!(g[n1].successors, vec![n1, n2]);
        g.remove_edge(n1, n1);
        assert_eq!(g.len(), 2);
        assert_eq!(g[n1].predecessors, vec![n2]);
        assert_eq!(g[n1].successors, vec![n2]);
        assert_eq!(g[n2].predecessors, vec![n1]);
        assert_eq!(g[n2].successors, vec![n1]);
        g.remove_edge(n1, n2);
        assert_eq!(g[n1].predecessors, vec![n2]);
        assert_eq!(g[n1].successors, vec![]);
        assert_eq!(g[n2].predecessors, vec![]);
        assert_eq!(g[n2].successors, vec![n1]);
        assert_eq!(g.iter().map(|x| x.value).collect_vec(), vec![1, 2]);
    }

    #[test]
    fn test_remove_all_successors() {
        let (mut g, (n1, n2, n3, n4, n5, n6, n7, n8)) = fixture_2();
        g.remove_all_successors(n4);
        assert_eq!(g[n4].predecessors, vec![n3]);
        assert_eq!(g[n4].successors, vec![]);
    }
}
