use crate::function::structure::ControlFlowGraph;
use crate::graph::{Graph, NodeId, NodeMap, NodeSet, Order};
use std::fmt;

/// Possible loop type for [`Loop`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LoopKind {
    /// Evaluate condition before evaluating body (e.g. `while` loop).
    PreTested,
    /// Evaluate body at least once before evaluating condition (e.g. `do-while` loop).
    PostTested,
    // Endless, (unsupported)
}

/// Identified pre-/post-tested loop in a [`ControlFlowGraph`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Loop {
    /// Whether condition is evaluated before or after body.
    pub kind: LoopKind,
    /// Entrypoint of loop. For pre-tested loops, this must be a conditional branch.
    pub header: NodeId,
    /// Node with back edge to `header` in the loop. For post-tested loops, this must be a
    /// conditional branch.
    pub latching: NodeId,
    /// Node immediately after exiting the loop.
    pub follow: NodeId,
}

impl fmt::Display for Loop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} -> {} => {} ({:?})",
            self.header, self.latching, self.follow, self.kind,
        )
    }
}

/// Returns `true` if the provided derived sequence is for a reducible graph.
///
/// A graph is reducible if the final graph in provided derived sequence of intervals `G` is
/// trivial (single node and no edges).
#[allow(non_snake_case)]
fn is_reducible(G: &[Graph<Vec<NodeId>>]) -> bool {
    // Graph is reducible if final graph in derived sequence is trivial (just 1 node, 0 edges)
    let last = G
        .last()
        .expect("Unable to find last derived sequence graph");
    let last_start = last
        .entry
        .expect("Unable to find last derived sequence graph entrypoint");
    last.len() == 1 && last[last_start].successors.is_empty()
}

impl ControlFlowGraph {
    /// Identifies all pre- and post-tested looks in the control flow graph, returning
    /// loop kinds and header/latching/follow nodes, using the algorithm described in Figure 6.25
    /// of "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland University of
    /// Technology, 1994".
    ///
    /// See [`Loop`]'s fields' documentation for more details on the types of identified nodes.
    ///
    /// This should be called after structuring compound short-circuit conditionals, as these might
    /// be used in loop header/latching nodes (e.g. `while (a && b) { ... }`).
    ///
    /// # Overview
    ///
    /// This analysis uses the [derived sequence of intervals](Graph::intervals_derived_sequence).
    /// For each graph in the derived sequence, we can find loops by looking for back edges in each
    /// interval from latching nodes to headers. The interval fully contains the corresponding loop
    /// body by definition. The type of loop can be identified from the out degrees of header and
    /// latching nodes. Similarly, the follow node can be identified from the loop type and which
    /// successor of the header of latching node is outside the loop body.
    #[allow(non_snake_case)]
    pub fn find_loops(&self) -> anyhow::Result<NodeMap<Loop>> {
        let mut in_loop = NodeSet::with_capacity_for(self);
        let mut loops = NodeMap::with_capacity_for(self);

        let reverse_post_order = self.depth_first(Order::ReversePostOrder);

        let (G, I) = self.intervals_derived_sequence();

        // Make sure the graph is reducible
        let reducible = is_reducible(&G);
        ensure!(reducible, "Irreducible flow graphs are not yet supported");

        // For each graph in the derived sequence...
        for (i, G_i) in G.into_iter().enumerate() {
            // For each interval in this part of the derived sequence...
            // ...is there a latching node, in that same interval
            for I_j_derived in &I[i] {
                let h_j_derived = I_j_derived.header();
                let h_j = G_i[h_j_derived].value[0];

                for &x_derived in I_j_derived.iter() {
                    // Find first potential latching node in interval that has a back edge to h_j
                    let x = G_i[x_derived]
                        .value
                        .iter()
                        .find(|&&n| self[n].successors.contains(&h_j));
                    if x == None {
                        continue;
                    }
                    let x = *x.unwrap();

                    // Make sure the header is in the same interval as the potential latching
                    // node, and the latching node isn't in a loop yet
                    if !(G_i[x_derived]
                        .successors
                        .iter()
                        .any(|&target| target == h_j_derived)
                        && !in_loop.contains(x))
                    {
                        continue;
                    }

                    // h_j is the loop header node  (x in thesis)
                    // x is the latching node       (y in thesis)
                    // x -> h_j is a back edge      (y -> x in thesis)
                    // h_j and x are both indices into the original graph
                    assert!(reverse_post_order.cmp(h_j, x).is_ge());

                    // Mark nodes in this loop
                    // TODO: extract this out into function
                    let I_j = I_j_derived
                        .iter()
                        .flat_map(|&derived| G_i[derived].value.iter())
                        .copied()
                        .collect::<NodeSet>();
                    let mut body = NodeSet::new();
                    body.insert(h_j);
                    for n in reverse_post_order.range(x, h_j) {
                        if I_j.contains(n) {
                            in_loop.insert(n);
                            body.insert(n);
                        }
                    }

                    // Identify loop type and follow node
                    let kind = self.find_loop_kind(h_j, x, &body)?;
                    let follow = self.find_loop_follow(h_j, x, &body, kind);

                    let l = Loop {
                        kind,
                        header: h_j,
                        latching: x,
                        follow,
                    };
                    loops.insert(h_j, l);
                }
            }
        }

        Ok(loops)
    }

    /// Identifies the type of the loop induced by the back edge `x` -> `h_j` with `body`, using the
    /// algorithm described in Figure 6.28 of "Cristina Cifuentes. Reverse Compilation Techniques.
    /// PhD thesis, Queensland University of Technology, 1994".
    ///
    /// ![Loop Types](../../../images/looptypes.png)
    fn find_loop_kind(&self, h_j: NodeId, x: NodeId, body: &NodeSet) -> anyhow::Result<LoopKind> {
        if self[x].out_degree() == 2 {
            if self[h_j].out_degree() == 2 {
                if self[h_j].successors.iter().all(|&n| body.contains(n)) {
                    Ok(LoopKind::PostTested)
                } else {
                    Ok(LoopKind::PreTested)
                }
            } else {
                Ok(LoopKind::PostTested)
            }
        } else {
            // 1-way latching node
            if self[h_j].out_degree() == 2 {
                Ok(LoopKind::PreTested)
            } else {
                bail!("Endless loops are not yet supported")
            }
        }
    }

    /// Identifies the follow node (after loop exit) for the loop induced by the back edge
    /// `x` -> `h_j` with `body` and type `kind`, using the algorithm described in Figure 6.29 of
    /// "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland University of
    /// Technology, 1994".
    fn find_loop_follow(&self, h_j: NodeId, x: NodeId, body: &NodeSet, kind: LoopKind) -> NodeId {
        match kind {
            LoopKind::PreTested => {
                if body.contains(self[h_j].successors[0]) {
                    self[h_j].successors[1]
                } else {
                    self[h_j].successors[0]
                }
            }
            LoopKind::PostTested => {
                if body.contains(self[x].successors[0]) {
                    self[x].successors[1]
                } else {
                    self[x].successors[0]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::function::structure::{ConditionalKind, ControlFlowGraph, LoopKind, Structure};
    use crate::tests::load_basic_blocks;

    #[test]
    fn invalid_loop_irreducible_flow() {
        // Construct canonical irreducible flow graph
        let mut g = ControlFlowGraph::new();
        let entry = g.add_node(Structure::default());
        let left = g.add_node(Structure::default());
        let right = g.add_node(Structure::default());
        g.add_edge(entry, left);
        g.add_edge(entry, right);
        g.add_edge(left, right);
        g.add_edge(right, left);
        let err = g.find_loops().unwrap_err();
        assert_eq!(
            format!("{}", err),
            "Irreducible flow graphs are not yet supported"
        );
    }

    #[test]
    fn loop_case_1_pre_tested_unconditional_latching() -> anyhow::Result<()> {
        let mut g = load_basic_blocks("while (n > 1) { n--; } return n;")?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 1);

        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let latching = g[entry].successors[0];
        let follow = g[entry].successors[1];
        let loop_info = loops[entry];
        assert_eq!(loop_info.kind, LoopKind::PreTested);
        assert_eq!(loop_info.header, entry);
        assert_eq!(loop_info.latching, latching);
        assert_eq!(loop_info.follow, follow);

        Ok(())
    }

    #[test]
    fn loop_case_2_pre_tested_conditional_latching() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "while (n > 1) {
                if (n > 2) { n -= 2; } else { n--; }
            }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 1);

        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let follow = g[entry].successors[1];
        let loop_info = loops[entry];
        assert_eq!(loop_info.kind, LoopKind::PreTested);
        assert_eq!(loop_info.header, entry);
        assert_eq!(loop_info.follow, follow);
        assert_eq!(g[loop_info.latching].value, Structure::default()); // placeholder

        Ok(())
    }

    #[test]
    fn loop_case_3_post_tested_conditional_header() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "do {
                if (n > 2) { n -= 2; } else { n--; }
            } while (n > 1);
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 1);

        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2); // 2-way conditional header node
        let loop_info = loops[entry];
        assert_eq!(loop_info.kind, LoopKind::PostTested);
        assert_eq!(loop_info.header, entry);
        assert_eq!(g[loop_info.latching].successors.len(), 2);
        assert_eq!(g[loop_info.latching].successors[1], entry);
        let follow = g[loop_info.latching].successors[0];
        assert_eq!(loop_info.follow, follow);
        assert_eq!(g[follow].successors.len(), 0);

        Ok(())
    }

    #[test]
    fn loop_case_4_post_tested_unconditional_header() -> anyhow::Result<()> {
        let mut g = load_basic_blocks("do { n--; } while (n > 1); return n;")?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 1);

        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2); // post-tested latching node
        let loop_info = loops[entry];
        // This is detected as a single node pre-tested loop, but handled as a post-tested loop
        assert_eq!(loop_info.kind, LoopKind::PreTested);
        assert_eq!(loop_info.header, entry);
        assert_eq!(loop_info.latching, entry);
        let follow = g[entry].successors[0];
        assert_eq!(loop_info.follow, follow);
        assert_eq!(g[follow].successors.len(), 0);

        Ok(())
    }

    #[test]
    fn invalid_loop_case_5_endless() -> anyhow::Result<()> {
        let g = load_basic_blocks("while (true) {}")?;
        let err = g.find_loops().unwrap_err();
        assert_eq!(format!("{}", err), "Endless loops are not yet supported");
        Ok(())
    }

    #[test]
    fn loop_nested_pre_tested_inside_post_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "do {
                while (n > 2) { n--; }
            } while (n > 1);
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 2);

        let entry = g.entry.unwrap();
        let post_tested_loop = loops[entry];
        assert_eq!(post_tested_loop.kind, LoopKind::PostTested);
        assert_eq!(post_tested_loop.header, entry);
        assert_eq!(g[entry].successors.len(), 1);
        let pre_tested_loop_header = g[entry].successors[0];
        let pre_tested_loop = loops[pre_tested_loop_header];
        assert_eq!(pre_tested_loop.kind, LoopKind::PreTested);
        assert_eq!(pre_tested_loop.header, pre_tested_loop_header);
        assert_eq!(g[pre_tested_loop_header].successors.len(), 2);
        let pre_tested_loop_follow = g[pre_tested_loop_header].successors[1];
        assert_eq!(pre_tested_loop.follow, pre_tested_loop_follow);
        assert_eq!(g[pre_tested_loop_follow].successors.len(), 2);
        assert_eq!(g[pre_tested_loop_follow].successors[1], entry);
        assert_eq!(post_tested_loop.latching, pre_tested_loop_follow);

        Ok(())
    }

    #[test]
    fn loop_nested_post_tested_inside_pre_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "while (n > 2) {
                do { n--; } while (n > 1);
            }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 2);

        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors.len(), 2);
        let post_tested_loop_header_latching = g[entry].successors[0];
        let pre_tested_follow = g[entry].successors[1];

        let pre_tested_loop = loops[entry];
        assert_eq!(pre_tested_loop.kind, LoopKind::PreTested);
        assert_eq!(pre_tested_loop.header, entry);
        assert_eq!(pre_tested_loop.follow, pre_tested_follow);

        let post_tested_loop = loops[post_tested_loop_header_latching];
        // This is detected as a single node pre-tested loop, but handled as a post-tested loop
        assert_eq!(post_tested_loop.kind, LoopKind::PreTested);
        assert_eq!(post_tested_loop.header, post_tested_loop_header_latching);
        assert_eq!(post_tested_loop.latching, post_tested_loop_header_latching);
        assert_eq!(post_tested_loop.follow, pre_tested_loop.latching);

        Ok(())
    }

    #[test]
    fn loop_nested_pre_tested_inside_if_inside_post_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "do {
                if (n == 2) {
                    n = 1;
                } else {
                    while (n > 2) {
                        n -= 2;
                    }
                }
            } while (n > 1);
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 2);

        // Extract key nodes
        let entry = g.entry.unwrap(); // if (n == 2) {
        assert_eq!(g[entry].successors.len(), 2);
        let false_node = g[entry].successors[0]; // n = 1;
        let true_node = g[entry].successors[1]; // while (n > 2) {
        assert_eq!(g[false_node].successors.len(), 1);
        assert_eq!(g[true_node].successors.len(), 2);
        let post_tested_loop_latching = g[false_node].successors[0]; // } while (n > 1)
        let pre_tested_loop_latching = g[true_node].successors[0]; // n -= 2;
        assert_eq!(g[true_node].successors[1], post_tested_loop_latching);
        assert_eq!(g[post_tested_loop_latching].successors.len(), 2);
        let post_tested_loop_follow = g[post_tested_loop_latching].successors[0]; // return n;

        // Check loops
        let post_tested_loop = loops[entry];
        assert_eq!(post_tested_loop.kind, LoopKind::PostTested);
        assert_eq!(post_tested_loop.header, entry);
        assert_eq!(post_tested_loop.latching, post_tested_loop_latching);
        assert_eq!(post_tested_loop.follow, post_tested_loop_follow);
        let pre_tested_loop = loops[true_node];
        assert_eq!(pre_tested_loop.kind, LoopKind::PreTested);
        assert_eq!(pre_tested_loop.header, true_node);
        assert_eq!(pre_tested_loop.latching, pre_tested_loop_latching);
        assert_eq!(pre_tested_loop.follow, post_tested_loop_latching);

        Ok(())
    }

    #[test]
    fn loop_compound_conditional() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "boolean a = false; boolean b = false;
            while (a && b) { n--; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();
        let loops = g.find_loops()?;
        assert_eq!(loops.iter().count(), 1);

        // Extract key nodes
        let entry = g.entry.unwrap(); // boolean a = false; boolean b = false;
        assert_eq!(g[entry].successors.len(), 1);
        let header = g[entry].successors[0]; // while (a && b) {
        assert_eq!(g[header].successors.len(), 2);
        let latching = g[header].successors[0]; // n--;
        let follow = g[header].successors[1]; // return n;

        // Check loop
        let loop_info = loops[header];
        assert_eq!(loop_info.kind, LoopKind::PreTested);
        assert_eq!(loop_info.header, header);
        assert_eq!(loop_info.latching, latching);
        assert_eq!(loop_info.follow, follow);

        // Check loop header is compound conditional
        assert!(matches!(
            g[header].value,
            Structure::CompoundConditional {
                left_negated: false,
                kind: ConditionalKind::Disjunction,
                ..
            }
        ));

        Ok(())
    }
}
