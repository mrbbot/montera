use crate::class::{Class, ConstantPool, MethodId};
use crate::function::Function;
use anyhow::Context;
use classfile_parser::attribute_info::code_attribute_parser;
use classfile_parser::class_parser;
use classfile_parser::code_attribute::{code_parser, Instruction as JVMInstruction};
use classfile_parser::field_info::{FieldAccessFlags, FieldInfo};
use classfile_parser::method_info::{MethodAccessFlags, MethodInfo};
use std::collections::HashMap;
use std::mem::take;
use std::sync::{Arc, Mutex};

/// Parses the contents of a Java `.class` file as defined in [chapter 4] of the Java
/// Virtual Machine Specification, returning a [`Class`].
///
/// This will include the class name, the name of the super class, field offsets, total class
/// size, the constant pool, method signatures and code.
///
/// [chapter 4]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html
pub fn parse_class(data: &[u8]) -> anyhow::Result<Class> {
    // Parse class file
    let (_, mut class_file) = class_parser(&data).map_err(|_| anyhow!("Unable to parse class"))?;

    // Move constant pool out of class_file, parse it, then make class_file immutable
    let const_pool = take(&mut class_file.const_pool);
    let const_pool = Arc::new(ConstantPool::new(const_pool));
    let class_file = class_file;

    // Extract this and super class names
    let class_name = const_pool.class_name(class_file.this_class);
    let super_class_name = const_pool.class_name(class_file.super_class);

    // Extract class fields, relative offsets and total class size
    let (field_offsets, size) = parse_fields(&const_pool, &class_file.fields)?;

    // Parse all instance/static methods`
    let functions = class_file
        .methods
        .iter()
        .map(|method| parse_function(&class_name, &const_pool, method))
        .collect::<anyhow::Result<Vec<_>>>()?;

    // Build and return Class value
    let class = Class {
        class_name,
        super_class_name,
        size,
        field_offsets,
        const_pool,
        methods: functions,
    };
    Ok(class)
}

/// Parses class fields, returning field offsets and the total class size.
///
/// Static fields are currently ignored and dropped. Whilst these are required by assertions, they
/// are special-cased later on in compilation.
fn parse_fields(
    const_pool: &ConstantPool,
    fields: &[FieldInfo],
) -> anyhow::Result<(HashMap<Arc<String>, u32>, u32)> {
    let mut field_offsets = HashMap::new();
    let mut size = 0;

    for field in fields {
        // Extract name and descriptor (lazily parsing) from the constant pool
        let field_name = const_pool.str(field.name_index);
        let descriptor = const_pool.field_descriptor(field.descriptor_index);

        // Ignore static fields which are required by assertions
        if field.access_flags.contains(FieldAccessFlags::STATIC) {
            warn!(
                "Static fields are not yet supported, ignoring {}...",
                field_name
            );
            continue;
        }

        // Current size is the offset for this field
        let offset = size;
        field_offsets.insert(field_name, offset);
        size += descriptor.size();
    }

    Ok((field_offsets, size))
}

/// Parses a class static or instance method, including its code if any, returning a [`Function`].
///
/// Static class initializers are currently ignored and replaced with no-ops. Whilst these are
/// required by assertions, they are special-cased later on in compilation.
fn parse_function(
    class_name: &Arc<String>,
    const_pool: &Arc<ConstantPool>,
    method: &MethodInfo,
) -> anyhow::Result<Arc<Function>> {
    // Extract method name and descriptor from constant pool
    let name = const_pool.str(method.name_index);
    let descriptor = const_pool.method_descriptor(method.descriptor_index);

    // Parse function instructions (if any), ignoring class initializers, which are used by
    // assertions
    let code = if *name == "<clinit>" {
        warn!(
            "Class initializers fields are not yet supported, ignoring {}'s...",
            class_name
        );
        Some(vec![(0, JVMInstruction::Nop)])
    } else {
        parse_code(const_pool, method)
            .with_context(|| format!("Unable to parse code for {}", name))?
    };

    // Build and return Function value
    let id = MethodId {
        class_name: Arc::clone(class_name),
        name,
        descriptor: Arc::clone(&descriptor),
    };
    let function = Function {
        id,
        flags: method.access_flags,
        descriptor,
        const_pool: Arc::clone(const_pool),
        code: Mutex::new(code),
    };
    Ok(Arc::new(function))
}

/// Parses the code if any for a function.
///
/// Note `native` and `abstract` methods will return [`Option::None`] as they don't have a Java
/// implementation.
fn parse_code(
    const_pool: &ConstantPool,
    method: &MethodInfo,
) -> anyhow::Result<Option<Vec<(usize, JVMInstruction)>>> {
    // If this is a native/abstract function, it won't have any Java code
    if method
        .access_flags
        .intersects(MethodAccessFlags::NATIVE | MethodAccessFlags::ABSTRACT)
    {
        return Ok(None);
    }

    // Extract and parse code attribute
    let code_attr_info = method
        .attributes
        .iter()
        .find(|attr| *const_pool.str(attr.attribute_name_index) == "Code")
        .ok_or_else(|| anyhow!("Unable to find code"))?;
    let (_, code_attr) = code_attribute_parser(&code_attr_info.info)
        .map_err(|_| anyhow!("Unable to parse code attribute"))?;
    let (_, code) = code_parser(&code_attr.code).map_err(|_| anyhow!("Unable to parse code"))?;

    Ok(Some(code))
}

#[cfg(test)]
mod tests {
    use crate::class::JAVA_LANG_OBJECT;
    use crate::tests::{load_code, load_many_code};
    use classfile_parser::code_attribute::Instruction as JVMInstruction;
    use classfile_parser::method_info::MethodAccessFlags;
    use std::sync::Arc;

    #[test]
    fn parse_class_names() -> anyhow::Result<()> {
        let classes = load_many_code(
            "static class A {}
            static class B extends A {}
            static class C extends B {}",
        )?;

        assert_eq!(*classes["Test"].class_name, "Test");
        assert_eq!(*classes["Test$A"].class_name, "Test$A");
        assert_eq!(*classes["Test$B"].class_name, "Test$B");
        assert_eq!(*classes["Test$C"].class_name, "Test$C");

        assert_eq!(*classes["Test"].super_class_name, JAVA_LANG_OBJECT);
        assert_eq!(*classes["Test$A"].super_class_name, JAVA_LANG_OBJECT);
        assert_eq!(*classes["Test$B"].super_class_name, "Test$A");
        assert_eq!(*classes["Test$C"].super_class_name, "Test$B");

        Ok(())
    }

    #[test]
    fn parse_class_fields() -> anyhow::Result<()> {
        let class = load_code("int a; float b; long c; double d;")?;
        assert_eq!(class.size, 4 + 4 + 8 + 8);
        assert_eq!(class.field_offsets[&Arc::new(String::from("a"))], 0);
        assert_eq!(class.field_offsets[&Arc::new(String::from("b"))], 4);
        assert_eq!(class.field_offsets[&Arc::new(String::from("c"))], 4 + 4);
        assert_eq!(class.field_offsets[&Arc::new(String::from("d"))], 4 + 4 + 8);
        Ok(())
    }

    #[test]
    fn parse_class_skips_static_fields() -> anyhow::Result<()> {
        let class = load_code("int a; static long b; float c;")?;
        assert_eq!(class.size, 4 + 4);
        assert_eq!(class.field_offsets[&Arc::new(String::from("a"))], 0);
        assert_eq!(class.field_offsets[&Arc::new(String::from("c"))], 4);
        assert!(!class
            .field_offsets
            .contains_key(&Arc::new(String::from("b"))));
        Ok(())
    }

    #[test]
    fn parse_function_code_static_method() -> anyhow::Result<()> {
        let class = load_code("static int add(int a, int b) { return a + b; }")?;
        assert_eq!(class.methods.len(), 1 /* <init> */ + 1);
        assert_eq!(*class.methods[0].id.name, "<init>");

        // Check method signature
        let method = &class.methods[1];
        assert_eq!(format!("{}", method.id), "Test.add(II)I");
        assert_eq!(format!("{}", method.descriptor), "(II)I");
        assert_eq!(method.flags, MethodAccessFlags::STATIC);

        // Check constant pool equal class's
        assert!(Arc::ptr_eq(&class.const_pool, &method.const_pool));

        // Check code
        let code = method.code.lock().unwrap();
        let expected_code = vec![
            (0, JVMInstruction::Iload0),
            (1, JVMInstruction::Iload1),
            (2, JVMInstruction::Iadd),
            (3, JVMInstruction::Ireturn),
        ];
        assert_eq!(code.as_ref().unwrap(), &expected_code);

        Ok(())
    }

    #[test]
    fn parse_function_code_instance_method() -> anyhow::Result<()> {
        let class = load_code("public float sub(float a, float b) { return a - b; }")?;
        assert_eq!(class.methods.len(), 1 /* <init> */ + 1);
        assert_eq!(*class.methods[0].id.name, "<init>");

        // Check method signature
        let method = &class.methods[1];
        assert_eq!(format!("{}", method.id), "Test.sub(FF)F");
        assert_eq!(format!("{}", method.descriptor), "(FF)F");
        assert_eq!(method.flags, MethodAccessFlags::PUBLIC);

        // Check constant pool equal class's
        assert!(Arc::ptr_eq(&class.const_pool, &method.const_pool));

        // Check code
        let code = method.code.lock().unwrap();
        let expected_code = vec![
            (0, JVMInstruction::Fload1), // 0 is the implicit this parameter
            (1, JVMInstruction::Fload2),
            (2, JVMInstruction::Fsub),
            (3, JVMInstruction::Freturn),
        ];
        assert_eq!(code.as_ref().unwrap(), &expected_code);

        Ok(())
    }

    #[test]
    fn parse_function_abstract_method() -> anyhow::Result<()> {
        // `abstract` methods must belong to `abstract` `class`es or `interface`s
        let classes = load_many_code("static abstract class A { abstract int getSize(); }")?;
        let class = &classes["Test$A"];
        assert_eq!(class.methods.len(), 1 /* <init> */ + 1);
        assert_eq!(*class.methods[0].id.name, "<init>");

        // Check method signature
        let method = &class.methods[1];
        assert_eq!(format!("{}", method.id), "Test$A.getSize()I");
        assert_eq!(format!("{}", method.descriptor), "()I");
        assert_eq!(method.flags, MethodAccessFlags::ABSTRACT);

        // Check no implementation as abstract
        assert_eq!(*method.code.lock().unwrap(), None);

        Ok(())
    }

    #[test]
    fn parse_function_native_method() -> anyhow::Result<()> {
        let class = load_code("public native void log(int x);")?;
        assert_eq!(class.methods.len(), 1 /* <init> */ + 1);
        assert_eq!(*class.methods[0].id.name, "<init>");

        // Check method signature
        let method = &class.methods[1];
        assert_eq!(format!("{}", method.id), "Test.log(I)V");
        assert_eq!(format!("{}", method.descriptor), "(I)V");
        assert_eq!(
            method.flags,
            MethodAccessFlags::PUBLIC | MethodAccessFlags::NATIVE
        );

        // Check no Java implementation as native
        assert_eq!(*method.code.lock().unwrap(), None);

        Ok(())
    }

    #[test]
    fn parse_function_skips_class_initializer() -> anyhow::Result<()> {
        let class = load_code("static { System.out.println(\"Hello?\"); }")?;
        assert_eq!(class.methods.len(), 1 /* <init> */ + 1 /*<clinit> */);
        assert_eq!(*class.methods[0].id.name, "<init>");

        // Check code ignored
        let method = &class.methods[1];
        assert_eq!(*method.id.name, "<clinit>");
        let code = method.code.lock().unwrap();
        let expected_code = vec![(0, JVMInstruction::Nop)];
        assert_eq!(code.as_ref().unwrap(), &expected_code);

        Ok(())
    }
}
