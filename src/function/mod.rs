mod locals;
pub mod structure;
mod types;
mod visitor;

pub use self::types::*;
use crate::function::locals::LocalInterpretation;
use crate::function::structure::structure_code;
use crate::function::visitor::Visitor;
use crate::scheduler::Job;
use classfile_parser::method_info::MethodAccessFlags;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Worker thread job for compiling a JVM bytecode function to WebAssembly with pseudo-instructions.
///
/// See [`CompiledFunction`] for the output and [`Instruction`] for details on pseudo-instructions.
///
/// If the function doesn't have any code (i.e. `native` or `abstract`), no compilation will take
/// place, but a `CompiledFunction` will still be sent on the
/// results channel.
pub struct CompileFunctionJob {
    /// JVM bytecode function to compile.
    pub function: Arc<Function>,
    /// Render intermediate control flow graphs using Graphviz to the specific directory (if any).
    /// See [`structure_code`] for details on rendered graphs.
    pub graphs_dir: Option<PathBuf>,
    /// Channel to send compilation result back to the main thread on.
    pub result_tx: Sender<anyhow::Result<CompiledFunction>>,
}

impl Job for CompileFunctionJob {
    fn process(&self) {
        let result = self.compile_function();
        self.result_tx.send(result).unwrap();
    }
}

impl CompileFunctionJob {
    /// Compiles this job's JVM bytecode [`Function`] to WebAssembly with pseudo-[`Instruction`]s.
    fn compile_function(&self) -> anyhow::Result<CompiledFunction> {
        let f = self.function.as_ref();

        let (code, locals) = match f.code.lock().unwrap().take() {
            // Compile code if this is a non-native/abstract function
            Some(code) => {
                // Remap locals
                let is_static = f.flags.contains(MethodAccessFlags::STATIC);
                let locals = Arc::new(LocalInterpretation::from_code(
                    is_static,
                    &f.descriptor.params,
                    &code,
                ));

                // Structure the function's code
                let len = code.len();
                let structure = structure_code(code, self.graphs_dir.as_ref())?;

                // Visit control flow graph to produce WebAssembly instructions,
                // pre-allocating 1.25x the number of JVM instructions for WebAssembly ones
                let mut out = Vec::with_capacity(((len as f32) * 1.25) as usize);
                let visitor = Visitor {
                    const_pool: Arc::clone(&f.const_pool),
                    locals: Arc::clone(&locals),
                    code: structure,
                };
                visitor.visit_all(&mut out)?;

                // TODO: instrument shadow stack here

                (Some(out), Some(locals))
            }
            None => (None, None),
        };

        // Even if this function doesn't have code, convert it to a compiled function
        let func = CompiledFunction {
            id: f.id.clone(),
            flags: f.flags,
            descriptor: Arc::clone(&f.descriptor),
            locals,
            code,
        };
        Ok(func)
    }
}
