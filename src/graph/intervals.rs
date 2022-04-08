use crate::graph::{Graph, NodeId, NodeMap};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Interval(Vec<NodeId>);

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
    pub fn header(&self) -> NodeId {
        *self.first().unwrap()
    }
}

impl<T> Graph<T> {
    #[allow(non_snake_case)]
    pub fn intervals(&self) -> Vec<Interval> {
        let len = self.len();
        let start = self.entry.expect("intervals needs entrypoint");

        let mut I = vec![];
        let mut H = vec![start];
        let mut H_i = 0;
        // Intervals must be disjoint, so make sure we only partition each node once
        let mut partitioned = Vec::with_capacity(len);
        partitioned.push(start);
        while H_i < H.len() {
            let n = H[H_i];
            H_i += 1;
            let header = n;
            let mut I_n = Interval(vec![header]);
            let mut changed = true;
            while changed {
                changed = false;
                for m in self.iter_id() {
                    if !I_n.contains(&m)
                        && !partitioned.contains(&m)
                        && self[m].predecessors.iter().all(|p| I_n.contains(p))
                    {
                        I_n.push(m);
                        partitioned.push(m);
                        changed = true;
                    }
                }
            }
            for n in self.iter_id() {
                if !H.contains(&n)
                    && !I_n.contains(&n)
                    && self[n].predecessors.iter().any(|p| I_n.contains(p))
                {
                    H.push(n);
                }
            }
            I.push(I_n);
        }
        I
    }

    #[allow(non_snake_case)]
    pub fn intervals_derived_sequence(&self) -> (Vec<Graph<Vec<NodeId>>>, Vec<Vec<Interval>>) {
        let mut G = vec![self.map(|n, _| vec![n])];
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

            I.push(G_i.intervals());
            G.push(G_i);

            node_intervals.clear();
            interval_nodes.clear();
            edges.clear();
        }

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
    fn derived_sequence_2() {
        let (g, (n1, n2, n3, n4, n5, n6, n7, n8)) = fixture_2();
        let (derived_sequence, intervals) = g.intervals_derived_sequence();
        assert_eq!(derived_sequence.len(), 4);
        assert_eq!(intervals.len(), 4);

        // Final graph should be trivial
        assert_eq!(
            derived_sequence.last().unwrap()[n1].value,
            vec![n1, n2, n3, n4, n5, n6, n7, n8]
        );

        // TODO: more asserts
        println!(
            "Derived Sequence: {:?}\nDerived Sequence Intervals: {:?}",
            derived_sequence, intervals
        );
    }
}
