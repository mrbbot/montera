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

fn parse_fields(
    const_pool: &ConstantPool,
    fields: &[FieldInfo],
) -> anyhow::Result<(HashMap<Arc<String>, u32>, u32)> {
    let mut field_offsets = HashMap::new();
    let mut size = 0;

    for field in fields {
        ensure!(
            !field.access_flags.contains(FieldAccessFlags::STATIC),
            "Static fields are not yet supported"
        );

        let field_name = const_pool.str(field.name_index);
        let descriptor = const_pool.field_descriptor(field.descriptor_index);

        let offset = size;
        field_offsets.insert(field_name, offset);
        size += descriptor.size();
    }

    Ok((field_offsets, size))
}

fn parse_function(
    class_name: &Arc<String>,
    const_pool: &Arc<ConstantPool>,
    method: &MethodInfo,
) -> anyhow::Result<Arc<Function>> {
    // Extract method name and descriptor from constant pool
    let name = const_pool.str(method.name_index);
    let descriptor = const_pool.method_descriptor(method.descriptor_index);

    // Parse function instructions (if any)
    let code = parse_code(const_pool, method)
        .with_context(|| format!("Unable to parse code for {}", name))?;

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

fn parse_code(
    const_pool: &ConstantPool,
    method: &MethodInfo,
) -> anyhow::Result<Option<Vec<(usize, JVMInstruction)>>> {
    // If this is a native function, it won't have any Java code
    if method.access_flags.contains(MethodAccessFlags::NATIVE) {
        return Ok(None);
    }

    // Extract and parse code attribute
    let code_attr_info = method
        .attributes
        .iter()
        .find(|attr| const_pool.str(attr.attribute_name_index).as_str() == "Code")
        .ok_or_else(|| anyhow!("Unable to find code"))?;
    let (_, code_attr) = code_attribute_parser(&code_attr_info.info)
        .map_err(|_| anyhow!("Unable to parse code attribute"))?;
    let (_, code) = code_parser(&code_attr.code).map_err(|_| anyhow!("Unable to parse code"))?;

    Ok(Some(code))
}
