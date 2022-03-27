use crate::graph::Graph;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io;
use std::io::Write;
use std::process::{Command, ExitStatus, Stdio};

#[derive(Default)]
pub struct DotOptions {
    pub omit_node_ids: bool,
    pub omit_branch_ids: bool,
    pub subgraph: Option<usize>,
}

impl<T: Debug> Graph<T> {
    //noinspection RsLiveness
    pub fn as_dot(&self, opts: &DotOptions) -> String {
        const FONT_NAME: &str = "fontname=\"Menlo\"";
        const FONT_SIZE: &str = "fontsize=\"12\"";

        let prefix = &opts.subgraph.map_or(String::new(), |i| format!("s{}_", i));
        let lines = self.iter().flat_map(|node| {
            let label = if opts.omit_node_ids {
                format!("{value:?}", value=node.value)
            } else {
                let entry = match self.entry {
                    Some(id) if id == node.id => "*",
                    _ => "",
                };
                format!("{id}{entry}\\n{value:?}", id = node.id, value = node.value)
            };
            let node_string = format!(
                "  {prefix}{id} [label=\"{label}\",shape=\"box\",{FONT_NAME},{FONT_SIZE}];", 
                id=node.id
            );

            let single_successor = node.out_degree() == 1;
            let edge_strings = node
                .successors
                .iter()
                .enumerate()
                .map(move |(branch, target)| {
                    let label = if opts.omit_branch_ids || single_successor {
                        String::new()
                    } else {
                        format!("{branch}")
                    };
                    format!(
                        "  {prefix}{id} -> {prefix}{target} [label=\"{label}\",{FONT_NAME},{FONT_SIZE}];", 
                        id=node.id
                    )
                });

            std::iter::once(node_string).chain(edge_strings)
        });
        let lines = itertools::join(lines, "\n");

        match opts.subgraph {
            Some(i) => format!(
                "subgraph cluster_{i} {{\nlabel = \"{i}\"\n{FONT_NAME}\n{FONT_SIZE}\n{lines}\n}}\n"
            ),
            None => format!("digraph {{\n{lines}\n}}\n"),
        }
    }
}

pub fn run_graphviz<S: AsRef<OsStr>>(dot: &str, output: S) -> io::Result<ExitStatus> {
    let mut process = Command::new("dot")
        .arg("-Tpng")
        .arg("-o")
        .arg(output)
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = process.stdin.take() {
        stdin.write(dot.as_ref())?;
    }

    process.wait()
}
