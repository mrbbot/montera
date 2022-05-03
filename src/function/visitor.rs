use crate::class::{ConstantPool, FieldDescriptor, NumericConstant, JAVA_LANG_OBJECT};
use crate::function::locals::LocalInterpretation;
use crate::function::structure::{ConditionalKind, Loop, LoopKind, Structure, StructuredCode};
use crate::function::Instruction::{self, I};
use crate::function::NaNBehaviour;
use crate::graph::{Node, NodeId};
use classfile_parser::code_attribute::Instruction as JVMInstruction;
use std::sync::Arc;
use wasm_encoder::ValType;
use wasm_encoder::{BlockType, Instruction as WASMInstruction};

/// WebAssembly generation visiting phase operating on individual functions.
/// Performed in parallel by [`crate::function::CompileFunctionJob`].
///
/// The visiting phase takes a structured control flow graph, with identified loops and 2-way
/// conditionals, and produces a list of WebAssembly instructions. Pseudo-instructions are produced
/// for operations requiring custom built-in WebAssembly functions, or program-wide information such
/// as the virtual method table. These are lowered to real WebAssembly instructions in the rendering
/// phase. See [`Instruction`] for more details.
pub struct Visitor {
    pub const_pool: Arc<ConstantPool>,
    pub locals: Arc<LocalInterpretation>,
    pub code: StructuredCode,
}

impl Visitor {
    /// Translates a single JVM instruction into one or more WebAssembly (pseudo-)instructions.
    ///
    /// This is arguably the most important function in the project. An exhaustive `match` statement
    /// ensures all parsed JVM instructions are handled or explicitly marked as unimplemented.
    /// If instructions are added in the future, a compile time error will be produced.
    fn visit(
        &self,
        out: &mut Vec<Instruction<'_>>,
        instruction: &JVMInstruction,
    ) -> anyhow::Result<()> {
        let const_pool = &*self.const_pool;
        let locals = &self.locals;
        // Instructions defined here: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-6.html
        // Unimplemented instructions have the blocking feature in brackets.
        match instruction {
            JVMInstruction::Aaload => bail!("Aaload instruction unimplemented (Array)"),
            JVMInstruction::Aastore => bail!("Aastore instruction unimplemented (Array)"),
            JVMInstruction::Aconstnull => out.push(I(WASMInstruction::I32Const(0))),
            JVMInstruction::Aload(n) => locals.get(out, ValType::I32, *n as u32),
            JVMInstruction::AloadWide(n) => locals.get(out, ValType::I32, *n as u32),
            JVMInstruction::Aload0 => locals.get(out, ValType::I32, 0),
            JVMInstruction::Aload1 => locals.get(out, ValType::I32, 1),
            JVMInstruction::Aload2 => locals.get(out, ValType::I32, 2),
            JVMInstruction::Aload3 => locals.get(out, ValType::I32, 3),
            JVMInstruction::Anewarray(_) => bail!("Anewarray instruction unimplemented (Array)"),
            JVMInstruction::Areturn => out.push(I(WASMInstruction::Return)),
            JVMInstruction::Arraylength => bail!("Arraylength instruction unimplemented (Array)"),
            JVMInstruction::Astore(n) => locals.set(out, ValType::I32, *n as u32),
            JVMInstruction::AstoreWide(n) => locals.set(out, ValType::I32, *n as u32),
            JVMInstruction::Astore0 => locals.set(out, ValType::I32, 0),
            JVMInstruction::Astore1 => locals.set(out, ValType::I32, 1),
            JVMInstruction::Astore2 => locals.set(out, ValType::I32, 2),
            JVMInstruction::Astore3 => locals.set(out, ValType::I32, 3),
            JVMInstruction::Athrow => {
                // Exceptions are not yet supported, but are required for assertions.
                // In this case, emit an unreachable instruction to cause a trap.
                out.push(I(WASMInstruction::Unreachable))
            }
            JVMInstruction::Baload => bail!("Baload instruction unimplemented (Array)"),
            JVMInstruction::Bastore => bail!("Bastore instruction unimplemented (Array)"),
            JVMInstruction::Bipush(n) => out.push(I(WASMInstruction::I32Const(*n as i32))),
            JVMInstruction::Caload => bail!("Caload instruction unimplemented (Array)"),
            JVMInstruction::Castore => bail!("Castore instruction unimplemented (Array)"),
            JVMInstruction::Checkcast(_) => {
                bail!("Checkcast instruction unimplemented (Exception)")
            }
            JVMInstruction::D2f => out.push(I(WASMInstruction::F32DemoteF64)),
            JVMInstruction::D2i => out.push(I(WASMInstruction::I32TruncF64S)),
            JVMInstruction::D2l => out.push(I(WASMInstruction::I64TruncF64S)),
            JVMInstruction::Dadd => out.push(I(WASMInstruction::F64Add)),
            JVMInstruction::Daload => bail!("Daload instruction unimplemented (Array)"),
            JVMInstruction::Dastore => bail!("Dastore instruction unimplemented (Array)"),
            JVMInstruction::Dcmpg => out.push(Instruction::DoubleCmp(NaNBehaviour::Greater)),
            JVMInstruction::Dcmpl => out.push(Instruction::DoubleCmp(NaNBehaviour::Lesser)),
            JVMInstruction::Dconst0 => out.push(I(WASMInstruction::F64Const(0.0))),
            JVMInstruction::Dconst1 => out.push(I(WASMInstruction::F64Const(1.0))),
            JVMInstruction::Ddiv => out.push(I(WASMInstruction::F64Div)),
            JVMInstruction::Dload(n) => locals.get(out, ValType::F64, *n as u32),
            JVMInstruction::DloadWide(n) => locals.get(out, ValType::F64, *n as u32),
            JVMInstruction::Dload0 => locals.get(out, ValType::F64, 0),
            JVMInstruction::Dload1 => locals.get(out, ValType::F64, 1),
            JVMInstruction::Dload2 => locals.get(out, ValType::F64, 2),
            JVMInstruction::Dload3 => locals.get(out, ValType::F64, 3),
            JVMInstruction::Dmul => out.push(I(WASMInstruction::F64Mul)),
            JVMInstruction::Dneg => out.push(I(WASMInstruction::F64Neg)),
            JVMInstruction::Drem => bail!("Drem instruction unimplemented"), // TODO: implement
            JVMInstruction::Dreturn => out.push(I(WASMInstruction::Return)),
            JVMInstruction::Dstore(n) => locals.set(out, ValType::F64, *n as u32),
            JVMInstruction::DstoreWide(n) => locals.set(out, ValType::F64, *n as u32),
            JVMInstruction::Dstore0 => locals.set(out, ValType::F64, 0),
            JVMInstruction::Dstore1 => locals.set(out, ValType::F64, 1),
            JVMInstruction::Dstore2 => locals.set(out, ValType::F64, 2),
            JVMInstruction::Dstore3 => locals.set(out, ValType::F64, 3),
            JVMInstruction::Dsub => out.push(I(WASMInstruction::F64Sub)),
            // The semantics of Dup* instructions depends on the type of the stack at runtime.
            // Some of these instructions also insert copies 2/3 values down the stack.
            // To implement these properly, we'd need to perform type inference on the emitted
            // instructions, then insert the appropriate scratch locals and instructions.
            //
            // The Dup instruction is used after a New to run the constructor and store a reference
            // in a local. Technically, it can be used with any category 1 computational type
            // (https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-2.html#jvms-2.11.1),
            // but I've only observed it being used with int (i32) computational types.
            //
            // Therefore, if this instruction is produced, we add an additional i32 scratch local
            // to the function, and use local_tee/get instructions to duplicate the value.
            JVMInstruction::Dup => out.push(Instruction::Dup),
            JVMInstruction::Dupx1 => bail!("Dupx1 instruction unimplemented (Stack Type)"),
            JVMInstruction::Dupx2 => bail!("Dupx2 instruction unimplemented (Stack Type)"),
            JVMInstruction::Dup2 => bail!("Dup2 instruction unimplemented (Stack Type)"),
            JVMInstruction::Dup2x1 => bail!("Dup2x1 instruction unimplemented (Stack Type)"),
            JVMInstruction::Dup2x2 => bail!("Dup2x2 instruction unimplemented (Stack Type)"),
            JVMInstruction::F2d => out.push(I(WASMInstruction::F64PromoteF32)),
            JVMInstruction::F2i => out.push(I(WASMInstruction::I32TruncF32S)),
            JVMInstruction::F2l => out.push(I(WASMInstruction::I64TruncF32S)),
            JVMInstruction::Fadd => out.push(I(WASMInstruction::F32Add)),
            JVMInstruction::Faload => bail!("Faload instruction unimplemented (Array)"),
            JVMInstruction::Fastore => bail!("Fastore instruction unimplemented (Array)"),
            JVMInstruction::Fcmpg => out.push(Instruction::FloatCmp(NaNBehaviour::Greater)),
            JVMInstruction::Fcmpl => out.push(Instruction::FloatCmp(NaNBehaviour::Lesser)),
            JVMInstruction::Fconst0 => out.push(I(WASMInstruction::F32Const(0.0))),
            JVMInstruction::Fconst1 => out.push(I(WASMInstruction::F32Const(1.0))),
            JVMInstruction::Fconst2 => out.push(I(WASMInstruction::F32Const(2.0))),
            JVMInstruction::Fdiv => out.push(I(WASMInstruction::F32Div)),
            JVMInstruction::Fload(n) => locals.get(out, ValType::F32, *n as u32),
            JVMInstruction::FloadWide(n) => locals.get(out, ValType::F32, *n as u32),
            JVMInstruction::Fload0 => locals.get(out, ValType::F32, 0),
            JVMInstruction::Fload1 => locals.get(out, ValType::F32, 1),
            JVMInstruction::Fload2 => locals.get(out, ValType::F32, 2),
            JVMInstruction::Fload3 => locals.get(out, ValType::F32, 3),
            JVMInstruction::Fmul => out.push(I(WASMInstruction::F32Mul)),
            JVMInstruction::Fneg => out.push(I(WASMInstruction::F32Neg)),
            JVMInstruction::Frem => bail!("Frem instruction unimplemented"), // TODO: implement
            JVMInstruction::Freturn => out.push(I(WASMInstruction::Return)),
            JVMInstruction::Fstore(n) => locals.set(out, ValType::F32, *n as u32),
            JVMInstruction::FstoreWide(n) => locals.set(out, ValType::F32, *n as u32),
            JVMInstruction::Fstore0 => locals.set(out, ValType::F32, 0),
            JVMInstruction::Fstore1 => locals.set(out, ValType::F32, 1),
            JVMInstruction::Fstore2 => locals.set(out, ValType::F32, 2),
            JVMInstruction::Fstore3 => locals.set(out, ValType::F32, 3),
            JVMInstruction::Fsub => out.push(I(WASMInstruction::F32Sub)),
            JVMInstruction::Getfield(n) => {
                let id = const_pool.field(*n);
                out.push(Instruction::GetField(id));
            }
            JVMInstruction::Getstatic(n) => {
                // Static fields are not yet supported, but are required for assertions
                let id = const_pool.field(*n);
                if *id.name == "$assertionsDisabled" && *id.descriptor == FieldDescriptor::Boolean {
                    // Always enable assertions
                    out.push(I(WASMInstruction::I32Const(0)));
                } else {
                    bail!("Getstatic instruction unimplemented (Static Field)")
                }
            }
            JVMInstruction::Goto(_) => out.push(I(WASMInstruction::Nop)),
            JVMInstruction::GotoW(_) => out.push(I(WASMInstruction::Nop)),
            JVMInstruction::I2b => out.push(I(WASMInstruction::Nop)),
            JVMInstruction::I2c => out.push(I(WASMInstruction::Nop)),
            JVMInstruction::I2d => out.push(I(WASMInstruction::F64ConvertI32S)),
            JVMInstruction::I2f => out.push(I(WASMInstruction::F32ConvertI32S)),
            JVMInstruction::I2l => out.push(I(WASMInstruction::I64ExtendI32S)),
            JVMInstruction::I2s => out.push(I(WASMInstruction::Nop)),
            JVMInstruction::Iadd => out.push(I(WASMInstruction::I32Add)),
            JVMInstruction::Iaload => bail!("Iaload instruction unimplemented (Array)"),
            JVMInstruction::Iand => out.push(I(WASMInstruction::I32And)),
            JVMInstruction::Iastore => bail!("Iastore instruction unimplemented (Array)"),
            JVMInstruction::Iconstm1 => out.push(I(WASMInstruction::I32Const(-1))),
            JVMInstruction::Iconst0 => out.push(I(WASMInstruction::I32Const(0))),
            JVMInstruction::Iconst1 => out.push(I(WASMInstruction::I32Const(1))),
            JVMInstruction::Iconst2 => out.push(I(WASMInstruction::I32Const(2))),
            JVMInstruction::Iconst3 => out.push(I(WASMInstruction::I32Const(3))),
            JVMInstruction::Iconst4 => out.push(I(WASMInstruction::I32Const(4))),
            JVMInstruction::Iconst5 => out.push(I(WASMInstruction::I32Const(5))),
            JVMInstruction::Idiv => out.push(I(WASMInstruction::I32DivS)),
            JVMInstruction::IfAcmpeq(_) => out.push(I(WASMInstruction::I32Eq)),
            JVMInstruction::IfAcmpne(_) => out.push(I(WASMInstruction::I32Neq)),
            JVMInstruction::IfIcmpeq(_) => out.push(I(WASMInstruction::I32Eq)),
            JVMInstruction::IfIcmpne(_) => out.push(I(WASMInstruction::I32Neq)),
            JVMInstruction::IfIcmplt(_) => out.push(I(WASMInstruction::I32LtS)),
            JVMInstruction::IfIcmpge(_) => out.push(I(WASMInstruction::I32GeS)),
            JVMInstruction::IfIcmpgt(_) => out.push(I(WASMInstruction::I32GtS)),
            JVMInstruction::IfIcmple(_) => out.push(I(WASMInstruction::I32LeS)),
            JVMInstruction::Ifeq(_) | JVMInstruction::Ifnull(_) => {
                out.push(I(WASMInstruction::I32Eqz))
            }
            JVMInstruction::Ifne(_) | JVMInstruction::Ifnonnull(_) => {
                out.push(I(WASMInstruction::I32Const(0)));
                out.push(I(WASMInstruction::I32Neq));
            }
            JVMInstruction::Iflt(_) => {
                out.push(I(WASMInstruction::I32Const(0)));
                out.push(I(WASMInstruction::I32LtS));
            }
            JVMInstruction::Ifge(_) => {
                out.push(I(WASMInstruction::I32Const(0)));
                out.push(I(WASMInstruction::I32GeS));
            }
            JVMInstruction::Ifgt(_) => {
                out.push(I(WASMInstruction::I32Const(0)));
                out.push(I(WASMInstruction::I32GtS));
            }
            JVMInstruction::Ifle(_) => {
                out.push(I(WASMInstruction::I32Const(0)));
                out.push(I(WASMInstruction::I32LeS));
            }
            JVMInstruction::Iinc { index, value } => {
                let local_index = locals.get_local_index(ValType::I32, *index as u32);
                out.push(I(WASMInstruction::LocalGet(local_index)));
                out.push(I(WASMInstruction::I32Const(*value as i32)));
                out.push(I(WASMInstruction::I32Add));
                out.push(I(WASMInstruction::LocalSet(local_index)));
            }
            JVMInstruction::IincWide { index, value } => {
                let local_index = locals.get_local_index(ValType::I32, *index as u32);
                out.push(I(WASMInstruction::LocalGet(local_index)));
                out.push(I(WASMInstruction::I32Const(*value as i32)));
                out.push(I(WASMInstruction::I32Add));
                out.push(I(WASMInstruction::LocalSet(local_index)));
            }
            JVMInstruction::Iload(n) => locals.get(out, ValType::I32, *n as u32),
            JVMInstruction::IloadWide(n) => locals.get(out, ValType::I32, *n as u32),
            JVMInstruction::Iload0 => locals.get(out, ValType::I32, 0),
            JVMInstruction::Iload1 => locals.get(out, ValType::I32, 1),
            JVMInstruction::Iload2 => locals.get(out, ValType::I32, 2),
            JVMInstruction::Iload3 => locals.get(out, ValType::I32, 3),
            JVMInstruction::Imul => out.push(I(WASMInstruction::I32Mul)),
            JVMInstruction::Ineg => {
                out.push(I(WASMInstruction::I32Const(-1)));
                out.push(I(WASMInstruction::I32Mul));
            }
            JVMInstruction::Instanceof(n) => {
                let class_name = const_pool.class_name(*n);
                out.push(Instruction::InstanceOf(class_name));
            }
            JVMInstruction::Invokedynamic(_) => {
                bail!("Invokedynamic instruction unimplemented (Dynamic Type)")
            }
            JVMInstruction::Invokeinterface { .. } => {
                bail!("Invokeinterface instruction unimplemented (Interface)")
            }
            JVMInstruction::Invokespecial(n) => {
                let id = const_pool.method(*n);
                if *id.class_name == JAVA_LANG_OBJECT && *id.name == "<init>" {
                    // Implicit Object super(), no-op, but need to consume this reference
                    out.push(I(WASMInstruction::Drop))
                } else {
                    out.push(Instruction::CallStatic(id));
                }
            }
            JVMInstruction::Invokestatic(n) => {
                let id = const_pool.method(*n);
                out.push(Instruction::CallStatic(id));
            }
            JVMInstruction::Invokevirtual(n) => {
                let id = const_pool.method(*n);
                out.push(Instruction::CallVirtual(id));
            }
            JVMInstruction::Ior => out.push(I(WASMInstruction::I32Or)),
            JVMInstruction::Irem => out.push(I(WASMInstruction::I32RemS)),
            JVMInstruction::Ireturn => out.push(I(WASMInstruction::Return)),
            JVMInstruction::Ishl => out.push(I(WASMInstruction::I32Shl)),
            JVMInstruction::Ishr => out.push(I(WASMInstruction::I32ShrS)),
            JVMInstruction::Istore(n) => locals.set(out, ValType::I32, *n as u32),
            JVMInstruction::IstoreWide(n) => locals.set(out, ValType::I32, *n as u32),
            JVMInstruction::Istore0 => locals.set(out, ValType::I32, 0),
            JVMInstruction::Istore1 => locals.set(out, ValType::I32, 1),
            JVMInstruction::Istore2 => locals.set(out, ValType::I32, 2),
            JVMInstruction::Istore3 => locals.set(out, ValType::I32, 3),
            JVMInstruction::Isub => out.push(I(WASMInstruction::I32Sub)),
            JVMInstruction::Iushr => out.push(I(WASMInstruction::I32ShrU)),
            JVMInstruction::Ixor => out.push(I(WASMInstruction::I32Xor)),
            JVMInstruction::Jsr(_) => bail!("Jsr instruction unimplemented (Irreducible)"),
            JVMInstruction::JsrW(_) => bail!("JsrW instruction unimplemented (Irreducible)"),
            JVMInstruction::L2d => out.push(I(WASMInstruction::F64ConvertI64S)),
            JVMInstruction::L2f => out.push(I(WASMInstruction::F32ConvertI64S)),
            JVMInstruction::L2i => out.push(I(WASMInstruction::I32WrapI64)),
            JVMInstruction::Ladd => out.push(I(WASMInstruction::I64Add)),
            JVMInstruction::Laload => bail!("Laload instruction unimplemented (Array)"),
            JVMInstruction::Land => out.push(I(WASMInstruction::I64And)),
            JVMInstruction::Lastore => bail!("Lastore instruction unimplemented (Array)"),
            JVMInstruction::Lcmp => out.push(Instruction::LongCmp),
            JVMInstruction::Lconst0 => out.push(I(WASMInstruction::I64Const(0))),
            JVMInstruction::Lconst1 => out.push(I(WASMInstruction::I64Const(1))),
            JVMInstruction::Ldc(n) => {
                let num = const_pool.num(*n as u16);
                out.push(match num {
                    NumericConstant::Integer(num) => I(WASMInstruction::I32Const(num)),
                    NumericConstant::Float(num) => I(WASMInstruction::F32Const(num)),
                    // TODO (someday): Ldc can be reference to String, Class or Method
                    _ => bail!("Ldc constants other than int/float unimplemented"),
                })
            }
            JVMInstruction::LdcW(n) => {
                let num = const_pool.num(*n);
                out.push(match num {
                    NumericConstant::Integer(num) => I(WASMInstruction::I32Const(num)),
                    NumericConstant::Float(num) => I(WASMInstruction::F32Const(num)),
                    // TODO (someday): LdcW can be reference to String, Class or Method
                    _ => bail!("LdcW constants other than int/float unimplemented"),
                })
            }
            JVMInstruction::Ldc2W(n) => {
                let num = const_pool.num(*n);
                out.push(match num {
                    NumericConstant::Long(num) => I(WASMInstruction::I64Const(num)),
                    NumericConstant::Double(num) => I(WASMInstruction::F64Const(num)),
                    _ => unreachable!("Ldc2W expected long/double constant"),
                })
            }
            JVMInstruction::Ldiv => out.push(I(WASMInstruction::I64DivS)),
            JVMInstruction::Lload(n) => locals.get(out, ValType::I64, *n as u32),
            JVMInstruction::LloadWide(n) => locals.get(out, ValType::I64, *n as u32),
            JVMInstruction::Lload0 => locals.get(out, ValType::I64, 0),
            JVMInstruction::Lload1 => locals.get(out, ValType::I64, 1),
            JVMInstruction::Lload2 => locals.get(out, ValType::I64, 2),
            JVMInstruction::Lload3 => locals.get(out, ValType::I64, 3),
            JVMInstruction::Lmul => out.push(I(WASMInstruction::I64Mul)),
            JVMInstruction::Lneg => {
                out.push(I(WASMInstruction::I64Const(-1)));
                out.push(I(WASMInstruction::I64Mul));
            }
            JVMInstruction::Lookupswitch { .. } => {
                bail!("Lookupswitch instruction unimplemented (n-Way Branch)")
            }
            JVMInstruction::Lor => out.push(I(WASMInstruction::I64Or)),
            JVMInstruction::Lrem => out.push(I(WASMInstruction::I64RemS)),
            JVMInstruction::Lreturn => out.push(I(WASMInstruction::Return)),
            JVMInstruction::Lshl => {
                // JVM requires second operand (top of stack) is int, but WebAssembly requires i64
                out.push(I(WASMInstruction::I64ExtendI32S));
                out.push(I(WASMInstruction::I64Shl))
            }
            JVMInstruction::Lshr => {
                // JVM requires second operand (top of stack) is int, but WebAssembly requires i64
                out.push(I(WASMInstruction::I64ExtendI32S));
                out.push(I(WASMInstruction::I64ShrS))
            }
            JVMInstruction::Lstore(n) => locals.set(out, ValType::I64, *n as u32),
            JVMInstruction::LstoreWide(n) => locals.set(out, ValType::I64, *n as u32),
            JVMInstruction::Lstore0 => locals.set(out, ValType::I64, 0),
            JVMInstruction::Lstore1 => locals.set(out, ValType::I64, 1),
            JVMInstruction::Lstore2 => locals.set(out, ValType::I64, 2),
            JVMInstruction::Lstore3 => locals.set(out, ValType::I64, 3),
            JVMInstruction::Lsub => out.push(I(WASMInstruction::I64Sub)),
            JVMInstruction::Lushr => {
                // JVM requires second operand (top of stack) is int, but WebAssembly requires i64
                out.push(I(WASMInstruction::I64ExtendI32S));
                out.push(I(WASMInstruction::I64ShrU))
            }
            JVMInstruction::Lxor => out.push(I(WASMInstruction::I64Xor)),
            JVMInstruction::Monitorenter => {
                bail!("Monitorenter instruction unimplemented (Monitor)")
            }
            JVMInstruction::Monitorexit => bail!("Monitorexit instruction unimplemented (Monitor)"),
            JVMInstruction::Multianewarray { .. } => {
                bail!("Multianewarray instruction unimplemented (Array)")
            }
            JVMInstruction::New(n) => {
                let class_name = const_pool.class_name(*n);
                out.push(Instruction::New(class_name));
            }
            JVMInstruction::Newarray(_) => bail!("Newarray instruction unimplemented (Array)"),
            JVMInstruction::Nop => out.push(I(WASMInstruction::Nop)),
            JVMInstruction::Pop => out.push(I(WASMInstruction::Drop)),
            JVMInstruction::Pop2 => out.push(I(WASMInstruction::Drop)),
            JVMInstruction::Putfield(n) => {
                let id = const_pool.field(*n);
                out.push(Instruction::PutField(id));
            }
            JVMInstruction::Putstatic(_) => {
                bail!("Putstatic instruction unimplemented (Static Field)")
            }
            JVMInstruction::Ret(_) => bail!("Ret instruction unimplemented (Irreducible)"),
            JVMInstruction::RetWide(_) => bail!("RetWide instruction unimplemented (Irreducible)"),
            JVMInstruction::Return => out.push(I(WASMInstruction::Return)),
            JVMInstruction::Saload => bail!("Saload instruction unimplemented (Array)"),
            JVMInstruction::Sastore => bail!("Sastore instruction unimplemented (Array)"),
            JVMInstruction::Sipush(n) => out.push(I(WASMInstruction::I32Const(*n as i32))),
            JVMInstruction::Swap => bail!("Swap instruction unimplemented (Stack Type)"),
            JVMInstruction::Tableswitch { .. } => {
                bail!("Tableswitch instruction unimplemented (n-Way Branch)")
            }
        };
        Ok(())
    }

    /// Translates a [`Structure`] (either a basic block or compound short-circuit conditional) into
    /// one or more WebAssembly (pseudo-)instructions.
    fn visit_struct(
        &self,
        out: &mut Vec<Instruction<'_>>,
        structure: &Structure,
    ) -> anyhow::Result<()> {
        match structure {
            Structure::Block(instructions) => {
                // Basic block, visit all instructions in sequence
                for instruction in instructions {
                    self.visit(out, instruction)?;
                }
            }
            Structure::CompoundConditional {
                kind,
                left_negated,
                left,
                right,
            } => {
                // Short-circuit conditional, always visit left branch, then maybe short-circuit
                // evaluation of right branch
                self.visit_struct(out, &left)?;
                // Loop/2-way conditional headers/latchings expect an `i32` value on the top of the
                // stack, so produce an `if` expression, for early returns, evaluating to an `i32`
                out.push(I(WASMInstruction::If(BlockType::Result(ValType::I32))));
                match (left_negated, kind) {
                    // if left && right
                    (false, ConditionalKind::Conjunction) => {
                        // if condition is TRUE, left is TRUE, check right too
                        self.visit_struct(out, right)?;
                        out.push(I(WASMInstruction::Else));
                        // else left is FALSE, conjunction must be FALSE, short-circuit
                        out.push(I(WASMInstruction::I32Const(0)));
                    }
                    // if !left && right
                    (true, ConditionalKind::Conjunction) => {
                        // if NEGATED condition is TRUE, !left is FALSE, conjunction must be FALSE, short-circuit
                        out.push(I(WASMInstruction::I32Const(0)));
                        out.push(I(WASMInstruction::Else));
                        // else !left is TRUE, check right too
                        self.visit_struct(out, right)?;
                    }
                    // if left || right
                    (false, ConditionalKind::Disjunction) => {
                        // if condition is TRUE, left is TRUE, disjunction must be TRUE, short-circuit
                        out.push(I(WASMInstruction::I32Const(1)));
                        out.push(I(WASMInstruction::Else));
                        // else left is FALSE, check right too
                        self.visit_struct(out, right)?;
                    }
                    // if !left || right
                    (true, ConditionalKind::Disjunction) => {
                        // if NEGATED condition is TRUE, !left is FALSE, check right too
                        self.visit_struct(out, right)?;
                        out.push(I(WASMInstruction::Else));
                        // else !left is TRUE, disjunction must be TRUE, short-circuit
                        out.push(I(WASMInstruction::I32Const(1)));
                    }
                };
                out.push(I(WASMInstruction::End));
            }
        }
        Ok(())
    }

    /// Translates a control flow graph node containing a [`Structure`] (either a basic block or
    /// compound short-circuit conditional) into one or more WebAssembly (pseudo-)instructions.
    #[inline]
    fn visit_node(
        &self,
        out: &mut Vec<Instruction<'_>>,
        node: &Node<Structure>,
    ) -> anyhow::Result<()> {
        self.visit_struct(out, &node.value)
    }

    /// Translates a structured [`Loop`] (with identified type, header, latching and follow node)
    /// into multiple WebAssembly (pseudo-)instructions.
    fn visit_loop(&self, out: &mut Vec<Instruction<'_>>, loop_info: Loop) -> anyhow::Result<()> {
        // Allow easily breaking out of the loop...
        out.push(I(WASMInstruction::Block(BlockType::Empty)));
        // ...and continuing to the next iteration
        out.push(I(WASMInstruction::Loop(BlockType::Empty)));
        // (this will almost certainly get optimised by wasm-opt to just "loop")

        match loop_info.kind {
            LoopKind::PreTested => {
                let header = &self.code.g[loop_info.header];
                assert_eq!(header.out_degree(), 2); // Header should be 2-way conditional

                // If this is a pre-tested loop, the condition is in the header, so evaluate it
                self.visit_node(out, header)?;

                if loop_info.header == loop_info.latching
                    && header.successors[1 /* true */] == loop_info.header
                {
                    // Special case: single node post-tested loop where latching node is the header,
                    // and the true branch is the header again. In this case, branch back to the
                    // start of the loop if the condition is true, and break out otherwise.
                    out.push(I(WASMInstruction::BrIf(0)));
                    // No need for explicit Br(1) as we'll fall out of the block naturally
                } else {
                    // Follow should be true branch of header conditional...
                    assert_eq!(header.successors[1], loop_info.follow);
                    // ...so the body should be the false branch
                    let body = header.successors[0];

                    // Otherwise, break out of the loop if the header condition is true
                    out.push(I(WASMInstruction::BrIf(1)));

                    // Run the loop body (not the follow node), until we return to the header
                    self.visit_until(out, body, Some(loop_info.header), false)?;

                    // ...then branch back to the start of the loop
                    out.push(I(WASMInstruction::Br(0)));
                }
            }
            LoopKind::PostTested => {
                let latching = &self.code.g[loop_info.latching];
                assert_eq!(latching.out_degree(), 2); // Latching should be 2-way conditional

                // If this is a post-tested loop, the condition is in the latching node, so visit
                // all nodes up to it, making sure not to treat the first node as a loop (`true`)
                // as that would look to infinite recursion
                self.visit_until(out, loop_info.header, Some(loop_info.latching), true)?;
                // ...then evaluate the latching condition
                self.visit_node(out, latching)?;

                // Follow should be false branch of latching conditional...
                assert_eq!(latching.successors[0], loop_info.follow);
                // ...and the header should be the true branch...
                assert_eq!(latching.successors[1], loop_info.header);
                // ...so branch back to the start of the loop if the latching condition is true...
                out.push(I(WASMInstruction::BrIf(0)));
                // ...and break out of the loop otherwise.
                // No need for explicit Br(1) as we'll fall out of the block naturally
            }
        }

        out.push(I(WASMInstruction::End));
        out.push(I(WASMInstruction::End));

        Ok(())
    }

    /// Translates a structured 2-way conditional (with identified header and follow node) into
    /// multiple WebAssembly (pseudo-)instructions.
    fn visit_conditional(
        &self,
        out: &mut Vec<Instruction<'_>>,
        header: NodeId,
        follow: NodeId,
    ) -> anyhow::Result<()> {
        let node = &self.code.g[header];
        assert_eq!(node.out_degree(), 2);
        let true_node = node.successors[1];
        let false_node = node.successors[0];

        self.visit_node(out, node)?;
        out.push(I(WASMInstruction::If(BlockType::Empty)));
        {
            self.visit_until(out, true_node, Some(follow), false)?;
        }
        out.push(I(WASMInstruction::Else));
        {
            self.visit_until(out, false_node, Some(follow), false)?;
        }
        out.push(I(WASMInstruction::End));

        Ok(())
    }

    /// Translates all nodes from `n` up `until` an optional node into one or more WebAssembly
    /// (pseudo-)instructions.
    ///
    /// Setting `n` to the control flow graph's entrypoint and `until` to `None` will translate the
    /// entire function.
    ///
    /// If `ignore_first_loop` is `true`, the first visited node `n` will not be considered as a
    /// potential loop. If this function is called when visiting the header of a post-tested loop
    /// (which may be a 2-way conditional header), this must be set to `true` to avoid infinite
    /// recursion.
    fn visit_until(
        &self,
        out: &mut Vec<Instruction<'_>>,
        mut n: NodeId,
        until: Option<NodeId>,
        mut ignore_first_loop: bool,
    ) -> anyhow::Result<()> {
        while Some(n) != until {
            // If this function is called when visiting the header of a post-tested loop,
            // do not treat it as a loop, as that would lead to infinite recursion
            if !ignore_first_loop {
                if let Some(loop_info) = self.code.loops.get(n) {
                    // If n is a loop header node...
                    self.visit_loop(out, *loop_info)?;
                    n = loop_info.follow;
                    continue;
                }
            }
            ignore_first_loop = false;

            if let Some(&follow) = self.code.conditionals.get(n) {
                // If n is a 2-way conditional header node...
                self.visit_conditional(out, n, follow)?;
                n = follow;
            } else {
                // Otherwise, it's a regular block
                let node = &self.code.g[n];
                assert!(node.out_degree() <= 1);
                self.visit_node(out, node)?;
                if node.out_degree() == 0 {
                    break; // If this is an exit node, we're done
                } else {
                    n = node.successors[0];
                }
            }
        }
        Ok(())
    }

    /// Translates the entire JVM bytecode function into one or more WebAssembly
    /// (pseudo-)instructions.
    ///
    /// # Panics
    ///
    /// Panics if the control flow graph doesn't have an entrypoint.
    pub fn visit_all(&self, out: &mut Vec<Instruction<'_>>) -> anyhow::Result<()> {
        let start = self.code.g.entry.expect("visit_all needs entrypoint");
        self.visit_until(out, start, None, false)?;
        out.push(I(WASMInstruction::End));
        Ok(())
    }
}
