use crate::class::FunctionType;
use crate::virtuals::VIRTUAL_CLASS_ID_MEM_ARG;
use wasm_encoder::{BlockType, Function as WASMFunction, Instruction as WASMInstruction, ValType};

/// Constructs a function (type and body) for checking if an object reference is an instance of a
/// target class (or any subclass of the target). This function has the signature:
/// `[ptr: i32, target_virtual_class_id: i32] -> [is: i32]`, returning `1` if `ptr` is an instance
/// of `target_virtual_class_id` and `0` otherwise.
///
/// This function will check the associated virtual class ID (or any of its superclasses) of the
/// object reference matches `target_virtual_class_id`. To get superclasses, constant super ID
/// functions (with type `[] -> [i32]` or `super_id_type_index`) at virtual class ID indices in the
/// virtual table will be called. See [`crate::virtuals::VirtualTable`] for more details.
///
/// When virtual class ID `0` (`java/lang/Object`) is reached, this function terminates with `0`.
pub fn construct_instanceof(super_id_type_index: u32) -> (FunctionType, WASMFunction) {
    let func_type = FunctionType {
        params: vec![ValType::I32, ValType::I32], // [ptr: i32, target_virtual_class_id: i32]
        results: vec![ValType::I32],              // [is: i32]
    };
    let mut f = WASMFunction::new(vec![]);

    // 1. Get virtual class ID for value, and store in value's local
    f.instruction(&WASMInstruction::LocalGet(/* ptr */ 0))
        .instruction(&WASMInstruction::I32Load(VIRTUAL_CLASS_ID_MEM_ARG))
        .instruction(&WASMInstruction::LocalSet(/* current_vid */ 0));

    f.instruction(&WASMInstruction::Loop(BlockType::Empty));
    {
        // 2. If current virtual class ID matches target class ID, return true
        f.instruction(&WASMInstruction::LocalGet(/* current_vid */ 0))
            .instruction(&WASMInstruction::LocalGet(/* target_vid */ 1))
            .instruction(&WASMInstruction::I32Eq);
        f.instruction(&WASMInstruction::If(BlockType::Empty));
        {
            f.instruction(&WASMInstruction::I32Const(/* true */ 1))
                .instruction(&WASMInstruction::Return);
        }
        f.instruction(&WASMInstruction::End);

        // 3. If current virtual class ID matches 0 (reached java/lang/Object), return false
        f.instruction(&WASMInstruction::LocalGet(/* current_vid */ 0))
            .instruction(&WASMInstruction::I32Eqz /* java/lang/Object */);
        f.instruction(&WASMInstruction::If(BlockType::Empty));
        {
            f.instruction(&WASMInstruction::I32Const(/* false */ 0))
                .instruction(&WASMInstruction::Return);
        }
        f.instruction(&WASMInstruction::End);

        // 4. Otherwise, get virtual class ID of superclass of current class, then repeat from 2.
        f.instruction(&WASMInstruction::LocalGet(/* current_vid */ 0))
            // super_id() is always the class's first entry in the virtual table
            .instruction(&WASMInstruction::CallIndirect {
                ty: super_id_type_index, // [] -> [super_vid: i32]
                table: 0,
            })
            .instruction(&WASMInstruction::LocalSet(/* current_vid */ 0))
            .instruction(&WASMInstruction::Br(0)); // Restart loop
    }
    f.instruction(&WASMInstruction::End);

    // Always getting superclass of `current class`, so should eventually reach shared base class
    // "java/lang/Object" and return 0 earlier on
    f.instruction(&WASMInstruction::Unreachable);
    f.instruction(&WASMInstruction::End);

    (func_type, f)
}

#[cfg(test)]
mod tests {
    use crate::class::FunctionType;
    use crate::output::builtin::BuiltinFunction;
    use crate::tests::{construct_builtin_module, WASM_ENGINE};
    use std::sync::Arc;
    use wasm_encoder::{
        Elements, Function as WASMFunction, Instruction as WASMInstruction, TableType, ValType,
    };
    use wasmtime::{Linker, Module, Store};

    fn render_super_id_function(out: &mut crate::Module, type_index: u32, super_id: i32) -> u32 {
        let mut f = WASMFunction::new(vec![]);
        f.instruction(&WASMInstruction::I32Const(super_id))
            .instruction(&WASMInstruction::End);
        let super_id_index = out.next_function_index;
        out.next_function_index += 1;
        out.functions.function(type_index);
        out.codes.function(&f);
        super_id_index
    }

    #[test]
    fn instanceof() -> anyhow::Result<()> {
        let mut module = construct_builtin_module(&[BuiltinFunction::InstanceOf]);

        // Insert a virtual method table corresponding to the following inheritance tree:
        //    0
        //   / \
        //  1   4
        //  |
        //  2
        //  |
        //  3
        //
        // Get type of super ID functions: [] -> [super_vid: i32]
        let super_id_func_type = Arc::new(FunctionType {
            params: vec![],
            results: vec![ValType::I32],
        });
        let super_id_type_index = module.ensure_type(&super_id_func_type);
        // Render constant functions returning 0, 1 and 2
        let super_id_0_index = render_super_id_function(&mut module, super_id_type_index, 0);
        let super_id_1_index = render_super_id_function(&mut module, super_id_type_index, 1);
        let super_id_2_index = render_super_id_function(&mut module, super_id_type_index, 2);
        let super_ids = [
            /* super(1) = 0 */ super_id_0_index,
            /* super(2) = 1 */ super_id_1_index,
            /* super(3) = 2 */ super_id_2_index,
            /* super(4) = 0 */ super_id_0_index,
        ];
        module.elements.active(
            None,
            &WASMInstruction::I32Const(1),
            ValType::FuncRef,
            Elements::Functions(&super_ids),
        );
        module.tables.table(TableType {
            element_type: ValType::FuncRef,
            minimum: 5,
            maximum: Some(5),
        });

        // Instantiate WebAssembly module
        let module = Module::new(&WASM_ENGINE, module.finish())?;
        let linker = Linker::new(&WASM_ENGINE);
        let mut store = Store::new(&WASM_ENGINE, 0);
        let instance = linker.instantiate(&mut store, &module)?;

        // Get reference to exports
        let instanceof =
            instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "!InstanceOf")?;
        let memory = instance.get_memory(&mut store, "memory").unwrap();

        // Create instance of class 2 in memory
        let ptr = 4i32;
        let vid = 2i32;
        memory.write(&mut store, ptr as usize, &vid.to_le_bytes())?;

        // Check class instanceof itself
        assert_eq!(instanceof.call(&mut store, (ptr, vid))?, 1);
        // Check class instanceof superclass
        assert_eq!(instanceof.call(&mut store, (ptr, 1))?, 1);
        // Check class instanceof java/lang/Object
        assert_eq!(instanceof.call(&mut store, (ptr, 0))?, 1);
        // Check class not instanceof subclass
        assert_eq!(instanceof.call(&mut store, (ptr, 3))?, 0);
        // Check class not instanceof of unrelated class
        assert_eq!(instanceof.call(&mut store, (ptr, 4))?, 0);

        Ok(())
    }
}
