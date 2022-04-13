mod constants;
mod descriptors;
mod parser;
mod types;

pub use self::constants::*;
pub use self::descriptors::*;
pub use self::types::*;

use crate::class::parser::parse_class;
use crate::scheduler::Job;
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

pub struct LoadClassJob {
    pub path: PathBuf,
    pub result_tx: Sender<anyhow::Result<Class>>,
}

impl Job for LoadClassJob {
    fn process(&self) {
        let maybe_class = load_class(&self.path);
        self.result_tx.send(maybe_class).unwrap()
    }
}

pub fn load_class<P: AsRef<Path>>(path: P) -> anyhow::Result<Class> {
    // Load class from disk
    let path = path.as_ref();
    let display = path.display();
    let data = fs::read(path).with_context(|| format!("Unable to read {}", display))?;

    // Parse and return class file
    parse_class(&data)
}
