use crate::tests::{construct_code_module, WASM_ENGINE};
use itertools::Itertools;
use wasmtime::{Linker, Module, Store};

fn construct_java_loads(name: &str, java_type: &str) -> String {
    format!(
        "public static {java_type} {name}load0({java_type} p) {{ return p; }}
         public static {java_type} {name}load1(int p1, {java_type} p) {{ return p; }}
         public static {java_type} {name}load2(int p1, int p2, {java_type} p) {{ return p; }}
         public static {java_type} {name}load3(int p1, int p2, int p3, {java_type} p) {{ return p; }}
         public static {java_type} {name}load(int p1, int p2, int p3, int p4, {java_type} p) {{ return p; }}",
    )
}

macro_rules! assert_loads {
    ($instance:ident, $store:ident, $name:expr, $descriptor:expr, $ty:ty, $val:expr) => {
        let load0 = $instance.get_typed_func::<$ty, $ty, _>(
            &mut $store,
            concat!("Test.", $name, "load0(", $descriptor, ")", $descriptor),
        )?;
        let load1 = $instance.get_typed_func::<(i32, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "load1(I", $descriptor, ")", $descriptor),
        )?;
        let load2 = $instance.get_typed_func::<(i32, i32, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "load2(II", $descriptor, ")", $descriptor),
        )?;
        let load3 = $instance.get_typed_func::<(i32, i32, i32, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "load3(III", $descriptor, ")", $descriptor),
        )?;
        let load = $instance.get_typed_func::<(i32, i32, i32, i32, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "load(IIII", $descriptor, ")", $descriptor),
        )?;

        assert_eq!(load0.call(&mut $store, $val)?, $val);
        assert_eq!(load1.call(&mut $store, (1, $val))?, $val);
        assert_eq!(load2.call(&mut $store, (1, 2, $val))?, $val);
        assert_eq!(load3.call(&mut $store, (1, 2, 3, $val))?, $val);
        assert_eq!(load.call(&mut $store, (1, 2, 3, 4, $val))?, $val);
    };
}

/// ALOAD <local>, ILOAD <local>, FLOAD <local>, LLOAD <local>, DLOAD <local>
#[test]
fn a_i_f_l_d_load() -> anyhow::Result<()> {
    let mut code = String::new();
    code.push_str(&construct_java_loads("a", "Object"));
    code.push_str(&construct_java_loads("i", "int"));
    code.push_str(&construct_java_loads("f", "float"));
    code.push_str(&construct_java_loads("l", "long"));
    code.push_str(&construct_java_loads("d", "double"));

    let module = construct_code_module(&code)?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    assert_loads!(instance, store, "a", "Ljava/lang/Object;", i32, 42);
    assert_loads!(instance, store, "i", "I", i32, 42);
    assert_loads!(instance, store, "f", "F", f32, 42.0);
    assert_loads!(instance, store, "l", "J", i64, 42);
    assert_loads!(instance, store, "d", "D", f64, 42.0);

    Ok(())
}

fn construct_java_stores(name: &str, java_type: &str, java_value: &str) -> String {
    format!(
        "public static {java_type} {name}store0() {{ {java_type} p = {java_value}; return p; }}
         public static {java_type} {name}store1(int p1) {{ {java_type} p = {java_value}; return p; }}
         public static {java_type} {name}store2(int p1, int p2) {{ {java_type} p = {java_value}; return p; }}
         public static {java_type} {name}store3(int p1, int p2, int p3) {{ {java_type} p = {java_value}; return p; }}
         public static {java_type} {name}store(int p1, int p2, int p3, int p4) {{ {java_type} p = {java_value}; return p; }}",
    )
}

macro_rules! assert_stores {
    ($instance:ident, $store:ident, $name:expr, $descriptor:expr, $ty:ty, $val:expr) => {
        let store0 = $instance.get_typed_func::<(), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "store0()", $descriptor),
        )?;
        let store1 = $instance.get_typed_func::<i32, $ty, _>(
            &mut $store,
            concat!("Test.", $name, "store1(I)", $descriptor),
        )?;
        let store2 = $instance.get_typed_func::<(i32, i32), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "store2(II)", $descriptor),
        )?;
        let store3 = $instance.get_typed_func::<(i32, i32, i32), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "store3(III)", $descriptor),
        )?;
        let store = $instance.get_typed_func::<(i32, i32, i32, i32), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "store(IIII)", $descriptor),
        )?;

        assert_eq!(store0.call(&mut $store, ())?, $val);
        assert_eq!(store1.call(&mut $store, (1))?, $val);
        assert_eq!(store2.call(&mut $store, (1, 2))?, $val);
        assert_eq!(store3.call(&mut $store, (1, 2, 3))?, $val);
        assert_eq!(store.call(&mut $store, (1, 2, 3, 4))?, $val);
    };
}

/// ASTORE <local>, ISTORE <local>, FSTORE <local>, LSTORE <local>, DSTORE <local>
#[test]
fn a_i_f_l_d_store() -> anyhow::Result<()> {
    let mut code = String::new();
    code.push_str(&construct_java_stores("a", "Object", "null"));
    code.push_str(&construct_java_stores("i", "int", "42"));
    code.push_str(&construct_java_stores("f", "float", "42f"));
    code.push_str(&construct_java_stores("l", "long", "42L"));
    code.push_str(&construct_java_stores("d", "double", "42.0"));

    let module = construct_code_module(&code)?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    assert_stores!(instance, store, "a", "Ljava/lang/Object;", i32, 0);
    assert_stores!(instance, store, "i", "I", i32, 42);
    assert_stores!(instance, store, "f", "F", f32, 42.0);
    assert_stores!(instance, store, "l", "J", i64, 42);
    assert_stores!(instance, store, "d", "D", f64, 42.0);

    Ok(())
}

/// ALOAD_WIDE <local>, ILOAD_WIDE <local>, FLOAD_WIDE <local>, LLOAD_WIDE <local>, DLOAD_WIDE <local>,
/// ASTORE_WIDE <local>, ISTORE_WIDE <local>, FSTORE_WIDE <local>, LSTORE_WIDE <local>, DSTORE_WIDE <local>
#[test]
fn a_i_f_l_d_load_store_wide() -> anyhow::Result<()> {
    // 256 `int` variables to exceed `u8`'s max value requiring a `LOAD_WIDE` instruction
    let params_256: String = (1..=256).map(|i| format!("int p{i} = 0;")).join("");
    let code = format!(
        "public static Object aload_store_wide() {{ {params_256} Object p = new Object(); return p; }}
        public static int iload_store_wide() {{ {params_256} int p = 42; return p; }}
        public static float fload_store_wide() {{ {params_256} float p = 42f; return p; }}
        public static long lload_store_wide() {{ {params_256} long p = 42; return p; }}
        public static double dload_store_wide() {{ {params_256} double p = 42.0; return p; }}",
    );

    let module = construct_code_module(&code)?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let aload_store_wide = instance
        .get_typed_func::<(), i32, _>(&mut store, "Test.aload_store_wide()Ljava/lang/Object;")?;
    let iload_store_wide =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.iload_store_wide()I")?;
    let fload_store_wide =
        instance.get_typed_func::<(), f32, _>(&mut store, "Test.fload_store_wide()F")?;
    let lload_store_wide =
        instance.get_typed_func::<(), i64, _>(&mut store, "Test.lload_store_wide()J")?;
    let dload_store_wide =
        instance.get_typed_func::<(), f64, _>(&mut store, "Test.dload_store_wide()D")?;

    assert_eq!(aload_store_wide.call(&mut store, ())?, 8);
    assert_eq!(iload_store_wide.call(&mut store, ())?, 42);
    assert_eq!(fload_store_wide.call(&mut store, ())?, 42.0);
    assert_eq!(lload_store_wide.call(&mut store, ())?, 42);
    assert_eq!(dload_store_wide.call(&mut store, ())?, 42.0);

    Ok(())
}
