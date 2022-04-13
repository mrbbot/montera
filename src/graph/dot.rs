use crate::graph::Graph;
use itertools::Itertools;
use std::ffi::OsStr;
use std::fmt::Debug;
use std::io;
use std::io::Write;
use std::process::{Command, ExitStatus, Stdio};

#[derive(Default)]
pub struct DotOptions {
    /// Hide node IDs (including which node is the entrypoint) from the output
    pub omit_node_ids: bool,
    /// Hide branch indices from the output
    pub omit_branch_ids: bool,
    /// Prefix nodes with optional subgraph identifier & return a `subgraph` instead of a `digraph`
    pub subgraph: Option<usize>,
}

impl<T: Debug> Graph<T> {
    /// Converts graph to the [Graphviz DOT Language] for visualisation and debugging.
    ///
    /// See [`DotOptions`] for output format options.
    ///
    /// [Graphviz DOT Language]: https://graphviz.org/doc/info/lang.html
    //noinspection RsLiveness
    pub fn as_dot(&self, opts: &DotOptions) -> String {
        const FONT_NAME: &str = "fontname=\"Menlo\"";
        const FONT_SIZE: &str = "fontsize=\"12\"";

        // If this is a subgraph, prefix all nodes with the subgraph index
        let prefix = &opts.subgraph.map_or(String::new(), |i| format!("s{}_", i));
        // Build iterator for output separated by newlines
        let lines = self.iter().flat_map(|node| {
            // Build label for this node, optionally containing the node ID
            let label = if opts.omit_node_ids {
                format!("{value:?}", value=node.value)
            } else {
                // If we're including node IDs and this is the entrypoint, mark it with an "*"
                let entry = match self.entry {
                    Some(id) if id == node.id => "*",
                    _ => "",
                };
                format!("{id}{entry}\\n{value:?}", id = node.id, value = node.value)
            };
            // Build full, styled DOT string for this node
            let node_string = format!(
                "  {prefix}{id} [label=\"{label}\",shape=\"box\",{FONT_NAME},{FONT_SIZE}];",
                id=node.id
            );

            // Only show branch indices if this node has more than 1 outgoing edge
            let single_successor = node.out_degree() == 1;
            // Build iterator for all edges' outputs
            let edge_strings = node
                .successors
                .iter()
                .enumerate()
                .map(move |(branch, target)| {
                    // Build label for this edge, only including the branch ID if more than 1
                    // outgoing and not omitting
                    let label = if opts.omit_branch_ids || single_successor {
                        String::new()
                    } else {
                        format!("{branch}")
                    };
                    // Build full, styled DOT string for this edge
                    format!(
                        "  {prefix}{id} -> {prefix}{target} [label=\"{label}\",{FONT_NAME},{FONT_SIZE}];", 
                        id=node.id
                    )
                });

            // Output node string followed by all edges' strings
            std::iter::once(node_string).chain(edge_strings)
        });
        // Join lines with newlines characters
        let lines = lines.format("\n");

        // Wrap lines with appropriate DOT graph type
        match opts.subgraph {
            Some(i) => format!(
                "subgraph cluster_{i} {{\nlabel = \"{i}\"\n{FONT_NAME}\n{FONT_SIZE}\n{lines}\n}}\n"
            ),
            None => format!("digraph {{\n{lines}\n}}\n"),
        }
    }
}

/// Renders a Graphviz `dot` string to the specified `output` file.
///
/// This requires the `dot` executable to be accessible under the current `PATH`.
pub fn run_graphviz<S: AsRef<OsStr>>(dot: &str, output: S) -> io::Result<ExitStatus> {
    let mut process = Command::new("dot")
        .arg("-Tpng")
        .arg("-o")
        .arg(output)
        .stdin(Stdio::piped())
        .spawn()?;

    // Write dot string to stdin
    if let Some(mut stdin) = process.stdin.take() {
        stdin.write(dot.as_ref())?;
    }

    // Block waiting for rendering to complete
    process.wait()
}

#[cfg(test)]
mod tests {
    use crate::graph::tests::fixture_cyclic;
    use crate::graph::DotOptions;
    use crate::run_graphviz;
    use crate::tests::cache_path;
    use std::fs;
    use std::io::ErrorKind;

    #[test]
    fn as_dot() {
        let (g, _) = fixture_cyclic();
        let dot = g.as_dot(&DotOptions::default());
        assert_eq!(
            dot,
            "digraph {
  0 [label=\"0*\\n1\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  0 -> 0 [label=\"0\",fontname=\"Menlo\",fontsize=\"12\"];
  0 -> 1 [label=\"1\",fontname=\"Menlo\",fontsize=\"12\"];
  1 [label=\"1\\n2\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  1 -> 0 [label=\"\",fontname=\"Menlo\",fontsize=\"12\"];
}
"
        );
    }

    #[test]
    fn as_dot_omit_node_ids() {
        let (g, _) = fixture_cyclic();
        let dot = g.as_dot(&DotOptions {
            omit_node_ids: true,
            ..DotOptions::default()
        });
        assert_eq!(
            dot,
            "digraph {
  0 [label=\"1\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  0 -> 0 [label=\"0\",fontname=\"Menlo\",fontsize=\"12\"];
  0 -> 1 [label=\"1\",fontname=\"Menlo\",fontsize=\"12\"];
  1 [label=\"2\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  1 -> 0 [label=\"\",fontname=\"Menlo\",fontsize=\"12\"];
}
"
        );
    }

    #[test]
    fn as_dot_omit_branch_ids() {
        let (g, _) = fixture_cyclic();
        let dot = g.as_dot(&DotOptions {
            omit_branch_ids: true,
            ..DotOptions::default()
        });
        assert_eq!(
            dot,
            "digraph {
  0 [label=\"0*\\n1\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  0 -> 0 [label=\"\",fontname=\"Menlo\",fontsize=\"12\"];
  0 -> 1 [label=\"\",fontname=\"Menlo\",fontsize=\"12\"];
  1 [label=\"1\\n2\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  1 -> 0 [label=\"\",fontname=\"Menlo\",fontsize=\"12\"];
}
"
        );
    }

    #[test]
    fn as_dot_subgraph() {
        let (g, _) = fixture_cyclic();
        let dot = g.as_dot(&DotOptions {
            subgraph: Some(3),
            ..DotOptions::default()
        });
        assert_eq!(
            dot,
            "subgraph cluster_3 {
label = \"3\"
fontname=\"Menlo\"
fontsize=\"12\"
  s3_0 [label=\"0*\\n1\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  s3_0 -> s3_0 [label=\"0\",fontname=\"Menlo\",fontsize=\"12\"];
  s3_0 -> s3_1 [label=\"1\",fontname=\"Menlo\",fontsize=\"12\"];
  s3_1 [label=\"1\\n2\",shape=\"box\",fontname=\"Menlo\",fontsize=\"12\"];
  s3_1 -> s3_0 [label=\"\",fontname=\"Menlo\",fontsize=\"12\"];
}
"
        );
    }

    #[test]
    fn runs_graphviz() -> anyhow::Result<()> {
        // Get path to temporary output file, making sure directory exists
        let output = cache_path("run_graphviz_graph.png");
        fs::create_dir_all(output.parent().unwrap())?;

        // Delete existing file (if any), allow file not found errors here on first run
        match fs::remove_file(&output) {
            Ok(_) => (),
            Err(err) if err.kind() == ErrorKind::NotFound => (),
            Err(err) => return Err(err.into()),
        }

        // Render simple graph and make sure the output file is created
        assert!(!output.exists());
        run_graphviz("digraph { 1 }", &output)?;
        assert!(output.exists());
        Ok(())
    }
}
