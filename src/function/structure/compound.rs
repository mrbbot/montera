use crate::function::structure::{ControlFlowGraph, Structure};
use crate::graph::{Node, NodeId, Order};
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use std::fmt;
use std::mem::take;

/// Possible short-circuit conditional types for [`Structure::CompoundConditional`]s.
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

/// Helper function for [`ControlFlowGraph::structure_compound_conditionals`], returning `true` if
/// and only if the `node` is a conditional branching node (i.e. it branches based on a value).
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
    /// Helper function for [`ControlFlowGraph::structure_compound_conditionals`], replacing the
    /// node at `left_index` with a [`Structure::CompoundConditional`] using nodes at `left_index`
    /// and `right_index` as its `left` and `right` expressions, with `false_index` and `true_index`
    /// as the false and true branches respectively. The node at `right_index` will be removed.
    ///
    /// The node at `left_index` is effectively rewritten to:
    ///
    /// ```
    /// if (!)left_index &&/|| right_index { true_index } else { false_index }
    /// ```
    fn rewrite_compound_conditional(
        &mut self,
        kind: ConditionalKind,
        left_negated: bool,
        left_index: NodeId,
        right_index: NodeId,
        false_index: NodeId,
        true_index: NodeId,
    ) {
        // Extract left and right values to avoid cloning (we'll be replacing left_index and
        // removing right_index, so this is safe)
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
        self.add_edge(left_index, /* 0 */ false_index);
        self.add_edge(left_index, /* 1 */ true_index);
    }

    /// Repeatedly rewrites all short-circuit conditional patterns in this control flow graph to
    /// single [`Structure::CompoundConditional`] nodes using the algorithm described in Figure 6.34
    /// of "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland University of
    /// Technology, 1994".
    ///
    /// This is required because short-circuit constructs produce irreducible flow graphs, which
    /// would require code duplication to be represented in a structured language like WebAssembly.
    ///
    /// This should be performed before finding loops and two-way conditionals as these may use
    /// compound conditionals in their headers/latchings.
    ///
    /// ![Short Circuit Conditional Rewrite Rules](../../../images/shortcircuit.png)
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
                            // !n || e
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

#[cfg(test)]
mod tests {
    use crate::function::structure::{ConditionalKind, ControlFlowGraph, Structure};
    use crate::graph::NodeId;
    use crate::tests::load_basic_blocks;
    use classfile_parser::code_attribute::Instruction as JVMInstruction;

    fn compound_conditional_fixture() -> (ControlFlowGraph, (NodeId, NodeId, NodeId, NodeId)) {
        let mut g = ControlFlowGraph::new();
        let x = g.add_node(Structure::Block(vec![
            JVMInstruction::Iload0,
            JVMInstruction::Iconst1,
            JVMInstruction::IfIcmple(0),
        ]));
        let y = g.add_node(Structure::Block(vec![
            JVMInstruction::Iload1,
            JVMInstruction::Iconst1,
            JVMInstruction::IfIcmple(0),
        ]));
        let f = g.add_node(Structure::Block(vec![
            JVMInstruction::Iconst0,
            JVMInstruction::Ireturn,
        ]));
        let t = g.add_node(Structure::Block(vec![
            JVMInstruction::Iconst1,
            JVMInstruction::Ireturn,
        ]));
        (g, (x, y, f, t))
    }

    #[test]
    fn compound_conditional_conjunction() {
        // Construct `a && b` graph
        let (mut g, (x, y, f, t)) = compound_conditional_fixture();
        g.add_edge(x, f);
        g.add_edge(x, y);
        g.add_edge(y, f);
        g.add_edge(y, t);
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 3);
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors, [f, t]);
        assert!(matches!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: false,
                kind: ConditionalKind::Conjunction,
                ..
            }
        ));
    }

    #[test]
    fn compound_conditional_negated_conjunction() {
        // Construct `!a && b` graph
        let (mut g, (x, y, f, t)) = compound_conditional_fixture();
        g.add_edge(x, y);
        g.add_edge(x, f);
        g.add_edge(y, f);
        g.add_edge(y, t);
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 3);
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors, [f, t]);
        assert!(matches!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: true,
                kind: ConditionalKind::Conjunction,
                ..
            }
        ));
    }

    #[test]
    fn compound_conditional_disjunction() {
        // Construct `a || b` graph
        let (mut g, (x, y, f, t)) = compound_conditional_fixture();
        g.add_edge(x, y);
        g.add_edge(x, t);
        g.add_edge(y, f);
        g.add_edge(y, t);
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 3);
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors, [f, t]);
        assert!(matches!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: false,
                kind: ConditionalKind::Disjunction,
                ..
            }
        ));
    }

    #[test]
    fn compound_conditional_negated_disjunction() {
        // Construct `!a || b` graph
        let (mut g, (x, y, f, t)) = compound_conditional_fixture();
        g.add_edge(x, t);
        g.add_edge(x, y);
        g.add_edge(y, f);
        g.add_edge(y, t);
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 3);
        let entry = g.entry.unwrap();
        assert_eq!(g[entry].successors, [f, t]);
        assert!(matches!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: true,
                kind: ConditionalKind::Disjunction,
                ..
            }
        ));
    }

    fn compound_conditional_single_code(
        expression: &str,
        expected_left_negated: bool,
        expected_kind: ConditionalKind,
        expected_left_conditional_instruction: JVMInstruction,
        expected_right_conditional_instruction: JVMInstruction,
    ) -> anyhow::Result<()> {
        // Load graph containing basic blocks for expression
        let mut g = load_basic_blocks(&format!(
            "boolean a = false, b = false;
            if ({}) {{ n = 1; }} else {{ n = 0; }}
            return n;",
            expression
        ))?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 4);
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: expected_left_negated,
                kind: expected_kind,
                left: Box::new(Structure::Block(vec![
                    JVMInstruction::Iconst0,
                    JVMInstruction::Istore1,
                    JVMInstruction::Iconst0,
                    JVMInstruction::Istore2,
                    JVMInstruction::Iload1,
                    expected_left_conditional_instruction,
                ])),
                right: Box::new(Structure::Block(vec![
                    JVMInstruction::Iload2,
                    expected_right_conditional_instruction,
                ]))
            }
        );
        assert_eq!(g[entry].successors.len(), 2);
        assert_eq!(
            g[g[entry].successors[0]].value,
            Structure::Block(vec![
                JVMInstruction::Iconst1,
                JVMInstruction::Istore0,
                JVMInstruction::Goto(5),
            ])
        );
        assert_eq!(
            g[g[entry].successors[1]].value,
            Structure::Block(vec![JVMInstruction::Iconst0, JVMInstruction::Istore0])
        );

        Ok(())
    }

    #[test]
    fn compound_conditional_conjunction_code() -> anyhow::Result<()> {
        compound_conditional_single_code(
            "a && b",
            false,
            ConditionalKind::Disjunction,
            JVMInstruction::Ifeq(12),
            JVMInstruction::Ifeq(8),
        )
    }

    #[test]
    fn compound_conditional_negated_conjunction_code() -> anyhow::Result<()> {
        compound_conditional_single_code(
            "!a && b",
            false,
            ConditionalKind::Disjunction,
            JVMInstruction::Ifne(12),
            JVMInstruction::Ifeq(8),
        )
    }

    #[test]
    fn compound_conditional_disjunction_code() -> anyhow::Result<()> {
        compound_conditional_single_code(
            "a || b",
            true,
            ConditionalKind::Conjunction,
            JVMInstruction::Ifne(7),
            JVMInstruction::Ifeq(8),
        )
    }

    #[test]
    fn compound_conditional_negated_disjunction_code() -> anyhow::Result<()> {
        compound_conditional_single_code(
            "!a || b",
            true,
            ConditionalKind::Conjunction,
            JVMInstruction::Ifeq(7),
            JVMInstruction::Ifeq(8),
        )
    }

    #[test]
    fn compound_conditional_multiple_code() -> anyhow::Result<()> {
        // Load graph containing basic blocks for multiple sequential short-circuit expressions
        let mut g = load_basic_blocks(
            "boolean a = false, b = false;
            if (a && b) { n = 1; } else { n = 0; }
            if (a || b) { n = 1; } else { n = 0; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 3 + 3 + 1); // `a && b` + `a || b` + follow2
        let entry = g.entry.unwrap();
        assert!(matches!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: false,
                kind: ConditionalKind::Disjunction,
                ..
            }
        ));
        let follow1 = g[g[entry].successors[0]].successors[0];
        assert!(matches!(
            g[follow1].value,
            Structure::CompoundConditional {
                left_negated: true,
                kind: ConditionalKind::Conjunction,
                ..
            }
        ));

        Ok(())
    }

    #[test]
    fn compound_conditional_nested_code() -> anyhow::Result<()> {
        // Load graph containing basic blocks for nested short-circuit expression
        let mut g = load_basic_blocks(
            "boolean a = false, b = false, c = false, d = false;
            if (((a || b) && c) || (a && d)) { n = 1; } else { n = 0; }
            return n;",
        )?;
        g.insert_placeholder_nodes();
        g.structure_compound_conditionals();

        // Check graph rewritten correctly
        assert_eq!(g.len(), 4);
        let entry = g.entry.unwrap();
        assert_eq!(
            g[entry].value,
            Structure::CompoundConditional {
                left_negated: true,
                kind: ConditionalKind::Conjunction,
                left: Box::new(Structure::CompoundConditional {
                    left_negated: true,
                    kind: ConditionalKind::Conjunction,
                    left: Box::new(Structure::CompoundConditional {
                        left_negated: true,
                        kind: ConditionalKind::Conjunction,
                        left: Box::new(Structure::Block(vec![
                            JVMInstruction::Iconst0,
                            JVMInstruction::Istore1,
                            JVMInstruction::Iconst0,
                            JVMInstruction::Istore2,
                            JVMInstruction::Iconst0,
                            JVMInstruction::Istore3,
                            JVMInstruction::Iconst0,
                            JVMInstruction::Istore(4),
                            JVMInstruction::Iload1,
                            JVMInstruction::Ifne(7),
                        ])),
                        right: Box::new(Structure::Block(vec![
                            JVMInstruction::Iload2,
                            JVMInstruction::Ifeq(7),
                        ]))
                    }),
                    right: Box::new(Structure::Block(vec![
                        JVMInstruction::Iload3,
                        JVMInstruction::Ifne(12),
                    ]))
                }),
                right: Box::new(Structure::CompoundConditional {
                    left_negated: false,
                    kind: ConditionalKind::Disjunction,
                    left: Box::new(Structure::Block(vec![
                        JVMInstruction::Iload1,
                        JVMInstruction::Ifeq(13),
                    ])),
                    right: Box::new(Structure::Block(vec![
                        JVMInstruction::Iload(4),
                        JVMInstruction::Ifeq(8),
                    ]))
                }),
            }
        );

        Ok(())
    }
}
