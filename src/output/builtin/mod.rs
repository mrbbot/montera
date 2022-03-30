mod allocate;
mod compare;
mod instanceof;

pub use self::allocate::*;
pub use self::compare::*;
pub use self::instanceof::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BuiltinFunction {
    // [size: i32, virtual_class_id: i32] -> [ptr: i32]
    Allocate,
    // [ptr: i32, target_virtual_class_id: i32] -> [is: i32]
    InstanceOf,

    // [a: i64, b: i64] -> [ord: i32]
    LongCmp,
    // [a: f32, b: f32, nan_greater: i32] -> [ord: i32]
    FloatCmp,
    // [a: f64, b: f64, nan_greater: i32] -> [ord: i32]
    DoubleCmp,
}

impl BuiltinFunction {
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
