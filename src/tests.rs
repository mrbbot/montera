//! Common testing helper functions

use crate::class::load_class;
use crate::Class;
use data_encoding::HEXLOWER;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::{env, fs};

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

lazy_static! {
    static ref JAVAC_MUTEX: Mutex<()> = Mutex::new(());
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
