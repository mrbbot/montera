mod class;
mod function;
mod graph;
mod options;
mod output;
mod scheduler;
#[cfg(test)]
mod tests;
mod virtuals;

#[macro_use]
extern crate maplit;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use crate::class::{Class, LoadClassJob};
use crate::function::{CompileFunctionJob, CompiledFunction, Function};
use crate::graph::run_graphviz;
use crate::options::Options;
use crate::output::{Module, Renderer};
use crate::scheduler::Scheduler;
use crate::virtuals::VirtualTable;
use anyhow::Context;
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::exit;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;
use std::{fs, panic};

/// Queues jobs to load and parse all classes at `input_paths`, returning a channel to receive
/// parsed [`Class`]es on. See [`LoadClassJob`] for more details.
fn load_classes(
    schd: &impl Scheduler,
    input_paths: Vec<PathBuf>,
) -> Receiver<anyhow::Result<Class>> {
    let (class_tx, class_rx) = channel();
    for path in input_paths {
        info!("Loading {}...", path.display());
        let result_tx = class_tx.clone();
        let job = LoadClassJob { path, result_tx };
        schd.schedule(Box::new(job));
    }
    // Implicitly drop our copy of the sender, so the channel closes when all classes finish parsing
    // and drop their copies. This will terminate the returned receiver's iterator.
    class_rx
}

/// Creates a directory (and all parents) for a function's intermediate graphs.
fn create_graphs_dir(
    graphs_root_dir: Option<&PathBuf>,
    function: &Function,
) -> anyhow::Result<Option<PathBuf>> {
    let graphs_dir = graphs_root_dir.map(|d| d.join(format!("{}", function.id)));
    if let Some(graphs_dir) = &graphs_dir {
        fs::create_dir_all(graphs_dir).with_context(|| {
            format!("Unable to create graph directory: {}", graphs_dir.display())
        })?;
    }
    Ok(graphs_dir)
}

/// Queues jobs to compile all functions of [`Class`]es, returning all parsed classes, the total
/// number of functions, and a channel to receive [`CompiledFunction`]s on. If `graphs_root_dir`
/// is specified, intermediate structuring graphs will be rendered. See [`CompileFunctionJob`] for
/// more details.
fn compile_functions<'a>(
    schd: &impl Scheduler,
    graphs_root_dir: Option<&PathBuf>,
    class_count: usize,
    class_rx: Receiver<anyhow::Result<Class>>,
) -> anyhow::Result<(
    HashMap<Arc<String>, Class>,
    usize,
    Receiver<anyhow::Result<CompiledFunction>>,
)> {
    // Record all received classes for building virtual method table
    let mut classes = HashMap::with_capacity(class_count);
    let mut function_count = 0;
    let (function_tx, function_rx) = channel();

    // Enqueue function compilation jobs as classes are loaded
    for class in class_rx {
        let class = class.context("Unable to load class")?;

        // Log class if debugging
        class.dump();

        for function in &class.methods {
            info!("Compiling {}...", function.id);

            // Create directory for intermediate graphs
            let graphs_dir = create_graphs_dir(graphs_root_dir, &function)?;

            // Enqueue job for compiling function
            let result_tx = function_tx.clone();
            let job = CompileFunctionJob {
                function: Arc::clone(function),
                graphs_dir,
                result_tx,
            };
            schd.schedule(Box::new(job));
        }

        // Record function count for pre-allocating and class for building virtual method table
        function_count += class.methods.len();
        classes.insert(Arc::clone(&class.class_name), class);
    }

    Ok((classes, function_count, function_rx))
}

/// Constructs a reference-counted virtual method table from a set of parsed classes. If
/// `graphs_root_dir` is specified, the virtual table's inheritance tree will be rendered.
fn construct_virtual_table(
    graphs_root_dir: Option<&PathBuf>,
    classes: &Arc<HashMap<Arc<String>, Class>>,
) -> anyhow::Result<Rc<VirtualTable>> {
    let virtual_table = Rc::new(VirtualTable::from_classes(classes));
    if let Some(graphs_dir) = graphs_root_dir {
        let dot = virtual_table.as_dot();
        run_graphviz(&dot, graphs_dir.join("virtual.png"))
            .context("Unable to render virtual table")?;
    }
    virtual_table.dump();
    Ok(virtual_table)
}

/// Waits for the results of all function compilations, storing them in a single `Vec`.
fn collect_functions(
    function_count: usize,
    function_rx: Receiver<anyhow::Result<CompiledFunction>>,
) -> anyhow::Result<Vec<CompiledFunction>> {
    let mut functions = Vec::with_capacity(function_count);
    for function in function_rx {
        let function = function.context("Unable to compile function")?;
        functions.push(function);
    }
    Ok(functions)
}

/// Performs the rendering phase of WebAssembly generation, lowering all pseudo-instructions to real
/// WebAssembly instructions using program wide information. See [`Renderer`] for more details.
fn render_module(
    classes: Arc<HashMap<Arc<String>, Class>>,
    virtual_table: Rc<VirtualTable>,
    functions: Vec<CompiledFunction>,
) -> Module {
    info!("Rendering WebAssembly module...");
    let mut module = Module::new();

    // Render all functions to WebAssembly module
    let renderer = Renderer::new(classes, Rc::clone(&virtual_table), functions);
    let function_indices = renderer.render_all(&mut module);

    // Render virtual method table to WebAssembly module
    virtual_table.render(&mut module, &function_indices);

    module
}

/// Writes a WebAssembly module's bytes to disk, in both the binary `.wasm` and text `.wat` formats.
fn write_module(
    output_path: &PathBuf,
    wasm: &[u8],
    wasm_ext: &str,
    wat_ext: &str,
) -> anyhow::Result<()> {
    let wat = wasmprinter::print_bytes(&wasm).context("Unable to render module to text")?;
    fs::write(output_path.with_extension(wat_ext), wat).context("Unable to write text")?;
    fs::write(output_path.with_extension(wasm_ext), &wasm).context("Unable to write binary")?;
    Ok(())
}

/// Optimises a binary WebAssembly module using [Binaryen](https://github.com/WebAssembly/binaryen).
fn optimise_module(wasm: &[u8]) -> anyhow::Result<Vec<u8>> {
    info!("Optimising WebAssembly module...");
    // Optimise module using Binaryen, note this doesn't tell us what went wrong yet, see:
    // https://github.com/pepyakin/binaryen-rs/blob/5b5e4778c29fd609e7ec16956599d9bc2d2f182a/binaryen-sys/Shim.cpp#L29
    let mut binaryen_module =
        binaryen::Module::read(&wasm).map_err(|_| anyhow!("Unable to optimise module"))?;
    binaryen_module.optimize(&binaryen::CodegenConfig {
        shrink_level: 2,       // max is 2
        optimization_level: 2, // max is 4
        debug_info: false,
    });
    Ok(binaryen_module.write())
}

/// Main entrypoint for the command line interface. Compiles `.class` files to WebAssembly.
fn main() -> anyhow::Result<()> {
    // Get the current time for logging the total execution time at the end
    let start = Instant::now();
    // Parse command line arguments
    let opts = Options::parse();

    // Immediately terminate the program if any thread panics
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        default_hook(panic_info);
        exit(1);
    }));

    // Setup logger and parse command line options
    env_logger::builder().format_timestamp(None).init();

    // Initialise appropriate job scheduler
    #[cfg(feature = "parallel_scheduler")]
    let schd = {
        let workers = num_cpus::get_physical();
        info!("Using {} worker(s)...", workers);
        crate::scheduler::WorkerScheduler::new(workers)
    };
    #[cfg(not(feature = "parallel_scheduler"))]
    let schd = {
        info!("Using 1 worker...");
        crate::scheduler::SerialScheduler {}
    };

    // Queue jobs for loading input classes
    let class_count = opts.input_paths.len();
    let class_rx = load_classes(&schd, opts.input_paths);

    // Queue jobs for function compilation as classes are loaded
    let graphs_root_dir = opts.graphs_root_dir.as_ref();
    let (classes, function_count, function_rx) =
        compile_functions(&schd, graphs_root_dir, class_count, class_rx)?;

    // Construct virtual method table containing virtual class and method IDs
    let classes = Arc::new(classes);
    let virtual_table = construct_virtual_table(graphs_root_dir, &classes)?;

    // Collect function compilation results
    let functions = collect_functions(function_count, function_rx)?;

    // Render functions and virtual table to WebAssembly module
    let module = render_module(classes, virtual_table, functions);

    // Make sure output directory exists
    if let Some(parent) = opts.output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Unable to create output directory: {}", parent.display()))?;
    }

    // Write unoptimized WebAssembly module to disk in both binary and text forms
    let wasm_bytes = module.finish();
    info!("Writing unoptimised WebAssembly module...");
    write_module(&opts.output_path, &wasm_bytes, "wasm", "wat")
        .context("Unable to write unoptimised module")?;

    if opts.optimise {
        // Optimise module and write to disk in both binary and text forms
        let opt_wasm_bytes = optimise_module(&wasm_bytes)?;
        info!("Writing optimised WebAssembly module...");
        write_module(&opts.output_path, &opt_wasm_bytes, "opt.wasm", "opt.wat")
            .context("Unable to write optimised module")?;
    }

    info!("Finished in {}ms!", start.elapsed().as_millis());
    Ok(())
}
