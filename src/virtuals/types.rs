use crate::class::MethodId;
use crate::graph::{DotOptions, Graph, NodeId};
use crate::Class;
use itertools::Itertools;
use log::Level;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::Arc;
use wasm_encoder::MemArg;

/// Number of bytes required to store virtual class ID before fields begin.
pub const VIRTUAL_CLASS_ID_SIZE: u32 = 4;
/// Location of virtual class ID relative to instance pointers.
pub const VIRTUAL_CLASS_ID_MEM_ARG: MemArg = MemArg {
    offset: 0,
    align: 2, // log2(4) = 2
    memory_index: 0,
};

/// Maps all methods callable on a class to their implementations. Used as node values in the
/// inheritance tree when constructing the virtual table.
pub struct VirtualClass {
    pub class_name: Arc<String>,
    /// List of methods callable on this class. This includes (abstract) methods defined by this
    /// class, and any methods defined in superclasses.
    ///
    /// We reuse the [`MemberId`] struct but flip it: the `class_name` field represents the name of
    /// the class containing the implementation that should be called when the method with the
    /// `name` and `descriptor` is called on this class.
    pub methods: Vec<MethodId>,
}

impl VirtualClass {
    /// Constructs a new named virtual class, with an empty list of methods.
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

/// Indices for each [`VirtualClass`].
pub struct VirtualClassIndex {
    /// Index into virtual table's inheritance tree (graph).
    pub node: NodeId,
    /// Index in the final output WebAssembly module's table. This is the **Virtual Class ID**.
    pub id: u32,
}

/// Unified virtual method table for all classes in a module.
///
/// Java supports dynamic dispatch. A common approach to implementing this is to define a virtual
/// method table per class, mapping each method to its implementation and storing a reference to
/// this table with each instance.
///
/// WebAssembly only supports a single table per module, so we need to merge all class's virtual
/// tables into one. Each class is assigned a virtual class ID that gets stored with each instance.
/// Methods are identified by their offset from this virtual class ID which is shared by all
/// subclasses. The table element at the virtual class ID references a constant function returning
/// the virtual class ID of the superclass for `instanceof` checking.
///
/// See [`VirtualTable::from_classes`] for more details on construction.
///
/// # Dynamic Dispatch
///
/// Method offsets required to index the virtual method table are relative to the virtual class IDs
/// stored with each instance. The calling convention for instance methods is to have an implicit
/// `this` parameter followed by any others on the top of the stack. This means finding the correct
/// implementation's index requires accessing a `this` reference underneath an arbitrary number of
/// parameters in the stack. Random stack access is not supported by WebAssembly. To solve this
/// problem, we define a dispatcher function accepting the required number of parameters (including
/// the implicit this). WebAssembly functions store their parameters as local variables, allowing
/// easy copying of the implicit this and computing of the virtual method table index using a single
/// add for `call_indirect`. Note that a unique dispatcher function is required for each instance
/// method parameter and return type combination used, as WebAssembly does not support user-defined
/// polymorphic functions.
pub struct VirtualTable {
    pub(super) classes: Arc<HashMap<Arc<String>, Class>>,
    pub(super) inheritance_tree: Graph<VirtualClass>,
    pub(super) class_indices: HashMap<Arc<String>, VirtualClassIndex>,
}

impl VirtualTable {
    /// Returns the virtual class ID for a class included in this virtual table.
    ///
    /// This will be included in the first 4 bytes of all instances of this class.
    /// All table method offsets will be relative to this ID.
    pub fn get_virtual_class_id(&self, class_name: &Arc<String>) -> i32 {
        i32::try_from(self.class_indices[class_name].id)
            .expect("Virtual class ID exceeded i32 bounds")
    }

    /// Returns the virtual method offset for a method included in this virtual table.
    ///
    /// This offset will be relative to a virtual class ID.
    pub fn get_method_virtual_offset(&self, id: &MethodId) -> i32 {
        let node_id = self.class_indices[&id.class_name].node;
        let methods = &self.inheritance_tree[node_id].value.methods;
        methods
            .iter()
            .position(|method| method.name == id.name && method.descriptor == id.descriptor)
            .unwrap() as i32
            + 1 // for super_id() function
    }

    /// Converts the inheritance tree used to construct the virtual method table to the
    /// [Graphviz DOT Language] for visualisation and debugging.
    ///
    /// [Graphviz DOT Language]: https://graphviz.org/doc/info/lang.html
    pub fn as_dot(&self) -> String {
        self.inheritance_tree.as_dot(&DotOptions {
            omit_node_ids: true,
            omit_branch_ids: true,
            subgraph: None,
        })
    }

    /// Logs all virtual class IDs to the console at log level [`Level::Debug`].
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
