use crate::function::structure::ConditionalKind;
use crate::graph::{Graph, NodeId, Order};
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use itertools::Itertools;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone)]
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

    pub fn insert_dummy_nodes(&mut self) {
        // Whenever there is a node with 2 or more back edges to it, insert a dummy node and
        // connect the back edges to it instead, then connect it to the original node.
        // This ensures a 2-way conditional's follow node is never a loop header. This breaks the
        // loop structuring algorithm, as it requires each loop to have a single back edge.
        let post_order = self.depth_first(Order::PostOrder);
        for &i in &post_order.traversal {
            let latching = self[i]
                .predecessors
                .iter()
                .filter(|&&j| post_order.cmp(j, i).is_lt())
                .copied()
                .collect_vec();

            if latching.len() >= 2 {
                let dummy = self.add_node(Structure::Block(vec![]));

                // Re-connect all of node's back edges to dummy
                for l in latching {
                    self.remove_edge(l, i);
                    self.add_edge(l, dummy);
                }

                // Connect dummy to original node
                self.add_edge(dummy, i);
            }
        }
    }
}
