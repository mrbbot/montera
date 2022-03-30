use crate::class::FunctionType;
use crate::output::builtin::{
    construct_allocate, construct_compare, construct_instanceof, BuiltinFunction,
};
use crate::output::types::EnsuredFunction;
use crate::output::Module;
use crate::virtuals::VIRTUAL_CLASS_ID_MEM_ARG;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use wasm_encoder::{
    Function as WASMFunction, GlobalType, Instruction as WASMInstruction, TypeSection, ValType,
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Ensurable {
    Type(Arc<FunctionType>),
    Dispatcher(Arc<FunctionType>),
    Builtin(BuiltinFunction),
}

fn ensure_type(
    ensured: &mut HashMap<Ensurable, u32>,
    next_type_index: &mut u32,
    types: &mut TypeSection,
    func_type: &Arc<FunctionType>,
) -> u32 {
    // Return existing type index or create a new one
    *ensured
        .entry(Ensurable::Type(Arc::clone(func_type)))
        .or_insert_with_key(|_| {
            // Write to type section
            types.function(
                func_type.params.iter().copied(),
                func_type.results.iter().copied(),
            );
            // Return and increment current type index
            let index = *next_type_index;
            *next_type_index += 1;
            index
        })
}

impl Module {
    pub(super) fn render_ensured_functions_queue(&mut self) {
        let Module {
            functions,
            codes,
            function_names,
            ..
        } = self;
        for func in self.ensured_functions.drain(..) {
            functions.function(func.type_index);
            codes.function(&func.function);
            function_names.append(func.function_index, &func.name);
        }
    }

    pub fn ensure_type(&mut self, func_type: &Arc<FunctionType>) -> u32 {
        let Module {
            ensured,
            next_type_index,
            types,
            ..
        } = self;
        ensure_type(ensured, next_type_index, types, func_type)
    }

    pub(super) fn ensure_dispatcher_function(&mut self, func_type: &Arc<FunctionType>) -> u32 {
        let Module {
            ensured,
            next_type_index,
            next_function_index,
            ensured_functions,
            types,
            ..
        } = self;
        // Return existing dispatcher function index or create a new one
        match ensured.entry(Ensurable::Dispatcher(Arc::clone(func_type))) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                // Get, store and increment current function index (do this here not on return
                // as we need mutable borrow to `ensured` later on, but need to store on
                // mutably borrowed `entry` from `ensured` here)
                let index = *next_function_index;
                *next_function_index += 1;
                entry.insert(index);

                // Get name for debug info
                let name = func_type.dispatcher_name();

                // Construct type of dispatcher function
                let mut func_type = func_type.with_implicit_this();
                // Get type without virtual offset for indirect call (clone()ing f as we still
                // need to mutate it later on, and ensure_type may take ownership if type hasn't
                // been inserted yet)
                let original_func_type = Arc::new(func_type.clone());
                let original_type_index =
                    ensure_type(ensured, next_type_index, types, &original_func_type);
                // Get number of parameters that should be copied before indirect call
                let call_params_len = func_type.params.len() as u32;
                // Insert virtual method offset parameter
                func_type.params.push(ValType::I32);
                // Get type of dispatcher
                let dispatcher_type_index =
                    ensure_type(ensured, next_type_index, types, &Arc::new(func_type));

                // Construct dispatcher function code
                let mut f = WASMFunction::new(vec![]);
                // 1. Get all parameters for function indirect call
                for i in 0..call_params_len {
                    f.instruction(&WASMInstruction::LocalGet(i));
                }
                // 2. Get implicit this parameter again...
                f.instruction(&WASMInstruction::LocalGet(0));
                //     ...for extracting virtual class ID
                f.instruction(&WASMInstruction::I32Load(VIRTUAL_CLASS_ID_MEM_ARG));
                // 3. Add virtual method offset
                f.instruction(&WASMInstruction::LocalGet(call_params_len));
                f.instruction(&WASMInstruction::I32Add);
                // 4. Call correct function, using parameters from start of this call (3a)
                f.instruction(&WASMInstruction::CallIndirect {
                    ty: original_type_index,
                    table: 0,
                });
                f.instruction(&WASMInstruction::End);

                // Queue writing function to sections
                ensured_functions.push(EnsuredFunction {
                    type_index: dispatcher_type_index,
                    function_index: index,
                    function: f,
                    name,
                });

                // Return function index
                index
            }
        }
    }

    pub(super) fn ensure_builtin_function(&mut self, builtin: BuiltinFunction) -> u32 {
        let Module {
            ensured,
            next_type_index,
            next_function_index,
            next_global_index,
            ensured_functions,
            types,
            globals,
            ..
        } = self;
        // Return existing builtin function index or create a new one
        match ensured.entry(Ensurable::Builtin(builtin)) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                // Get, store and increment current function index (do this here not on return
                // as we need mutable borrow to `ensured` later on, but need to store on
                // mutably borrowed `entry` from `ensured` here)
                //
                // NOTE: this is only ok because we're only writing a single built-in function,
                // if built-in functions needed to ensure other functions we'd have problems
                let index = *next_function_index;
                *next_function_index += 1;
                entry.insert(index);

                // Construct builtin function
                let (func_type, f) = match builtin {
                    BuiltinFunction::Allocate => {
                        // Get global for bump allocator's heap next pointer
                        let heap_next_global_index = *next_global_index;
                        *next_global_index += 1;
                        globals.global(
                            GlobalType {
                                val_type: ValType::I32,
                                mutable: true,
                            },
                            // Start at 8, so we can use 0 as null reference, whilst still being 8-byte aligned
                            &WASMInstruction::I32Const(8),
                        );
                        construct_allocate(heap_next_global_index)
                    }
                    BuiltinFunction::InstanceOf => {
                        // Get type of super ID functions: [] -> [super_vid: i32]
                        let super_id_func_type = Arc::new(FunctionType {
                            params: vec![],
                            results: vec![ValType::I32],
                        });
                        let super_id_type_index =
                            ensure_type(ensured, next_type_index, types, &super_id_func_type);
                        construct_instanceof(super_id_type_index)
                    }
                    BuiltinFunction::LongCmp => construct_compare(ValType::I64),
                    BuiltinFunction::FloatCmp => construct_compare(ValType::F32),
                    BuiltinFunction::DoubleCmp => construct_compare(ValType::F64),
                };

                // Get type of constructed function
                let type_index = ensure_type(ensured, next_type_index, types, &Arc::new(func_type));
                // Queue writing function to sections
                ensured_functions.push(EnsuredFunction {
                    type_index,
                    function_index: index,
                    function: f,
                    name: String::from(builtin.name()),
                });

                index
            }
        }
    }
}
