use crate::function::structure::ConditionalKind;
use crate::graph::{remove_element, Graph, NodeId, NodeOrder, Order};
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use std::collections::HashMap;
use std::fmt;

/// Node value for control flow graphs, either a basic block or compound conditional.
#[derive(Eq, PartialEq)]
pub enum Structure {
    /// Basic block consisting of a sequence of instructions executed in order.
    Block(Vec<JVMInstruction>),
    /// Short-circuit conditional, `left` is always evaluated, but `right` may not be evaluated
    /// if the result of the conditional can be determined from `left` only.
    CompoundConditional {
        kind: ConditionalKind,
        left_negated: bool,
        left: Box<Structure>,
        right: Box<Structure>,
    },
}

impl Default for Structure {
    /// Returns an empty basic block, doesn't allocate until items are added to the block.
    fn default() -> Self {
        Structure::Block(vec![])
    }
}

impl fmt::Debug for Structure {
    //noinspection RsLiveness
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Structure::Block(instructions) => {
                let mut iter = instructions.iter().peekable();
                while let Some(instruction) = iter.next() {
                    write!(f, "{:?}", instruction)?;
                    if iter.peek().is_some() {
                        write!(f, "\\n")?;
                    }
                }
            }
            Structure::CompoundConditional {
                kind,
                left_negated,
                left,
                right,
            } => {
                write!(
                    f,
                    "{left_negated}{{\n{left:?}\n}} {kind} {{\n{right:?}\n}}",
                    left_negated = if *left_negated { "! " } else { "" },
                )?;
            }
        };
        Ok(())
    }
}

/// Type specialisation of single-entry directed [`Graph`]s for control flow graphs, using
/// [`Structure`]s for node values.
pub type ControlFlowGraph = Graph<Structure>;

/// Macro for matching on conditional, unconditional and not branching JVM instructions, at a
/// specific label.
///
/// All `IF*` instructions are conditional branches. `GOTO` and `GOTO_W `are unconditional branches.
/// All remaining instructions are not branching.
///
/// For conditional and unconditional branches, the absolute target label of the branch is bound to
/// the match arm.
macro_rules! match_branches {
    ($label:expr, $instruction:expr, {
        None => $no_branch:block,
        Unconditional($uncond_target:ident) => $uncond_branch:block,
        Conditional($cond_target:ident) => $cond_branch:block,
    }) => {
        match $instruction {
            JVMInstruction::IfAcmpeq(n)
            | JVMInstruction::IfAcmpne(n)
            | JVMInstruction::IfIcmpeq(n)
            | JVMInstruction::IfIcmpne(n)
            | JVMInstruction::IfIcmplt(n)
            | JVMInstruction::IfIcmpge(n)
            | JVMInstruction::IfIcmpgt(n)
            | JVMInstruction::IfIcmple(n)
            | JVMInstruction::Ifnull(n)
            | JVMInstruction::Ifnonnull(n)
            | JVMInstruction::Ifeq(n)
            | JVMInstruction::Ifne(n)
            | JVMInstruction::Iflt(n)
            | JVMInstruction::Ifge(n)
            | JVMInstruction::Ifgt(n)
            | JVMInstruction::Ifle(n) => {
                let $cond_target = ($label as i32 + *n as i32) as usize;
                $cond_branch
            }
            JVMInstruction::Goto(n) => {
                let $uncond_target = ($label as i32 + *n as i32) as usize;
                $uncond_branch
            }
            JVMInstruction::GotoW(n) => {
                let $uncond_target = ($label as i32 + *n) as usize;
                $uncond_branch
            }
            _ => $no_branch,
        };
    };
}

impl ControlFlowGraph {
    /// Helper function for [`ControlFlowGraph::insert_basic_blocks`] that ensures this graph
    /// contains a node for the leader at `label` and this node's ID is stored in the `leaders` map.
    ///
    /// Returns the ID of the inserted or existing node.
    #[inline]
    fn ensure_leader(&mut self, leaders: &mut HashMap<usize, NodeId>, label: usize) -> NodeId {
        *leaders
            .entry(label)
            .or_insert_with(|| self.add_node(Structure::default()))
    }

    /// Adds all basic blocks in the JVM byte`code` to this graph.
    ///
    /// This should only be called once per `ControlFlowGraph` instance.
    ///
    /// # Definitions
    ///
    /// A basic block is a maximal instruction sequence with no internal control flow. All
    /// instruction nodes inside a basic block have a single predecessor and successor, except the
    /// first node which may have many predecessors, and the last node which may have many
    /// successors.
    ///
    /// To find basic blocks, we look for *leader* nodes which are the first nodes in each basic
    /// blocks. Leaders delimit basic blocks, which are made up of the leader and all instructions
    /// up to the next leader.
    ///
    /// A node is a leader if it is:
    ///
    /// - The first instruction (entrypoint)
    /// - The target of a branch
    /// - The instruction immediately following a branch
    pub fn insert_basic_blocks(&mut self, code: Vec<(usize, JVMInstruction)>) {
        // Maps JVM labels at the start of basic block (leaders) to node IDs
        let mut leaders = HashMap::new();

        // Find all leaders, first off, the entrypoint (0) is always a leader
        self.ensure_leader(&mut leaders, 0);
        let mut iter = code.iter().peekable();
        while let Some((label, instruction)) = iter.next() {
            let next = iter.peek();
            match_branches!(*label, instruction, {
                None => {},
                Unconditional(target) => {
                    // Target of branch is leader
                    self.ensure_leader(&mut leaders, target);
                },
                Conditional(target) => {
                    // Instruction following conditional is leader (false branch)
                    if let Some((label, _)) = next {
                        self.ensure_leader(&mut leaders, *label);
                    }
                    // Target of branch is leader (true branch)
                    self.ensure_leader(&mut leaders, target);
                },
            });
        }

        // Node we're adding instructions to, this starts as the function entrypoint
        let mut current_node = leaders[&0];
        let mut iter = code.into_iter().peekable();
        while let Some((label, instruction)) = iter.next() {
            if let Some(label_node) = leaders.get(&label) {
                current_node = *label_node;
            }

            let next = iter.peek();

            match_branches!(label, &instruction, {
                None => {
                    let next_node = next.and_then(|(next_label, _)| leaders.get(next_label));
                    if let Some(next_node) = next_node {
                        self.add_edge(current_node, *next_node);
                    }
                },
                Unconditional(target) => {
                    // Target of branch is leader
                    self.add_edge(current_node, leaders[&target]);
                },
                Conditional(target) => {
                    // Instruction following conditional is leader (false branch)
                    if let Some((label, _)) = next {
                        self.add_edge(current_node, leaders[label]);
                    }
                    // Target of branch is leader (true branch)
                    self.add_edge(current_node, leaders[&target]);
                },
            });

            match &mut self[current_node].value {
                Structure::Block(instructions) => instructions.push(instruction),
                _ => unreachable!("Always inserted with empty Structure::Block"),
            }
        }
    }

    /// Helper function for [`ControlFlowGraph::insert_placeholder_nodes`] that returns all nodes
    /// in the graph with a back edge to the `header` node.
    #[inline]
    fn find_latching_nodes_for(&self, post_order: &NodeOrder, header: NodeId) -> Vec<NodeId> {
        self[header]
            .predecessors
            .iter()
            .filter(|&&x| post_order.cmp(x, header).is_lt())
            .copied()
            .collect()
    }

    /// Ensures there are no nodes with 2 or more back edges to them in this graph as the loop
    /// structuring algorithm requires a unique back edge for each loop.
    ///  
    /// There are 2 cases where this will happen:
    ///  
    /// 1. A pre and post-tested loop share a header node, with the post-tested 2-way latching node
    ///    having a back edge to the header and an edge to the follow.
    ///
    ///    (e.g. pre-tested at start of post-tested body, `do { while(...) {...} } while(...)` )
    ///
    /// 2. A 2-way conditional has a follow node as a loop header, meaning the 2 branches converge
    ///    via back edges.
    ///
    ///    (e.g. if-else at end of pre-tested body, `while(...) { if(...) {...} else {...} }`)
    pub fn insert_placeholder_nodes(&mut self) {
        // We iterate in post-order, handling nested structures first. We do the traversal once at
        // the start to make sure we ignore placeholder nodes added by this function.
        let post_order = self.depth_first(Order::PostOrder);

        // We use immediate post-dominance to differentiate between post-tested loop latching nodes
        // and 2-way conditionals. An alternative would be to check if the latching node had out-
        // degree 2, but this wouldn't work for 2-way conditionals where one branch of the
        // conditional header pointed back to the original loop header directly.
        let ipdom = self.immediate_post_dominators();

        for &header in &post_order.traversal {
            // Find all latching nodes with back edges to the header
            let mut latching = self.find_latching_nodes_for(&post_order, header);

            if latching.len() >= 2 {
                // Case 1: latching node is a post-tested loop latching node
                // (i.e. if the header isn't the immediate post-dominator of the latching node)
                //
                // To fix this, we insert a new placeholder node above the header node, reconnect
                // the back edge to that, then connect it to the header node. If the header node was
                // the entry point, this is updated to the placeholder. This creates a new interval,
                // ensuring the derived sequence of intervals properly captures the loop nesting
                // order, and maintains the property of a single loop per interval.
                let mut loop_latchings = latching.iter().filter(|&&x| ipdom[x] != header);
                if let Some(&loop_latching) = loop_latchings.next() {
                    // We don't support multi-exit loops, so make sure there's at most 1 of these.
                    assert_eq!(loop_latchings.next(), None);

                    let placeholder = self.add_node(Structure::default());

                    // Re-connect back edge to placeholder
                    self.swap_edge(loop_latching, header, placeholder);

                    // If header is the entrypoint, update it to point to the placeholder
                    if self.entry == Some(header) {
                        self.entry = Some(placeholder);
                    } else {
                        // Otherwise, re-connect header's predecessors (that are not back edges themselves)
                        // to placeholder. (clone() as swap_edge will mutate header's predecessors
                        // and requires a mutable borrow on self)
                        for pred in self[header].predecessors.clone() {
                            // Only re-connect if pred -> header is not a back edge
                            if post_order.cmp(pred, header).is_ge() {
                                self.swap_edge(pred, header, placeholder);
                            }
                        }
                    }

                    // Connect placeholder to header
                    self.add_edge(placeholder, header);

                    // Ignore this node when checking for Case 2
                    remove_element(&mut latching, &loop_latching);
                }
            }

            if latching.len() >= 2 {
                // If we still have more than 2 latching nodes for the header...
                //
                // Case 2: 2-way conditionals with loop headers as follow nodes
                //
                // To fix this, insert a placeholder node and connect all back edges to it instead,
                // then connect it to the original header node. This ensures a 2-way conditional's
                // follow node is never a loop header.
                let placeholder = self.add_node(Structure::default());

                // Re-connect all remaining latching node's back edges to placeholder
                for x in latching {
                    // swap_edge maintains the correct true/false branching
                    self.swap_edge(x, header, placeholder);
                }

                // Connect placeholder to original header node
                self.add_edge(placeholder, header);
            }
        }
    }
}

#[allow(non_snake_case)]
#[cfg(test)]
mod tests {
    use crate::function::structure::Structure;
    use crate::tests::load_basic_blocks;
    use classfile_parser::code_attribute::Instruction as JVMInstruction;

    #[test]
    fn basic_blocks_sequence() -> anyhow::Result<()> {
        let g = load_basic_blocks("return 1;")?;
        assert_eq!(g.len(), 1);
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::Block(vec![JVMInstruction::Iconst1, JVMInstruction::Ireturn])
        );
        Ok(())
    }

    #[test]
    fn basic_blocks_if() -> anyhow::Result<()> {
        let g = load_basic_blocks(
            "int a;
            if (n > 1) { a = 1; } else { a = 2; };
            return a;",
        )?;
        assert_eq!(g.len(), 4);

        // Check entrypoint
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::Block(vec![
                JVMInstruction::Iload0,
                JVMInstruction::Iconst1,
                JVMInstruction::IfIcmple(8),
            ])
        );
        assert_eq!(g[entry].successors.len(), 2);

        // Check false/true branches
        let false_node = g[entry].successors[0];
        let true_node = g[entry].successors[1];
        assert_eq!(
            g[false_node].value,
            Structure::Block(vec![
                JVMInstruction::Iconst1,
                JVMInstruction::Istore1,
                JVMInstruction::Goto(5),
            ])
        );
        assert_eq!(
            g[true_node].value,
            Structure::Block(vec![JVMInstruction::Iconst2, JVMInstruction::Istore1])
        );
        assert_eq!(g[false_node].successors.len(), 1);
        assert_eq!(g[true_node].successors.len(), 1);

        // Check follow
        let false_follow = g[false_node].successors[0];
        let true_follow = g[true_node].successors[0];
        assert_eq!(false_follow, true_follow);
        assert_eq!(
            g[false_follow].value,
            Structure::Block(vec![JVMInstruction::Iload1, JVMInstruction::Ireturn])
        );

        Ok(())
    }

    #[test]
    fn basic_blocks_pre_tested_loop() -> anyhow::Result<()> {
        let g = load_basic_blocks("while (n > 1) { n--; } return n;")?;
        assert_eq!(g.len(), 3);

        // Check entrypoint/header
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::Block(vec![
                JVMInstruction::Iload0,
                JVMInstruction::Iconst1,
                JVMInstruction::IfIcmple(9)
            ])
        );
        assert_eq!(g[entry].successors.len(), 2);

        // Check latching
        let latching = g[entry].successors[0];
        assert_eq!(
            g[latching].value,
            Structure::Block(vec![
                JVMInstruction::Iinc {
                    index: 0,
                    value: -1
                },
                JVMInstruction::Goto(-8),
            ])
        );
        assert_eq!(g[latching].successors, [entry]);

        // Check follow
        let follow = g[entry].successors[1];
        assert_eq!(
            g[follow].value,
            Structure::Block(vec![JVMInstruction::Iload0, JVMInstruction::Ireturn])
        );

        Ok(())
    }

    #[test]
    fn basic_blocks_post_tested_loop() -> anyhow::Result<()> {
        let g = load_basic_blocks("do { n--; } while (n > 1); return n;")?;
        assert_eq!(g.len(), 2);

        // Check entrypoint/header/latching
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::Block(vec![
                JVMInstruction::Iinc {
                    index: 0,
                    value: -1
                },
                JVMInstruction::Iload0,
                JVMInstruction::Iconst1,
                JVMInstruction::IfIcmpgt(-5),
            ])
        );
        assert_eq!(g[entry].successors.len(), 2);
        assert_eq!(g[entry].successors[1], entry);

        // Check follow
        let follow = g[entry].successors[0];
        assert_eq!(
            g[follow].value,
            Structure::Block(vec![JVMInstruction::Iload0, JVMInstruction::Ireturn])
        );

        Ok(())
    }

    #[test]
    fn basic_blocks_nested_loops() -> anyhow::Result<()> {
        let g = load_basic_blocks(
            "do {
                while (n > 2) { n--; }
            } while (n > 1);
            return n;",
        )?;
        assert_eq!(g.len(), 4);

        // Check entry/inner-loop header
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::Block(vec![
                JVMInstruction::Iload0,
                JVMInstruction::Iconst2,
                JVMInstruction::IfIcmple(9),
            ])
        );
        assert_eq!(g[entry].successors.len(), 2);

        // Check inner-loop latching
        let inner_latching = g[entry].successors[0];
        assert_eq!(
            g[inner_latching].value,
            Structure::Block(vec![
                JVMInstruction::Iinc {
                    index: 0,
                    value: -1
                },
                JVMInstruction::Goto(-8),
            ])
        );
        assert_eq!(g[inner_latching].successors, [entry]);

        // Check inner-loop follow/outer-loop latching
        let outer_latching = g[entry].successors[1];
        assert_eq!(
            g[outer_latching].value,
            Structure::Block(vec![
                JVMInstruction::Iload0,
                JVMInstruction::Iconst1,
                JVMInstruction::IfIcmpgt(-13)
            ])
        );
        assert_eq!(g[outer_latching].successors.len(), 2);
        assert_eq!(g[outer_latching].successors[1], entry);

        // Check outer-loop follow
        let outer_follow = g[outer_latching].successors[0];
        assert_eq!(
            g[outer_follow].value,
            Structure::Block(vec![JVMInstruction::Iload0, JVMInstruction::Ireturn])
        );

        Ok(())
    }

    #[test]
    fn placeholders_pre_tested_at_start_of_post_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "do {
                while (n > 2) { n--; }
            } while (n > 1);
            return n;",
        )?;

        // Check before inserting placeholders
        assert_eq!(g.len(), 4);
        // There are 2 loops here, so ideally our derived sequence will have 3 graphs: one for each
        // loop and a trivial one at the end marking reducibility
        let (G, _) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 2);

        g.insert_placeholder_nodes();

        // Check after inserting placeholders
        assert_eq!(g.len(), 5);
        let (G, _) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 3);

        // Check entry (should be placeholder)
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].value, Structure::Block(vec![]));
        assert_eq!(g[entry].successors.len(), 1);

        // Check edges
        let inner_header = g[entry].successors[0];
        assert_eq!(g[inner_header].successors.len(), 2);
        let inner_latching = g[inner_header].successors[0];
        assert_eq!(g[inner_latching].successors, [inner_header]);
        let outer_latching = g[inner_header].successors[1];
        assert_eq!(g[outer_latching].successors.len(), 2);
        let outer_follow = g[outer_latching].successors[0];
        assert_eq!(g[outer_follow].successors.len(), 0);
        assert_eq!(g[outer_latching].successors[1], entry);

        Ok(())
    }

    #[test]
    fn placeholders_if_else_at_end_of_pre_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "while (n > 1) {
                if (n > 2) { n -= 2; } else { n--; }
            }
            return n;",
        )?;

        // Check placeholder inserted
        assert_eq!(g.len(), 5);
        g.insert_placeholder_nodes();
        assert_eq!(g.len(), 6);

        // Check entrypoint/loop header
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::Block(vec![
                JVMInstruction::Iload0,
                JVMInstruction::Iconst1,
                JVMInstruction::IfIcmple(20),
            ])
        );
        assert_eq!(g[entry].successors.len(), 2);

        // Check 2-way conditional header
        let conditional_header = g[entry].successors[0];
        assert_eq!(
            g[conditional_header].value,
            Structure::Block(vec![
                JVMInstruction::Iload0,
                JVMInstruction::Iconst2,
                JVMInstruction::IfIcmple(9),
            ])
        );
        assert_eq!(g[conditional_header].successors.len(), 2);

        // Check false/true branches
        let false_node = g[conditional_header].successors[0];
        let true_node = g[conditional_header].successors[1];
        assert_eq!(
            g[false_node].value,
            Structure::Block(vec![
                JVMInstruction::Iinc {
                    index: 0,
                    value: -2
                },
                JVMInstruction::Goto(-13), // Note two back edges to same node
            ])
        );
        assert_eq!(
            g[true_node].value,
            Structure::Block(vec![
                JVMInstruction::Iinc {
                    index: 0,
                    value: -1
                },
                JVMInstruction::Goto(-19) // Note two back edges to same node
            ])
        );
        assert_eq!(g[false_node].successors.len(), 1);
        assert_eq!(g[true_node].successors.len(), 1);

        // Check 2-way conditional follow (should be placeholder)
        let false_follow_node = g[false_node].successors[0];
        let true_follow_node = g[true_node].successors[0];
        assert_eq!(false_follow_node, true_follow_node);
        assert_eq!(g[false_follow_node].value, Structure::Block(vec![]));
        assert_eq!(g[false_follow_node].successors, [entry]);

        // Check loop follow
        let loop_follow = g[entry].successors[1];
        assert_eq!(
            g[loop_follow].value,
            Structure::Block(vec![JVMInstruction::Iload0, JVMInstruction::Ireturn])
        );

        Ok(())
    }

    #[test]
    fn placeholder_if_else_at_end_of_pre_tested_at_start_of_post_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "do {
                while (n > 1) {
                    if (n > 2) { n -= 2; } else { n--; };
                }
            } while (n > 3);
            return n;",
        )?;

        // Check placeholders inserted
        assert_eq!(g.len(), 6);
        let (G, _) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 2);
        g.insert_placeholder_nodes();
        assert_eq!(g.len(), 8);
        let (G, _) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 3 /* inner loop, outer loop, trivial graph */);

        // Check edges
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].value, Structure::Block(vec![])); // placeholder
        assert_eq!(g[entry].successors.len(), 1);
        let inner_loop_header = g[entry].successors[0];
        assert_eq!(g[inner_loop_header].successors.len(), 2);
        let conditional_header = g[inner_loop_header].successors[0];
        let outer_loop_latching = g[inner_loop_header].successors[1];
        assert_eq!(g[conditional_header].successors.len(), 2);
        let false_node = g[conditional_header].successors[0];
        let true_node = g[conditional_header].successors[1];
        assert_eq!(g[false_node].successors.len(), 1);
        assert_eq!(g[true_node].successors.len(), 1);
        let false_follow = g[false_node].successors[0];
        let true_follow = g[true_node].successors[0];
        assert_eq!(false_follow, true_follow);
        assert_eq!(g[false_follow].value, Structure::Block(vec![])); // placeholder
        assert_eq!(g[false_follow].successors, [inner_loop_header]);
        assert_eq!(g[outer_loop_latching].successors.len(), 2);
        assert_eq!(g[outer_loop_latching].successors[1], entry);

        Ok(())
    }

    #[test]
    fn placeholder_if_at_end_of_pre_tested_at_start_of_post_tested() -> anyhow::Result<()> {
        let mut g = load_basic_blocks(
            "do {
                while (n > 2) {
                    if (n > 1) { n--; };
                }
            } while (n > 3);
            return n;",
        )?;

        // Check placeholders inserted
        assert_eq!(g.len(), 5);
        let (G, _) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 2);
        g.insert_placeholder_nodes();
        assert_eq!(g.len(), 7);
        let (G, _) = g.intervals_derived_sequence();
        assert_eq!(G.len(), 3 /* inner loop, outer loop, trivial graph */);

        // Check edges
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].value, Structure::Block(vec![])); // placeholder
        assert_eq!(g[entry].successors.len(), 1);
        let inner_loop_header = g[entry].successors[0];
        assert_eq!(g[inner_loop_header].successors.len(), 2);
        let conditional_header = g[inner_loop_header].successors[0];
        let outer_loop_latching = g[inner_loop_header].successors[1];
        assert_eq!(g[conditional_header].successors.len(), 2);
        let false_node = g[conditional_header].successors[0];
        let true_node = g[conditional_header].successors[1];
        assert_eq!(g[false_node].successors.len(), 1);
        let false_follow = g[false_node].successors[0];
        assert_eq!(false_follow, true_node); // Note conditional true branch is follow directly
        assert_eq!(g[false_follow].value, Structure::Block(vec![])); // placeholder
        assert_eq!(g[false_follow].successors, [inner_loop_header]);
        assert_eq!(g[outer_loop_latching].successors.len(), 2);
        assert_eq!(g[outer_loop_latching].successors[1], entry);

        Ok(())
    }
}
