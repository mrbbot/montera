use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[clap(version, about)]
pub struct Options {
    /// Path to output file (.wasm or .wat)
    #[clap(short = 'o', long = "output", value_name = "PATH", parse(from_os_str))]
    pub output_path: PathBuf,

    /// Optimise WebAssembly using Binaryen
    #[clap(short = 'O', long)]
    pub optimise: bool,

    /// Render intermediate control flow graphs
    #[clap(short = 'g', long = "graphs", value_name = "DIR", parse(from_os_str))]
    pub graphs_root_dir: Option<PathBuf>,

    /// Input class files (.class)
    #[clap(required = true, value_name = "CLASS", parse(from_os_str))]
    pub input_paths: Vec<PathBuf>,
}
