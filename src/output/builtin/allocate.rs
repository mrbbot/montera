use crate::class::FunctionType;
use crate::virtuals::VIRTUAL_CLASS_ID_MEM_ARG;
use wasm_encoder::{Function as WASMFunction, Instruction as WASMInstruction, ValType};

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
