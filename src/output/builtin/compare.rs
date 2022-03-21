use crate::class::FunctionType;
use wasm_encoder::{BlockType, Function as WASMFunction, Instruction as WASMInstruction, ValType};

struct CompareInstructions<'a> {
    gt: WASMInstruction<'a>,
    eq: WASMInstruction<'a>,
    lt: WASMInstruction<'a>,
    has_nan: bool,
}

impl CompareInstructions<'_> {
    fn from_type(t: ValType) -> Self {
        match t {
            ValType::I64 => CompareInstructions {
                gt: WASMInstruction::I64GtS,
                eq: WASMInstruction::I64Eq,
                lt: WASMInstruction::I64LtS,
                has_nan: false,
            },
            ValType::F32 => CompareInstructions {
                gt: WASMInstruction::F32Gt,
                eq: WASMInstruction::F32Eq,
                lt: WASMInstruction::F32Lt,
                has_nan: true,
            },
            ValType::F64 => CompareInstructions {
                gt: WASMInstruction::F64Gt,
                eq: WASMInstruction::F64Eq,
                lt: WASMInstruction::F64Lt,
                has_nan: true,
            },
            _ => unreachable!("Expected ValType::I64/ValType::F32/ValType::F64"),
        }
    }
}

pub fn construct_compare(t: ValType) -> (FunctionType, WASMFunction) {
    let cmp = CompareInstructions::from_type(t);
    let func_type = FunctionType {
        params: if cmp.has_nan {
            vec![t, t, ValType::I32] // [a: t, b: t, nan_greater: i32]
        } else {
            vec![t, t] // [a: t, b: t]
        },
        results: vec![ValType::I32], // [ord: i32]
    };
    let mut f = WASMFunction::new(vec![]);

    // 1. Return 1 if a > b
    f.instruction(&WASMInstruction::LocalGet(0))
        .instruction(&WASMInstruction::LocalGet(1))
        .instruction(&cmp.gt);
    f.instruction(&WASMInstruction::If(BlockType::Empty));
    {
        f.instruction(&WASMInstruction::I32Const(1))
            .instruction(&WASMInstruction::Return);
    }
    f.instruction(&WASMInstruction::End);

    // 2. Return 0 if a = b
    f.instruction(&WASMInstruction::LocalGet(0))
        .instruction(&WASMInstruction::LocalGet(1))
        .instruction(&cmp.eq);
    f.instruction(&WASMInstruction::If(BlockType::Empty));
    {
        f.instruction(&WASMInstruction::I32Const(0))
            .instruction(&WASMInstruction::Return);
    }
    f.instruction(&WASMInstruction::End);

    // 3. Return -1 if a < b
    f.instruction(&WASMInstruction::LocalGet(0))
        .instruction(&WASMInstruction::LocalGet(1))
        .instruction(&cmp.lt);
    f.instruction(&WASMInstruction::If(BlockType::Empty));
    {
        f.instruction(&WASMInstruction::I32Const(-1))
            .instruction(&WASMInstruction::Return);
    }
    f.instruction(&WASMInstruction::End);

    // 4. Otherwise, one value is NaN. If treating NaNs as greater than, return 1, else -1.
    //    If this type doesn't have NaN's, something's gone wrong, this should be unreachable.
    if cmp.has_nan {
        f.instruction(&WASMInstruction::LocalGet(/* nan_greater */ 2));
        f.instruction(&WASMInstruction::If(BlockType::Result(ValType::I32)));
        {
            f.instruction(&WASMInstruction::I32Const(1));
        }
        f.instruction(&WASMInstruction::Else);
        {
            f.instruction(&WASMInstruction::I32Const(-1));
        }
        f.instruction(&WASMInstruction::End);
    } else {
        f.instruction(&WASMInstruction::Unreachable);
    }

    f.instruction(&WASMInstruction::End);

    (func_type, f)
}
