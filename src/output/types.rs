use crate::output::ensure::Ensurable;
use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, ElementSection, Export, ExportSection, Function as WASMFunction, FunctionSection,
    GlobalSection, ImportSection, MemorySection, MemoryType, Module as WASMModule, TableSection,
    TypeSection,
};

pub struct Module {
    pub(super) ensured: HashMap<Ensurable, u32>,
    pub(super) next_type_index: u32,
    pub next_function_index: u32,
    pub(super) next_global_index: u32,
    // Instead of directly writing ensured functions to the function/code sections, delay writing
    // them until all user functions have been written so we can predict their IDs for calls
    pub(super) ensured_functions: Vec<(u32, WASMFunction)>, // (type_index, function)

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

    fn add_heap(&mut self) {
        // Create and export memory for heap
        self.memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
        });
        self.exports.export("memory", Export::Memory(0));
    }

    pub fn finish(self) -> Vec<u8> {
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
        // Convert to bytes
        module.finish()
    }
}
