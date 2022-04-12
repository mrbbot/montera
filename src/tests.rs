//! Common testing helper functions

use std::env;
use std::path::PathBuf;

const CACHE_DIR: &str = ".cache";

pub fn cache_path(key: &str) -> PathBuf {
    env::current_dir().unwrap().join(CACHE_DIR).join(key)
}
