//! Common testing helper functions

use crate::class::load_class;
use crate::function::structure::ControlFlowGraph;
use crate::output::BuiltinFunction;
use crate::{Class, Module};
use data_encoding::HEXLOWER;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::{env, fs};
use wasm_encoder::Export;
use wasmtime::Engine;

const CACHE_DIR: &str = ".cache";

/// Returns a `key`ed-path persisted between test runs.
pub fn cache_path(key: &str) -> PathBuf {
    env::current_dir().unwrap().join(CACHE_DIR).join(key)
}

/// Returns the SHA-1 hex digest of a string (useful for caching).
pub fn sha1_digest(data: &str) -> String {
    let digest = Sha1::digest(data);
    HEXLOWER.encode(&digest)
}

/// Returns an atomically reference-counted owned string from a borrowed string.
pub fn str_arc(value: &str) -> Arc<String> {
    Arc::new(String::from(value))
}

lazy_static! {
    static ref JAVAC_MUTEX: Mutex<()> = Mutex::new(());
}

lazy_static! {
    pub static ref WASM_ENGINE: Engine = Engine::default();
}

/// Compiles, loads and parses Java code, returning a map of class names to parsed classes.
///
/// Compilation will be cached. `code` may include methods, fields or static inner classes and will
/// be placed inside the following template:
///
/// ```java
/// public class Test {
///     // `code` goes here
/// }
/// ```
pub fn load_many_code(code: &str) -> anyhow::Result<HashMap<String, Class>> {
    // Check if we've already compiled this code (global mutex ensures cache is in consistent state)
    let javac_guard = JAVAC_MUTEX.lock().unwrap();
    let java = format!("public class Test {{\n{}\n}}", code);
    let hash = sha1_digest(&java);
    let cache = cache_path(&hash);
    if !cache.exists() {
        // If not, compile it. First, write Java code...
        fs::create_dir_all(&cache)?;
        fs::write(cache.join("Test.java"), java)?;
        // ...then run `javac` on it
        let result = Command::new("javac")
            .arg("Test.java")
            .current_dir(&cache)
            .output()?;
        if !result.status.success() {
            // Make sure we don't cache failure
            fs::remove_dir_all(&cache).unwrap();
            bail!(
                "Unable to compile Java:\n{}",
                String::from_utf8(result.stderr)?
            );
        }
    }
    drop(javac_guard); // Unlock JAVAC_MUTEX

    // Load all .class files in cache directory
    let mut classes = HashMap::new();
    for file in fs::read_dir(&cache)? {
        let path = file?.path();
        if let Some("class") = path.extension().and_then(|s| s.to_str()) {
            let name = path.file_stem().and_then(|s| s.to_str()).map(String::from);
            let class = load_class(&path)?;
            classes.insert(name.unwrap(), class);
        }
    }

    Ok(classes)
}

/// Compiles, loads and parses Java code, returning a parsed class.
///
/// Compilation will be cached. `code` may include methods or fields and will be placed inside the
/// following template:
///
/// ```java
/// public class Test {
///     // `code` goes here
/// }
/// ```
pub fn load_code(code: &str) -> anyhow::Result<Class> {
    load_many_code(code).map(|mut classes| classes.remove("Test").unwrap())
}

/// Compiles, loads and parses Java code, returning a control flow graph containing basic blocks.
///
/// Compilation will be cached. `code` should be the body of a function, returning an integer, and
/// will be placed inside the following template:
///
/// ```java
/// public class Test {
///     static int test(int n) {
///         // `code` goes here
///     }
/// }
/// ```
pub fn load_basic_blocks(code: &str) -> anyhow::Result<ControlFlowGraph> {
    // Compile function
    let class = load_code(&format!("static int test(int n) {{\n{}\n}}", code))?;
    // Make sure class has expected format, implicit constructor followed by our test method
    assert_eq!(class.methods.len(), 2);
    assert_eq!(*class.methods[0].id.name, "<init>");
    assert_eq!(*class.methods[1].id.name, "test");
    // Extract code out of parsed class
    let mut code_guard = class.methods[1].code.lock().unwrap();
    let code = code_guard.take().unwrap();
    // Build and return control flow graph containing basic blocks
    let mut g = ControlFlowGraph::new();
    g.insert_basic_blocks(code);
    Ok(g)
}

/// Constructs a WebAssembly module exporting the specified built-in functions.
pub fn construct_builtin_module(builtins: &[BuiltinFunction]) -> Module {
    let mut module = Module::new();
    for &builtin in builtins {
        module.ensure_builtin_function(builtin);
        // Previous function index should be ensured built-in
        module.exports.export(
            builtin.name(),
            Export::Function(module.next_function_index - 1),
        );
    }
    module.render_ensured_functions_queue();
    module
}
