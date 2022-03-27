use crate::class::{ConstantPool, FieldDescriptor, MethodDescriptor};
use crate::function::Function;
use itertools::Itertools;
use log::Level;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Class {
    pub class_name: Arc<String>,
    pub super_class_name: Arc<String>,
    pub size: u32,
    pub field_offsets: HashMap<Arc<String>, u32>,
    pub const_pool: Arc<ConstantPool>,
    pub methods: Vec<Arc<Function>>,
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct MethodId {
    pub class_name: Arc<String>,
    pub name: Arc<String>,
    pub descriptor: Arc<MethodDescriptor>,
}

impl fmt::Display for MethodId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}{}", self.class_name, self.name, self.descriptor)
    }
}

impl fmt::Debug for MethodId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "MethodId {{ {} }}", self)
    }
}

#[derive(Clone)]
pub struct FieldId {
    pub class_name: Arc<String>,
    pub name: Arc<String>,
    pub descriptor: Arc<FieldDescriptor>,
}

impl fmt::Display for FieldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}{}", self.class_name, self.name, self.descriptor)
    }
}

impl fmt::Debug for FieldId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "FieldId {{ {} }}", self)
    }
}

impl Class {
    pub fn dump(&self) {
        if !log_enabled!(Level::Debug) {
            return;
        }
        debug!(
            "Class: {} (extends {})",
            self.class_name, self.super_class_name
        );
        trace!("  Constant Pool:");
        for (i, const_info) in (&self.const_pool.iter()).into_iter().enumerate() {
            trace!("{:>6}: {:?}", i + 1, const_info);
        }
        if !self.field_offsets.is_empty() {
            trace!("  Fields:");
            for (field_name, offset) in self
                .field_offsets
                .iter()
                .sorted_by_key(|(_, offset)| *offset)
            {
                trace!("{:>6}: {}", offset, field_name);
            }
            trace!("{:>6}:", self.size);
        }
        for function in &self.methods {
            debug!(
                "  Method: ({:?}) {}{}",
                function.flags, function.id.name, function.id.descriptor,
            );
            if let Some(code) = function.code.lock().unwrap().deref() {
                for (label, instruction) in code {
                    trace!("{:>6}: {:?}", label, instruction);
                }
            }
        }
    }
}
