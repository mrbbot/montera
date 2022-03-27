use crate::class::{ConstantPool, FieldId, MethodDescriptor, MethodId};
use crate::function::locals::LocalInterpretation;
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use classfile_parser::method_info::MethodAccessFlags;
use std::sync::{Arc, Mutex};
use wasm_encoder::Instruction as WASMInstruction;

#[derive(Debug)]
pub struct Function {
    pub id: MethodId,
    pub flags: MethodAccessFlags,
    pub descriptor: Arc<MethodDescriptor>,
    pub const_pool: Arc<ConstantPool>,
    // Mutex provides interior mutability, we want to take ownership of the code when structuring
    pub code: Mutex<Option<Vec<(usize, JVMInstruction)>>>,
}

#[derive(Debug)]
pub enum Instruction<'a> {
    // Simple instructions
    I(WASMInstruction<'a>),

    // Complex pseudo-instructions requiring virtual method tables or built-in functions,
    // lowered to simple instructions when rendering final WebAssembly module

    // [value: t] -> [value: t, value: t]
    Dup,

    // [] -> [ptr: i32]
    New(Arc<String>),
    // [ptr: i32] -> [is: i32]
    InstanceOf(Arc<String>),

    // [this: i32] -> [value: t]
    GetField(FieldId),
    // [this: i32, value: t] -> []
    PutField(FieldId),

    // [...] -> [return: t]
    CallStatic(MethodId),
    // [this: i32, ...] -> [return: t]
    CallVirtual(MethodId),

    // [a: i64, b: i64] -> [ord: i32]
    LongCmp,
    // [a: f32, b: f32] -> [ord: i32]
    FloatCmp(NaNBehaviour),
    // [a: f64, b: f64] -> [ord: i32]
    DoubleCmp(NaNBehaviour),
}

#[derive(Debug, Copy, Clone)]
pub enum NaNBehaviour {
    Greater, // If either a or b is NaN, say a > b
    Lesser,  // If either a or b is NaN, say a < b
}

impl NaNBehaviour {
    pub fn as_nan_greater_int(&self) -> i32 {
        // As expected by `nan_greater` parameter in output/builtin/compare.rs
        match self {
            NaNBehaviour::Greater => 1,
            NaNBehaviour::Lesser => 0,
        }
    }
}

#[derive(Debug)]
pub struct CompiledFunction {
    pub id: MethodId,
    pub flags: MethodAccessFlags,
    pub descriptor: Arc<MethodDescriptor>,
    // TODO (someday): maybe split this out into separate struct, then only one Option<...>,
    //  then assert is_none() in visit_import() to remove expect()s in visit_function()
    pub locals: Option<Arc<LocalInterpretation>>,
    pub code: Option<Vec<Instruction<'static>>>,
}

impl CompiledFunction {
    pub fn is_import(&self) -> bool {
        self.flags.contains(MethodAccessFlags::NATIVE)
    }

    pub fn is_static(&self) -> bool {
        self.flags.contains(MethodAccessFlags::STATIC)
    }

    pub fn is_export(&self) -> bool {
        self.flags
            .contains(MethodAccessFlags::PUBLIC | MethodAccessFlags::STATIC)
    }
}
