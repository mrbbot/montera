use crate::class::{Class, MethodId, JAVA_LANG_OBJECT};
use crate::graph::{Graph, NodeId};
use crate::virtuals::{VirtualClass, VirtualClassIndex};
use crate::VirtualTable;
use itertools::Itertools;
use std::collections::HashMap;
use std::sync::Arc;

impl VirtualTable {
    /// Constructs the unified virtual method table. See [`VirtualTable`].
    ///
    /// To do this, we construct an inheritance tree with nodes for all classes in the program, and
    /// edges connecting classes to their superclasses ([`construct_inheritance_tree`]). We then
    /// traverse the tree, adding all defined methods, and assign an index to them
    /// ([`populate_tree_methods`]). This gives a unique offset shared by all subclasses.
    /// Iterating classes deterministically gives a single virtual method table ([`index_tree`]).
    pub fn from_classes(classes: &Arc<HashMap<Arc<String>, Class>>) -> Self {
        // Construct inheritance tree
        let mut inheritance_tree = construct_inheritance_tree(classes);

        // Add all possible methods that could be called on a class to the tree
        let root = inheritance_tree.entry.unwrap();
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
}

/// Constructs an inheritance tree with nodes for all classes in the program, and edges connecting
/// classes to their superclasses. The root of this tree is `java/lang/Object`: the shared base
/// class for all classes in Java.
pub fn construct_inheritance_tree(classes: &HashMap<Arc<String>, Class>) -> Graph<VirtualClass> {
    // Create nodes for all classes, including shared base class Object
    let mut g = Graph::new();
    let mut class_nodes = HashMap::new();
    let java_lang_object = Arc::new(String::from(JAVA_LANG_OBJECT));
    let root = g.add_node(VirtualClass::new(&java_lang_object));
    class_nodes.insert(java_lang_object, root);
    // Sort by class name to make virtual table order deterministic
    // (Rust's HashMap contains built-in randomness)
    let sorted_names = classes.values().map(|class| &class.class_name).sorted();
    for class_name in sorted_names {
        let virtual_class = VirtualClass::new(&class_name);
        class_nodes.insert(Arc::clone(&class_name), g.add_node(virtual_class));
    }

    // Add inheritance relation
    for class in classes.values() {
        let super_node = class_nodes[&class.super_class_name];
        let this_node = class_nodes[&class.class_name];
        g.add_edge(super_node, this_node);
    }

    g
}

/// Add a list of methods callable on instances of each class and the class that providing the
/// implementation.
///
/// To build this, we copy all methods from the superclass, checking if the current class overrides
/// them. We then add all new methods defined in that class. Methods declared abstract have no
/// implementation but are still included.
pub fn populate_tree_methods(
    classes: &HashMap<Arc<String>, Class>,
    g: &mut Graph<VirtualClass>,
    current_id: NodeId,
    mut current_methods: Vec<MethodId>,
) {
    let class_name = &g[current_id].value.class_name;

    // Build a list of methods implemented/overridden by this class. Overridden methods
    // will already exist in the methods list, but should be updated to point to this class.
    // This ensures they share the same index all the way down the inheritance tree, allowing
    // this index to be used for dynamic dispatch. class_name may be "java/lang/Object" which
    // won't have an entry in classes, hence the `let Some(...)`.
    if let Some(class) = classes.get(class_name) {
        for method in &class.methods {
            if *method.id.name == "<init>" {
                // Ignore constructors, classes always (potentially implicitly) define their own
                // and they have special handling via the invokespecial JVM instruction
                continue;
            }
            let existing = current_methods
                .iter_mut()
                .find(|m| m.name == method.id.name && m.descriptor == method.id.descriptor);
            match existing {
                // If `methods` already contains a method with the same name and descriptor,
                // update it to point to this class' implementation instead
                Some(existing) => existing.class_name = Arc::clone(class_name),
                // Otherwise, add it to the end of `methods` (MethodIds are a collection of Arcs so clone is cheap)
                None => current_methods.push(method.id.clone()),
            }
        }
    }

    // Populate methods for all child classes, using this class' methods as a base
    // (clone() required here as mutable borrow passed to populate_tree_methods each iteration)
    for &subclass_id in &g[current_id].successors.clone() {
        populate_tree_methods(classes, g, subclass_id, current_methods.clone());
    }

    g[current_id].value.methods = current_methods;
}

/// Assign a unique virtual class ID to each class.
///
/// Because classes are added to the tree in lexicographic order, virtual class IDs will be
/// alphabetic, with `java/lang/Object` always having ID 0.
pub fn index_tree(g: &Graph<VirtualClass>) -> HashMap<Arc<String>, VirtualClassIndex> {
    let mut class_indices = HashMap::new();

    let mut offset = 0;
    for class in g.iter() {
        let index = VirtualClassIndex {
            node: class.id,
            id: offset,
        };
        class_indices.insert(Arc::clone(&class.value.class_name), index);
        offset += 1 + class.value.methods.len() as u32; // +1 for super_id() constant function
    }

    class_indices
}
