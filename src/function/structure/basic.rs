use crate::function::structure::ConditionalKind;
use crate::graph::{remove_element, Graph, NodeId, NodeOrder, Order};
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use std::collections::HashMap;
use std::fmt;

pub enum Structure {
    Block(Vec<JVMInstruction>),
    CompoundConditional {
        kind: ConditionalKind,
        left_negated: bool,
        left: Box<Structure>,
        right: Box<Structure>,
    },
}

impl Default for Structure {
    fn default() -> Self {
        // Doesn't allocate until items are added to the block
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

pub type ControlFlowGraph = Graph<Structure>;

macro_rules! match_branches {
    ($label:expr, $instruction:expr, {
        None => $no_branch:block,
        Unconditional($uncond_target:ident) => $uncond_branch:block,
        Conditional($cond_target:ident) => $cond_branch:block,
    }) => {
        match $instruction {
            JVMInstruction::IfIcmpeq(n)
            | JVMInstruction::IfIcmpne(n)
            | JVMInstruction::IfIcmplt(n)
            | JVMInstruction::IfIcmpge(n)
            | JVMInstruction::IfIcmpgt(n)
            | JVMInstruction::IfIcmple(n)
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
    #[inline]
    fn ensure_leader(&mut self, leaders: &mut HashMap<usize, NodeId>, label: usize) -> NodeId {
        *leaders
            .entry(label)
            .or_insert_with(|| self.add_node(Structure::default()))
    }

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

    fn find_latching_nodes_for(&self, post_order: &NodeOrder, header: NodeId) -> Vec<NodeId> {
        self[header]
            .predecessors
            .iter()
            .filter(|&&x| post_order.cmp(x, header).is_lt())
            .copied()
            .collect()
    }

    pub fn insert_placeholder_nodes(&mut self) {
        // Whenever there is a node with 2 or more back edges to it, we've got a problem.
        // The loop structuring algorithm requires a unique back edge for each loop.
        //
        // There are 2 cases where this will happen:
        //
        // 1. A pre and post-tested loop share a header node, with the post-tested 2-way latching
        //    node having a back edge to the header and an edge to the follow.
        //    (e.g. pre-tested at start of post-tested body, `do { while(...) {...} } while(...)` )
        // 2. A 2-way conditional has a follow node as a loop header, meaning the 2 branches
        //    converge via back edges.
        //    (e.g. if-else at end of pre-tested body, `while(...) { if(...) {...} else {...} }`)
        //

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
