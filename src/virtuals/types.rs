use crate::class::MethodId;
use crate::graph::{DotOptions, Graph, NodeId};
use crate::virtuals::construct::{construct_inheritance_tree, index_tree, populate_tree_methods};
use crate::Class;
use itertools::Itertools;
use log::Level;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::Arc;
use wasm_encoder::MemArg;

// Number of bytes required to store virtual class ID before fields begin
pub const VIRTUAL_CLASS_ID_SIZE: u32 = 4;
// Location of virtual class ID relative to instance pointers
pub const VIRTUAL_CLASS_ID_MEM_ARG: MemArg = MemArg {
    offset: 0,
    align: 2, // log2(4) = 2
    memory_index: 0,
};

pub struct VirtualClass {
    pub class_name: Arc<String>,
    // We reuse the MemberId struct but flip it: the class_name field represents the name of the
    // class containing the implementation that should be called when the method with the name and
    // descriptor is called on this class.
    pub methods: Vec<MethodId>,
}

impl VirtualClass {
    pub fn new(class_name: &Arc<String>) -> Self {
        Self {
            class_name: Arc::clone(class_name),
            methods: vec![],
        }
    }
}

impl fmt::Debug for VirtualClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.class_name)?;
        for (i, method) in self.methods.iter().enumerate() {
            write!(
                f,
                "\\l{id}: {name}{descriptor} -> {class_name}",
                id = i + 1,
                name = method.name,
                descriptor = method.descriptor,
                class_name = method.class_name,
            )?;
        }
        Ok(())
    }
}

pub struct VirtualClassIndex {
    // Index into virtual table's inheritance tree (graph)
    pub node: NodeId,
    // Index into final WebAssembly module's table
    pub id: u32,
}

pub struct VirtualTable {
    pub(super) classes: Arc<HashMap<Arc<String>, Class>>,
    pub(super) inheritance_tree: Graph<VirtualClass>,
    pub(super) class_indices: HashMap<Arc<String>, VirtualClassIndex>,
}

impl VirtualTable {
    pub fn from_classes(classes: &Arc<HashMap<Arc<String>, Class>>) -> Self {
        // Construct inheritance tree
        let mut inheritance_tree = construct_inheritance_tree(classes);

        // Add all possible methods that could be called on a class to the tree
        let root = inheritance_tree.entry_id().unwrap();
        populate_tree_methods(classes, &mut inheritance_tree, root, vec![]);

        // Build the class_indices map with entries mapping class names to IDs in the graph and
        // virtual class IDs into the final WebAssembly table
        let class_indices = index_tree(&inheritance_tree);

        Self {
            classes: Arc::clone(classes),
            inheritance_tree,
            class_indices,
        }
    }

    pub fn get_virtual_class_id(&self, class_name: &Arc<String>) -> i32 {
        i32::try_from(self.class_indices[class_name].id)
            .expect("Virtual class ID exceeded i32 bounds")
    }

    pub fn get_method_virtual_offset(&self, id: &MethodId) -> i32 {
        let node_id = self.class_indices[&id.class_name].node;
        let methods = &self.inheritance_tree[node_id].value.methods;
        methods
            .iter()
            .position(|method| method.name == id.name && method.descriptor == id.descriptor)
            .unwrap() as i32
            + 1 // for super_id() function
    }

    pub fn as_dot(&self) -> String {
        self.inheritance_tree.as_dot(&DotOptions {
            omit_node_ids: true,
            omit_branch_ids: true,
            subgraph: None,
        })
    }

    pub fn dump(&self) {
        if !log_enabled!(Level::Debug) {
            return;
        }
        debug!("Virtual Class Identifiers:");
        for (class_name, id) in self.class_indices.iter().sorted_by_key(|(_, id)| id.id) {
            debug!("{:>4}: {}", id.id, class_name);
        }
    }
}
