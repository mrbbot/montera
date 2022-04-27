use crate::output::ensure::Ensurable;
use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, ElementSection, Export, ExportSection, Function as WASMFunction, FunctionSection,
    GlobalSection, ImportSection, MemorySection, MemoryType, Module as WASMModule, NameMap,
    NameSection, TableSection, TypeSection,
};

/// Function that another function wants to **ensure** exists once in the output module.
/// This represents a function dependency.
pub(super) struct EnsuredFunction {
    /// Index of the WebAssembly function type in the output module.
    pub type_index: u32,
    /// Index of the WebAssembly function body in the output module.
    pub function_index: u32,
    /// WebAssembly function body.
    pub function: WASMFunction,
    /// Debug name of this function. Should start with an `!` indicating a system-defined function.
    pub name: String,
}

/// Output WebAssembly module including types, functions, memory and tables.
///
/// This has the following structure:
///
/// - Function Type Declarations (Type Section)
/// - User Imports (Import Section)
/// - User Functions (Function Section)
/// - Built-in/Dispatcher Functions (Function Section)
/// - Super Virtual ID Functions (Function Section)
/// - Table Declaration (Table Section)
/// - Memory Declaration (Memory Section)
/// - Virtual Table Elements (Element Section)
/// - Function Code (Code Section)
/// - Debug Function Names (Name Section)
///
/// When rendering user functions, we don't know what built-ins/dispatchers future functions will
/// require. We also need known indices for each user function so future functions can be called.
/// This means built-ins/dispatchers must come after user functions.
pub struct Module {
    /// Dependencies of user-functions already added to the module. This maps values are either
    /// type indices (for [`Ensurable::Type`]) or function indices (for [`Ensurable::Dispatcher`] or
    /// [`Ensurable::Builtin`]).
    pub(super) ensured: HashMap<Ensurable, u32>,
    /// Index in the module of the next added ensured function type.
    pub(super) next_type_index: u32,
    /// Index in the module of the next added function.
    pub next_function_index: u32,
    /// Index in the module of the next added global variable.
    pub(super) next_global_index: u32,
    /// Instead of directly writing ensured functions to the function/code sections, delay writing
    /// them until all user functions have been written so we can predict their IDs for calls.
    pub(super) ensured_functions: Vec<EnsuredFunction>,
    /// Debug names for each function, used in WebAssembly text output.
    pub function_names: NameMap,

    // https://webassembly.github.io/spec/core/binary/modules.html#sections
    pub types: TypeSection,         // 1
    pub imports: ImportSection,     // 2
    pub functions: FunctionSection, // 3
    pub tables: TableSection,       // 4
    pub memories: MemorySection,    // 5
    pub globals: GlobalSection,     // 6
    pub exports: ExportSection,     // 7
    pub elements: ElementSection,   // 9
    pub codes: CodeSection,         // 10
}

impl Module {
    /// Constructs a new empty module, with an empty heap memory.
    pub fn new() -> Self {
        let mut module = Self {
            ensured: HashMap::new(),
            next_type_index: 0,
            next_function_index: 0,
            next_global_index: 0,
            ensured_functions: Vec::new(),

            types: TypeSection::new(),
            imports: ImportSection::new(),
            functions: FunctionSection::new(),
            function_names: NameMap::new(),
            tables: TableSection::new(),
            memories: MemorySection::new(),
            globals: GlobalSection::new(),
            exports: ExportSection::new(),
            elements: ElementSection::new(),
            codes: CodeSection::new(),
        };
        module.add_heap();
        module
    }

    /// Adds and exports a memory for the heap to this module.
    fn add_heap(&mut self) {
        self.memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
        });
        self.exports.export("memory", Export::Memory(0));
    }

    /// Finalises this module and converts it to *unoptimised* executable bytes.
    /// This result can be written directly to a binary `.wasm` file.
    pub fn finish(self) -> Vec<u8> {
        // Build names section
        let mut names = NameSection::new();
        names.functions(&self.function_names);

        let mut module = WASMModule::new();
        // Attach sections to module
        module.section(&self.types);
        module.section(&self.imports);
        module.section(&self.functions);
        module.section(&self.tables);
        module.section(&self.memories);
        module.section(&self.globals);
        module.section(&self.exports);
        module.section(&self.elements);
        module.section(&self.codes);
        module.section(&names);
        // Convert to bytes
        module.finish()
    }
}
