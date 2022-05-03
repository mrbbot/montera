use crate::tests::{construct_code_module, WASM_ENGINE};
use wasmtime::{Linker, Module, Store};

/// I2B, I2C, I2S, I2L, I2F, I2D, L2I, L2F, L2D, F2I, F2L, F2D, D2I, D2L, D2F
#[test]
fn cast() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static byte i2b(int p) { return (byte) p; }
        public static char i2c(int p) { return (char) p; }
        public static short i2s(int p) { return (short) p; }
        public static long i2l(int p) { return (long) p; }
        public static float i2f(int p) { return (float) p; }
        public static double i2d(int p) { return (double) p; }

        public static int l2i(long p) { return (int) p; }
        public static float l2f(long p) { return (float) p; }
        public static double l2d(long p) { return (double) p; }

        public static int f2i(float p) { return (int) p; }
        public static long f2l(float p) { return (long) p; }
        public static double f2d(float p) { return (double) p; }

        public static int d2i(double p) { return (int) p; }
        public static long d2l(double p) { return (long) p; }
        public static float d2f(double p) { return (float) p; }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let i2b = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.i2b(I)B")?;
    let i2c = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.i2c(I)C")?;
    let i2s = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.i2s(I)S")?;
    let i2l = instance.get_typed_func::<i32, i64, _>(&mut store, "Test.i2l(I)J")?;
    let i2f = instance.get_typed_func::<i32, f32, _>(&mut store, "Test.i2f(I)F")?;
    let i2d = instance.get_typed_func::<i32, f64, _>(&mut store, "Test.i2d(I)D")?;

    let l2i = instance.get_typed_func::<i64, i32, _>(&mut store, "Test.l2i(J)I")?;
    let l2f = instance.get_typed_func::<i64, f32, _>(&mut store, "Test.l2f(J)F")?;
    let l2d = instance.get_typed_func::<i64, f64, _>(&mut store, "Test.l2d(J)D")?;

    let f2i = instance.get_typed_func::<f32, i32, _>(&mut store, "Test.f2i(F)I")?;
    let f2l = instance.get_typed_func::<f32, i64, _>(&mut store, "Test.f2l(F)J")?;
    let f2d = instance.get_typed_func::<f32, f64, _>(&mut store, "Test.f2d(F)D")?;

    let d2i = instance.get_typed_func::<f64, i32, _>(&mut store, "Test.d2i(D)I")?;
    let d2l = instance.get_typed_func::<f64, i64, _>(&mut store, "Test.d2l(D)J")?;
    let d2f = instance.get_typed_func::<f64, f32, _>(&mut store, "Test.d2f(D)F")?;

    assert_eq!(i2b.call(&mut store, 42)?, 42);
    assert_eq!(i2c.call(&mut store, 42)?, 42);
    assert_eq!(i2s.call(&mut store, 42)?, 42);
    assert_eq!(i2l.call(&mut store, 42)?, 42);
    assert_eq!(i2f.call(&mut store, 42)?, 42.0);
    assert_eq!(i2d.call(&mut store, 42)?, 42.0);

    assert_eq!(l2i.call(&mut store, 42)?, 42);
    assert_eq!(l2f.call(&mut store, 42)?, 42.0);
    assert_eq!(l2d.call(&mut store, 42)?, 42.0);

    assert_eq!(f2i.call(&mut store, 42.0)?, 42);
    assert_eq!(f2l.call(&mut store, 42.0)?, 42);
    assert_eq!(f2d.call(&mut store, 42.0)?, 42.0);

    assert_eq!(d2i.call(&mut store, 42.0)?, 42);
    assert_eq!(d2l.call(&mut store, 42.0)?, 42);
    assert_eq!(d2f.call(&mut store, 42.0)?, 42.0);

    Ok(())
}
