use itertools::Itertools;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write;
use std::sync::Arc;
use wasm_encoder::ValType;

/// Parsed field descriptor representing the type of a field as defined in section [4.3.2] of the
/// Java Virtual Machine Specification.
///
/// See [`field_descriptor_parser`] for the parser implementation.
///
/// Derives [`Ord`] so methods can be sorted for deterministic output. Note Rust requires the size
/// of `enum`s to be known at compile time, so we must add indirection (`Box`) to [`Array`].
///
/// [4.3.2]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
/// [`field_descriptor_parser`]: super::parser::field_descriptor_parser
/// [`Array`]: FieldDescriptor::Array
#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum FieldDescriptor {
    Byte,                        // B
    Char,                        // C
    Double,                      // D
    Float,                       // F
    Int,                         // I
    Long,                        // J
    Short,                       // S
    Boolean,                     // Z
    Object(String),              // L ClassName ;
    Array(Box<FieldDescriptor>), // [ ComponentType
}

impl fmt::Display for FieldDescriptor {
    //noinspection RsLiveness
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldDescriptor::Byte => f.write_char('B'),
            FieldDescriptor::Char => f.write_char('C'),
            FieldDescriptor::Double => f.write_char('D'),
            FieldDescriptor::Float => f.write_char('F'),
            FieldDescriptor::Int => f.write_char('I'),
            FieldDescriptor::Long => f.write_char('J'),
            FieldDescriptor::Short => f.write_char('S'),
            FieldDescriptor::Boolean => f.write_char('Z'),
            FieldDescriptor::Object(class_name) => write!(f, "L{class_name};"),
            FieldDescriptor::Array(component_type) => write!(f, "[{component_type}"),
        }
    }
}

impl fmt::Debug for FieldDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FieldDescriptor {{ {} }}", self)
    }
}

impl FieldDescriptor {
    pub fn as_type(&self) -> ValType {
        match self {
            FieldDescriptor::Byte => ValType::I32,
            FieldDescriptor::Char => ValType::I32,
            FieldDescriptor::Double => ValType::F64,
            FieldDescriptor::Float => ValType::F32,
            FieldDescriptor::Int => ValType::I32,
            FieldDescriptor::Long => ValType::I64,
            FieldDescriptor::Short => ValType::I32,
            FieldDescriptor::Boolean => ValType::I32,
            FieldDescriptor::Object(_) => ValType::I32, // Pointer
            FieldDescriptor::Array(_) => ValType::I32,  // Pointer
        }
    }

    pub fn size(&self) -> u32 {
        let field_type = self.as_type();
        match field_type {
            ValType::I32 | ValType::F32 => 4,
            ValType::I64 | ValType::F64 => 8,
            _ => unreachable!("{:?}", field_type),
        }
    }
}

/// Parsed return descriptor representing the return type of a method as defined in section [4.3.3]
/// of the Java Virtual Machine Specification.
///
/// See [`return_descriptor_parser`] for the parser implementation.
///
/// Derives [`Ord`] so methods can be sorted for deterministic output.
///
/// [4.3.3]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
/// [`return_descriptor_parser`]: super::parser::return_descriptor_parser
#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ReturnDescriptor {
    Void, // V
    Field(FieldDescriptor),
}

impl fmt::Display for ReturnDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReturnDescriptor::Void => f.write_char('V'),
            ReturnDescriptor::Field(field_type) => fmt::Display::fmt(field_type, f),
        }
    }
}

impl fmt::Debug for ReturnDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ReturnDescriptor {{ {} }}", self)
    }
}

/// WebAssembly function type corresponding to a parsed [`MethodDescriptor`].
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FunctionType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

impl FunctionType {
    /// Returns a copy of the `FunctionType` with an additional `i32` parameter at the start for the
    /// implicit `this` pointer in instance methods.
    pub fn with_implicit_this(&self) -> Self {
        let mut func_type = self.clone();
        func_type.params.insert(0, ValType::I32);
        func_type
    }

    /// Returns the name of the built-in dispatcher function for these parameter and return types.
    pub fn dispatcher_name(&self) -> String {
        let param_names = self.params.iter().copied().map(val_type_name).format("");
        let result_names = self.results.iter().copied().map(val_type_name).format("");
        // The WebAssembly text format does not support '(' and ')' characters in function names
        // without extra annotations, so use '_'s instead
        format!("!Dispatcher_{}_{}", param_names, result_names)
    }
}

/// Returns the WebAssembly text format's representation of a value type.
fn val_type_name(t: ValType) -> &'static str {
    match t {
        ValType::I32 => "i32",
        ValType::I64 => "i64",
        ValType::F32 => "f32",
        ValType::F64 => "f64",
        ValType::V128 => "v128",
        ValType::FuncRef => "funcref",
        ValType::ExternRef => "externref",
    }
}

/// Parsed method descriptor representing the parameter and return types of a method as defined in
/// section [4.3.3] of the Java Virtual Machine Specification.
///
/// See [`method_descriptor_parser`] for the parser implementation.
///
/// Implements [`Ord`] so methods can be sorted for deterministic output.
///
/// [4.3.3]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
/// [`method_descriptor_parser`]: super::parser::method_descriptor_parser
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct MethodDescriptor {
    pub params: Vec<FieldDescriptor>,
    pub returns: ReturnDescriptor,

    /// WebAssembly function type corresponding to this descriptor. Computed at construction and
    /// reference counted to avoid doing this multiple times later on.
    pub function_type: Arc<FunctionType>,
}

impl fmt::Display for MethodDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('(')?;
        for param in &self.params {
            fmt::Display::fmt(param, f)?;
        }
        f.write_char(')')?;
        fmt::Display::fmt(&self.returns, f)?;
        Ok(())
    }
}

impl fmt::Debug for MethodDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MethodDescriptor {{ {} }}", self)
    }
}

impl PartialOrd<Self> for MethodDescriptor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MethodDescriptor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.params
            .cmp(&other.params)
            .then_with(|| self.returns.cmp(&other.returns))
    }
}

impl MethodDescriptor {
    /// Constructs a new `MethodDescriptor` from parameter and return types.
    ///
    /// Also computes the WebAssembly function type corresponding to this descriptor to avoid doing
    /// this multiple times later on. It's likely we'll need this multiple times for every method
    /// descriptor we constructor.
    pub fn new(params: Vec<FieldDescriptor>, returns: ReturnDescriptor) -> Self {
        // Compute WebAssembly function type
        let function_type = {
            let params = params.iter().map(FieldDescriptor::as_type).collect();
            let results = match &returns {
                ReturnDescriptor::Void => vec![],
                ReturnDescriptor::Field(field) => vec![field.as_type()],
            };
            Arc::new(FunctionType { params, results })
        };

        Self {
            params,
            returns,
            function_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::class::{FieldDescriptor, FunctionType, MethodDescriptor, ReturnDescriptor};
    use std::cmp::Ordering;
    use wasm_encoder::ValType;

    // Descriptor `Display` implementations are tested in the `super::parser::tests` module

    #[test]
    fn field_descriptor_as_type() {
        // Integer types
        assert_eq!(FieldDescriptor::Boolean.as_type(), ValType::I32);
        assert_eq!(FieldDescriptor::Byte.as_type(), ValType::I32);
        assert_eq!(FieldDescriptor::Char.as_type(), ValType::I32);
        assert_eq!(FieldDescriptor::Short.as_type(), ValType::I32);
        assert_eq!(FieldDescriptor::Int.as_type(), ValType::I32);
        assert_eq!(FieldDescriptor::Long.as_type(), ValType::I64);

        // Floating point types
        assert_eq!(FieldDescriptor::Float.as_type(), ValType::F32);
        assert_eq!(FieldDescriptor::Double.as_type(), ValType::F64);

        // Reference types
        assert_eq!(
            FieldDescriptor::Object(String::new()).as_type(),
            ValType::I32
        );
        assert_eq!(
            FieldDescriptor::Array(Box::new(FieldDescriptor::Int)).as_type(),
            ValType::I32
        );
    }

    #[test]
    fn field_descriptor_size() {
        // Single word (4 byte) types
        assert_eq!(FieldDescriptor::Boolean.size(), 4);
        assert_eq!(FieldDescriptor::Byte.size(), 4);
        assert_eq!(FieldDescriptor::Char.size(), 4);
        assert_eq!(FieldDescriptor::Short.size(), 4);
        assert_eq!(FieldDescriptor::Int.size(), 4);

        assert_eq!(FieldDescriptor::Float.size(), 4);

        assert_eq!(FieldDescriptor::Object(String::new()).size(), 4);
        assert_eq!(
            FieldDescriptor::Array(Box::new(FieldDescriptor::Int)).size(),
            4
        );

        // Double word (8 byte) types
        assert_eq!(FieldDescriptor::Long.size(), 8);
        assert_eq!(FieldDescriptor::Double.size(), 8);
    }

    #[test]
    fn function_type_with_implicit_this() {
        let func_type = FunctionType {
            params: vec![ValType::F32],
            results: vec![],
        };
        let func_type = func_type.with_implicit_this();
        assert_eq!(func_type.params, [ValType::I32, ValType::F32]);
        assert_eq!(func_type.results, []);
    }

    #[test]
    fn function_type_dispatcher_name() {
        let func_type = FunctionType {
            params: vec![ValType::I32, ValType::I64, ValType::F32, ValType::F64],
            results: vec![ValType::I32],
        };
        assert_eq!(func_type.dispatcher_name(), "!Dispatcher_i32i64f32f64_i32");
        let func_type = FunctionType {
            params: vec![],
            results: vec![],
        };
        assert_eq!(func_type.dispatcher_name(), "!Dispatcher__");
    }

    #[test]
    fn method_descriptor_new() {
        // Check computes `FunctionType` with return
        let d = MethodDescriptor::new(
            vec![FieldDescriptor::Int],
            ReturnDescriptor::Field(FieldDescriptor::Float),
        );
        assert_eq!(d.function_type.params, [ValType::I32]);
        assert_eq!(d.function_type.results, [ValType::F32]);

        // Check computes `FunctionType` with void return
        let d = MethodDescriptor::new(vec![], ReturnDescriptor::Void);
        assert_eq!(d.function_type.params, []);
        assert_eq!(d.function_type.results, []);
    }

    #[test]
    fn method_descriptor_ord() {
        // Check initially orders by params...
        let d1 = MethodDescriptor::new(
            vec![
                FieldDescriptor::Byte,
                FieldDescriptor::Object(String::from("A")),
            ],
            ReturnDescriptor::Field(FieldDescriptor::Object(String::from("B"))),
        );
        let d2 = MethodDescriptor::new(
            vec![
                FieldDescriptor::Byte,
                FieldDescriptor::Object(String::from("B")),
            ],
            ReturnDescriptor::Field(FieldDescriptor::Object(String::from("A"))),
        );
        assert_eq!(d1.cmp(&d2), Ordering::Less);
        assert_eq!(d1.cmp(&d1), Ordering::Equal);
        assert_eq!(d2.cmp(&d1), Ordering::Greater);

        // ...and then by returns if params match
        let d1 = MethodDescriptor::new(
            vec![FieldDescriptor::Int],
            ReturnDescriptor::Field(FieldDescriptor::Object(String::from("B"))),
        );
        let d2 = MethodDescriptor::new(
            vec![FieldDescriptor::Int],
            ReturnDescriptor::Field(FieldDescriptor::Object(String::from("A"))),
        );
        assert_eq!(d1.cmp(&d2), Ordering::Greater);
        assert_eq!(d2.cmp(&d1), Ordering::Less);
    }
}
