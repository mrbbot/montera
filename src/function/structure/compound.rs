use crate::function::structure::{ControlFlowGraph, Structure};
use crate::graph::{Node, NodeId, Order};
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use std::fmt;
use std::mem::take;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConditionalKind {
    Disjunction, // || OR
    Conjunction, // && AND
}

impl fmt::Display for ConditionalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConditionalKind::Disjunction => write!(f, "||"),
            ConditionalKind::Conjunction => write!(f, "&&"),
        }
    }
}

fn is_conditional_branch(node: &Node<Structure>) -> bool {
    match &node.value {
        Structure::Block(instructions) => {
            // Check loads or computes something, then jumps
            instructions.len() >= 2
                && matches!(
                    instructions.last().unwrap(),
                    JVMInstruction::IfAcmpeq(_)
                        | JVMInstruction::IfAcmpne(_)
                        | JVMInstruction::IfIcmpeq(_)
                        | JVMInstruction::IfIcmpne(_)
                        | JVMInstruction::IfIcmplt(_)
                        | JVMInstruction::IfIcmpge(_)
                        | JVMInstruction::IfIcmpgt(_)
                        | JVMInstruction::IfIcmple(_)
                        | JVMInstruction::Ifeq(_)
                        | JVMInstruction::Ifne(_)
                        | JVMInstruction::Iflt(_)
                        | JVMInstruction::Ifge(_)
                        | JVMInstruction::Ifgt(_)
                        | JVMInstruction::Ifle(_)
                        | JVMInstruction::Ifnonnull(_)
                        | JVMInstruction::Ifnull(_)
                )
        }
        // Compound conditional is by definition a conditional instruction
        Structure::CompoundConditional { .. } => true,
    }
}

impl ControlFlowGraph {
    fn rewrite_compound_conditional(
        &mut self,
        // if (!)left_index &&/|| right_index { true_index } else { false_index }
        kind: ConditionalKind,
        left_negated: bool,
        left_index: NodeId,
        right_index: NodeId,
        false_index: NodeId,
        true_index: NodeId,
    ) {
        // Extract left and right values to avoid cloning
        let left_value = take(&mut self[left_index].value);
        let right_value = take(&mut self[right_index].value);

        // Replace left node with new compound node in graph
        self[left_index].value = Structure::CompoundConditional {
            kind,
            left_negated,
            left: Box::new(left_value),
            right: Box::new(right_value),
        };

        // Remove right node from graph
        self.remove_node(right_index);

        // Remove all of new node's existing outgoing edges
        self.remove_all_successors(left_index);

        // Connect new node's false/true branches to false_node/true_node respectively
        // (note the ordering of these calls is important)
        self.add_edge(left_index, false_index);
        self.add_edge(left_index, true_index);
    }

    pub fn structure_compound_conditionals(&mut self) {
        let mut change = true;
        while change {
            change = false;

            for n in self.depth_first(Order::PostOrder).traversal {
                let n_node = &self[n];

                if n_node.out_degree() == 2 {
                    let t = n_node.successors[0]; // false branch
                    let e = n_node.successors[1]; // true branch

                    let t_node = &self[t];
                    let e_node = &self[e];

                    if t_node.out_degree() == 2
                        && is_conditional_branch(t_node)
                        && t_node.in_degree() == 1
                        && t != n
                    {
                        if t_node.successors[0] == e {
                            change = true;
                            // !n && t
                            let other_t_edge = t_node.successors[1];
                            self.rewrite_compound_conditional(
                                ConditionalKind::Conjunction,
                                /* left_negated */ true,
                                /* left  */ n,
                                /* right */ t,
                                /* false */ e,
                                /* true  */ other_t_edge,
                            );
                        } else if t_node.successors[1] == e {
                            change = true;
                            // n || t
                            let other_t_edge = t_node.successors[0];
                            self.rewrite_compound_conditional(
                                ConditionalKind::Disjunction,
                                /* left_negated */ false,
                                /* left  */ n,
                                /* right */ t,
                                /* false */ other_t_edge,
                                /* true  */ e,
                            );
                        }
                    } else if e_node.out_degree() == 2
                        && is_conditional_branch(e_node)
                        && e_node.in_degree() == 1
                        && e != n
                    {
                        if e_node.successors[0] == t {
                            change = true;
                            // n && e
                            let other_e_edge = e_node.successors[1];
                            self.rewrite_compound_conditional(
                                ConditionalKind::Conjunction,
                                /* left_negated */ false,
                                /* left  */ n,
                                /* right */ e,
                                /* false */ t,
                                /* true  */ other_e_edge,
                            );
                        } else if e_node.successors[1] == t {
                            change = true;
                            let other_e_edge = e_node.successors[0];
                            self.rewrite_compound_conditional(
                                ConditionalKind::Disjunction,
                                /* left_negated */ true,
                                /* left  */ n,
                                /* right */ e,
                                /* false */ other_e_edge,
                                /* true  */ t,
                            );
                        }
                    }
                }
            }
        }
    }
}
