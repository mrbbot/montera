use crate::class::FunctionType;
use crate::output::builtin::number::NumericInstructions;
use wasm_encoder::{BlockType, Function as WASMFunction, Instruction as WASMInstruction, ValType};

/// Constructs a function (type and body) for computing the remainder of two floating point numbers
/// of the same type `t`. This function has the signature: `[a: t, b: t] -> [c: t]`.
///
/// Multiple instances of this function may be included in a module, for each of the value types
/// `f32` and `f64`.
///
/// The semantics for this instruction are defined in chapter 6 ([`DREM`] and [`FREM`]) of the Java
/// Virtual Machine Specification.
///
/// [`DREM`](https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-6.html#jvms-6.5.drem)
/// [`FREM`](https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-6.html#jvms-6.5.frem)
pub fn construct_rem(t: ValType) -> (FunctionType, WASMFunction) {
    let num = NumericInstructions::from_type(t);
    let nan = num.nan.as_ref().unwrap();
    let func_type = FunctionType {
        params: vec![t, t], // [a (dividend): t, b (divisor): t]
        results: vec![t],   // [c: t]
    };
    let mut f = WASMFunction::new(vec![]);

    // 1. If either value is NaN, the result is NaN
    //
    // To check for NaNs, we use the fact that `NaN == NaN` is always false, comparing values to
    // themselves. This is based on AssemblyScript's isNaN builtin:
    // https://github.com/AssemblyScript/assemblyscript/blob/ac01b0a7e1c356101948e29d27e14415a9c10758/src/builtins.ts#L1961
    //
    // Check if a is NaN
    num.is_nan(&mut f, /* a */ 0)
        .instruction(&WASMInstruction::If(BlockType::Empty))
        .instruction(&nan)
        .instruction(&WASMInstruction::Return)
        .instruction(&WASMInstruction::End);
    // Check if b is NaN
    num.is_nan(&mut f, /* b */ 1)
        .instruction(&WASMInstruction::If(BlockType::Empty))
        .instruction(&nan)
        .instruction(&WASMInstruction::Return)
        .instruction(&WASMInstruction::End);

    // 2. If neither value is NaN, the sign of the result is the sign of the dividend

    // 3. If the dividend is Infinity, or the divisor is 0, or both, the result is NaN
    //
    // To check for Infinity, we use the fact that `a - a == 0` but `Infinity - Infinity` is NaN.
    // `NaN - NaN` is also NaN, but we've already checked for those. This is based on
    // AssemblyScript's isFinite builtin:
    // https://github.com/AssemblyScript/assemblyscript/blob/ac01b0a7e1c356101948e29d27e14415a9c10758/src/builtins.ts#L2037
    //
    // Check if a is Infinity
    num.is_finite(&mut f, /* a */ 0)
        .instruction(&WASMInstruction::If(BlockType::Empty))
        .instruction(&WASMInstruction::Else)
        .instruction(&nan)
        .instruction(&WASMInstruction::Return)
        .instruction(&WASMInstruction::End);
    // Check if b is 0
    f.instruction(&WASMInstruction::LocalGet(/* b */ 1))
        .instruction(&num.zero)
        .instruction(&num.eq)
        .instruction(&WASMInstruction::If(BlockType::Empty))
        .instruction(&nan)
        .instruction(&WASMInstruction::Return)
        .instruction(&WASMInstruction::End);

    // 4. If the dividend if finite and the divisor is Infinity, the result is the dividend
    // Check if b is Infinity
    num.is_finite(&mut f, /* b */ 1)
        .instruction(&WASMInstruction::If(BlockType::Empty))
        .instruction(&WASMInstruction::Else)
        .instruction(&WASMInstruction::LocalGet(/* a */ 0))
        .instruction(&WASMInstruction::Return)
        .instruction(&WASMInstruction::End);

    // 5. If the dividend is 0, and the divisor is finite, the result is the dividend
    // Check if a is 0
    f.instruction(&WASMInstruction::LocalGet(/* a */ 0))
        .instruction(&num.zero)
        .instruction(&num.eq)
        .instruction(&WASMInstruction::If(BlockType::Empty))
        .instruction(&WASMInstruction::LocalGet(/* a */ 0))
        .instruction(&WASMInstruction::Return)
        .instruction(&WASMInstruction::End);

    // 6. Otherwise, the result is a-(b*q) where q is the integer with |q| <= |a/b|
    f.instruction(&WASMInstruction::LocalGet(/* a */ 0))
        .instruction(&WASMInstruction::LocalGet(/* a */ 0))
        .instruction(&WASMInstruction::LocalGet(/* b */ 1))
        .instruction(&num.div)
        .instruction(&num.trunc.unwrap())
        .instruction(&WASMInstruction::LocalGet(/* b */ 1))
        .instruction(&num.mul)
        .instruction(&num.sub);

    f.instruction(&WASMInstruction::End);

    (func_type, f)
}

#[cfg(test)]
mod tests {
    use crate::output::builtin::BuiltinFunction;
    use crate::tests::{construct_builtin_module, WASM_ENGINE};
    use crate::write_module;
    use std::path::PathBuf;
    use std::str::FromStr;
    use wasmtime::{Linker, Module, Store};

    #[test]
    fn rem() -> anyhow::Result<()> {
        // Instantiate WebAssembly module
        let module =
            construct_builtin_module(&[BuiltinFunction::FloatRem, BuiltinFunction::DoubleRem]);

        let bytes = module.finish();
        write_module(&PathBuf::from_str("test.wasm")?, &bytes, "wasm", "wat")?;

        let module = Module::new(&WASM_ENGINE, bytes)?;
        let linker = Linker::new(&WASM_ENGINE);
        let mut store = Store::new(&WASM_ENGINE, 0);
        let instance = linker.instantiate(&mut store, &module)?;

        // Get references to exports
        let float_rem = instance.get_typed_func::<(f32, f32), f32, _>(&mut store, "!FloatRem")?;
        let double_rem = instance.get_typed_func::<(f64, f64), f64, _>(&mut store, "!DoubleRem")?;

        // 1. If either value is NaN, the result should be NaN
        assert!(f32::is_nan(float_rem.call(&mut store, (f32::NAN, 1.0))?));
        assert!(f32::is_nan(float_rem.call(&mut store, (1.0, f32::NAN))?));
        assert!(f64::is_nan(double_rem.call(&mut store, (f64::NAN, 1.0))?));
        assert!(f64::is_nan(double_rem.call(&mut store, (1.0, f64::NAN))?));

        // 3. If the dividend is Infinity, or the divisor is 0, or both, the result should be NaN
        assert!(f32::is_nan(
            float_rem.call(&mut store, (f32::INFINITY, 1.0))?
        ));
        assert!(f32::is_nan(float_rem.call(&mut store, (1.0, 0.0))?));
        assert!(f32::is_nan(
            float_rem.call(&mut store, (f32::INFINITY, 0.0))?
        ));
        assert!(f64::is_nan(
            double_rem.call(&mut store, (f64::INFINITY, 1.0))?
        ));
        assert!(f64::is_nan(double_rem.call(&mut store, (1.0, 0.0))?));
        assert!(f64::is_nan(
            double_rem.call(&mut store, (f64::INFINITY, 0.0))?
        ));

        // 4. If the dividend if finite and the divisor is Infinity, the result should be the dividend
        assert_eq!(float_rem.call(&mut store, (42.0, f32::INFINITY))?, 42.0);
        assert_eq!(double_rem.call(&mut store, (42.0, f64::INFINITY))?, 42.0);

        // 5. If the dividend is 0, and the divisor is finite, the result should be the dividend
        assert_eq!(float_rem.call(&mut store, (0.0, 3.0))?, 0.0);
        assert_eq!(double_rem.call(&mut store, (0.0, 3.0))?, 0.0);

        // 6. Otherwise, the result should be a-(b*q) where q is the integer with |q| <= |a/b|
        assert_eq!(float_rem.call(&mut store, (7.5, 2.0))?, 1.5);
        assert_eq!(double_rem.call(&mut store, (7.5, 2.0))?, 1.5);

        // 2. If neither value is NaN, the sign of the result should be the sign of the dividend
        assert_eq!(float_rem.call(&mut store, (-7.5, 2.0))?, -1.5);
        assert_eq!(double_rem.call(&mut store, (-7.5, 2.0))?, -1.5);

        Ok(())
    }
}
