use crate::tests::{construct_code_module, WASM_ENGINE};
use wasmtime::{Linker, Module, Store};

/// IF_ACMPEQ, IF_ACMPNE, IF_CMPEQ, IF_CMPNE, IF_CMPLT, IF_CMPLE, IF_CMPGT, IF_CMPGE
#[test]
fn if_cmp() -> anyhow::Result<()> {
    let module = construct_code_module(
        // Generated instructions will actually be the opposite to method name, but since we're
        // testing all of them, all instructions will be tested.
        // Note if's returning values aren't yet supported, hence the explicit returns.
        "public static boolean if_acmpeq(Object a, Object b) { if (a == b) { return true; } else { return false; } }
        public static boolean if_acmpne(Object a, Object b) { if (a != b) { return true; } else { return false; } }
        public static boolean if_cmpeq(int a, int b) { if (a == b) { return true; } else { return false; } }
        public static boolean if_cmpne(int a, int b) { if (a != b) { return true; } else { return false; } }
        public static boolean if_cmplt(int a, int b) { if (a < b) { return true; } else { return false; } }
        public static boolean if_cmple(int a, int b) { if (a <= b) { return true; } else { return false; } }
        public static boolean if_cmpgt(int a, int b) { if (a > b) { return true; } else { return false; } }
        public static boolean if_cmpge(int a, int b) { if (a >= b) { return true; } else { return false; } }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let if_acmpeq = instance.get_typed_func::<(i32, i32), i32, _>(
        &mut store,
        "Test.if_acmpeq(Ljava/lang/Object;Ljava/lang/Object;)Z",
    )?;
    let if_acmpne = instance.get_typed_func::<(i32, i32), i32, _>(
        &mut store,
        "Test.if_acmpne(Ljava/lang/Object;Ljava/lang/Object;)Z",
    )?;
    let if_cmpeq =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.if_cmpeq(II)Z")?;
    let if_cmpne =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.if_cmpne(II)Z")?;
    let if_cmplt =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.if_cmplt(II)Z")?;
    let if_cmple =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.if_cmple(II)Z")?;
    let if_cmpgt =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.if_cmpgt(II)Z")?;
    let if_cmpge =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.if_cmpge(II)Z")?;

    assert_eq!(if_acmpeq.call(&mut store, (8, 8))?, 1);
    assert_eq!(if_acmpeq.call(&mut store, (8, 4))?, 0);

    assert_eq!(if_acmpne.call(&mut store, (8, 8))?, 0);
    assert_eq!(if_acmpne.call(&mut store, (8, 4))?, 1);

    assert_eq!(if_cmpeq.call(&mut store, (1, 1))?, 1);
    assert_eq!(if_cmpeq.call(&mut store, (1, 2))?, 0);

    assert_eq!(if_cmpne.call(&mut store, (1, 1))?, 0);
    assert_eq!(if_cmpne.call(&mut store, (1, 2))?, 1);

    assert_eq!(if_cmplt.call(&mut store, (1, 0))?, 0);
    assert_eq!(if_cmplt.call(&mut store, (1, 1))?, 0);
    assert_eq!(if_cmplt.call(&mut store, (1, 2))?, 1);

    assert_eq!(if_cmple.call(&mut store, (1, 0))?, 0);
    assert_eq!(if_cmple.call(&mut store, (1, 1))?, 1);
    assert_eq!(if_cmple.call(&mut store, (1, 2))?, 1);

    assert_eq!(if_cmpgt.call(&mut store, (1, 0))?, 1);
    assert_eq!(if_cmpgt.call(&mut store, (1, 1))?, 0);
    assert_eq!(if_cmpgt.call(&mut store, (1, 2))?, 0);

    assert_eq!(if_cmpge.call(&mut store, (1, 0))?, 1);
    assert_eq!(if_cmpge.call(&mut store, (1, 1))?, 1);
    assert_eq!(if_cmpge.call(&mut store, (1, 2))?, 0);

    Ok(())
}

/// IFNULL, IFNONNULL, IFEQ, IFNE, IFLT, IFLE, IFGT, IFGE
#[test]
fn if_cmp_zero() -> anyhow::Result<()> {
    let module = construct_code_module(
        // Generated instructions will actually be the opposite to method name, but since we're
        // testing all of them, all instructions will be tested.
        // Note if's returning values aren't yet supported, hence the explicit returns.
        "public static boolean ifnull(Object a) { if (a == null) { return true; } else { return false; } }
        public static boolean ifnonnull(Object a) { if (a != null) { return true; } else { return false; } }
        public static boolean ifeq(int a) { if (a == 0) { return true; } else { return false; } }
        public static boolean ifne(int a) { if (a != 0) { return true; } else { return false; } }
        public static boolean iflt(int a) { if (a < 0) { return true; } else { return false; } }
        public static boolean ifle(int a) { if (a <= 0) { return true; } else { return false; } }
        public static boolean ifgt(int a) { if (a > 0) { return true; } else { return false; } }
        public static boolean ifge(int a) { if (a >= 0) { return true; } else { return false; } }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let ifnull =
        instance.get_typed_func::<i32, i32, _>(&mut store, "Test.ifnull(Ljava/lang/Object;)Z")?;
    let ifnonnull = instance
        .get_typed_func::<i32, i32, _>(&mut store, "Test.ifnonnull(Ljava/lang/Object;)Z")?;
    let ifeq = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.ifeq(I)Z")?;
    let ifne = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.ifne(I)Z")?;
    let iflt = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.iflt(I)Z")?;
    let ifle = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.ifle(I)Z")?;
    let ifgt = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.ifgt(I)Z")?;
    let ifge = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.ifge(I)Z")?;

    assert_eq!(ifnull.call(&mut store, 0)?, 1);
    assert_eq!(ifnull.call(&mut store, 8)?, 0);

    assert_eq!(ifnonnull.call(&mut store, 0)?, 0);
    assert_eq!(ifnonnull.call(&mut store, 8)?, 1);

    assert_eq!(ifeq.call(&mut store, 0)?, 1);
    assert_eq!(ifeq.call(&mut store, 2)?, 0);

    assert_eq!(ifne.call(&mut store, 0)?, 0);
    assert_eq!(ifne.call(&mut store, 2)?, 1);

    assert_eq!(iflt.call(&mut store, -1)?, 1);
    assert_eq!(iflt.call(&mut store, 0)?, 0);
    assert_eq!(iflt.call(&mut store, 1)?, 0);

    assert_eq!(ifle.call(&mut store, -1)?, 1);
    assert_eq!(ifle.call(&mut store, 0)?, 1);
    assert_eq!(ifle.call(&mut store, 1)?, 0);

    assert_eq!(ifgt.call(&mut store, -1)?, 0);
    assert_eq!(ifgt.call(&mut store, 0)?, 0);
    assert_eq!(ifgt.call(&mut store, 1)?, 1);

    assert_eq!(ifge.call(&mut store, -1)?, 0);
    assert_eq!(ifge.call(&mut store, 0)?, 1);
    assert_eq!(ifge.call(&mut store, 1)?, 1);

    Ok(())
}

/// LCMP, FCMP, DCMP
#[test]
fn l_f_d_cmp() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int lcmp(long a, long b) {
            if (a < b) { return -1; } else if (a > b) { return 1; } else { return 0; }
        }
        public static int fcmp(float a, float b) {
            if (a < b) { return -1; } else if (a > b) { return 1; } else { return 0; }
        }
        public static int dcmp(double a, double b) {
            if (a < b) { return -1; } else if (a > b) { return 1; } else { return 0; }
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let lcmp = instance.get_typed_func::<(i64, i64), i32, _>(&mut store, "Test.lcmp(JJ)I")?;
    let fcmp = instance.get_typed_func::<(f32, f32), i32, _>(&mut store, "Test.fcmp(FF)I")?;
    let dcmp = instance.get_typed_func::<(f64, f64), i32, _>(&mut store, "Test.dcmp(DD)I")?;

    assert_eq!(lcmp.call(&mut store, (1, 0))?, 1);
    assert_eq!(lcmp.call(&mut store, (1, 1))?, 0);
    assert_eq!(lcmp.call(&mut store, (1, 2))?, -1);

    assert_eq!(fcmp.call(&mut store, (1.0, 0.0))?, 1);
    assert_eq!(fcmp.call(&mut store, (1.0, 1.0))?, 0);
    assert_eq!(fcmp.call(&mut store, (1.0, 2.0))?, -1);

    assert_eq!(dcmp.call(&mut store, (1.0, 0.0))?, 1);
    assert_eq!(dcmp.call(&mut store, (1.0, 1.0))?, 0);
    assert_eq!(dcmp.call(&mut store, (1.0, 2.0))?, -1);

    Ok(())
}

#[test]
fn if_nested() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int grade(int mark) {
            if (mark > 50) {
                if (mark > 75) {
                    if (mark == 100) {
                        return 5;
                    } else {
                        return 4;
                    }
                } else {
                    return 3;
                }
            } else {
                if (mark < 25) {
                    return 1;
                } else {
                    return 2;
                }
            }
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let grade = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.grade(I)I")?;

    assert_eq!(grade.call(&mut store, 0)?, 1);
    assert_eq!(grade.call(&mut store, 25)?, 2);
    assert_eq!(grade.call(&mut store, 30)?, 2);
    assert_eq!(grade.call(&mut store, 50)?, 2);
    assert_eq!(grade.call(&mut store, 60)?, 3);
    assert_eq!(grade.call(&mut store, 75)?, 3);
    assert_eq!(grade.call(&mut store, 80)?, 4);
    assert_eq!(grade.call(&mut store, 100)?, 5);

    Ok(())
}

#[test]
fn if_short_circuit() -> anyhow::Result<()> {
    let module = construct_code_module(
        // Note if's returning values aren't yet supported, hence the explicit returns.
        "public static boolean and(boolean a, boolean b) { if (a && b) { return true; } else { return false; } }
        public static boolean and_neg(boolean a, boolean b) { if (!a && b) { return true; } else { return false; } }
        public static boolean or(boolean a, boolean b) { if (a || b) { return true; } else { return false; } }
        public static boolean or_neg(boolean a, boolean b) { if (!a || b) { return true; } else { return false; } }

        public static int nested(boolean a, boolean b, boolean c, boolean d) {
            if ((a && b) || (c && d)) {
                return 2;
            } else {
                if (a || b || c || d) {
                    return 1; 
                }
                return 0;
            }
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let and = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.and(ZZ)Z")?;
    let and_neg = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.and_neg(ZZ)Z")?;
    let or = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.or(ZZ)Z")?;
    let or_neg = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.or_neg(ZZ)Z")?;
    let nested = instance
        .get_typed_func::<(i32, i32, i32, i32), i32, _>(&mut store, "Test.nested(ZZZZ)I")?;

    assert_eq!(and.call(&mut store, (0, 0))?, 0);
    assert_eq!(and.call(&mut store, (0, 1))?, 0);
    assert_eq!(and.call(&mut store, (1, 0))?, 0);
    assert_eq!(and.call(&mut store, (1, 1))?, 1);

    assert_eq!(and_neg.call(&mut store, (0, 0))?, 0);
    assert_eq!(and_neg.call(&mut store, (0, 1))?, 1);
    assert_eq!(and_neg.call(&mut store, (1, 0))?, 0);
    assert_eq!(and_neg.call(&mut store, (1, 1))?, 0);

    assert_eq!(or.call(&mut store, (0, 0))?, 0);
    assert_eq!(or.call(&mut store, (0, 1))?, 1);
    assert_eq!(or.call(&mut store, (1, 0))?, 1);
    assert_eq!(or.call(&mut store, (1, 1))?, 1);

    assert_eq!(or_neg.call(&mut store, (0, 0))?, 1);
    assert_eq!(or_neg.call(&mut store, (0, 1))?, 1);
    assert_eq!(or_neg.call(&mut store, (1, 0))?, 0);
    assert_eq!(or_neg.call(&mut store, (1, 1))?, 1);

    assert_eq!(nested.call(&mut store, (0, 0, 0, 0))?, 0);
    assert_eq!(nested.call(&mut store, (0, 0, 0, 1))?, 1);
    assert_eq!(nested.call(&mut store, (0, 0, 1, 0))?, 1);
    assert_eq!(nested.call(&mut store, (0, 0, 1, 1))?, 2);
    assert_eq!(nested.call(&mut store, (0, 1, 0, 0))?, 1);
    assert_eq!(nested.call(&mut store, (0, 1, 0, 1))?, 1);
    assert_eq!(nested.call(&mut store, (0, 1, 1, 0))?, 1);
    assert_eq!(nested.call(&mut store, (0, 1, 1, 1))?, 2);
    assert_eq!(nested.call(&mut store, (1, 0, 0, 0))?, 1);
    assert_eq!(nested.call(&mut store, (1, 0, 0, 1))?, 1);
    assert_eq!(nested.call(&mut store, (1, 0, 1, 0))?, 1);
    assert_eq!(nested.call(&mut store, (1, 0, 1, 1))?, 2);
    assert_eq!(nested.call(&mut store, (1, 1, 0, 0))?, 2);
    assert_eq!(nested.call(&mut store, (1, 1, 0, 1))?, 2);
    assert_eq!(nested.call(&mut store, (1, 1, 1, 0))?, 2);
    assert_eq!(nested.call(&mut store, (1, 1, 1, 1))?, 2);

    Ok(())
}

#[test]
fn pre_tested_loop() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int copy_while(int a) {
            int b = 0;
            while (a > 0) {
                b++;
                a--;
            }
            return b;
        }

        public static int copy_for(int a) {
            int b = 0;
            for (int i = 0; i < a; i++) {
                b++;
            }
            return b;
        }

        public static int gcd(int a, int b) {
            while (b != 0) {
                int tmp = b;
                b = a % b;
                a = tmp;
            }
            return a;
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let copy_while = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.copy_while(I)I")?;
    let copy_for = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.copy_for(I)I")?;
    let gcd = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.gcd(II)I")?;

    assert_eq!(copy_while.call(&mut store, 0)?, 0);
    assert_eq!(copy_while.call(&mut store, 5)?, 5);

    assert_eq!(copy_for.call(&mut store, 0)?, 0);
    assert_eq!(copy_for.call(&mut store, 5)?, 5);

    assert_eq!(gcd.call(&mut store, (36, 27))?, 9);
    assert_eq!(gcd.call(&mut store, (5, 7))?, 1);

    Ok(())
}

#[test]
fn post_tested_loop() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static int copy_ish_do_while(int a) {
            int b = 0;
            do {
                b++;
                a--;
            } while (a > 0);
            return b;
        }
        
        public static int sum_ish(int a, int b) {
            int c = 1;
            do {
                while (b > a) {
                    b--;
                    a++;
                }
                c++;
                a--;
            } while (a > 0);
            return c; 
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let copy_ish_do_while =
        instance.get_typed_func::<i32, i32, _>(&mut store, "Test.copy_ish_do_while(I)I")?;
    let sum_ish = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.sum_ish(II)I")?;

    assert_eq!(copy_ish_do_while.call(&mut store, 0)?, 1);
    assert_eq!(copy_ish_do_while.call(&mut store, 5)?, 5);

    assert_eq!(sum_ish.call(&mut store, (1, 2))?, 3);
    assert_eq!(sum_ish.call(&mut store, (3, 5))?, 8);

    Ok(())
}
