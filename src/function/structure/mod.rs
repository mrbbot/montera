mod basic;
mod compound;
mod loops;
mod two_way;

use crate::graph::{run_graphviz, DotOptions, NodeId, NodeMap};
use anyhow::Context;
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use itertools::Itertools;
use std::path::PathBuf;

pub use self::basic::*;
pub use self::compound::*;
pub use self::loops::*;
pub use self::two_way::*;

/// Output of [`structure_code`], containing a structured control flow graph with extracted control
/// flow constructs.
pub struct StructuredCode {
    /// Control flow graph including structured compound conditionals.
    pub g: ControlFlowGraph,
    /// Identified pre/post-tested loops in `g`, including header, latching and follow nodes.
    pub loops: NodeMap<Loop>,
    /// Maps identified 2-way conditional headers in `g` to their follow nodes.
    pub conditionals: NodeMap<NodeId>,
}

/// Structures JVM bytecode, identifying control flow constructs using the algorithms described in
/// Chapter 6 of "Cristina Cifuentes. Reverse Compilation Techniques. PhD thesis, Queensland
/// University of Technology, 1994".
///
/// See the following functions for more details on each stage of the process:
///
/// 1. [`ControlFlowGraph::insert_basic_blocks`]: finds basic blocks in bytecode
/// 2. [`ControlFlowGraph::insert_placeholder_nodes`]: inserts placeholders for loop structuring
/// 3. [`ControlFlowGraph::structure_compound_conditionals`]: rewrite irreducible short-circuit
///    patterns to single nodes
/// 4. [`ControlFlowGraph::find_loops`]: identify pre/post-tested loops
/// 5. [`ControlFlowGraph::find_2_way_conditionals`]: identify 2-way conditionals (if-statements)
///
/// If `graphs_dir` is provided, the following graphs will be rendered using Graphviz. Note this
/// significantly slows down compilation:
///
/// - `<graphs_dir>/basic.png`: after stage 1, basic blocks only
/// - `<graphs_dir>/placeholder.png`: after stage 2, basic blocks with inserted placeholder nodes
/// - `<graphs_dir>/compound.png`: after stage 3, basic blocks with rewritten short-circuit nodes
/// - `<graphs_dir>/derived.png`: after stage 3, derived sequence of intervals of control flow graph
pub fn structure_code(
    code: Vec<(usize, JVMInstruction)>,
    graphs_dir: Option<&PathBuf>,
) -> anyhow::Result<StructuredCode> {
    // Create new control flow graph and build basic blocks from function's code
    let mut g = ControlFlowGraph::new();
    g.insert_basic_blocks(code);

    // Write intermediate graph if enabled
    let dot_opts = DotOptions::default();
    if let Some(graphs_dir) = graphs_dir {
        run_graphviz(&g.as_dot(&dot_opts), graphs_dir.join("basic.png"))
            .context("Unable to render basic graph")?;
    }

    // Insert dummy nodes where nodes have 2 or more back edges to ensure each loop has a single
    // unique back edge
    g.insert_placeholder_nodes();
    if let Some(graphs_dir) = graphs_dir {
        run_graphviz(&g.as_dot(&dot_opts), graphs_dir.join("placeholder.png"))
            .context("Unable to render placeholder graph")?;
    }

    // Combine short-circuit conditionals in single nodes
    g.structure_compound_conditionals();
    // Write intermediate graph if enabled
    if let Some(graphs_dir) = graphs_dir {
        run_graphviz(&g.as_dot(&dot_opts), graphs_dir.join("compound.png"))
            .context("Unable to render compound graph")?;
    }

    // Write derived sequence of graphs if enabled
    if let Some(graphs_dir) = graphs_dir {
        run_graphviz(&derived_sequence_as_dot(&g), graphs_dir.join("derived.png"))
            .context("Unable to render derived sequence graph")?;
    }

    // Structure loops, finding header, latching & follow nodes (ensures flow graph is reducible)
    let loops = g.find_loops()?;

    // Structure conditionals, excluding loop headers/latching nodes, but including short-circuit
    // conditionals from earlier
    let ignored_headers = loops
        .values()
        .map(|l| match l.kind {
            LoopKind::PreTested => l.header,
            LoopKind::PostTested => l.latching,
        })
        .collect();
    let conditionals = g.find_2_way_conditionals(&ignored_headers);

    let structured = StructuredCode {
        g,
        loops,
        conditionals,
    };
    Ok(structured)
}

fn derived_sequence_as_dot(g: &ControlFlowGraph) -> String {
    #[allow(non_snake_case)]
    let (G, _) = g.intervals_derived_sequence();
    let dots = G.iter().enumerate().map(|(i, g)| {
        g.as_dot(&DotOptions {
            subgraph: Some(i),
            ..Default::default()
        })
    });
    let dot = format!("digraph {{\n{}\n}}\n", dots.format("\n"));
    dot
}
