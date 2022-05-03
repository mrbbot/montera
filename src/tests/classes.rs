use crate::tests::{construct_code_module, WASM_ENGINE};
use wasmtime::{Linker, Module, Store};

/// NEW <class>, INSTANCEOF <class>
#[allow(non_snake_case)]
#[test]
fn new_instanceof() -> anyhow::Result<()> {
    let module = construct_code_module(
        "static class A {}
        static class B extends A {}
        static class C extends B {}
        static class D {}
        
        // Note if's returning values aren't yet supported, hence the explicit returns.
        public static boolean B_instanceof_Object() { Object p = new B(); if (p instanceof Object) { return true; } return false; }
        public static boolean B_instanceof_A() { Object p = new B(); if (p instanceof A) { return true; } return false; }
        public static boolean B_instanceof_B() { Object p = new B(); if (p instanceof B) { return true; } return false; }
        public static boolean B_instanceof_C() { Object p = new B(); if (p instanceof C) { return true; } return false; }
        public static boolean B_instanceof_D() { Object p = new B(); if (p instanceof D) { return true; } return false; }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let B_instanceof_Object =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.B_instanceof_Object()Z")?;
    let B_instanceof_A =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.B_instanceof_A()Z")?;
    let B_instanceof_B =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.B_instanceof_B()Z")?;
    let B_instanceof_C =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.B_instanceof_C()Z")?;
    let B_instanceof_D =
        instance.get_typed_func::<(), i32, _>(&mut store, "Test.B_instanceof_D()Z")?;

    assert_eq!(B_instanceof_Object.call(&mut store, ())?, 1);
    assert_eq!(B_instanceof_A.call(&mut store, ())?, 1);
    assert_eq!(B_instanceof_B.call(&mut store, ())?, 1);
    assert_eq!(B_instanceof_C.call(&mut store, ())?, 0);
    assert_eq!(B_instanceof_D.call(&mut store, ())?, 0);

    Ok(())
}

/// GETFIELD <field>, PUTFIELD <field>
#[test]
fn get_put_field() -> anyhow::Result<()> {
    let module = construct_code_module(
        "private final int i;
        private final float f;
        private final long l;
        private final double d;

        Test(int i, float f, long l, double d) {
            this.i = i;
            this.f = f;
            this.l = l;
            this.d = d;
        }

        public static int copy_int(int i) { return new Test(i, 0f, 0L, 0.0).i; }
        public static float copy_float(float f) { return new Test(0, f, 0L, 0.0).f; }
        public static long copy_long(long l) { return new Test(0, 0f, l, 0.0).l; }
        public static double copy_double(double d) { return new Test(0, 0f, 0L, d).d; }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let copy_int = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.copy_int(I)I")?;
    let copy_float = instance.get_typed_func::<f32, f32, _>(&mut store, "Test.copy_float(F)F")?;
    let copy_long = instance.get_typed_func::<i64, i64, _>(&mut store, "Test.copy_long(J)J")?;
    let copy_double = instance.get_typed_func::<f64, f64, _>(&mut store, "Test.copy_double(D)D")?;

    assert_eq!(copy_int.call(&mut store, 42)?, 42);
    assert_eq!(copy_float.call(&mut store, 42.5)?, 42.5);
    assert_eq!(copy_long.call(&mut store, 42)?, 42);
    assert_eq!(copy_double.call(&mut store, 42.5)?, 42.5);

    Ok(())
}

/// GETFIELD <field>, PUTFIELD <field>
#[test]
fn get_put_field_hidden() -> anyhow::Result<()> {
    let module = construct_code_module(
        "static class Pair {
            int a; int b;
            Pair(int a, int b) { this.a = a; this.b = b; }
        }

        static class Triple extends Pair {
            int a;
            Triple(int a1, int b1, int a2) { super(a1, b1); this.a = a2; }
            int sum() { return super.a + this.b + this.a; }
        }
        
        public static int sum(int a, int b, int c) { return new Triple(a, b, c).sum(); }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let sum = instance.get_typed_func::<(i32, i32, i32), i32, _>(&mut store, "Test.sum(III)I")?;
    assert_eq!(sum.call(&mut store, (1, 2, 3))?, 6);

    Ok(())
}

/// INVOKESTATIC <method>
#[test]
fn invoke_static() -> anyhow::Result<()> {
    let module = construct_code_module(
        // Check recursive and non-recursive calls
        "static int add(int a, int b) { return a + b; }
        public static int fib(int n) {
            if (n <= 1) return 1;
            return add(fib(n - 1), fib(n - 2));
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let fib = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.fib(I)I")?;
    assert_eq!(fib.call(&mut store, 0)?, 1);
    assert_eq!(fib.call(&mut store, 1)?, 1);
    assert_eq!(fib.call(&mut store, 2)?, 2);
    assert_eq!(fib.call(&mut store, 3)?, 3);
    assert_eq!(fib.call(&mut store, 4)?, 5);
    assert_eq!(fib.call(&mut store, 5)?, 8);

    Ok(())
}

/// INVOKEVIRTUAL <method>
#[test]
fn invoke_virtual() -> anyhow::Result<()> {
    let module = construct_code_module(
        "static abstract class Vehicle {
            abstract double getSpeed();
            public double travelTime(double distance) { return distance / this.getSpeed(); }
        }
        
        static class Bicycle extends Vehicle {
            @Override
            public double getSpeed() { return 10.0; }
        }

        static class Car extends Vehicle {
            @Override
            public double getSpeed() { return 60.0; }
        }

        static class Van extends Car {
            @Override
            public double getSpeed() { return 40.0; }
        }

        public static double get_bicycle_time(double distance) { return new Bicycle().travelTime(distance); }
        public static double get_car_time(double distance) { return new Car().travelTime(distance); }
        public static double get_van_time(double distance) { return new Van().travelTime(distance); }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let get_bicycle_time =
        instance.get_typed_func::<f64, f64, _>(&mut store, "Test.get_bicycle_time(D)D")?;
    let get_car_time =
        instance.get_typed_func::<f64, f64, _>(&mut store, "Test.get_car_time(D)D")?;
    let get_van_time =
        instance.get_typed_func::<f64, f64, _>(&mut store, "Test.get_van_time(D)D")?;

    assert_eq!(get_bicycle_time.call(&mut store, 120.0)?, 12.0);
    assert_eq!(get_car_time.call(&mut store, 120.0)?, 2.0);
    assert_eq!(get_van_time.call(&mut store, 120.0)?, 3.0);

    Ok(())
}
