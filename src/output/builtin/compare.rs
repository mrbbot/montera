use crate::class::FunctionType;
use wasm_encoder::{BlockType, Function as WASMFunction, Instruction as WASMInstruction, ValType};

/// Set of instructions to use for comparing two numbers of [`ValType`].
struct CompareInstructions<'a> {
    /// Instruction to evaluate `a > b`, pushing `1` if true, and `0` otherwise.
    gt: WASMInstruction<'a>,
    /// Instruction to evaluate `a = b`, pushing `1` if true, and `0` otherwise.
    eq: WASMInstruction<'a>,
    /// Instruction to evaluate `a < b`, pushing `1` if true, and `0` otherwise.
    lt: WASMInstruction<'a>,
    /// Whether this type permits `NaN` values. If so, the generated function will include an extra
    /// parameter to determine what happens on `NaN` values. Note all above instructions should
    /// return `0` if either `a` or `b` is `NaN`.
    has_nan: bool,
}

impl CompareInstructions<'_> {
    /// Returns the set of instructions to use for comparing two numbers of the same type `t`.
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

/// Constructs a function (type and body) for comparing two numbers of the same type `t`.
///
/// Multiple instances of this function may be included in a module, for each of the value types
/// `i64`, `f32` and `f64`.
///
/// For `i64`, this function has the signature `[a: i64, b: i64] -> [ord: i32]`,
/// returning 1 if a > b, 0 if a = b, and -1 otherwise.
///
/// For `f32`/`f64`, this function has the signature `[a: t, b: t, nan_greater: i32] -> [ord: i32]`,
/// returning 1 if a > b, 0 if a = b, and -1 if a < b. If either a or b is NaN, and `nan_greater` is
/// 1, it returns 1, otherwise it returns -1. This allows the same function specialisation to be
/// used for _CMPG and _CMPL instructions.
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

    // 3. Otherwise, if this type doesn't have NaN's, we know a < b, so return -1.
    //   If the type does have NaN's, explicitly check a < b, then if that fails,
    //   we know one value is NaN.
    if cmp.has_nan {
        // 3a. Return -1 if a < b
        f.instruction(&WASMInstruction::LocalGet(0))
            .instruction(&WASMInstruction::LocalGet(1))
            .instruction(&cmp.lt);
        f.instruction(&WASMInstruction::If(BlockType::Empty));
        {
            f.instruction(&WASMInstruction::I32Const(-1))
                .instruction(&WASMInstruction::Return);
        }
        f.instruction(&WASMInstruction::End);

        // 3b. Otherwise, one value is NaN. If treating NaNs as greater than, return 1, else -1.
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
        // 3a. We know a < b, so return -1.
        f.instruction(&WASMInstruction::I32Const(-1));
    }
    f.instruction(&WASMInstruction::End);

    (func_type, f)
}
