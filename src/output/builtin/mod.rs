mod allocate;
mod compare;
mod instanceof;

pub use self::allocate::*;
pub use self::compare::*;
pub use self::instanceof::*;

/// Possible built-in functions for high-level JVM instructions that are not supported by
/// WebAssembly. These will be included once in the module only if required.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BuiltinFunction {
    /// See [`allocate::construct_allocate`] for more details.
    /// `[size: i32, virtual_class_id: i32] -> [ptr: i32]`
    Allocate,
    /// See [`instanceof::construct_instanceof`] for more details.
    /// `[ptr: i32, target_virtual_class_id: i32] -> [is: i32]`
    InstanceOf,

    /// See [`compare::construct_compare`] for more details.
    /// `[a: i64, b: i64] -> [ord: i32]`
    LongCmp,
    /// See [`compare::construct_compare`] for more details.
    /// `[a: f32, b: f32, nan_greater: i32] -> [ord: i32]`
    FloatCmp,
    /// See [`compare::construct_compare`] for more details.
    /// `[a: f64, b: f64, nan_greater: i32] -> [ord: i32]`
    DoubleCmp,
}

impl BuiltinFunction {
    /// Returns the name to use for the WebAssembly function corresponding to this built-in.
    pub fn name(&self) -> &'static str {
        match self {
            BuiltinFunction::Allocate => "!Allocate",
            BuiltinFunction::InstanceOf => "!InstanceOf",
            BuiltinFunction::LongCmp => "!LongCmp",
            BuiltinFunction::FloatCmp => "!FloatCmp",
            BuiltinFunction::DoubleCmp => "!DoubleCmp",
        }
    }
}
