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
