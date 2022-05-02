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

#[cfg(test)]
mod tests {
    use crate::class::{
        FieldDescriptor, MethodDescriptor, MethodId, ReturnDescriptor, JAVA_LANG_OBJECT,
    };
    use crate::tests::{load_many_code, str_arc};
    use crate::VirtualTable;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[allow(non_snake_case)]
    #[test]
    fn from_classes() -> anyhow::Result<()> {
        // Construct virtual table from Java code
        let classes = load_many_code(
            "static abstract class Vehicle {
                private int wheels;
                public Vehicle(int wheels) { this.wheels = wheels; }
                public int getWheels() { return this.wheels; }
                abstract double getSpeed();
                public double travelTime(double distance) { return distance / this.getSpeed(); }
            }
            
            static class Bicycle extends Vehicle {
                private double frameSize;
                public Bicycle() { super(2); }
                public double getSpeed() { return 10.0; }
                public double getFrameSize() { return this.frameSize; }
            }

            static class Car extends Vehicle {
                public Car() { super(4); }
                public double getSpeed() { return 60.0; }
                public boolean isElectric() { return true; }
            }

            static class Van extends Car {
                public double getSpeed() { return 40.0; }
            }",
        )?;
        let classes = classes
            .into_iter()
            .filter(|(k, _v)| k != "Test")
            .map(|(k, v)| (Arc::new(k), v))
            .collect::<HashMap<_, _>>();
        let table = VirtualTable::from_classes(&Arc::new(classes));

        // Construct reference-counted class name strings
        let class_object = str_arc(JAVA_LANG_OBJECT);
        let class_bicycle = str_arc("Test$Bicycle");
        let class_car = str_arc("Test$Car");
        let class_van = str_arc("Test$Van");
        let class_vehicle = str_arc("Test$Vehicle");
        // Construct reference-counted method name strings
        let name_get_wheels = str_arc("getWheels");
        let name_get_speed = str_arc("getSpeed");
        let name_travel_time = str_arc("travelTime");
        let name_get_frame_size = str_arc("getFrameSize");
        let name_is_electric = str_arc("isElectric");

        // Extract virtual class indices and methods
        let index_object = &table.class_indices[&class_object];
        let index_bicycle = &table.class_indices[&class_bicycle];
        let index_car = &table.class_indices[&class_car];
        let index_van = &table.class_indices[&class_van];
        let index_vehicle = &table.class_indices[&class_vehicle];

        let methods_object = &table.inheritance_tree[index_object.node].value.methods;
        let methods_bicycle = &table.inheritance_tree[index_bicycle.node].value.methods;
        let methods_car = &table.inheritance_tree[index_car.node].value.methods;
        let methods_van = &table.inheritance_tree[index_van.node].value.methods;
        let methods_vehicle = &table.inheritance_tree[index_vehicle.node].value.methods;

        // Check virtual class IDs are ordered lexicographically with java/lang/Object first
        assert_eq!(index_object.id, 0);
        assert_eq!(index_bicycle.id, 1);
        assert_eq!(index_car.id, 6);
        assert_eq!(index_van.id, 11);
        assert_eq!(index_vehicle.id, 16);

        // Check virtual methods point to correct implementations
        assert_eq!(methods_object.len(), 0);

        assert_eq!(methods_vehicle.len(), 3);
        assert_eq!(methods_vehicle[0].name, name_get_wheels);
        assert_eq!(methods_vehicle[0].class_name, class_vehicle);
        assert_eq!(methods_vehicle[1].name, name_get_speed);
        assert_eq!(methods_vehicle[2].name, name_travel_time);
        assert_eq!(methods_vehicle[2].class_name, class_vehicle);

        assert_eq!(methods_bicycle.len(), 4);
        assert_eq!(methods_bicycle[0].name, name_get_wheels);
        assert_eq!(methods_bicycle[0].class_name, class_vehicle);
        assert_eq!(methods_bicycle[1].name, name_get_speed);
        assert_eq!(methods_bicycle[1].class_name, class_bicycle);
        assert_eq!(methods_bicycle[2].name, name_travel_time);
        assert_eq!(methods_bicycle[2].class_name, class_vehicle);
        assert_eq!(methods_bicycle[3].name, name_get_frame_size);
        assert_eq!(methods_bicycle[3].class_name, class_bicycle);

        assert_eq!(methods_car.len(), 4);
        assert_eq!(methods_car[0].name, name_get_wheels);
        assert_eq!(methods_car[0].class_name, class_vehicle);
        assert_eq!(methods_car[1].name, name_get_speed);
        assert_eq!(methods_car[1].class_name, class_car);
        assert_eq!(methods_car[2].name, name_travel_time);
        assert_eq!(methods_car[2].class_name, class_vehicle);
        assert_eq!(methods_car[3].name, name_is_electric);
        assert_eq!(methods_car[3].class_name, class_car);

        assert_eq!(methods_van.len(), 4);
        assert_eq!(methods_van[0].name, name_get_wheels);
        assert_eq!(methods_van[0].class_name, class_vehicle);
        assert_eq!(methods_van[1].name, name_get_speed);
        assert_eq!(methods_van[1].class_name, class_van);
        assert_eq!(methods_van[2].name, name_travel_time);
        assert_eq!(methods_van[2].class_name, class_vehicle);
        assert_eq!(methods_van[3].name, name_is_electric);
        assert_eq!(methods_van[3].class_name, class_car);

        // Check get_virtual_class_id returns correct value
        assert_eq!(table.get_virtual_class_id(&class_van), 11);

        // Check get_method_virtual_offset returns correct value
        let id = MethodId {
            class_name: class_car,
            name: name_get_speed,
            descriptor: Arc::new(MethodDescriptor::new(
                vec![],
                ReturnDescriptor::Field(FieldDescriptor::Double),
            )),
        };
        assert_eq!(table.get_method_virtual_offset(&id), 2); // +1 for super_id() function

        Ok(())
    }
}
