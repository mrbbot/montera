use itertools::Itertools;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write;
use std::sync::Arc;
use wasm_encoder::ValType;

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
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

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FunctionType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

impl FunctionType {
    pub fn with_implicit_this(&self) -> Self {
        let mut func_type = self.clone();
        func_type.params.insert(0, ValType::I32);
        func_type
    }

    pub fn dispatcher_name(&self) -> String {
        let param_names = self.params.iter().copied().map(val_type_name).format("");
        let result_names = self.results.iter().copied().map(val_type_name).format("");
        format!("!Dispatcher_{}_{}", param_names, result_names)
    }
}

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct MethodDescriptor {
    pub params: Vec<FieldDescriptor>,
    pub returns: ReturnDescriptor,
    pub function_type: Arc<FunctionType>,
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
    pub fn new(params: Vec<FieldDescriptor>, returns: ReturnDescriptor) -> Self {
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
