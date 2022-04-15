use crate::function::structure::{ControlFlowGraph, Loop, LoopKind};
use crate::graph::{NodeId, NodeMap, NodeSet, Order};

/// Returns all pre-tested loop header nodes and post-tested loop latching nodes that should be
/// excluded from two-way conditional (if-statement) structuring.
///
/// Note a post-tested loop may have a if-statement as a header (e.g. `do { if (...) {`).
/// A pre-tested loop may have an if-statement as a latching, but a placeholder node will be
/// inserted in this case (see [`ControlFlowGraph::insert_placeholder_nodes`]).
pub fn ignored_loop_headers(loops: &NodeMap<Loop>) -> NodeSet {
    loops
        .values()
        .map(|l| match l.kind {
            LoopKind::PreTested => l.header,
            LoopKind::PostTested => l.latching,
        })
        .collect()
}

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

#[cfg(test)]
mod tests {
    use crate::function::structure::{ignored_loop_headers, ConditionalKind, Structure};
    use crate::graph::NodeSet;
    use crate::tests::load_basic_blocks;

    #[test]
    fn conditional_if() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "if (n > 1) { n = 1; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let conditionals = g.find_2_way_conditionals(&NodeSet::new());
        assert_eq!(conditionals.iter().count(), 1);

        // Extract key nodes
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let false_node = g[entry].successors[0];
        let true_node = g[entry].successors[1];
        assert_eq!(g[false_node].successors, [true_node]);

        // Check conditional
        assert_eq!(conditionals[entry], true_node);

        Ok(())
    }

    #[test]
    fn conditional_if_else() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "if (n > 1) { n = 1; } else { n = 0; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let conditionals = g.find_2_way_conditionals(&NodeSet::new());
        assert_eq!(conditionals.iter().count(), 1);

        // Extract key nodes
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let false_node = g[entry].successors[0];
        let true_node = g[entry].successors[1];
        assert_eq!(g[false_node].successors, g[true_node].successors);
        assert_eq!(g[false_node].successors.len(), 1);
        let follow = g[false_node].successors[0];

        // Check conditional
        assert_eq!(conditionals[entry], follow);

        Ok(())
    }

    #[test]
    fn conditional_multiple_if_else() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "
            if (n > 2) { n = 2; }
            if (n > 1) { n = 1; } else { n = 0; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let conditionals = g.find_2_way_conditionals(&NodeSet::new());
        assert_eq!(conditionals.iter().count(), 2);

        // Extract key nodes
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let false_node1 = g[entry].successors[0];
        let true_node1 = g[entry].successors[1];
        assert_eq!(g[false_node1].successors, [true_node1]);

        assert_eq!(g[true_node1].successors.len(), 2);
        let false_node2 = g[true_node1].successors[0];
        let true_node2 = g[true_node1].successors[1];
        assert_eq!(g[false_node2].successors, g[true_node2].successors);
        assert_eq!(g[false_node2].successors.len(), 1);
        let follow2 = g[false_node2].successors[0];

        // Check conditionals
        assert_eq!(conditionals[entry], true_node1);
        assert_eq!(conditionals[true_node1], follow2);

        Ok(())
    }

    #[test]
    fn conditional_nested_if_else() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "int a = 0, b = 0;
            if (a == b) {
                n = 0;
            } else {
                if (a < b) {
                    n = -1;
                } else {
                    n = 1;
                }
            }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let conditionals = g.find_2_way_conditionals(&NodeSet::new());
        assert_eq!(conditionals.iter().count(), 2);

        // Extract key nodes
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let false_node1 = g[entry].successors[0];
        let true_node1 = g[entry].successors[1];

        assert_eq!(g[true_node1].successors.len(), 2);
        let false_node2 = g[true_node1].successors[0];
        let true_node2 = g[true_node1].successors[1];
        assert_eq!(g[false_node1].successors, g[false_node2].successors);
        assert_eq!(g[false_node1].successors, g[true_node2].successors);
        assert_eq!(g[false_node1].successors.len(), 1);
        let follow = g[false_node1].successors[0];

        // Check conditionals
        assert_eq!(conditionals[entry], follow);
        assert_eq!(conditionals[true_node1], follow);

        Ok(())
    }

    #[test]
    fn conditional_if_compound() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "boolean a = false; boolean b = false;
            if (a || b) { n--; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let conditionals = g.find_2_way_conditionals(&NodeSet::new());
        assert_eq!(conditionals.iter().count(), 1);

        // Extract key nodes
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let false_node = g[entry].successors[0];
        let true_node = g[entry].successors[1];
        assert_eq!(g[false_node].successors, [true_node]);

        // Check conditional
        assert_eq!(conditionals[entry], true_node);
        assert!(matches!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: true,
                kind: ConditionalKind::Conjunction,
                ..
            }
        ));

        Ok(())
    }

    #[test]
    fn conditional_nested_if_at_end_of_pre_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "while (n > 2) {
                if (n > 1) { n--; }
            }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        let ignored_headers = ignored_loop_headers(&loops);
        let conditionals = g.find_2_way_conditionals(&ignored_headers);
        assert_eq!(conditionals.iter().count(), 1);

        // Extract key nodes
        let entry = g.entry.unwrap(); // while (n > 2) {
        assert_eq!(g[entry].successors.len(), 2);
        let header = g[entry].successors[0]; // if (n > 1) {
        assert_eq!(g[header].successors.len(), 2);
        let false_node = g[header].successors[0];
        let true_node = g[header].successors[1];
        assert_eq!(g[false_node].successors, [true_node]);

        // Check conditional
        assert_eq!(conditionals[header], true_node);
        assert_eq!(g[true_node].value, Structure::default()); // placeholder

        Ok(())
    }

    #[test]
    fn conditional_nested_if_else_at_end_of_pre_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "while (n > 1) {
                if (n > 2) { n -= 2; } else { n--; }
            }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        let ignored_headers = ignored_loop_headers(&loops);
        let conditionals = g.find_2_way_conditionals(&ignored_headers);
        assert_eq!(conditionals.iter().count(), 1);

        // Extract key nodes
        let entry = g.entry.unwrap(); // while (n > 1)
        assert_eq!(g[entry].successors.len(), 2);
        let header = g[entry].successors[0];
        assert_eq!(g[header].successors.len(), 2);
        let false_node = g[header].successors[0];
        let true_node = g[header].successors[1];
        assert_eq!(g[false_node].successors, g[true_node].successors);
        assert_eq!(g[false_node].successors.len(), 1);
        let follow = g[false_node].successors[0];

        // Check conditional
        assert_eq!(conditionals[header], follow);
        assert_eq!(g[follow].value, Structure::default()); // placeholder

        Ok(())
    }
}
