use crate::class::{FieldId, MethodId, JAVA_LANG_OBJECT};
use crate::function::{CompiledFunction, Instruction};
use crate::output::builtin::BuiltinFunction;
use crate::virtuals::VIRTUAL_CLASS_ID_SIZE;
use crate::{Class, Module, VirtualTable};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::mem::take;
use std::rc::Rc;
use std::sync::Arc;
use wasm_encoder::{
    EntityType, Export, Function as WASMFunction, Instruction as WASMInstruction, MemArg, ValType,
};

/// WebAssembly generation rendering phase operating on the whole program.
/// Performed on the main thread once all functions have been compiled by
/// [`crate::function::CompileFunctionJob`].
///
/// The rendering phase takes structured functions, a virtual table, and produces an executable
/// WebAssembly [`Module`] for optimisation and writing. This requires lowering pseudo-instructions
/// to real WebAssembly instructions using program-wide information. Appropriate built-in functions
/// and virtual dispatcher functions will be included in the final module.
pub struct Renderer {
    classes: Arc<HashMap<Arc<String>, Class>>,
    virtual_table: Rc<VirtualTable>,
    functions: Vec<CompiledFunction>,
    /// Maps user-defined methods to their function index in the final module. Populated by
    /// [`Renderer::index_functions`].
    function_indices: HashMap<MethodId, u32>,
}

impl Renderer {
    /// Constructs a new renderer, with an empty mapping between user-defined methods and their
    /// function indices in the final module.
    pub fn new(
        classes: Arc<HashMap<Arc<String>, Class>>,
        virtual_table: Rc<VirtualTable>,
        functions: Vec<CompiledFunction>,
    ) -> Self {
        Self {
            classes,
            virtual_table,
            functions,
            function_indices: HashMap::new(),
        }
    }

    /// Assign an index to each user-defined function, placing imports first as required by WASM.
    ///
    /// This must be called before [`Renderer::render`], as the index of a function must be known
    /// to `call` it.
    fn index_functions(&mut self, out: &mut Module) {
        // Sort functions alphabetically, with imports first as required by WebAssembly
        self.functions.sort_by(|a, b| {
            let a_import = a.is_import();
            let b_import = b.is_import();
            if a_import == b_import {
                a.id.cmp(&b.id)
            } else if a_import {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        });
        // Assign functions an index, this will be the order they're rendered in the module
        debug!("Function Identifiers:");
        for (i, func) in self.functions.iter().enumerate() {
            assert_eq!(i, out.next_function_index as usize);
            debug!("{:>4}: {}", i, func.id);
            // MethodId are a collection of Arc's so clone() is cheap
            self.function_indices
                .insert(func.id.clone(), out.next_function_index);
            // Record method name for debug info
            out.function_names
                .append(out.next_function_index, &func.id.name());
            out.next_function_index += 1;
        }
    }

    /// Renders a WebAssembly import (external method) to the module.
    fn render_import(&self, out: &mut Module, func: CompiledFunction) {
        let name = format!("{}", func.id);
        // Get the index corresponding to this import's function type
        let type_index = out.ensure_type(&func.descriptor.function_type);
        // Write the named import to the module with the required type
        let import_type = EntityType::Function(type_index);
        out.imports.import("imports", Some(&name), import_type);
    }

    /// Renders an abstract function (without an implementation) to WebAssembly as an `unreachable`.
    fn render_abstract(&self, out: &mut Module, func: CompiledFunction) {
        // Static functions cannot be abstract
        assert!(!func.is_static());

        // Create unreachable function with no locals
        let mut f = WASMFunction::new(vec![]);
        f.instruction(&WASMInstruction::Unreachable)
            .instruction(&WASMInstruction::End);

        // Render function to module
        let func_type = Arc::new(func.descriptor.function_type.with_implicit_this());
        let type_index = out.ensure_type(&func_type);
        out.functions.function(type_index);
        out.codes.function(&f);
    }

    /// Computes the total size of the named class's fields, including subclasses' and the virtual
    /// class ID.
    fn get_class_size<'a>(&'a self, mut class_name: &'a Arc<String>) -> i32 {
        let mut size = VIRTUAL_CLASS_ID_SIZE; // First 4 bytes for virtual class ID
        while class_name.as_str() != JAVA_LANG_OBJECT {
            let class = &self.classes[class_name];
            size += class.size;
            class_name = &class.super_class_name;
        }
        i32::try_from(size).expect("Class size exceeded i32 bounds")
    }

    /// Returns the WebAssembly type, memory offset and alignment immediates for a class field.
    fn get_field_offset(&self, id: &FieldId) -> (ValType, MemArg) {
        // Find field in inheritance tree, starting with ID's class_name. Normally, the class_name
        // is the calling class, not the superclass the field was defined in. However, if a field has
        // the same name as a field in a superclass, the superclass will be used as the class name
        // if accessing the "hidden" field: https://docs.oracle.com/javase/tutorial/java/IandI/hidevariables.html
        let mut class_name = &id.class_name;
        let mut offset = None;
        while offset.is_none() {
            let class = &self.classes[class_name];
            offset = class.field_offsets.get(&id.name);
            class_name = &class.super_class_name;
        }
        let mut offset = *offset.unwrap() as u32;

        // Add size of all remaining superclasses' sizes to offset
        while class_name.as_str() != JAVA_LANG_OBJECT {
            let class = &self.classes[class_name];
            offset += class.size;
            class_name = &class.super_class_name;
        }

        // Add virtual class ID size to offset
        offset += VIRTUAL_CLASS_ID_SIZE;

        // Calculate field alignment in memory based on type
        let field_type = id.descriptor.as_type();
        // https://webassembly.github.io/spec/core/text/instructions.html#memory-instructions
        let align = match field_type {
            ValType::I32 | ValType::F32 => 2, // log2(4) = 2
            ValType::I64 | ValType::F64 => 3, // log2(8) = 3
            _ => unimplemented!("{:?}", field_type),
        };

        // Construct memory argument immediate containing offset
        let arg = MemArg {
            offset: offset as u64,
            align,
            memory_index: 0, // Index of memory we're addressing, not index into memory
        };

        (field_type, arg)
    }

    /// Renders a (pseudo-)instruction to a WebAssembly function body.
    ///
    /// Pseudo-instructions will likely require built-in or virtual dispatcher functions. The
    /// `Dup` instruction requires a temporary "scratch" local to duplicate from. This must be
    /// defined if `Dup` is used. See [`Instruction`] for more details on pseudo-instructions.
    ///
    /// Note [`Renderer::index_functions`] must be called before this function.
    fn render(
        &self,
        out: &mut Module,
        f: &mut WASMFunction,
        instruction: Instruction,
        scratch_local: Option<u32>,
    ) {
        match instruction {
            // Simple WebAssembly instruction, add to function directly
            Instruction::I(instruction) => f.instruction(&instruction),
            // Duplicates the value at the top of the stack
            Instruction::Dup => {
                let scratch_local = scratch_local.unwrap();
                // LocalTee is equivalent to LocalSet followed by LocalGet
                f.instruction(&WASMInstruction::LocalTee(scratch_local))
                    .instruction(&WASMInstruction::LocalGet(scratch_local))
            }
            //  Creates a new instance of the specified class on the heap returning a reference
            Instruction::New(class_name) => {
                if *class_name == "java/lang/AssertionError" {
                    // The Java standard library is not supported, but basic support is required
                    // for assertions. If we're creating an AssertionError, we've failed an
                    // assertion so the instruction following this will be a throw (which we
                    // currently translate to unreachable). Therefore, just emit null here.
                    f.instruction(&WASMInstruction::I32Const(0))
                } else {
                    let size = self.get_class_size(&class_name);
                    let virtual_class_id = self.virtual_table.get_virtual_class_id(&class_name);
                    let allocate_index = out.ensure_builtin_function(BuiltinFunction::Allocate);
                    f.instruction(&WASMInstruction::I32Const(size))
                        .instruction(&WASMInstruction::I32Const(virtual_class_id))
                        .instruction(&WASMInstruction::Call(allocate_index))
                }
            }
            // Checks if the reference is an `instanceof` the specified class
            Instruction::InstanceOf(class_name) => {
                let virtual_class_id = self.virtual_table.get_virtual_class_id(&class_name);
                let instanceof_index = out.ensure_builtin_function(BuiltinFunction::InstanceOf);
                f.instruction(&WASMInstruction::I32Const(virtual_class_id))
                    .instruction(&WASMInstruction::Call(instanceof_index))
            }
            // Gets the value of the specified field of the object reference on the top of the stack
            Instruction::GetField(id) => {
                let (field_type, arg) = self.get_field_offset(&id);
                f.instruction(&match field_type {
                    ValType::I32 => WASMInstruction::I32Load(arg),
                    ValType::I64 => WASMInstruction::I64Load(arg),
                    ValType::F32 => WASMInstruction::F32Load(arg),
                    ValType::F64 => WASMInstruction::F64Load(arg),
                    _ => unimplemented!("{:?}", field_type),
                })
            }
            // Puts the value into the specified field of the object reference on the top of the
            // stack
            Instruction::PutField(id) => {
                let (field_type, arg) = self.get_field_offset(&id);
                f.instruction(&match field_type {
                    ValType::I32 => WASMInstruction::I32Store(arg),
                    ValType::I64 => WASMInstruction::I64Store(arg),
                    ValType::F32 => WASMInstruction::F32Store(arg),
                    ValType::F64 => WASMInstruction::F64Store(arg),
                    _ => unimplemented!("{:?}", field_type),
                })
            }
            // Calls the specified static method (no dynamic dispatch), popping the required number
            // of parameters off the stack and pushing back the result
            Instruction::CallStatic(id) => {
                if *id.class_name == "java/lang/AssertionError" && id.name.as_str() == "<init>" {
                    // The Java standard library is not supported, but basic support is required
                    // for assertions. If we're constructing an AssertionError, we've failed an
                    // assertion so the instruction following this will be a throw (which we
                    // currently translate to unreachable). Therefore, just nop here.
                    f.instruction(&WASMInstruction::Nop)
                } else {
                    let index = self.function_indices[&id];
                    f.instruction(&WASMInstruction::Call(index))
                }
            }
            // Calls the specified instance method (using dynamic dispatch), popping the required
            // number of parameters off the stack (including an implicit `this` reference) and
            // pushing back the result
            Instruction::CallVirtual(id) => {
                let virtual_offset = self.virtual_table.get_method_virtual_offset(&id);
                let dispatcher_index = out.ensure_dispatcher_function(&id.descriptor.function_type);
                f.instruction(&WASMInstruction::I32Const(virtual_offset))
                    .instruction(&WASMInstruction::Call(dispatcher_index))
            }
            // Pops two `long` values `a` and `b` off the top of the stack, returning -1 if `a < b`,
            // 0 if `a = b` and 1 if `a > b`
            Instruction::LongCmp => {
                let long_cmp_index = out.ensure_builtin_function(BuiltinFunction::LongCmp);
                f.instruction(&WASMInstruction::Call(long_cmp_index))
            }
            // Pops two `float` values `a` and `b` off the top of the stack, returning -1 if `a < b`,
            // 0 if `a = b` and 1 if `a > b`. If either `a` or `b` is NaN, the result is determined
            // by the specified `NaNBehaviour`
            Instruction::FloatCmp(nan_behaviour) => {
                let float_cmp_index = out.ensure_builtin_function(BuiltinFunction::FloatCmp);
                let nan_greater = nan_behaviour.as_nan_greater_int();
                f.instruction(&WASMInstruction::I32Const(nan_greater))
                    .instruction(&WASMInstruction::Call(float_cmp_index))
            }
            // Pops two `double` values `a` and `b` off the top of the stack, returning -1 if `a < b`,
            // 0 if `a = b` and 1 if `a > b`. If either `a` or `b` is NaN, the result is determined
            // by the specified `NaNBehaviour`
            Instruction::DoubleCmp(nan_behaviour) => {
                let double_cmp_index = out.ensure_builtin_function(BuiltinFunction::DoubleCmp);
                let nan_greater = nan_behaviour.as_nan_greater_int();
                f.instruction(&WASMInstruction::I32Const(nan_greater))
                    .instruction(&WASMInstruction::Call(double_cmp_index))
            }
            // Pops two `float` values `a` and `b` off the top of the stack, returning `a % b`.
            Instruction::FloatRem => {
                let float_rem_index = out.ensure_builtin_function(BuiltinFunction::FloatRem);
                f.instruction(&WASMInstruction::Call(float_rem_index))
            }
            // Pops two `double` values `a` and `b` off the top of the stack, returning `a % b`.
            Instruction::DoubleRem => {
                let double_rem_index = out.ensure_builtin_function(BuiltinFunction::DoubleRem);
                f.instruction(&WASMInstruction::Call(double_rem_index))
            }
        };
    }

    /// Renders a WebAssembly function (with code) to the module. If the function is `public static`,
    /// it will be exported to the host.
    ///
    /// Each (pseudo-) instruction will be lowered to a real WebAssembly instruction by
    /// [`Renderer::render`].
    ///
    /// Note [`Renderer::index_functions`] must be called before this function.
    fn render_function(&self, out: &mut Module, func: CompiledFunction) {
        let is_static = func.is_static();
        let is_export = func.is_export();

        let locals = func.locals.expect("Non-imports must have locals");
        let code = func.code.expect("Non-imports must have code");

        // Check if code needs a scratch local (for Dup)
        let needs_scratch = code.iter().any(|i| matches!(i, Instruction::Dup));
        let (scratch_local, append_locals) = match needs_scratch {
            true => (Some(locals.len() as u32), vec![ValType::I32]),
            false => (None, vec![]),
        };

        // Create new function with required locals
        let locals_rle = locals.run_length_encode(&append_locals);
        let mut f = WASMFunction::new(locals_rle);

        // Write all instructions to function
        for instruction in code {
            self.render(out, &mut f, instruction, scratch_local);
        }

        // Render function to module
        let func_type = match is_static {
            true => Arc::clone(&func.descriptor.function_type),
            false => Arc::new(func.descriptor.function_type.with_implicit_this()),
        };
        let type_index = out.ensure_type(&func_type);
        out.functions.function(type_index);
        out.codes.function(&f);

        // If exported, render export to module
        if is_export {
            let name = format!("{}", func.id);
            let function_index = self.function_indices[&func.id];
            out.exports.export(&name, Export::Function(function_index));
        }
    }

    /// Renders all user-defined functions (including native imports) to the WebAssembly functions.
    pub fn render_all(mut self, out: &mut Module) -> HashMap<MethodId, u32> {
        // Sort and assign indices to functions
        self.index_functions(out);
        // Render each function, move functions out of self so we can mutably borrow again when
        // calling render_import()/render_function(). We shouldn't need them again anyways.
        for func in take(&mut self.functions) {
            if func.is_import() {
                self.render_import(out, func);
            } else if func.is_abstract() {
                self.render_abstract(out, func);
            } else {
                self.render_function(out, func);
            }
        }
        // Render any ensured functions (builtins and virtual dispatchers)
        out.render_ensured_functions_queue();
        // Return function indices for use in virtual table rendering
        self.function_indices
    }
}
