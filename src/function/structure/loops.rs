use crate::function::structure::ControlFlowGraph;
use crate::graph::{Graph, NodeId, Order};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Copy, Clone)]
pub enum LoopKind {
    PreTested,
    PostTested,
    // Endless, (unsupported)
}

#[derive(Debug, Copy, Clone)]
pub struct Loop {
    pub kind: LoopKind,
    pub header: NodeId,
    pub latching: NodeId,
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
    #[allow(non_snake_case)]
    pub fn find_loops(&self) -> anyhow::Result<HashMap<NodeId, Loop>> {
        let mut in_loop = HashSet::new();
        let mut loops = HashMap::new();

        let reverse_post_order = self.depth_first(Order::ReversePostOrder);

        let (G, I) = self.intervals_derived_sequence();
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
                        && !in_loop.contains(&x))
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
                        .collect::<HashSet<_>>();
                    let mut body = hashset! {h_j};
                    for n in reverse_post_order.range(x, h_j) {
                        if I_j.contains(&n) {
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

    fn find_loop_kind(
        &self,
        h_j: NodeId,
        x: NodeId,
        body: &HashSet<NodeId>,
    ) -> anyhow::Result<LoopKind> {
        if self[x].out_degree() == 2 {
            if self[h_j].out_degree() == 2 {
                if self[h_j].successors.iter().all(|n| body.contains(n)) {
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

    fn find_loop_follow(
        &self,
        h_j: NodeId,
        x: NodeId,
        body: &HashSet<NodeId>,
        kind: LoopKind,
    ) -> NodeId {
        match kind {
            LoopKind::PreTested => {
                if body.contains(&self[h_j].successors[0]) {
                    self[h_j].successors[1]
                } else {
                    self[h_j].successors[0]
                }
            }
            LoopKind::PostTested => {
                if body.contains(&self[x].successors[0]) {
                    self[x].successors[1]
                } else {
                    self[x].successors[0]
                }
            }
        }
    }
}
