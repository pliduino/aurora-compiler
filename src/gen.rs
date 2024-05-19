use std::collections::HashMap;

use cranelift::{
    codegen::{
        entity::EntityRef,
        ir::{condcodes::FloatCC, types, AbiParam, InstBuilder, Signature, Type, Value},
        isa::{self},
        settings::{self},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use target_lexicon::triple;

use crate::{
    ast::{BinaryOp, Expr, ExprType, Function, Parameter, Prototype},
    error::{Error, Result},
    typing::{self, get_type_from_str},
};

struct VariableBuilder {
    index: usize,
}

impl VariableBuilder {
    fn new() -> Self {
        Self { index: 0 }
    }

    fn define_var(&mut self, builder: &mut FunctionBuilder, type_: Type) -> Variable {
        let variable = Variable::new(self.index);
        builder.declare_var(variable, type_);
        self.index += 1;
        variable
    }

    fn create_var(&mut self, builder: &mut FunctionBuilder, value: Value, type_: Type) -> Variable {
        let variable = Variable::new(self.index);
        builder.declare_var(variable, type_);
        self.index += 1;
        builder.def_var(variable, value);
        variable
    }
}

struct CompiledFunction {
    defined: bool,
    id: FuncId,
    param_count: usize,
    return_count: usize,
}

pub struct FunctionGenerator<'a> {
    builder: FunctionBuilder<'a>,
    functions: &'a HashMap<String, CompiledFunction>,
    module: &'a mut ObjectModule,
    variable_builder: &'a mut VariableBuilder,
    values: HashMap<String, Variable>,
}

pub struct Generator {
    builder_context: FunctionBuilderContext,
    functions: HashMap<String, CompiledFunction>,
    pub module: ObjectModule,
    variable_builder: VariableBuilder,
}

struct ParseExpr {
    pub value: Option<Value>,
    pub is_return: bool,
}

impl ParseExpr {
    pub fn new(value: Option<Value>) -> Self {
        Self {
            value: value,
            is_return: false,
        }
    }

    pub fn new_return(value: Option<Value>) -> Self {
        Self {
            value: value,
            is_return: true,
        }
    }

    pub fn empty() -> Self {
        Self {
            value: None,
            is_return: false,
        }
    }

    pub fn empty_return() -> Self {
        Self {
            value: None,
            is_return: true,
        }
    }
}

impl<'a> FunctionGenerator<'a> {
    fn cast(&mut self, value: Value, from: &'static str, to: &'static str) -> Result<Value> {
        match self.functions.get(&format!("{}->{}", from, to)) {
            Some(func) => {
                let local_func = self
                    .module
                    .declare_func_in_func(func.id, &mut self.builder.func);
                let call = self.builder.ins().call(local_func, &vec![value]);
                // TODO: Current solution is not the best
                Ok(self.builder.inst_results(call)[0])
            }
            None => return Err(Error::Undefined(format!("can't cast {} to {}", from, to))),
        }
    }

    fn expr(&mut self, expr: &Expr) -> Result<ParseExpr> {
        let value = match &expr.expr_type {
            ExprType::Float(num) => match get_type_from_str(expr.type_) {
                Some(type_) => match type_ {
                    types::F32 => ParseExpr::new(Some(self.builder.ins().f32const(*num as f32))),
                    types::F64 => ParseExpr::new(Some(self.builder.ins().f64const(*num))),
                    _ => unimplemented!(),
                },
                None => ParseExpr::empty(),
            },
            ExprType::Integer(num) => match get_type_from_str(expr.type_) {
                Some(type_) => match type_ {
                    types::I8 => ParseExpr::new(Some(self.builder.ins().iconst(types::I8, *num))),
                    types::I16 => ParseExpr::new(Some(self.builder.ins().iconst(types::I16, *num))),
                    types::I32 => ParseExpr::new(Some(self.builder.ins().iconst(types::I32, *num))),
                    types::I64 => ParseExpr::new(Some(self.builder.ins().iconst(types::I64, *num))),
                    _ => unimplemented!(),
                },
                None => ParseExpr::empty(),
            },
            ExprType::Variable(name) => match self.values.get(name) {
                Some(&variable) => ParseExpr::new(Some(self.builder.use_var(variable))),
                None => {
                    return Err(Error::Undefined(format!("variable {}", name)));
                }
            },
            ExprType::Binary(op, left, right) => {
                let left_value = self.expr(&left)?.value.unwrap(); // TODO: unwrap these properly
                let mut right_value = self.expr(&right)?.value.unwrap();
                match op {
                    BinaryOp::Plus => match left.type_ {
                        typing::I64 => {
                            // TODO: Add more basic type conversions
                            if right.type_ != left.type_ {
                                return Err(Error::MismatchedTypes(left.type_, right.type_));
                            }
                            ParseExpr::new(Some(self.builder.ins().iadd(left_value, right_value)))
                        }
                        typing::F64 => {
                            // TODO: Change this into a function
                            if right.type_ != left.type_ {
                                right_value = self.cast(right_value, right.type_, left.type_)?;
                            }
                            ParseExpr::new(Some(self.builder.ins().fadd(left_value, right_value)))
                        }
                        _ => return Err(Error::Unexpected("can't add this type")),
                    },
                    BinaryOp::Minus => {
                        ParseExpr::new(Some(self.builder.ins().fsub(left_value, right_value)))
                    }
                    BinaryOp::Times => {
                        ParseExpr::new(Some(self.builder.ins().fmul(left_value, right_value)))
                    }
                    BinaryOp::LessThan => ParseExpr::new(Some(self.builder.ins().fcmp(
                        FloatCC::LessThan,
                        left_value,
                        right_value,
                    ))),
                }
            }
            ExprType::Call(name, args) => match self.functions.get(name) {
                Some(func) => {
                    if func.param_count != args.len() {
                        return Err(Error::WrongArgumentCount);
                    }
                    let local_func = self
                        .module
                        .declare_func_in_func(func.id, &mut self.builder.func);
                    let arguments: Result<Vec<_>> =
                        args.into_iter().map(|arg| self.expr(&arg)).collect();
                    let arguments: Vec<Value> = arguments?
                        .into_iter()
                        .map(|arg| arg.value.unwrap()) // TODO: Properly unwrap arguments
                        .collect();

                    let call = self.builder.ins().call(local_func, &arguments);
                    if func.return_count > 0 {
                        // TODO: Current solution is not the best
                        return Ok(ParseExpr::new(Some(self.builder.inst_results(call)[0])));
                    }
                    ParseExpr::empty()
                }
                None => return Err(Error::Undefined(format!("function {}", name))),
            },
            ExprType::Block(exprs) => {
                for expr in exprs {
                    let parse_expr = self.expr(&expr)?;
                    if parse_expr.is_return {
                        return Ok(parse_expr);
                    }
                }
                self.builder.ins().return_(&[]);
                ParseExpr::empty()
            }
            ExprType::Return(expr) => {
                match expr {
                    Some(expr) => {
                        let value = self.expr(&*expr)?;
                        self.builder.ins().return_(&[value.value.unwrap()]); // TODO: Properly unwrap this
                        ParseExpr::new_return(value.value)
                    }
                    None => {
                        self.builder.ins().return_(&[]);
                        ParseExpr::empty_return()
                    }
                }
            }
            ExprType::Let(name, int_expr) => match int_expr {
                None => {
                    let variable = self
                        .variable_builder
                        .define_var(&mut self.builder, get_type_from_str(&expr.type_).unwrap());
                    self.values.insert(name.clone(), variable);
                    ParseExpr::empty()
                }
                Some(value) => {
                    let parse_expr = self.expr(&*value)?;
                    let variable = self.variable_builder.create_var(
                        &mut self.builder,
                        parse_expr.value.expect("value"),
                        get_type_from_str(&expr.type_).unwrap(),
                    );
                    self.values.insert(name.clone(), variable);
                    parse_expr
                }
            },
            ExprType::Assign(name, value) => {
                let val = self.expr(&*value)?;
                let var = self.values.get(name);
                match var {
                    Some(variable) => {
                        self.builder.def_var(*variable, val.value.unwrap());
                        val
                    }
                    None => return Err(Error::Undefined(format!("variable {}", name))),
                }
            }
        };
        Ok(value)
    }
}

impl Generator {
    pub fn new() -> Self {
        let shared_builder = settings::builder();
        // shared_builder
        //     .set("opt_level", "best")
        //     .expect("set optlevel");
        let shared_flags = settings::Flags::new(shared_builder);
        let isa_builder = isa::lookup(triple!("x86_64-unknown-linux-gnu")).expect("isa");
        let isa = isa_builder.finish(shared_flags).expect("Isa error");

        let builder = ObjectBuilder::new(isa, "program", default_libcall_names()).unwrap(); //TODO: Fix tehse unwraps
        let module = ObjectModule::new(builder);
        Self {
            builder_context: FunctionBuilderContext::new(),
            functions: HashMap::new(),
            module,
            variable_builder: VariableBuilder::new(),
        }
    }

    pub fn init_essential_lib(&mut self) -> Result<()> {
        self.raw_func()?;
        Ok(())
    }

    // pub fn get_function_exe<T>(&mut self, func_name: String) -> Option<fn() -> T> {
    //     match self.functions.get(&func_name) {
    //         Some(func) => unsafe {
    //             let exe = self.module.get_finalized_function(func.id);
    //             Some(mem::transmute_copy(&exe))
    //         },
    //         None => None,
    //     }
    // }

    fn signature_append_from_prototype(&self, prototype: &Prototype, signature: &mut Signature) {
        for parameter in &prototype.parameters {
            let type_ = get_type_from_str(&parameter.type_).expect("Parameter can't be void");
            signature.params.push(AbiParam::new(type_));
        }

        let return_type = get_type_from_str(&prototype.return_type);
        if let Some(tp) = return_type {
            signature.returns.push(AbiParam::new(tp));
        }
    }

    pub fn prototype(&mut self, prototype: &Prototype, linkage: Linkage) -> Result<FuncId> {
        let function_name = &prototype.function_name;

        match self.functions.get(function_name) {
            None => {
                let mut signature = self.module.make_signature();
                self.signature_append_from_prototype(prototype, &mut signature);

                let id = self
                    .module
                    .declare_function(&function_name, linkage, &signature)?;
                self.functions.insert(
                    function_name.to_string(),
                    CompiledFunction {
                        defined: false,
                        id,
                        param_count: prototype.parameters.len(),
                        return_count: if prototype.return_type == "void" {
                            0
                        } else {
                            1
                        },
                    },
                );
                Ok(id)
            }
            Some(function) => {
                if function.defined {
                    return Err(Error::FunctionRedef);
                }
                if function.param_count != prototype.parameters.len() {
                    return Err(Error::FunctionRedefWithDifferentParams);
                }
                Ok(function.id)
            }
        }
    }

    pub fn raw_func(&mut self) -> Result<()> {
        macro_rules! decl_cast {
            ($from:literal,$to:literal,$exec:block) => {
                let mut context = self.module.make_context();
                let signature = &mut context.func.signature;
                signature
                    .params
                    .push(AbiParam::new(typing::get_type_from_str($from).unwrap()));
                signature
                    .returns
                    .push(AbiParam::new(typing::get_type_from_str($to).unwrap()));

                let parameters = vec![Parameter {
                    name: "val".to_string(),
                    type_: $from,
                }];

                let prototype = Prototype {
                    function_name: format!("{}->{}", $from, $to),
                    parameters,
                    return_type: $to,
                };

                let func_id = self.prototype(&prototype, Linkage::Export)?;

                // Creates new block for function
                let mut builder =
                    FunctionBuilder::new(&mut context.func, &mut self.builder_context);
                let entry_block = builder.create_block();
                builder.append_block_params_for_function_params(entry_block);
                builder.switch_to_block(entry_block);
                builder.seal_block(entry_block);

                // Add parameters to stack
                let val = builder.block_params(entry_block)[0];

                let return_value: Value = $exec(&mut builder, &val);

                builder.ins().return_(&[return_value]);

                if let Some(ref mut function) = self.functions.get_mut("i64->f64") {
                    function.defined = true;
                }
                builder.finalize();
                println!("{}", context.func.display().to_string());

                self.module.define_function(func_id, &mut context)?;
                self.module.clear_context(&mut context);
            };
        }

        // Int -> Float
        decl_cast!("i8", "f32", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F32, *val)
            }
        });

        decl_cast!("i16", "f32", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F32, *val)
            }
        });

        decl_cast!("i32", "f32", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F32, *val)
            }
        });

        decl_cast!("i64", "f32", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F32, *val)
            }
        });

        // Int -> Double
        decl_cast!("i8", "f64", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F64, *val)
            }
        });

        decl_cast!("i16", "f64", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F64, *val)
            }
        });

        decl_cast!("i32", "f64", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F64, *val)
            }
        });

        decl_cast!("i64", "f64", {
            |builder: &mut FunctionBuilder, val: &Value| {
                builder.ins().fcvt_from_sint(types::F64, *val)
            }
        });

        Ok(())
    }

    pub fn function(&mut self, function: Function) -> Result<()> {
        let mut context = self.module.make_context();
        let mut signature = &mut context.func.signature;
        let parameters = &function.prototype.parameters;

        self.signature_append_from_prototype(&function.prototype, &mut signature);

        let function_name = function.prototype.function_name.to_string();
        let func_id = self.prototype(&function.prototype, Linkage::Export)?;

        // Creates new block for function
        let mut builder = FunctionBuilder::new(&mut context.func, &mut self.builder_context);
        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Add parameters to stack
        let mut values = HashMap::new();
        for (i, parameter) in parameters.iter().enumerate() {
            let val = builder.block_params(entry_block)[i];
            let variable = self.variable_builder.create_var(
                &mut builder,
                val,
                get_type_from_str(&parameter.type_).unwrap(), // Safe to unwrap, it would've panicked while making the prototype otherwise
            );
            values.insert(parameter.name.clone(), variable);
        }

        if let Some(ref mut function) = self.functions.get_mut(&function_name) {
            function.defined = true;
        }

        let mut generator = FunctionGenerator {
            builder,
            functions: &self.functions,
            module: &mut self.module,
            values,
            variable_builder: &mut self.variable_builder,
        };

        match generator.expr(&function.body) {
            Ok(value) => value,
            Err(error) => {
                dbg!(&error);
                generator.builder.finalize();
                self.functions.remove(&function_name);
                return Err(error);
            }
        };

        generator.builder.finalize();
        // optimize(&mut context, self.module.isa().to_owned());
        println!("{}", context.func.display().to_string());

        self.module.define_function(func_id, &mut context)?;
        self.module.clear_context(&mut context);
        // self.module.finalize_definitions();
        Ok(())
    }
}
