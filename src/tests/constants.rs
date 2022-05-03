use crate::tests::{construct_code_module, WASM_ENGINE};
use wasmtime::{Linker, Module, Store};

/// ACONSTNULL, ICONST_<n>, LCONST_<n>, FCONST_<n>, DCONST<n>
/// ARETURN, IRETURN, LRETURN, FRETURN, DRETURN
#[test]
fn a_i_f_l_d_const() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static Object aconstnull() { return null; }

        public static int iconstm1() { return -1; }
        public static int iconst0() { return 0; }
        public static int iconst1() { return 1; }
        public static int iconst2() { return 2; }
        public static int iconst3() { return 3; }
        public static int iconst4() { return 4; }
        public static int iconst5() { return 5; }

        public static long lconst0() { return 0L; }
        public static long lconst1() { return 1L; }

        public static float fconst0() { return 0f; }
        public static float fconst1() { return 1f; }
        public static float fconst2() { return 2f; }

        public static double dconst0() { return 0.0; }
        public static double dconst1() { return 1.0; }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let aconstnull =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.aconstnull()Ljava/lang/Object;")?;

    let iconstm1 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconstm1()I")?;
    let iconst0 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconst0()I")?;
    let iconst1 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconst1()I")?;
    let iconst2 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconst2()I")?;
    let iconst3 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconst3()I")?;
    let iconst4 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconst4()I")?;
    let iconst5 = instance.get_typed_func::<(), i32, _>(&mut store, "Test.iconst5()I")?;

    let lconst0 = instance.get_typed_func::<(), i64, _>(&mut store, "Test.lconst0()J")?;
    let lconst1 = instance.get_typed_func::<(), i64, _>(&mut store, "Test.lconst1()J")?;

    let fconst0 = instance.get_typed_func::<(), f32, _>(&mut store, "Test.fconst0()F")?;
    let fconst1 = instance.get_typed_func::<(), f32, _>(&mut store, "Test.fconst1()F")?;
    let fconst2 = instance.get_typed_func::<(), f32, _>(&mut store, "Test.fconst2()F")?;

    let dconst0 = instance.get_typed_func::<(), f64, _>(&mut store, "Test.dconst0()D")?;
    let dconst1 = instance.get_typed_func::<(), f64, _>(&mut store, "Test.dconst1()D")?;

    assert_eq!(aconstnull.call(&mut store, ())?, 0);

    assert_eq!(iconstm1.call(&mut store, ())?, -1);
    assert_eq!(iconst0.call(&mut store, ())?, 0);
    assert_eq!(iconst1.call(&mut store, ())?, 1);
    assert_eq!(iconst2.call(&mut store, ())?, 2);
    assert_eq!(iconst3.call(&mut store, ())?, 3);
    assert_eq!(iconst4.call(&mut store, ())?, 4);
    assert_eq!(iconst5.call(&mut store, ())?, 5);

    assert_eq!(lconst0.call(&mut store, ())?, 0);
    assert_eq!(lconst1.call(&mut store, ())?, 1);

    assert_eq!(fconst0.call(&mut store, ())?, 0.0);
    assert_eq!(fconst1.call(&mut store, ())?, 1.0);
    assert_eq!(fconst2.call(&mut store, ())?, 2.0);

    assert_eq!(dconst0.call(&mut store, ())?, 0.0);
    assert_eq!(dconst1.call(&mut store, ())?, 1.0);

    Ok(())
}

/// BIPUSH <n>, SIPUSH <n>
#[test]
fn bi_si_push() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int bipush() { return 100; } // <= Byte.MAX_VALUE
        public static int sipush() { return 500; } // > Byte.MAX_VALUE and <= Short.MAX_VALUE",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let bipush = instance.get_typed_func::<(), i32, _>(&mut store, "Test.bipush()I")?;
    let sipush = instance.get_typed_func::<(), i32, _>(&mut store, "Test.sipush()I")?;

    assert_eq!(bipush.call(&mut store, ())?, 100);
    assert_eq!(sipush.call(&mut store, ())?, 500);

    Ok(())
}

/// LDC <index>, LDC2_W <index>
#[test]
fn ldc() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int ldc_int() { return 32768; } // > Short.MAX_VALUE
        public static float ldc_float() { return 42f; }
        public static long ldc2_w_long() { return 42L; }
        public static double ldc2_w_double() { return 42.0; }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let ldc_int = instance.get_typed_func::<(), i32, _>(&mut store, "Test.ldc_int()I")?;
    let ldc_float = instance.get_typed_func::<(), f32, _>(&mut store, "Test.ldc_float()F")?;
    let ldc2_w_long = instance.get_typed_func::<(), i64, _>(&mut store, "Test.ldc2_w_long()J")?;
    let ldc2_w_double =
        instance.get_typed_func::<(), f64, _>(&mut store, "Test.ldc2_w_double()D")?;

    assert_eq!(ldc_int.call(&mut store, ())?, 32768);
    assert_eq!(ldc_float.call(&mut store, ())?, 42.0);
    assert_eq!(ldc2_w_long.call(&mut store, ())?, 42);
    assert_eq!(ldc2_w_double.call(&mut store, ())?, 42.0);

    Ok(())
}
