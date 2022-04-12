//! Collections exploiting NodeId's integer representation and the (mostly) fixed size of graphs

use crate::graph::{Graph, NodeId};
use bit_set::BitSet;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::FromIterator;
use std::{iter, ops, vec};

/// Set data structure for [`NodeId`]s.
///
/// Internally, uses an efficient bit set representation, with each [`NodeId`] corresponding to a
/// single bit. Most of our use cases for a set (e.g. storing visited nodes) eventually fill the
/// set with all nodes in the graph, so pre-allocating space for all nodes is desirable.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NodeSet {
    inner: BitSet,
}

impl NodeSet {
    /// Constructs a new, empty `NodeSet`.
    ///
    /// The set will not allocate until nodes are added to it.
    pub fn new() -> Self {
        let inner = BitSet::new();
        Self { inner }
    }

    /// Constructs a new `NodeSet` able to hold all [`Graph`]'s [`NodeId`]s without reallocating.
    ///
    /// Note this will allocate space for deleted nodes too, but these are infrequent.
    pub fn with_capacity_for<G>(g: &Graph<G>) -> Self {
        let inner = BitSet::with_capacity(g.capacity());
        Self { inner }
    }

    /// Adds a node to the set.
    ///
    /// Returns `true` if and only if the set did not previously contain this node.
    pub fn insert(&mut self, item: NodeId) -> bool {
        self.inner.insert(item.0)
    }

    /// Removes a node from the set.
    ///
    /// Returns `true` if and only if the set contained this node.
    pub fn remove(&mut self, item: NodeId) -> bool {
        self.inner.remove(item.0)
    }

    /// Empties the set.
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// Returns `true` if and only if the set contains the node.
    pub fn contains(&self, item: NodeId) -> bool {
        self.inner.contains(item.0)
    }

    /// Returns an iterator over [`NodeId`]s contained within the set.
    pub fn iter(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.inner.iter().map(|item| NodeId(item))
    }
}

// Permit `collect()`ing to a `NodeSet` from an `Iterator<NodeId>`
impl FromIterator<NodeId> for NodeSet {
    fn from_iter<T: IntoIterator<Item = NodeId>>(iter: T) -> Self {
        let iter = iter.into_iter();
        // If we have an upper bound on the iterator size, use that as the initial capacity
        let (_, upper) = iter.size_hint();
        let mut inner = match upper {
            Some(upper) => BitSet::with_capacity(upper),
            None => BitSet::new(),
        };
        // We don't just use `extend()` as `BitSet::extend()`'s implementation doesn't use
        // `size_hint()` itself
        inner.extend(iter.map(|item| item.0));
        Self { inner }
    }
}

/// Map data structure where keys are [`NodeId`]s.
///
/// Internally, uses a vector indexed by [`NodeId`] integers. [`Option::None`] is used to indicate
/// a vacant slot. This provides constant time lookup, and is most efficient when the map contains
/// a value for every [`Node`] in a [`Graph`].
///
/// [`Node`]: super::types::Node
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NodeMap<T> {
    inner: Vec<Option<T>>,
}

impl<T> NodeMap<T> {
    /// Constructs a new, empty `NodeMap`.
    ///
    /// The map will not allocate until node-value pairs are added to it.
    pub fn new() -> Self {
        Self { inner: vec![] }
    }

    /// Constructs a new `NodeSet` able to hold values for all [`Graph`]'s [`NodeId`]s without
    /// reallocating.
    ///
    /// Note this will allocate space for deleted nodes too, but these are infrequent.
    pub fn with_capacity_for<G>(g: &Graph<G>) -> Self {
        // Cannot use `vec![None; g.nodes_len()]` here as T might not satisfy Clone trait
        let inner = (0..g.capacity()).map(|_| None).collect();
        Self { inner }
    }

    /// Adds a node-value pair to the map.
    ///
    /// Returns the previous value (if any) associated with the node.
    pub fn insert(&mut self, key: NodeId, value: T) -> Option<T> {
        if key.0 >= self.inner.len() {
            self.inner.resize_with(key.0 + 1, || None)
        }
        self.inner[key.0].replace(value)
    }

    /// Removes a node-value pair from the map.
    ///
    /// Returns the current value (if any) associated with the node.
    pub fn remove(&mut self, key: NodeId) -> Option<T> {
        self.inner.get_mut(key.0).and_then(|value| value.take())
    }

    /// Empties the map.
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// Returns the current value (if any) associated with the node.
    pub fn get(&self, key: NodeId) -> Option<&T> {
        self.inner.get(key.0).and_then(|value| value.as_ref())
    }

    /// Returns `true` if and only if the map contains a value for the node.
    pub fn contains_key(&self, key: NodeId) -> bool {
        self.inner.get(key.0).map_or(false, |value| value.is_some())
    }

    /// Returns an iterator over node-value pairs within the map.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.inner
            .iter()
            .enumerate()
            .filter_map(|(key, value)| match value {
                Some(value) => Some((NodeId(key), value)),
                None => None, // Exclude pairs without a value
            })
    }

    /// Returns an iterator over [`NodeId`]s with values in the map.
    pub fn keys(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.iter().map(|(key, _)| key)
    }

    /// Returns an iterator over just values in the map.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.iter().map(|(_, value)| value)
    }
}

// Permit array-style subscripting of `NodeMap`s by `NodeId`s (e.g. `map[n]`)
impl<T> ops::Index<NodeId> for NodeMap<T> {
    type Output = T;

    fn index(&self, index: NodeId) -> &Self::Output {
        self.get(index).expect("Not found")
    }
}

impl<T> iter::IntoIterator for NodeMap<T> {
    type Item = (NodeId, T);
    type IntoIter = iter::FilterMap<
        iter::Enumerate<vec::IntoIter<Option<T>>>,
        fn((usize, Option<T>)) -> Option<(NodeId, T)>,
    >;

    /// Returns an iterator over node-value pairs within the map.
    fn into_iter(self) -> Self::IntoIter {
        self.inner
            .into_iter()
            .enumerate()
            .filter_map(|(key, value)| match value {
                Some(value) => Some((NodeId(key), value)),
                None => None,
            })
    }
}

// Permit `collect()`ing to a `NodeMap<T>` from an `Iterator<(NodeId, T)>`
impl<T> iter::FromIterator<(NodeId, T)> for NodeMap<T> {
    fn from_iter<I: IntoIterator<Item = (NodeId, T)>>(iter: I) -> Self {
        let items = iter.into_iter().collect::<Vec<_>>();
        // max may not necessarily be the maximum possible NodeId for a graph,
        // but NodeMap can grow dynamically
        let max_key = items.iter().map(|(key, _)| key.0).max();
        let mut inner = match max_key {
            Some(max_key) => (0..max_key + 1).map(|_| None).collect(),
            None => vec![],
        };
        for (key, value) in items {
            inner[key.0] = Some(value);
        }
        Self { inner }
    }
}

// Permit conversion from a `HashMap<NodeId, T>` to a `NodeMap<T>` for easy construction using
// `maplit`'s `hashmap!` macro
impl<T> From<HashMap<NodeId, T>> for NodeMap<T> {
    fn from(map: HashMap<NodeId, T>) -> Self {
        map.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::{Graph, NodeMap, NodeSet};
    use itertools::Itertools;

    #[test]
    fn set() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);

        // Create new set and check it's initially empty
        let mut set = NodeSet::with_capacity_for(&g);
        assert!(!set.contains(n1));
        assert!(!set.contains(n2));

        // Insert node 2, checking insert returns true iff item not present
        assert!(set.insert(n2));
        assert!(!set.insert(n2));

        // Check set only contains node 2
        assert!(!set.contains(n1));
        assert!(set.contains(n2));

        // Remove node 2 and insert node 1, checking remove returns true iff item present
        assert!(set.remove(n2));
        assert!(!set.remove(n2));
        assert!(set.insert(n1));

        // Check set only contains node 1
        assert!(set.contains(n1));
        assert!(!set.contains(n2));

        // Check dynamically resizes if node added
        let n3 = g.add_node(3);
        assert!(!set.contains(n3));
        assert!(!set.remove(n3));
        assert!(set.insert(n3));
        assert!(set.contains(n3));

        // Empty set
        set.clear();
        assert!(!set.contains(n1));
        assert!(!set.contains(n2));
        assert!(!set.contains(n3));
    }

    #[test]
    fn set_iter() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);

        let mut set = NodeSet::with_capacity_for(&g);
        set.insert(n1);
        set.insert(n3);

        // Check iterator
        assert_eq!(set.iter().collect_vec(), [n1, n3]);

        // Check construction from iterator
        set = vec![n1, n3].into_iter().collect();
        assert_eq!(set.inner.len(), 2);
        assert!(set.contains(n1));
        assert!(!set.contains(n2));
        assert!(set.contains(n3));
    }

    #[test]
    fn map() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);

        // Create new map and check it's initially empty
        let mut map = NodeMap::with_capacity_for(&g);
        assert!(!map.contains_key(n1));
        assert!(!map.contains_key(n2));

        // Insert value for node 2
        assert_eq!(map.insert(n2, 1), None);
        assert!(!map.contains_key(n1));
        assert!(map.contains_key(n2));
        assert_eq!(map.get(n1), None);
        assert_eq!(map.get(n2).copied(), Some(1));

        // Remove node 2 and insert node 1 twice
        assert_eq!(map.remove(n2), Some(1));
        assert_eq!(map.remove(n2), None);
        assert_eq!(map.insert(n1, 2), None);
        // Insert should return previous value if defined
        assert_eq!(map.insert(n1, 3), Some(2));

        // Check final map state
        assert!(map.contains_key(n1));
        assert!(!map.contains_key(n2));
        assert_eq!(map.get(n1).copied(), Some(3));
        assert_eq!(map.get(n2), None);

        // Check dynamically resizes if node added
        let n3 = g.add_node(3);
        assert!(!map.contains_key(n3));
        assert_eq!(map.get(n3), None);
        assert_eq!(map.remove(n3), None);
        assert_eq!(map.inner.len(), 2);
        assert_eq!(map.insert(n3, 30), None);
        assert_eq!(map.inner.len(), 3);
        assert_eq!(map.get(n3).copied(), Some(30));

        // Empty map
        map.clear();
        assert!(!map.contains_key(n1));
        assert!(!map.contains_key(n2));
        assert!(!map.contains_key(n3));
    }

    #[test]
    fn map_iter() {
        let mut g = Graph::new();
        let n1 = g.add_node(1);
        let n2 = g.add_node(2);
        let n3 = g.add_node(3);

        let mut map = NodeMap::with_capacity_for(&g);
        map.insert(n1, 10);
        map.insert(n3, 30);

        // Check all iterators
        assert_eq!(map.iter().collect_vec(), [(n1, &10), (n3, &30)]);
        assert_eq!(map.keys().collect_vec(), [n1, n3]);
        assert_eq!(map.values().collect_vec(), [&10, &30]);
        assert_eq!(map.into_iter().collect_vec(), [(n1, 10), (n3, 30)]);

        // Check construction from iterator
        map = vec![(n1, 15), (n3, 35)].into_iter().collect();
        assert_eq!(map.inner.len(), 3);
        assert_eq!(map.get(n1).copied(), Some(15));
        assert_eq!(map.get(n2).copied(), None);
        assert_eq!(map.get(n3).copied(), Some(35));
    }
}
