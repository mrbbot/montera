mod basic;
mod compound;
mod loops;
mod two_way;

use crate::graph::{run_graphviz, DotOptions, NodeId, NodeMap};
use anyhow::Context;
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use std::path::PathBuf;

pub use self::basic::*;
pub use self::compound::*;
pub use self::loops::*;
pub use self::two_way::*;

pub struct StructuredCode {
    pub g: ControlFlowGraph,
    pub loops: NodeMap<Loop>,
    pub conditionals: NodeMap<NodeId>,
}

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
    let dot = format!("digraph {{\n{}\n}}\n", itertools::join(dots, "\n"));
    dot
}
