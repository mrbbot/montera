use crate::class::FunctionType;
use crate::virtuals::VIRTUAL_CLASS_ID_MEM_ARG;
use wasm_encoder::{Function as WASMFunction, Instruction as WASMInstruction, ValType};

/// Constructs a function (type and body) for allocating empty memory blocks for object instances
/// on the heap. The function has the signature `[size: i32, virtual_class_id: i32] -> [ptr: i32]`.
///
/// This uses a bump allocator. The `mut i32` global variable at `heap_next_global_index` points
/// to the next free address in hte heap. On allocation, the current value of this variable is
/// returned (start of block) and incremented by the desired size of the block. This allocator
/// is very fast, but no garbage collection is performed.
///
/// This function will also store the 4 byte `virtual_class_id` at the start of the block to
/// identify the instance type.
pub fn construct_allocate(heap_next_global_index: u32) -> (FunctionType, WASMFunction) {
    let func_type = FunctionType {
        params: vec![ValType::I32, ValType::I32], // [size: i32, virtual_class_id: i32]
        results: vec![ValType::I32],              // [ptr: i32]
    };
    let mut f = WASMFunction::new(vec![]);
    f.instruction(&WASMInstruction::GlobalGet(heap_next_global_index))
        // 1. Store virtual class ID, so we can identify this class at runtime
        .instruction(&WASMInstruction::LocalGet(/* virtual class ID */ 1))
        .instruction(&WASMInstruction::I32Store(VIRTUAL_CLASS_ID_MEM_ARG))
        // 2. Get current next pointer twice, so we return its value before incrementing
        .instruction(&WASMInstruction::GlobalGet(heap_next_global_index))
        // 3. Increment next pointer by size bytes
        .instruction(&WASMInstruction::GlobalGet(heap_next_global_index))
        .instruction(&WASMInstruction::LocalGet(/* size */ 0))
        .instruction(&WASMInstruction::I32Add)
        .instruction(&WASMInstruction::GlobalSet(heap_next_global_index))
        // 4. Return next pointer before increment
        .instruction(&WASMInstruction::End);
    (func_type, f)
}

#[cfg(test)]
mod tests {
    use crate::output::builtin::BuiltinFunction;
    use crate::tests::{construct_builtin_module, WASM_ENGINE};
    use std::convert::TryInto;
    use wasmtime::{Linker, Module, Store};

    #[test]
    fn allocate() -> anyhow::Result<()> {
        // Instantiate WebAssembly module
        let module = construct_builtin_module(&[BuiltinFunction::Allocate]);
        let module = Module::new(&WASM_ENGINE, module.finish())?;
        let linker = Linker::new(&WASM_ENGINE);
        let mut store = Store::new(&WASM_ENGINE, 0);
        let instance = linker.instantiate(&mut store, &module)?;

        // Get references to exports
        let allocate = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "!Allocate")?;
        let memory = instance.get_memory(&mut store, "memory").unwrap();

        // Check correct pointer returned
        let p1 = allocate.call(&mut store, (/* size */ 16, /* virtual class ID */ 42))? as usize;
        let p2 = allocate.call(&mut store, (/* size */ 10, /* virtual class ID */ 5000))? as usize;
        assert_eq!(p1, 8);
        assert_eq!(p2, 8 + 16);

        // Check virtual class IDs stored
        let data = memory.data_mut(&mut store);
        let vid1 = i32::from_le_bytes(data[p1..p1 + 4].try_into().unwrap());
        let vid2 = i32::from_le_bytes(data[p2..p2 + 4].try_into().unwrap());
        assert_eq!(vid1, 42);
        assert_eq!(vid2, 5000);

        Ok(())
    }
}
