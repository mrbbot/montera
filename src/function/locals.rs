use crate::class::FieldDescriptor;
use crate::function::Instruction;
use crate::function::Instruction::I;
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use itertools::Itertools;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use wasm_encoder::Instruction as WASMInstruction;
use wasm_encoder::ValType;

/// Returns the JVM stack index and expected WebAssembly type accessed by an instruction (if any).
///
/// The instruction may be a load, store or both (IINC). References are treated as `i32` pointers.
///
/// This is a helper function for [`LocalInterpretation::from_code`].
fn instruction_local(instruction: &JVMInstruction) -> Option<(u32, ValType)> {
    match instruction {
        // References
        // TODO (someday): might be nice to keep these as a separate type for GCing
        JVMInstruction::Aload(n) | JVMInstruction::Astore(n) => Some((*n as u32, ValType::I32)),
        JVMInstruction::AloadWide(n) | JVMInstruction::AstoreWide(n) => {
            Some((*n as u32, ValType::I32))
        }
        JVMInstruction::Aload0 | JVMInstruction::Astore0 => Some((0, ValType::I32)),
        JVMInstruction::Aload1 | JVMInstruction::Astore1 => Some((1, ValType::I32)),
        JVMInstruction::Aload2 | JVMInstruction::Astore2 => Some((2, ValType::I32)),
        JVMInstruction::Aload3 | JVMInstruction::Astore3 => Some((3, ValType::I32)),

        // Doubles
        JVMInstruction::Dload(n) | JVMInstruction::Dstore(n) => Some((*n as u32, ValType::F64)),
        JVMInstruction::DloadWide(n) | JVMInstruction::DstoreWide(n) => {
            Some((*n as u32, ValType::F64))
        }
        JVMInstruction::Dload0 | JVMInstruction::Dstore0 => Some((0, ValType::F64)),
        JVMInstruction::Dload1 | JVMInstruction::Dstore1 => Some((1, ValType::F64)),
        JVMInstruction::Dload2 | JVMInstruction::Dstore2 => Some((2, ValType::F64)),
        JVMInstruction::Dload3 | JVMInstruction::Dstore3 => Some((3, ValType::F64)),

        // Floats
        JVMInstruction::Fload(n) | JVMInstruction::Fstore(n) => Some((*n as u32, ValType::F32)),
        JVMInstruction::FloadWide(n) | JVMInstruction::FstoreWide(n) => {
            Some((*n as u32, ValType::F32))
        }
        JVMInstruction::Fload0 | JVMInstruction::Fstore0 => Some((0, ValType::F32)),
        JVMInstruction::Fload1 | JVMInstruction::Fstore1 => Some((1, ValType::F32)),
        JVMInstruction::Fload2 | JVMInstruction::Fstore2 => Some((2, ValType::F32)),
        JVMInstruction::Fload3 | JVMInstruction::Fstore3 => Some((3, ValType::F32)),

        // Integers
        JVMInstruction::Iinc { index, .. } => Some((*index as u32, ValType::I32)),
        JVMInstruction::IincWide { index, .. } => Some((*index as u32, ValType::I32)),
        JVMInstruction::Iload(n) | JVMInstruction::Istore(n) => Some((*n as u32, ValType::I32)),
        JVMInstruction::IloadWide(n) | JVMInstruction::IstoreWide(n) => {
            Some((*n as u32, ValType::I32))
        }
        JVMInstruction::Iload0 | JVMInstruction::Istore0 => Some((0, ValType::I32)),
        JVMInstruction::Iload1 | JVMInstruction::Istore1 => Some((1, ValType::I32)),
        JVMInstruction::Iload2 | JVMInstruction::Istore2 => Some((2, ValType::I32)),
        JVMInstruction::Iload3 | JVMInstruction::Istore3 => Some((3, ValType::I32)),

        // Longs
        JVMInstruction::Lload(n) | JVMInstruction::Lstore(n) => Some((*n as u32, ValType::I64)),
        JVMInstruction::LloadWide(n) | JVMInstruction::LstoreWide(n) => {
            Some((*n as u32, ValType::I64))
        }
        JVMInstruction::Lload0 | JVMInstruction::Lstore0 => Some((0, ValType::I64)),
        JVMInstruction::Lload1 | JVMInstruction::Lstore1 => Some((1, ValType::I64)),
        JVMInstruction::Lload2 | JVMInstruction::Lstore2 => Some((2, ValType::I64)),
        JVMInstruction::Lload3 | JVMInstruction::Lstore3 => Some((3, ValType::I64)),

        _ => None,
    }
}

/// Returns the number of JVM words (32-bit integers) occupied by a WebAssembly type.
fn type_word_count(t: ValType) -> u32 {
    match t {
        ValType::I32 | ValType::F32 => 1,
        ValType::I64 | ValType::F64 => 2,
        _ => unimplemented!("{:?}", t),
    }
}

/// Mapping between JVM stack index and WebAssembly type pairs and their WebAssembly local indices
/// for a function.
///
/// # Overview
///
/// JVM stack frames contain zero-index array of 4-byte words for local variables, with method
/// parameters stored at the start. `boolean`s, `byte`s, `char`s, `short`s, `int`s, `float`s and
/// references occupy a single word. `long`s and `double`s occupy two consecutive words and are
/// accessed using the lower index. Local variables are addressed as indices into this array. If a
/// variable goes out of scope, its associated local slots may be reused by a new variables with
/// potentially different types.
///
/// WebAssembly locals are statically typed, requiring the type of each local to be defined ahead of
/// time. This causes some problems:
///
/// - Class files only define the maximum size of the local variable array for each method, not the
///   stored type at each index, so this needs to be inferred from the method signature and
///   instructions
/// - WebAssembly locals always occupy a single "slot" regardless of their type and size. This means
///   indices in JVM instructions cannot be used directly.
/// - Static typing means local slots cannot change their type part-way through a function call,
///   even if the previous local is dead and no longer used.
///
/// A solution to this problem is to remap unique JVM local variable index and WebAssembly type
/// pairs to WebAssembly locals (see [`instruction_local`] for extracting these).
#[derive(Debug)]
pub struct LocalInterpretation {
    /// Mapping between JVM stack index and WebAssembly type pairs and their WebAssembly locals.
    map: HashMap<(u32, ValType), u32>,
    /// Index where function parameters end and local variables start for run-length-encoding
    /// WebAssembly locals in the function body.
    local_start: u32,
}

impl LocalInterpretation {
    /// Constructs a new `LocalInterpretation` from method parameter descriptors and JVM byte`code`.
    /// If this isn't a `static` method, an implicit this parameter is assumed.
    pub fn from_code(
        is_static: bool,
        params: &[FieldDescriptor],
        code: &[(usize, JVMInstruction)],
    ) -> Self {
        let mut map = HashMap::new();
        let mut java_stack_index = 0;
        let mut wasm_local_index = 0;

        // Add implicit this parameter first if this isn't a static method
        if !is_static {
            map.insert((java_stack_index, ValType::I32), wasm_local_index);
            java_stack_index += 1;
            wasm_local_index += 1;
        }

        // Add method parameters next
        for param in params {
            let t = param.as_type();
            map.insert((java_stack_index, t), wasm_local_index);
            java_stack_index += type_word_count(t);
            wasm_local_index += 1;
        }

        // Only need this for parameters, instructions have their stack index encoded
        drop(java_stack_index);

        // Record index where local variables start
        let local_start = wasm_local_index;

        // Make sure each local-referencing instruction has a local matching its expected type
        for (_, instruction) in code {
            if let Some((instruction_index, instruction_type)) = instruction_local(instruction) {
                let entry = map.entry((instruction_index, instruction_type));
                if let Entry::Vacant(entry) = entry {
                    entry.insert(wasm_local_index);
                    wasm_local_index += 1;
                }
            }
        }

        LocalInterpretation { map, local_start }
    }

    /// Returns the corresponding WebAssembly local index for a unique JVM `stack_index` and
    /// WebAssembly `t`ype pair.
    pub fn get_local_index(&self, t: ValType, stack_index: u32) -> u32 {
        match self.map.get(&(stack_index, t)) {
            Some(local_index) => *local_index,
            None => panic!("Unable to find local index: {} @ {:?}", stack_index, t),
        }
    }

    /// Helper writing a `local_get` WebAssembly instruction from the corresponding WebAssembly
    /// local index for a unique JVM `stack_index` index and WebAssembly `t`ype pair.
    #[inline]
    pub fn get(&self, out: &mut Vec<Instruction>, t: ValType, index: u32) {
        out.push(I(WASMInstruction::LocalGet(self.get_local_index(t, index))));
    }

    /// Helper writing a `local_set` WebAssembly instruction to the corresponding WebAssembly local
    /// index for a unique JVM `stack_index` and WebAssembly `t`ype pair.
    #[inline]
    pub fn set(&self, out: &mut Vec<Instruction>, t: ValType, index: u32) {
        out.push(I(WASMInstruction::LocalSet(self.get_local_index(t, index))));
    }

    /// Returns the number of unique (JVM stack index and WebAssembly type pairs)/WebAssembly locals
    /// (including parameters) mapped to by this interpretation.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns the run-length encoding of this functions local variables for the code section
    /// (excluding parameters).
    ///
    /// Optionally, a set of additional local variables can be included (e.g. for temporaries).
    pub fn run_length_encode(&self, append: &[ValType]) -> Vec<(u32, ValType)> {
        // Get all local variables (excluding parameters), appending `append`
        let locals = self
            .map
            .iter()
            // Ignore function parameters
            .filter(|(_, local_index)| **local_index >= self.local_start)
            // Sort by local index so final ordering is correct (HashMap's have random order)
            .sorted_by_key(|(_, local_index)| **local_index)
            // Extract just the ValType
            .map(|((_, t), _)| *t)
            // Add any extra types on the end (e.g. scratch for Dup)
            .chain(append.into_iter().copied());

        // Perform run length encoding
        let mut result = vec![];
        let mut last = None;
        let mut length = 0;
        for t in locals {
            match last {
                None => {
                    last = Some(t);
                }
                Some(last_t) if last_t != t => {
                    result.push((length, last_t));
                    last = Some(t);
                    length = 0;
                }
                _ => {}
            }
            length += 1;
        }
        if let Some(last) = last {
            result.push((length, last))
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::class::FieldDescriptor;
    use crate::function::locals::LocalInterpretation;
    use classfile_parser::code_attribute::Instruction as JVMInstruction;
    use wasm_encoder::ValType;

    #[test]
    fn locals_from_static_method() {
        let params = [FieldDescriptor::Int, FieldDescriptor::Double];
        let code = [
            (0, JVMInstruction::Iload0),
            (1, JVMInstruction::I2d), // Check ignores non-local-referencing instructions
            (2, JVMInstruction::Dload1),
            (3, JVMInstruction::Dadd),
            (4, JVMInstruction::Fload3),
        ];
        let locals = LocalInterpretation::from_code(true, &params, &code);
        let expected_map = hashmap! {
            (0, ValType::I32) => 0,
            (1, ValType::F64) => 1,
            (3, ValType::F32) => 2,
        };
        assert_eq!(locals.map, expected_map);
    }

    #[test]
    fn locals_from_instance_method() {
        let params = [
            FieldDescriptor::Long,
            FieldDescriptor::Float,
            FieldDescriptor::Object(String::from("Test")),
        ];
        let code = [
            (0, JVMInstruction::Aload0),
            (1, JVMInstruction::Lload1),
            (2, JVMInstruction::Fload3),
            (3, JVMInstruction::Aload(4)),
        ];
        let locals = LocalInterpretation::from_code(false, &params, &code);
        let expected_map = hashmap! {
            (0, ValType::I32) => 0, // Implicit this
            (1, ValType::I64) => 1,
            (3, ValType::F32) => 2,
            (4, ValType::I32) => 3,
        };
        assert_eq!(locals.map, expected_map);
    }

    #[test]
    fn locals_with_slot_reuse() {
        let params = [FieldDescriptor::Int, FieldDescriptor::Double];
        let code = [
            (0, JVMInstruction::Iconst0),
            (1, JVMInstruction::Istore0),
            (2, JVMInstruction::Dconst0),
            (3, JVMInstruction::Dstore1),
            (4, JVMInstruction::Fconst0),
            (5, JVMInstruction::Fstore1), // Reusing double's slot for float
        ];
        let locals = LocalInterpretation::from_code(true, &params, &code);
        let expected_map = hashmap! {
            (0, ValType::I32) => 0,
            (1, ValType::F64) => 1,
            (1, ValType::F32) => 2,
        };
        assert_eq!(locals.map, expected_map);
    }

    #[test]
    fn locals_run_length_append() {
        let params = [FieldDescriptor::Int];
        let code = [
            (0, JVMInstruction::Aload0),
            (1, JVMInstruction::Iload1),
            (2, JVMInstruction::Iload2),
            (3, JVMInstruction::Dload3),
            (4, JVMInstruction::Dload(4)),
            (5, JVMInstruction::Dload(5)),
            (6, JVMInstruction::Aload(6)),
            (7, JVMInstruction::Iload(7)),
            (8, JVMInstruction::Lload(8)),
            (9, JVMInstruction::Fload(9)),
            (10, JVMInstruction::Fload(10)),
        ];
        let locals = LocalInterpretation::from_code(false, &params, &code);
        let mut expected_rle = vec![
            (1, ValType::I32), // Ignore implicit this and int parameters
            (3, ValType::F64),
            (2, ValType::I32), // Reference (from ALOAD) is an i32 too
            (1, ValType::I64),
            (2, ValType::F32),
        ];
        assert_eq!(locals.run_length_encode(&[]), expected_rle);

        // Check appended types coalesce
        let append = [ValType::F32, ValType::I32];
        expected_rle.pop();
        expected_rle.push((3, ValType::F32));
        expected_rle.push((1, ValType::I32));
        assert_eq!(locals.run_length_encode(&append), expected_rle);
    }

    #[test]
    fn locals_from_all_instructions() {
        fn local_from(instruction: JVMInstruction) -> (u32, ValType) {
            let locals = LocalInterpretation::from_code(true, &[], &[(0, instruction)]);
            *locals.map.keys().next().unwrap()
        }

        // References
        assert_eq!(local_from(JVMInstruction::Aload0), (0, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Aload1), (1, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Aload2), (2, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Aload3), (3, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Aload(4)), (4, ValType::I32));
        assert_eq!(local_from(JVMInstruction::AloadWide(4)), (4, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Astore0), (0, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Astore1), (1, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Astore2), (2, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Astore3), (3, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Astore(4)), (4, ValType::I32));
        assert_eq!(local_from(JVMInstruction::AstoreWide(4)), (4, ValType::I32));

        // Doubles
        assert_eq!(local_from(JVMInstruction::Dload0), (0, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dload1), (1, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dload2), (2, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dload3), (3, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dload(4)), (4, ValType::F64));
        assert_eq!(local_from(JVMInstruction::DloadWide(4)), (4, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dstore0), (0, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dstore1), (1, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dstore2), (2, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dstore3), (3, ValType::F64));
        assert_eq!(local_from(JVMInstruction::Dstore(4)), (4, ValType::F64));
        assert_eq!(local_from(JVMInstruction::DstoreWide(4)), (4, ValType::F64));

        // Floats
        assert_eq!(local_from(JVMInstruction::Fload0), (0, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fload1), (1, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fload2), (2, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fload3), (3, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fload(4)), (4, ValType::F32));
        assert_eq!(local_from(JVMInstruction::FloadWide(4)), (4, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fstore0), (0, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fstore1), (1, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fstore2), (2, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fstore3), (3, ValType::F32));
        assert_eq!(local_from(JVMInstruction::Fstore(4)), (4, ValType::F32));
        assert_eq!(local_from(JVMInstruction::FstoreWide(4)), (4, ValType::F32));

        // Integers
        assert_eq!(local_from(JVMInstruction::Iload0), (0, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Iload1), (1, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Iload2), (2, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Iload3), (3, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Iload(4)), (4, ValType::I32));
        assert_eq!(local_from(JVMInstruction::IloadWide(4)), (4, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Istore0), (0, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Istore1), (1, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Istore2), (2, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Istore3), (3, ValType::I32));
        assert_eq!(local_from(JVMInstruction::Istore(4)), (4, ValType::I32));
        assert_eq!(local_from(JVMInstruction::IstoreWide(4)), (4, ValType::I32));
        assert_eq!(
            local_from(JVMInstruction::Iinc { index: 5, value: 1 },),
            (5, ValType::I32)
        );
        assert_eq!(
            local_from(JVMInstruction::IincWide { index: 5, value: 1 },),
            (5, ValType::I32)
        );

        // Longs
        assert_eq!(local_from(JVMInstruction::Lload0), (0, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lload1), (1, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lload2), (2, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lload3), (3, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lload(4)), (4, ValType::I64));
        assert_eq!(local_from(JVMInstruction::LloadWide(4)), (4, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lstore0), (0, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lstore1), (1, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lstore2), (2, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lstore3), (3, ValType::I64));
        assert_eq!(local_from(JVMInstruction::Lstore(4)), (4, ValType::I64));
        assert_eq!(local_from(JVMInstruction::LstoreWide(4)), (4, ValType::I64));
    }
}
