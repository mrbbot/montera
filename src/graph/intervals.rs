use crate::graph::{Graph, NodeId, NodeMap};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

/// Partition of a [`Graph`] representing an interval, with a [`header`](Interval::header).
///
/// We define this as a wrapping struct for clarity and to define a `header` accessor.
///
/// See [`Graph::intervals`] for a definition of intervals.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Interval(Vec<NodeId>);

// Permit an `Interval` to be used as a `Vec<NodeId>` for convenience
impl Deref for Interval {
    type Target = Vec<NodeId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Interval {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Interval {
    /// Returns the single-entry header of this interval.
    pub fn header(&self) -> NodeId {
        *self.first().unwrap()
    }
}

impl<T> Graph<T> {
    /// Computes a partitioning set of intervals for this graph.
    ///
    /// The implementation is based on the algorithm described in Figure 6.8 (Page 132) of
    /// "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland University of
    /// Technology, 1994".
    ///
    /// # Definitions
    ///
    /// An interval `I(h)` of graph `G` is the largest subgraph with single-entry `h` where all
    /// cycles within the subgraph contain `h`. Intervals `I` can be selected such that they
    /// partition `G`, that is, each node in the graph belongs to exactly one interval in `I`.
    ///
    /// # Panics
    ///
    /// Panics if the graph doesn't have an entrypoint.
    #[allow(non_snake_case)]
    pub fn intervals(&self) -> Vec<Interval> {
        let len = self.len();
        let start = self.entry.expect("intervals needs entrypoint");

        // Set of intervals to return
        let mut I = vec![];
        // Header node queue for intervals, initialised to the start node
        let mut H = vec![start];
        // Pointer to currently visited header node, we need to keep a record of previous headers so
        // we can't just pop them off the front of the queue
        let mut H_i = 0;
        // Intervals must be disjoint, so make sure we only partition each node once
        let mut partitioned = Vec::with_capacity(len);
        partitioned.push(start);
        // While we still have headers to explore...
        while H_i < H.len() {
            // Get the header add the "front" of the queue, then advance the queue
            let h = H[H_i];
            H_i += 1;
            // I(h) is the maximal fixed point where all nodes' predecessors are in I(h)
            let mut I_h = Interval(vec![h]);
            let mut changed = true;
            while changed {
                changed = false;
                for m in self.iter_id() {
                    if !I_h.contains(&m)
                        && !partitioned.contains(&m) // Ensure each node only added to one I(h)
                        && self[m].predecessors.iter().all(|p| I_h.contains(p))
                    {
                        I_h.push(m);
                        partitioned.push(m);
                        changed = true;
                    }
                }
            }
            // Add unprocessed nodes not in this interval, but with a predecessor in this interval
            // as headers of their own intervals
            for n in self.iter_id() {
                if !H.contains(&n)
                    && !I_h.contains(&n)
                    && self[n].predecessors.iter().any(|p| I_h.contains(p))
                {
                    H.push(n);
                }
            }
            // Record this interval to return
            I.push(I_h);
        }
        I
    }

    /// Computes the derived sequence of higher-order intervals.
    ///
    /// The implementation is based on the algorithm described in Figure 6.10 (Page 134) of
    /// "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland University of
    /// Technology, 1994".
    ///
    /// # Definitions
    ///
    /// The derived sequence of intervals `(G[0], I[0]), ..., (G[n], I[n])` is a sequence of graphs
    /// and their corresponding intervals such that `G[0] = G` and the nodes of `G[i]` are the
    /// intervals of `G[i-1]`. Edges are added to `G[i]` if an edge exists between the intervals of
    /// `G[i-1]`. The sequence continues until there are no changes to the graph.
    ///
    /// Note, if `G` is reducible, the final `G[n]` will be a trivial graph with a single node and
    /// no edges.
    ///
    /// # Panics
    ///
    /// Panics if the graph doesn't have an entrypoint.
    #[allow(non_snake_case)]
    pub fn intervals_derived_sequence(&self) -> (Vec<Graph<Vec<NodeId>>>, Vec<Vec<Interval>>) {
        // Nodes' values in `G` will be lists of nodes in the original graph belonging to that node
        let G_0 = self.map(|n, _| vec![n]);
        // Initialise sequences G and I with G[0] and I[0]
        let mut G = vec![G_0];
        let mut I = vec![G[0].intervals()];

        // Maps ids of nodes in the previous graph to the head of the interval they belong to
        let mut node_intervals = NodeMap::new();
        // Maps heads of intervals in the previous graph to their node ids in the new graph
        let mut interval_nodes = NodeMap::new();
        // Set of already inserted edges in the new graph between nodes corresponding to intervals
        // in the previous graph
        let mut edges = HashSet::<(NodeId, NodeId)>::new();

        let mut i = 1;
        loop {
            let mut G_i = Graph::<Vec<NodeId>>::new();

            // Nodes of G_i are the intervals of G_{i-1}
            for I_n in &I[i - 1] {
                let interval = I_n.header(); // header of interval
                let mut value = vec![]; // value of new node
                for &n in I_n.iter() {
                    node_intervals.insert(n, interval);
                    value.extend_from_slice(&G[i - 1][n].value);
                }
                let interval_node = G_i.add_node(value);
                interval_nodes.insert(interval, interval_node);
            }

            // For every outgoing edge in the previous graph, if it crosses an interval boundary,
            // and there's not already an edge between those intervals' new nodes, add one
            for source in G[i - 1].iter() {
                let source_interval = *node_intervals.get(source.id).unwrap();
                let source_interval_node = *interval_nodes.get(source_interval).unwrap();
                for &target in &source.successors {
                    let target_interval = *node_intervals.get(target).unwrap();
                    if source_interval != target_interval {
                        let target_interval_node = *interval_nodes.get(target_interval).unwrap();
                        let edge = (source_interval_node, target_interval_node);
                        if !edges.contains(&edge) {
                            edges.insert(edge);
                            G_i.add_edge(source_interval_node, target_interval_node);
                        }
                    }
                }
            }

            // If the graph is the same as the previous, we've built the complete sequence
            if G_i == G[i - 1] {
                break;
            }

            i += 1;

            // Record the sequence
            I.push(G_i.intervals());
            G.push(G_i);

            // Reset temporaries, but retain allocated heap memory
            node_intervals.clear();
            interval_nodes.clear();
            edges.clear();
        }

        assert_eq!(G.len(), I.len());
        (G, I)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::types::tests::{fixture_1, fixture_2};

    #[test]
    fn intervals_1() {
        let (g, (n1, n2, n3, n4, n5, n6)) = fixture_1();
        let intervals = g.intervals();
        assert_eq!(intervals.len(), 2);
        assert_eq!(intervals[0], Interval(vec![n1]));
        assert_eq!(intervals[1], Interval(vec![n2, n3, n4, n5, n6]));
    }

    #[test]
    fn intervals_2() {
        let (g, (n1, n2, n3, n4, n5, n6, n7, n8)) = fixture_2();
        let intervals = g.intervals();
        assert_eq!(intervals.len(), 4);
        assert_eq!(intervals[0], Interval(vec![n1]));
        assert_eq!(intervals[1], Interval(vec![n2]));
        assert_eq!(intervals[2], Interval(vec![n3, n4, n5, n6]));
        assert_eq!(intervals[3], Interval(vec![n7, n8]));
    }

    #[test]
    #[allow(non_snake_case)]
    fn derived_sequence_2() {
        // Expected results based on Figure 4 (Page 7) from "Frances E. Allen. 1970. Control flow
        // analysis. SIGPLAN Not. 5, 7 (July 1970), 1–19".
        let (g, (n1, n2, n3, n4, n5, n6, n7, n8)) = fixture_2();
        let (G, I) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 4);
        assert_eq!(I.len(), 4);

        // Check final graph is trivial
        let last = G.last().unwrap();
        assert_eq!(last[n1].value, [n1, n2, n3, n4, n5, n6, n7, n8]);
        assert_eq!(last[n1].in_degree(), 0);
        assert_eq!(last[n1].out_degree(), 0);

        // Check intervals match expected
        assert_eq!(I[0].len(), 4);
        assert_eq!(I[0][0], Interval(vec![n1]));
        assert_eq!(I[0][1], Interval(vec![n2]));
        assert_eq!(I[0][2], Interval(vec![n3, n4, n5, n6]));
        assert_eq!(I[0][3], Interval(vec![n7, n8]));

        assert_eq!(I[1].len(), 2);
        assert_eq!(I[1][0], Interval(vec![n1]));
        assert_eq!(I[1][1], Interval(vec![n2, n3, n4]));

        assert_eq!(I[2].len(), 1);
        assert_eq!(I[2][0], Interval(vec![n1, n2]));

        assert_eq!(I[3].len(), 1);
        assert_eq!(I[3][0], Interval(vec![n1]));

        // Check node values match expected (nodes of `G[i]` are the intervals of `G[i-1]`)
        assert_eq!(G[0][n1].value, [n1]);
        assert_eq!(G[0][n2].value, [n2]);
        assert_eq!(G[0][n3].value, [n3]);
        assert_eq!(G[0][n4].value, [n4]);
        assert_eq!(G[0][n5].value, [n5]);
        assert_eq!(G[0][n6].value, [n6]);
        assert_eq!(G[0][n7].value, [n7]);
        assert_eq!(G[0][n8].value, [n8]);

        assert_eq!(G[1][n1].value, [n1]);
        assert_eq!(G[1][n2].value, [n2]);
        assert_eq!(G[1][n3].value, [n3, n4, n5, n6]);
        assert_eq!(G[1][n4].value, [n7, n8]);

        assert_eq!(G[2][n1].value, [n1]);
        assert_eq!(G[2][n2].value, [n2, n3, n4, n5, n6, n7, n8]);

        assert_eq!(G[3][n1].value, [n1, n2, n3, n4, n5, n6, n7, n8]);
    }
}
