use wasm_encoder::{BlockType, Function as WASMFunction, Instruction as WASMInstruction, ValType};

/// Set of instructions to use for comparing two numbers of [`ValType`].
pub(super) struct NumericInstructions<'a> {
    /// Instruction to evaluate `a = b`, pushing `1` if true, and `0` otherwise.
    pub(super) eq: WASMInstruction<'a>,
    /// Instruction to evaluate `a != b`, pushing `1` if true, and `0` otherwise.
    pub(super) ne: WASMInstruction<'a>,
    /// Instruction to evaluate `a < b`, pushing `1` if true, and `0` otherwise.
    pub(super) lt: WASMInstruction<'a>,
    /// Instruction to evaluate `a > b`, pushing `1` if true, and `0` otherwise.
    pub(super) gt: WASMInstruction<'a>,
    /// Instruction to evaluate `a + b`.
    pub(super) add: WASMInstruction<'a>,
    /// Instruction to evaluate `a - b`.
    pub(super) sub: WASMInstruction<'a>,
    /// Instruction to evaluate `a * b`.
    pub(super) mul: WASMInstruction<'a>,
    /// Instruction to evaluate `a / b`.
    pub(super) div: WASMInstruction<'a>,
    /// Instruction to push a zero value of this type.
    pub(super) zero: WASMInstruction<'a>,
    /// Instruction to push a NaN value of this type, if any.
    pub(super) nan: Option<WASMInstruction<'a>>,
    /// Instruction to truncate floating point number, if any.
    pub(super) trunc: Option<WASMInstruction<'a>>,
}

impl NumericInstructions<'_> {
    /// Returns the set of instructions to use for comparing two numbers of the same type `t`.
    pub(super) fn from_type(t: ValType) -> Self {
        match t {
            ValType::I32 => NumericInstructions {
                eq: WASMInstruction::I32Eq,
                ne: WASMInstruction::I32Neq,
                lt: WASMInstruction::I32LtS,
                gt: WASMInstruction::I32GtS,
                add: WASMInstruction::I32Add,
                sub: WASMInstruction::I32Sub,
                mul: WASMInstruction::I32Mul,
                div: WASMInstruction::I32DivS,
                zero: WASMInstruction::I32Const(0),
                nan: None,
                trunc: None,
            },
            ValType::I64 => NumericInstructions {
                eq: WASMInstruction::I64Eq,
                ne: WASMInstruction::I64Neq,
                lt: WASMInstruction::I64LtS,
                gt: WASMInstruction::I64GtS,
                add: WASMInstruction::I64Add,
                sub: WASMInstruction::I64Sub,
                mul: WASMInstruction::I64Mul,
                div: WASMInstruction::I64DivS,
                zero: WASMInstruction::I64Const(0),
                nan: None,
                trunc: None,
            },
            ValType::F32 => NumericInstructions {
                eq: WASMInstruction::F32Eq,
                ne: WASMInstruction::F32Neq,
                lt: WASMInstruction::F32Lt,
                gt: WASMInstruction::F32Gt,
                add: WASMInstruction::F32Add,
                sub: WASMInstruction::F32Sub,
                mul: WASMInstruction::F32Mul,
                div: WASMInstruction::F32Div,
                zero: WASMInstruction::F32Const(0.0),
                nan: Some(WASMInstruction::F32Const(f32::NAN)),
                trunc: Some(WASMInstruction::F32Trunc),
            },
            ValType::F64 => NumericInstructions {
                eq: WASMInstruction::F64Eq,
                ne: WASMInstruction::F64Neq,
                lt: WASMInstruction::F64Lt,
                gt: WASMInstruction::F64Gt,
                add: WASMInstruction::F64Add,
                sub: WASMInstruction::F64Sub,
                mul: WASMInstruction::F64Mul,
                div: WASMInstruction::F64Div,
                zero: WASMInstruction::F64Const(0.0),
                nan: Some(WASMInstruction::F64Const(f64::NAN)),
                trunc: Some(WASMInstruction::F64Trunc),
            },
            _ => unreachable!("Expected ValType::I32/ValType::I64/ValType::F32/ValType::F64"),
        }
    }

    /// Writes a check that `local` is NaN to the function, pushing `1` if true, and `0` otherwise.
    ///
    /// This uses the fact that `NaN == NaN` is always false.
    /// Based on AssemblyScript's [`isNaN` builtin](https://github.com/AssemblyScript/assemblyscript/blob/ac01b0a7e1c356101948e29d27e14415a9c10758/src/builtins.ts#L1961).
    pub(super) fn is_nan<'a, 'b>(
        &'a self,
        f: &'b mut WASMFunction,
        local: u32,
    ) -> &'b mut WASMFunction {
        f.instruction(&WASMInstruction::LocalGet(local))
            .instruction(&WASMInstruction::LocalGet(local))
            .instruction(&self.ne)
    }

    /// Writes a check that `local` is finite to the function, pushing `1` if true, and `0` otherwise.
    ///
    /// This uses the fact that `a - a` is usually zero, but if `a` is NaN or Infinity, `a - a` is NaN.
    /// Based on AssemblyScript's [`isFinite` builtin](https://github.com/AssemblyScript/assemblyscript/blob/ac01b0a7e1c356101948e29d27e14415a9c10758/src/builtins.ts#L2037).
    pub(super) fn is_finite<'a, 'b>(
        &'a self,
        f: &'b mut WASMFunction,
        local: u32,
    ) -> &'b mut WASMFunction {
        f.instruction(&WASMInstruction::LocalGet(local))
            .instruction(&WASMInstruction::LocalGet(local))
            .instruction(&self.sub)
            .instruction(&self.zero)
            .instruction(&self.eq)
    }
}
