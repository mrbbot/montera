use crate::tests::{construct_code_module, WASM_ENGINE};
use wasmtime::{Linker, Module, Store};

macro_rules! assert_int {
    ($instance:ident, $store:ident, $name:expr, $desc:expr, $ty:ty, $ushr:expr) => {
        let add = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "add(", $desc, $desc, ")", $desc),
        )?;
        let sub = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "sub(", $desc, $desc, ")", $desc),
        )?;
        let mul = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "mul(", $desc, $desc, ")", $desc),
        )?;
        let div = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "div(", $desc, $desc, ")", $desc),
        )?;
        let rem = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "rem(", $desc, $desc, ")", $desc),
        )?;
        let neg = $instance.get_typed_func::<$ty, $ty, _>(
            &mut $store,
            concat!("Test.", $name, "neg(", $desc, ")", $desc),
        )?;
        let shl = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "shl(", $desc, $desc, ")", $desc),
        )?;
        let shr = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "shr(", $desc, $desc, ")", $desc),
        )?;
        let ushr = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "ushr(", $desc, $desc, ")", $desc),
        )?;
        let and = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "and(", $desc, $desc, ")", $desc),
        )?;
        let or = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "or(", $desc, $desc, ")", $desc),
        )?;
        let xor = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "xor(", $desc, $desc, ")", $desc),
        )?;

        assert_eq!(add.call(&mut $store, (1, 2))?, 3);
        assert_eq!(sub.call(&mut $store, (3, 2))?, 1);
        assert_eq!(mul.call(&mut $store, (2, 3))?, 6);
        assert_eq!(div.call(&mut $store, (7, 3))?, 2);
        assert_eq!(rem.call(&mut $store, (7, 3))?, 1);
        assert_eq!(neg.call(&mut $store, 5)?, -5);
        assert_eq!(shl.call(&mut $store, (-8, 2))?, -32);
        assert_eq!(shr.call(&mut $store, (-8, 2))?, -2);
        assert_eq!(ushr.call(&mut $store, (-8, 2))?, $ushr);
        assert_eq!(and.call(&mut $store, (0b1001, 0b1010))?, 0b1000);
        assert_eq!(or.call(&mut $store, (0b1001, 0b1010))?, 0b1011);
        assert_eq!(xor.call(&mut $store, (0b1001, 0b1010))?, 0b0011);
    };
}

macro_rules! assert_float {
    ($instance:ident, $store:ident, $name:expr, $desc:expr, $ty:ty) => {
        let add = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "add(", $desc, $desc, ")", $desc),
        )?;
        let sub = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "sub(", $desc, $desc, ")", $desc),
        )?;
        let mul = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "mul(", $desc, $desc, ")", $desc),
        )?;
        let div = $instance.get_typed_func::<($ty, $ty), $ty, _>(
            &mut $store,
            concat!("Test.", $name, "div(", $desc, $desc, ")", $desc),
        )?;
        let neg = $instance.get_typed_func::<$ty, $ty, _>(
            &mut $store,
            concat!("Test.", $name, "neg(", $desc, ")", $desc),
        )?;

        assert_eq!(add.call(&mut $store, (1.25, 2.5))?, 3.75);
        assert_eq!(sub.call(&mut $store, (3.75, 2.5))?, 1.25);
        assert_eq!(mul.call(&mut $store, (2.5, 3.0))?, 7.5);
        assert_eq!(div.call(&mut $store, (7.5, 2.5))?, 3.0);
        assert_eq!(neg.call(&mut $store, 5.5)?, -5.5);
    };
}

/// IADD, ISUB, IMUL, IDIV, IREM, INEG, ISHL, ISHR, IUSHR, IAND, IOR, IXOR,
/// LADD, LSUB, LMUL, LDIV, LREM, LNEG, LSHL, LSHR, LUSHR, LAND, LOR, LXOR,
/// FADD, FSUB, FMUL, FDIV, FNEG,
/// DADD, DSUB, DMUL, DDIV, DNEG,
/// IINC, IINC_WIDE
#[test]
fn maths() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int iadd(int a, int b) { return a + b; }
        public static int isub(int a, int b) { return a - b; }
        public static int imul(int a, int b) { return a * b; }
        public static int idiv(int a, int b) { return a / b; }
        public static int irem(int a, int b) { return a % b; }
        public static int ineg(int a) { return -a; }
        public static int ishl(int a, int b) { return a << b; }
        public static int ishr(int a, int b) { return a >> b; }
        public static int iushr(int a, int b) { return a >>> b; }
        public static int iand(int a, int b) { return a & b; }
        public static int ior(int a, int b) { return a | b; }
        public static int ixor(int a, int b) { return a ^ b; }
        
        public static long ladd(long a, long b) { return a + b; }
        public static long lsub(long a, long b) { return a - b; }
        public static long lmul(long a, long b) { return a * b; }
        public static long ldiv(long a, long b) { return a / b; }
        public static long lrem(long a, long b) { return a % b; }
        public static long lneg(long a) { return -a; }
        public static long lshl(long a, long b) { return a << b; }
        public static long lshr(long a, long b) { return a >> b; }
        public static long lushr(long a, long b) { return a >>> b; }
        public static long land(long a, long b) { return a & b; }
        public static long lor(long a, long b) { return a | b; }
        public static long lxor(long a, long b) { return a ^ b; }

        public static float fadd(float a, float b) { return a + b; }
        public static float fsub(float a, float b) { return a - b; }
        public static float fmul(float a, float b) { return a * b; }
        public static float fdiv(float a, float b) { return a / b; }
        public static float fneg(float a) { return -a; }
 
        public static double dadd(double a, double b) { return a + b; }
        public static double dsub(double a, double b) { return a - b; }
        public static double dmul(double a, double b) { return a * b; }
        public static double ddiv(double a, double b) { return a / b; }
        public static double dneg(double a) { return -a; }
        
        public static int iinc(int a) { a += 1; return a; }
        public static int iinc_wide(int a) { a += 9000; return a; }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    assert_int!(instance, store, "i", "I", i32, 1073741822);
    assert_int!(instance, store, "l", "J", i64, 4611686018427387902);
    assert_float!(instance, store, "f", "F", f32);
    assert_float!(instance, store, "d", "D", f64);

    let iinc = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.iinc(I)I")?;
    let iinc_wide = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.iinc_wide(I)I")?;

    assert_eq!(iinc.call(&mut store, 5)?, 6);
    assert_eq!(iinc_wide.call(&mut store, 5)?, 9005);

    Ok(())
}
