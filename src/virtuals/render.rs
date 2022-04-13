use crate::class::{FunctionType, MethodId, JAVA_LANG_OBJECT};
use crate::{Module, VirtualTable};
use itertools::Itertools;
use std::collections::HashMap;
use std::iter::once;
use std::sync::Arc;
use wasm_encoder::{
    Elements, Function as WASMFunction, Instruction as WASMInstruction, TableType, ValType,
};

impl VirtualTable {
    pub fn render(&self, out: &mut Module, function_indices: &HashMap<MethodId, u32>) {
        // Get type of super ID functions: [] -> [super_vid: i32]
        let super_id_func_type = Arc::new(FunctionType {
            params: vec![],
            results: vec![ValType::I32],
        });
        let super_id_type_index = out.ensure_type(&super_id_func_type);

        let mut iter = self.inheritance_tree.iter();
        // First node in tree should always be java/lang/Object
        assert_eq!(*iter.next().unwrap().value.class_name, JAVA_LANG_OBJECT);

        // Add table elements, java/lang/Object has no methods, but leave a null where it's
        // super_id() function would go
        let mut offset = 1;
        for node in iter {
            let class = &self.classes[&node.value.class_name];

            // Get indices of all methods callable on this class in the final output
            let method_indices = node
                .value
                .methods
                .iter()
                .map(|method| function_indices[method]);

            // Render the function returning the superclass' virtual ID for this class
            // TODO: extract out into separate function, maybe move to output module so we
            //  can keep visibility on Module's fields pub(super)
            let super_id = self.get_virtual_class_id(&class.super_class_name);
            let mut f = WASMFunction::new(vec![]);
            f.instruction(&WASMInstruction::I32Const(super_id))
                .instruction(&WASMInstruction::End);
            let super_id_index = out.next_function_index;
            out.next_function_index += 1;
            out.functions.function(super_id_type_index);
            out.codes.function(&f);
            out.function_names
                .append(super_id_index, &format!("!Super_{}", class.class_name));

            // Add indices to table in output module
            let function_indices = once(super_id_index).chain(method_indices).collect_vec();
            out.elements.active(
                None,
                &WASMInstruction::I32Const(offset as i32),
                ValType::FuncRef,
                Elements::Functions(&function_indices),
            );

            offset += 1 + node.value.methods.len() as u32; // +1 for super_id() function
        }

        // Add known fixed-size table (will be rendered before elements in final output)
        out.tables.table(TableType {
            element_type: ValType::FuncRef,
            minimum: offset,
            maximum: Some(offset),
        });
    }
}
