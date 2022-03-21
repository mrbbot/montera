use crate::class::{
    field_descriptor_parser, method_descriptor_parser, FieldDescriptor, FieldId, MethodDescriptor,
    MethodId,
};
use classfile_parser::constant_info::{ConstantInfo, NameAndTypeConstant};
use std::mem::take;
use std::sync::{Arc, RwLock, RwLockReadGuard};

// Shared base class for all Java classes
pub const JAVA_LANG_OBJECT: &str = "java/lang/Object";

#[derive(Debug, Copy, Clone)]
pub enum NumericConstant {
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
}

#[derive(Debug, Clone)]
pub enum Constant {
    String(Arc<String>),     // Utf8, String
    Number(NumericConstant), // Integer, Float, Long, Double
    Class(Arc<String>),
    FieldDescriptor(Arc<FieldDescriptor>),
    Field(FieldId),
    MethodDescriptor(Arc<MethodDescriptor>),
    Method(MethodId),
    Unusable, // InterfaceMethodRef, MethodHandle, MethodType, InvokeDynamic, Unusable
}

#[derive(Debug)]
pub struct ConstantPool {
    // RwLock used for thread-safe interior mutability as we'd like to lazily parse field/method
    // descriptors once when they're actually needed.
    inner: RwLock<Vec<Constant>>,
}

macro_rules! const_index {
    ($name:ident, $variant:path => $return:ty) => {
        pub fn $name(&self, index: u16) -> $return {
            let info = &self.inner.read().unwrap()[index as usize];
            match info {
                // For Arcs, clone() will just increment the reference count.
                // Other possible return values are NumericConstant which is Copy,
                // and FieldId/MethodId which are both collections of Arcs.
                $variant(value) => value.clone(),
                _ => unreachable!("Expected {}, got {:?}", stringify!($variant), info),
            }
        }
    };
    // Lazily parsed string constant
    ($name:ident, $variant:path => Arc<$return:ty>, $parser:ident) => {
        pub fn $name(&self, index: u16) -> Arc<$return> {
            let mut inner = self.inner.write().unwrap();
            let info = &mut inner[index as usize];
            match info {
                $variant(value) => Arc::clone(value),
                Constant::String(string_value) => {
                    let (_, descriptor) = $parser(&string_value).unwrap_or_else(|_| {
                        panic!("Unable to parse {} {}", stringify!($name), string_value)
                    });
                    let descriptor = Arc::new(descriptor);
                    *info = $variant(Arc::clone(&descriptor));
                    descriptor
                }
                _ => unreachable!(
                    "Expected {}/ConstantInfo::String, got {:?}",
                    stringify!($variant),
                    info
                ),
            }
        }
    };
}

fn name_and_type(const_pool: &[ConstantInfo], index: u16) -> &NameAndTypeConstant {
    match &const_pool[index as usize - 1] {
        ConstantInfo::NameAndType(value) => value,
        _ => unreachable!("Expected ConstantInfo::NameAndType"),
    }
}

impl ConstantPool {
    const_index!(str, Constant::String => Arc<String>);
    const_index!(num, Constant::Number => NumericConstant);
    const_index!(class_name, Constant::Class => Arc<String>);
    const_index!(field_descriptor, Constant::FieldDescriptor => Arc<FieldDescriptor>, field_descriptor_parser);
    const_index!(field, Constant::Field => FieldId);
    const_index!(method_descriptor, Constant::MethodDescriptor => Arc<MethodDescriptor>, method_descriptor_parser);
    const_index!(method, Constant::Method => MethodId);

    #[inline]
    fn set(&self, index: usize, value: Constant) {
        self.inner.write().unwrap()[index] = value;
    }

    pub fn new(mut const_pool: Vec<ConstantInfo>) -> Self {
        let inner = RwLock::new(vec![Constant::Unusable; const_pool.len() + 1]);
        let pool = Self { inner };

        // First load simple value constants (utf8, integer, float, long)
        for (i, info) in const_pool.iter_mut().enumerate() {
            let i = i + 1;
            match info {
                ConstantInfo::Utf8(value) => {
                    // Take the string out of the constant pool so we don't have to clone it,
                    // we'll be referencing our version from now anyways
                    pool.set(i, Constant::String(Arc::new(take(&mut value.utf8_string))))
                }
                ConstantInfo::Integer(value) => {
                    pool.set(i, Constant::Number(NumericConstant::Integer(value.value)))
                }
                ConstantInfo::Float(value) => {
                    pool.set(i, Constant::Number(NumericConstant::Float(value.value)))
                }
                ConstantInfo::Long(value) => {
                    pool.set(i, Constant::Number(NumericConstant::Long(value.value)))
                }
                ConstantInfo::Double(value) => {
                    pool.set(i, Constant::Number(NumericConstant::Double(value.value)))
                }
                _ => {}
            }
        }

        // Then load constants pointing to utf8 constants (string, class)
        for (i, info) in const_pool.iter().enumerate() {
            let i = i + 1;
            match info {
                ConstantInfo::String(value) => {
                    pool.set(i, Constant::String(pool.str(value.string_index)))
                }
                ConstantInfo::Class(value) => {
                    pool.set(i, Constant::Class(pool.str(value.name_index)))
                }
                _ => {}
            }
        }

        // Finally load constants pointing to classes (method, field)
        for (i, info) in const_pool.iter().enumerate() {
            let i = i + 1;
            match info {
                ConstantInfo::FieldRef(value) => {
                    let name_type = name_and_type(&const_pool, value.name_and_type_index);
                    let class_name = pool.class_name(value.class_index);
                    let name = pool.str(name_type.name_index);
                    let descriptor = pool.field_descriptor(name_type.descriptor_index);
                    let field_id = FieldId {
                        class_name,
                        name,
                        descriptor,
                    };
                    pool.set(i, Constant::Field(field_id))
                }
                ConstantInfo::MethodRef(value) => {
                    let name_type = name_and_type(&const_pool, value.name_and_type_index);
                    let class_name = pool.class_name(value.class_index);
                    let name = pool.str(name_type.name_index);
                    let descriptor = pool.method_descriptor(name_type.descriptor_index);
                    let method_id = MethodId {
                        class_name,
                        name,
                        descriptor,
                    };
                    pool.set(i, Constant::Method(method_id))
                }
                _ => {}
            }
        }

        pool
    }

    pub fn iter(&self) -> RwLockIter<Constant> {
        let inner = self.inner.read().unwrap();
        RwLockIter { inner }
    }
}

pub struct RwLockIter<'a, T: 'a> {
    inner: RwLockReadGuard<'a, Vec<T>>,
}

impl<'a, 'b: 'a, T: 'a> IntoIterator for &'b RwLockIter<'a, T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}
