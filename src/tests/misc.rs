use crate::tests::{construct_code_module, WASM_ENGINE};
use wasmtime::{Linker, Module, Store, TrapCode};

/// DUP
#[test]
fn dup() -> anyhow::Result<()> {
    let module = construct_code_module(
        "int a;
        Test(int a) { this.a = a; }

        public static int construct(int a) {
            Test test = new Test(a); // DUP here, to invoke constructor and store in local variable
            return test.a;
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let construct = instance.get_typed_func::<i32, i32, _>(&mut store, "Test.construct(I)I")?;
    assert_eq!(construct.call(&mut store, 42)?, 42);

    Ok(())
}

/// ATHROW, RETURN
#[test]
fn assert() -> anyhow::Result<()> {
    let module =
        construct_code_module("public static void assert_eq(int a, int b) { assert a == b; }")?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let assert_eq =
        instance.get_typed_func::<(i32, i32), (), _>(&mut store, "Test.assert_eq(II)V")?;

    // Check returns correctly if assertion passes
    assert_eq.call(&mut store, (1, 1))?;

    // Check traps if assertion fails
    let res = assert_eq.call(&mut store, (1, 2)).unwrap_err();
    assert_eq!(res.trap_code(), Some(TrapCode::UnreachableCodeReached));

    Ok(())
}

#[test]
fn native() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static native int add_impl(int a, int b);
        public static int add(int a, int b) { return add_impl(a, b); }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;

    // Provide implementation for native method
    let mut linker = Linker::new(&WASM_ENGINE);
    linker.func_wrap("imports", "Test.add_impl(II)I", |a: i32, b: i32| a + b)?;

    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    // Check native implementation called
    let add = instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.add(II)I")?;
    assert_eq!(add.call(&mut store, (1, 2))?, 3);

    Ok(())
}

/// Code examples described in Project Proposal Appendix A
#[test]
fn proposal_examples() -> anyhow::Result<()> {
    let module = construct_code_module(
        "// add
        public static class Core1Add {
            public static void main(String[] args) {
                assert 1 + 2 == 3;
                assert 1.5f + 2.25f == 3.75f;
                assert 1.75 + 2 == 3.75;
            }
        }

        // gcd
        public static class Core1Gcd {
            public static void main(String[] args) {
                int a = 36;
                int b = 27;
                while(b != 0) {
                    int tmp = b;
                    b = a % b;
                    a = tmp;
                }
                assert a == 9;
              }
        }

        // fib
        public static class Core1Fib {
            public static int fib(int n) {
                if(n <= 1) return 1;
                return fib(n - 1) + fib(n - 2);
            }
            
            public static void main(String[] args) {
                assert fib(5) == 8;
            }
        }

        // object
        public static class Core2Object {
            private final int a;
            private final int b;
            
            public Core2Object(int a, int b) {
                this.a = a;
                this.b = b;
            }
            
            public int sum() {
                return a + b;
            }
            
            public static void main(String[] args) {
                Core2Object obj = new Core2Object(1, 2);
                assert obj.sum() == 3;
            }
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let core1add_main = instance
        .get_typed_func::<i32, (), _>(&mut store, "Test$Core1Add.main([Ljava/lang/String;)V")?;
    let core1gcd_main = instance
        .get_typed_func::<i32, (), _>(&mut store, "Test$Core1Gcd.main([Ljava/lang/String;)V")?;
    let core1fib_main = instance
        .get_typed_func::<i32, (), _>(&mut store, "Test$Core1Fib.main([Ljava/lang/String;)V")?;
    let core2object_main = instance
        .get_typed_func::<i32, (), _>(&mut store, "Test$Core2Object.main([Ljava/lang/String;)V")?;

    // Check all assertions pass (passing null `args` arrays)
    core1add_main.call(&mut store, 0)?;
    core1gcd_main.call(&mut store, 0)?;
    core1fib_main.call(&mut store, 0)?;
    core2object_main.call(&mut store, 0)?;

    Ok(())
}

/// Advanced compilation example described in Section 4.2
#[test]
fn advanced_kitchen_sink_example() -> anyhow::Result<()> {
    let module = construct_code_module(
        "public static class Pair {
            private final int a;
            private final int b;
            public Pair(int a, int b) { this.a = a; this.b = b; }
            public int sum() { return this.a + this.b; }
            public int hash() {
                int h = 0;
                int a = this.a;
                do {
                    int b = this.b;
                    while (a < b) {
                        if (a % 2 == 0 && a % 3 == 0) { h += 2; b -= 2; } else { h++; b--; }
                    }
                    a--;
                } while (a > 0);
                return h;
            }
        }
        
        public static class Triple extends Pair {
            private final int c;
            public Triple(int a, int b, int c) { super(a, b); this.c = c; }
            @Override
            public int sum() { return super.sum() + c; }
        }

        public static int pairHash(int a, int b) {
            Pair pair = new Pair(a, b);
            return pair.hash();
        }
        
        public static int tripleSum(int a, int b, int c) {
            Triple triple = new Triple(a, b, c);
            return triple.sum();
        }",
    )?;
    let module = Module::new(&WASM_ENGINE, module.finish())?;
    let linker = Linker::new(&WASM_ENGINE);
    let mut store = Store::new(&WASM_ENGINE, 0);
    let instance = linker.instantiate(&mut store, &module)?;

    let pair_hash =
        instance.get_typed_func::<(i32, i32), i32, _>(&mut store, "Test.pairHash(II)I")?;
    let triple_sum =
        instance.get_typed_func::<(i32, i32, i32), i32, _>(&mut store, "Test.tripleSum(III)I")?;

    // Rust implementations for comparison
    fn expected_pair_hash(this_a: i32, this_b: i32) -> i32 {
        let mut h = 0;
        let mut a = this_a;
        loop {
            let mut b = this_b;
            while a < b {
                if a % 2 == 0 && a % 3 == 0 {
                    h += 2;
                    b -= 2;
                } else {
                    h += 1;
                    b -= 1;
                }
            }
            a -= 1;
            // Rust doesn't have a do-while construct, so break on negated condition
            if !(a > 0) {
                break;
            }
        }
        h
    }
    fn expected_triple_sum(a: i32, b: i32, c: i32) -> i32 {
        a + b + c
    }

    // Check correct results produced on a range of inputs
    for a in 0..50 {
        for b in 0..50 {
            let value = pair_hash.call(&mut store, (a, b))?;
            let expected_value = expected_pair_hash(a, b);
            assert_eq!(value, expected_value, "pair_hash({a}, {b})");
        }
    }
    for a in 0..10 {
        for b in 0..10 {
            for c in 0..10 {
                let value = triple_sum.call(&mut store, (a, b, c))?;
                let expected_value = expected_triple_sum(a, b, c);
                assert_eq!(value, expected_value, "triple_sum({a}, {b}, {c})");
            }
        }
    }

    Ok(())
}
