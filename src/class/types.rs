use crate::class::{ConstantPool, FieldDescriptor, MethodDescriptor};
use crate::function::Function;
use itertools::Itertools;
use log::Level;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::sync::Arc;

/// Parsed Java `.class` file with fields and methods.
///
/// See [`parse_class`](super::parser::parse_class) for the parser implementation.
#[derive(Debug, Clone)]
pub struct Class {
    /// Name of this class.
    pub class_name: Arc<String>,
    /// Name of this class's superclass, or [`JAVA_LANG_OBJECT`](super::constants::JAVA_LANG_OBJECT)
    /// if this class doesn't explicitly inherit anything.
    pub super_class_name: Arc<String>,
    /// Number of bytes to allocate on the heap for this class (excluding super classes).
    pub size: u32,
    /// Byte offsets from the start of this class (excluding super classes) for each named field.
    ///
    /// All field offsets will be less than `size`. To get the actual offset relative to a pointer,
    /// add 4 (for virtual class identifier) + size of super classes.
    ///
    /// See [`parse_fields`](super::parser::parse_fields) for the parser implementation.
    pub field_offsets: HashMap<Arc<String>, u32>,
    /// Parsed constant pool associated with this class, containing strings, numbers & descriptors.
    ///
    /// See [`ConstantPool::new`] for the parser implementation.
    pub const_pool: Arc<ConstantPool>,
    /// Parsed static and instance methods belonging to this class.
    ///
    /// See [`parse_function`](super::parser::parse_function) for the parser implementation.
    pub methods: Vec<Arc<Function>>,
}

/// Unique universal identifier for a method in a program consisting of multiple classes.
///
/// Each `MethodId` corresponds to `func`tion in the output WebAssembly module.
#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct MethodId {
    pub class_name: Arc<String>,
    pub name: Arc<String>,
    // Method overloads will have different descriptors, meaning different IDs
    pub descriptor: Arc<MethodDescriptor>,
}

impl MethodId {
    /// Returns the name of the WebAssembly function corresponding to this identifier.
    pub fn name(&self) -> String {
        let params = self.descriptor.params.iter().format("");
        let returns = &self.descriptor.returns;
        // The WebAssembly text format does not support '(' and ')' characters in function names
        // without extra annotations, so use '_'s instead
        format!("{}.{}_{}_{}", self.class_name, self.name, params, returns)
    }
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

/// Unique universal identifier for a field in a program consisting of multiple classes.
#[derive(Clone, Eq, PartialEq)]
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
    /// Logs the entire class to the console at log level [`Level::Debug`].
    ///
    /// - Class Name
    /// - Super Class
    /// - Constant Pool (at [`Level::Trace`])
    /// - Field Offsets (at [`Level::Trace`])
    /// - Size (at [`Level::Trace`])
    /// - Methods
    /// - Code (at [`Level::Trace`])
    pub fn dump(&self) {
        // Early return, don't do expensive formatting/sorting if wouldn't log anything
        if !log_enabled!(Level::Debug) {
            return;
        }
        debug!(
            "Class: {} (extends {})",
            self.class_name, self.super_class_name
        );
        trace!("  Constant Pool:");
        for (i, const_info) in (&self.const_pool.iter()).into_iter().enumerate() {
            trace!("{:>6}: {:?}", i, const_info);
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

#[cfg(test)]
mod tests {
    use crate::class::{FieldDescriptor, FieldId, MethodDescriptor, MethodId, ReturnDescriptor};
    use std::sync::Arc;

    #[test]
    fn method_id_name() {
        let id = MethodId {
            class_name: Arc::new(String::from("Class")),
            name: Arc::new(String::from("method")),
            descriptor: Arc::new(MethodDescriptor::new(
                vec![FieldDescriptor::Int, FieldDescriptor::Float],
                ReturnDescriptor::Void,
            )),
        };
        assert_eq!(id.name(), "Class.method_IF_V");
    }

    #[test]
    fn method_id_format() {
        let id = MethodId {
            class_name: Arc::new(String::from("Class")),
            name: Arc::new(String::from("method")),
            descriptor: Arc::new(MethodDescriptor::new(
                vec![FieldDescriptor::Long, FieldDescriptor::Double],
                ReturnDescriptor::Field(FieldDescriptor::Boolean),
            )),
        };
        assert_eq!(format!("{}", id), "Class.method(JD)Z");
    }

    #[test]
    fn field_id_format() {
        let id = FieldId {
            class_name: Arc::new(String::from("Class")),
            name: Arc::new(String::from("field")),
            descriptor: Arc::new(FieldDescriptor::Int),
        };
        assert_eq!(format!("{}", id), "Class.fieldI");
    }
}
