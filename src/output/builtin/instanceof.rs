use crate::class::FunctionType;
use crate::virtuals::VIRTUAL_CLASS_ID_MEM_ARG;
use wasm_encoder::{BlockType, Function as WASMFunction, Instruction as WASMInstruction, ValType};

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
