use crate::class::{
    field_descriptor_parser, method_descriptor_parser, FieldDescriptor, FieldId, MethodDescriptor,
    MethodId,
};
use classfile_parser::constant_info::{ConstantInfo, NameAndTypeConstant};
use std::mem::take;
use std::sync::{Arc, RwLock, RwLockReadGuard};

/// Shared base class for all Java classes.
pub const JAVA_LANG_OBJECT: &str = "java/lang/Object";

/// Possible types for [`Constant::Number`] constants.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NumericConstant {
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
}

/// Possible item inside [`ConstantPool`]s.
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    String(Arc<String>),     // Utf8, String
    Number(NumericConstant), // Integer, Float, Long, Double
    Class(Arc<String>),
    FieldDescriptor(Arc<FieldDescriptor>),
    Field(FieldId),
    MethodDescriptor(Arc<MethodDescriptor>),
    Method(MethodId),
    Unusable, // InterfaceMethodRef, MethodHandle, MethodType, InvokeDynamic
}

/// Parsed set of constants in a Java `.class` file, as defined in section [4.4] of the Java Virtual
/// Machine Specification.
///
/// Whilst the [`classfile_parser`] crate already provides excellent constant pool parsing, there
/// are a several issues:
///
/// - String constants are stored as owned [`String`]s which cannot be cloned without allocating
/// - Variants for high-level compound constants store indices into the pool, meaning for field/
///   method references, 3 indirections are required to access data
/// - Field/method descriptors are stored as [`String`]s, meaning they must be parsed into
///   [`FieldDescriptor`]/[`MethodDescriptor`]s each time
///   they're required
/// - Typed accessors are not provided
///
/// This `struct` solves these problems by rebuilding the constant pool with reference counts,
/// lazily parsing field/method descriptors when they're first required, and providing typed
/// accessor functions.
///
/// [4.4]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.4
#[derive(Debug)]
pub struct ConstantPool {
    // RwLock used for thread-safe interior mutability as we'd like to lazily parse field/method
    // descriptors once when they're actually needed.
    inner: RwLock<Vec<Constant>>,
}

/// Macro for defining a typed [`ConstantPool`] accessor.
///
/// These are functions that take a single index, check the constant at that index matches the
/// expected type, then return a (cheap) clone of the value.
///
/// If a third parser combinator argument is provided, the `String` constant is lazily parsed using
/// the combinator on first access, updating the constant pool.
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
                // If the string has already been parsed, just return it
                $variant(value) => Arc::clone(value),
                // Otherwise, parse it the first time it is accessed
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

/// [`NameAndTypeConstant`] typed accessor helper function for [`ConstantPool::new`].
fn name_and_type(const_pool: &[ConstantInfo], index: u16) -> &NameAndTypeConstant {
    match &const_pool[index as usize - 1] {
        ConstantInfo::NameAndType(value) => value,
        _ => unreachable!("Expected ConstantInfo::NameAndType"),
    }
}

impl ConstantPool {
    // Typed accessors
    const_index!(str, Constant::String => Arc<String>);
    const_index!(num, Constant::Number => NumericConstant);
    const_index!(class_name, Constant::Class => Arc<String>);
    const_index!(field_descriptor, Constant::FieldDescriptor => Arc<FieldDescriptor>, field_descriptor_parser);
    const_index!(field, Constant::Field => FieldId);
    const_index!(method_descriptor, Constant::MethodDescriptor => Arc<MethodDescriptor>, method_descriptor_parser);
    const_index!(method, Constant::Method => MethodId);

    /// Helper function for [`ConstantPool::new`] to avoid having to explicitly acquiring the write
    /// lock each time we want to set something.
    #[inline]
    fn set(&self, index: usize, value: Constant) {
        self.inner.write().unwrap()[index] = value;
    }

    /// Constructs a new `ConstantPool` using a parsed constant pool from [`class_parser`].
    ///
    /// See [`ConstantPool`] for a description of the issues this aims to solve.
    ///
    /// [`class_parser`]: classfile_parser::class_parser
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

    /// Returns an iterator over all `Constant`s in this pool in index order
    pub fn iter(&self) -> RwLockIter<Constant> {
        let inner = self.inner.read().unwrap();
        RwLockIter { inner }
    }
}

/// Struct for iterating over a [`Vec`] guarded by a [`RwLock`].
///
/// Adapted from [this comment](https://www.reddit.com/r/rust/comments/7l97u0/comment/drl07ke/).
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

#[cfg(test)]
mod tests {
    use crate::class::{
        Constant, ConstantPool, FieldDescriptor, MethodDescriptor, NumericConstant,
        ReturnDescriptor,
    };
    use crate::tests::load_code;
    use crate::Function;
    use classfile_parser::code_attribute::Instruction as JVMInstruction;
    use classfile_parser::constant_info::{ConstantInfo, Utf8Constant};
    use std::sync::Arc;

    /// Helper method finding constant index of first LDC instruction in implicit constructor.
    fn ldc_index(method: &Function) -> u16 {
        assert_eq!(*method.id.name, "<init>");
        let code_guard = method.code.lock().unwrap();
        let code = code_guard.as_ref().unwrap();
        let index = code.iter().find_map(|(_, instruction)| match instruction {
            JVMInstruction::Ldc(index) => Some(*index as u16),
            JVMInstruction::LdcW(index) | JVMInstruction::Ldc2W(index) => Some(*index),
            _ => None,
        });
        index.unwrap()
    }

    #[test]
    fn constant_string() -> anyhow::Result<()> {
        // Constant will be loaded in implicit constructor, find index from LDC instruction
        let class = load_code("String msg = \"Hello\";")?;
        let index = ldc_index(&class.methods[0]);
        assert_eq!(*class.const_pool.str(index), "Hello");
        Ok(())
    }

    #[test]
    fn constant_number() -> anyhow::Result<()> {
        // Constant will be loaded in implicit constructors, find indices from LDC instructions

        // Integer constants must exceed `Short.MAX_VALUE` (32767) to use the LDC instruction,
        // otherwise the BIPUSH/SIPUSH instructions with integer immediates are used
        let class = load_code("int i = 32768;")?;
        let index = ldc_index(&class.methods[0]);
        assert_eq!(class.const_pool.num(index), NumericConstant::Integer(32768));

        let class = load_code("float f = 42f;")?;
        let index = ldc_index(&class.methods[0]);
        assert_eq!(class.const_pool.num(index), NumericConstant::Float(42.0));

        let class = load_code("long l = 42L;")?;
        let index = ldc_index(&class.methods[0]);
        assert_eq!(class.const_pool.num(index), NumericConstant::Long(42));

        let class = load_code("double d = 42.0;")?;
        let index = ldc_index(&class.methods[0]);
        assert_eq!(class.const_pool.num(index), NumericConstant::Double(42.0));

        Ok(())
    }

    #[test]
    fn constant_class() -> anyhow::Result<()> {
        let class = load_code("Class c = Test.class;")?;
        let index = ldc_index(&class.methods[0]);
        assert_eq!(*class.const_pool.class_name(index), "Test");
        Ok(())
    }

    #[test]
    fn constant_field_descriptor() {
        let utf8_string = String::from("I");
        let pool = ConstantPool::new(vec![ConstantInfo::Utf8(Utf8Constant {
            utf8_string: utf8_string.clone(),
            bytes: utf8_string.into_bytes(),
        })]);
        // Check initially stored as String...
        assert_eq!(
            pool.inner.read().unwrap()[1],
            Constant::String(Arc::new(String::from("I")))
        );
        // ...then lazily converted to FieldDescriptor on first access
        assert_eq!(*pool.field_descriptor(1), FieldDescriptor::Int);
        assert_eq!(
            pool.inner.read().unwrap()[1],
            Constant::FieldDescriptor(Arc::new(FieldDescriptor::Int))
        );
        assert_eq!(*pool.field_descriptor(1), FieldDescriptor::Int);
    }

    #[test]
    fn constant_field() -> anyhow::Result<()> {
        // Field will be accessed in implicit constructor, find index from PUTFIELD instruction
        let class = load_code("int i = 42;")?;
        let method = &class.methods[0];
        assert_eq!(*method.id.name, "<init>");
        let code_guard = method.code.lock().unwrap();
        let code = code_guard.as_ref().unwrap();
        let index = code.iter().find_map(|(_, instruction)| match instruction {
            JVMInstruction::Putfield(index) => Some(*index),
            _ => None,
        });
        let index = index.unwrap();

        // Check field parsed correctly
        let field_id = class.const_pool.field(index);
        assert_eq!(*field_id.class_name, "Test");
        assert_eq!(*field_id.name, "i");
        assert_eq!(*field_id.descriptor, FieldDescriptor::Int);

        Ok(())
    }

    #[test]
    fn constant_method_descriptor() {
        let utf8_string = String::from("(F)V");
        let pool = ConstantPool::new(vec![ConstantInfo::Utf8(Utf8Constant {
            utf8_string: utf8_string.clone(),
            bytes: utf8_string.into_bytes(),
        })]);
        // Check initially stored as String...
        assert_eq!(
            pool.inner.read().unwrap()[1],
            Constant::String(Arc::new(String::from("(F)V")))
        );
        // ...then lazily converted to MethodDescriptor on first access
        let expected_descriptor = Arc::new(MethodDescriptor::new(
            vec![FieldDescriptor::Float],
            ReturnDescriptor::Void,
        ));
        assert_eq!(pool.method_descriptor(1), expected_descriptor);
        assert_eq!(
            pool.inner.read().unwrap()[1],
            Constant::MethodDescriptor(expected_descriptor.clone())
        );
        assert_eq!(pool.method_descriptor(1), expected_descriptor);
    }

    #[test]
    fn constant_method() -> anyhow::Result<()> {
        // Method will be call in constructor, find index from INVOKESTATIC instruction
        let class = load_code(
            "Test() { succ(1); }
            static int succ(int x) { return x + 1; }",
        )?;
        let method = &class.methods[0];
        assert_eq!(*method.id.name, "<init>");
        let code_guard = method.code.lock().unwrap();
        let code = code_guard.as_ref().unwrap();
        let index = code.iter().find_map(|(_, instruction)| match instruction {
            JVMInstruction::Invokestatic(index) => Some(*index),
            _ => None,
        });
        let index = index.unwrap();

        // Check method parsed correctly
        let method_id = class.const_pool.method(index);
        assert_eq!(*method_id.class_name, "Test");
        assert_eq!(*method_id.name, "succ");
        let expected_descriptor = Arc::new(MethodDescriptor::new(
            vec![FieldDescriptor::Int],
            ReturnDescriptor::Field(FieldDescriptor::Int),
        ));
        assert_eq!(method_id.descriptor, expected_descriptor);

        Ok(())
    }
}
