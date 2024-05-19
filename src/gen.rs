use std::{collections::HashMap, mem, str::FromStr};

use cranelift::{
    codegen::{
        entity::EntityRef,
        ir::{condcodes::FloatCC, types, AbiParam, InstBuilder, Signature, Type, Value},
        isa,
        settings::{self, Configurable},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext, Variable},
};
use cranelift_module::{FuncId, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use target_lexicon::triple;

use crate::{
    ast::{BinaryOp, Expr, Function, Prototype},
    error::{Error, Result},
};

struct VariableBuilder {
    index: usize,
}

impl VariableBuilder {
    fn new() -> Self {
        Self { index: 0 }
    }

    fn define_var(&mut self, builder: &mut FunctionBuilder) -> Variable {
        let variable = Variable::new(self.index);
        builder.declare_var(variable, types::F64);
        self.index += 1;
        variable
    }

    fn create_var(&mut self, builder: &mut FunctionBuilder, value: Value) -> Variable {
        let variable = Variable::new(self.index);
        builder.declare_var(variable, types::F64);
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
    module: &'a mut Module<SimpleJITBackend>,
    variable_builder: &'a mut VariableBuilder,
    values: HashMap<String, Variable>,
}

pub struct Generator {
    builder_context: FunctionBuilderContext,
    functions: HashMap<String, CompiledFunction>,
    module: Module<SimpleJITBackend>,
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
    fn expr(&mut self, expr: Expr) -> Result<ParseExpr> {
        let value = match expr {
            Expr::Number(num) => ParseExpr::new(Some(self.builder.ins().f64const(num))),
            Expr::Variable(name) => match self.values.get(&name) {
                Some(&variable) => ParseExpr::new(Some(self.builder.use_var(variable))),
                None => {
                    dbg!(name);
                    return Err(Error::Undefined("variable"));
                }
            },
            Expr::Binary(op, left, right) => {
                let left = self.expr(*left)?.value.unwrap(); // TODO: unwrap these properly
                let right = self.expr(*right)?.value.unwrap();
                match op {
                    BinaryOp::Plus => ParseExpr::new(Some(self.builder.ins().fadd(left, right))),
                    BinaryOp::Minus => ParseExpr::new(Some(self.builder.ins().fsub(left, right))),
                    BinaryOp::Times => ParseExpr::new(Some(self.builder.ins().fmul(left, right))),
                    BinaryOp::LessThan => ParseExpr::new(Some(self.builder.ins().fcmp(
                        FloatCC::LessThan,
                        left,
                        right,
                    ))),
                }
            }
            Expr::Call(name, args) => match self.functions.get(&name) {
                Some(func) => {
                    if func.param_count != args.len() {
                        return Err(Error::WrongArgumentCount);
                    }
                    let local_func = self
                        .module
                        .declare_func_in_func(func.id, &mut self.builder.func);
                    let arguments: Result<Vec<_>> =
                        args.into_iter().map(|arg| self.expr(arg)).collect();
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
                None => return Err(Error::Undefined("function")),
            },
            Expr::Block(exprs) => {
                for expr in exprs {
                    let parse_expr = self.expr(expr)?;
                    if parse_expr.is_return {
                        return Ok(parse_expr);
                    }
                }
                self.builder.ins().return_(&[]);
                ParseExpr::empty()
            }
            Expr::Return(expr) => {
                match expr {
                    Some(expr) => {
                        let value = self.expr(*expr)?;
                        self.builder.ins().return_(&[value.value.unwrap()]); // TODO: Properly unwrap this
                        ParseExpr::new_return(value.value)
                    }
                    None => {
                        self.builder.ins().return_(&[]);
                        ParseExpr::empty_return()
                    }
                }
            }
            Expr::Let(name, int_expr) => match int_expr {
                None => {
                    let variable = self.variable_builder.define_var(&mut self.builder);
                    self.values.insert(name.clone(), variable);
                    ParseExpr::empty()
                }
                Some(value) => {
                    let parse_expr = self.expr(*value)?;
                    let variable = self
                        .variable_builder
                        .create_var(&mut self.builder, parse_expr.value.expect("value"));
                    self.values.insert(name.clone(), variable);
                    parse_expr
                }
            },
            Expr::Assign(name, value) => {
                let val = self.expr(*value)?;
                let var = self.values.get(&name);
                match var {
                    Some(variable) => {
                        self.builder.def_var(*variable, val.value.unwrap());
                        val
                    }
                    None => return Err(Error::Undefined("variable")),
                }
            }
        };
        Ok(value)
    }
}

impl Generator {
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "best").expect("set optlevel");
        let isa_builder = isa::lookup(triple!("x86_64-unknown-unknown-elf")).expect("isa");
        let isa = isa_builder.finish(settings::Flags::new(flag_builder));
        Self {
            builder_context: FunctionBuilderContext::new(),
            functions: HashMap::new(),
            module: Module::new(SimpleJITBuilder::with_isa(isa)),
            variable_builder: VariableBuilder::new(),
        }
    }

    pub fn get_function_exe<T: Fn() -> K, K>(&mut self, func_name: String) -> Option<T> {
        match self.functions.get(&func_name) {
            Some(func) => unsafe {
                let exe = self.module.get_finalized_function(func.id);
                Some(mem::transmute_copy(&exe))
            },
            None => None,
        }
    }

    fn signature_append_from_prototype(&self, prototype: &Prototype, signature: &mut Signature) {
        for _parameter in &prototype.parameters {
            signature.params.push(AbiParam::new(types::F64));
        }

        let return_type = Generator::get_type_from_str(&prototype.return_type);
        if let Some(tp) = return_type {
            signature.returns.push(AbiParam::new(tp));
        }
    }

    fn get_type_from_str(str: &str) -> Option<Type> {
        match str {
            "f64" => Some(types::F64),
            "void" => None,
            _ => None, // TODO: Trigger error
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

    pub fn function(&mut self, function: Function) -> Result<()> {
        let mut context = self.module.make_context();
        let mut signature = &mut context.func.signature;
        let parameters = &function.prototype.parameters;

        self.signature_append_from_prototype(&function.prototype, &mut signature);

        let function_name = function.prototype.function_name.to_string();
        let func_id = self.prototype(&function.prototype, Linkage::Export)?;

        // Creates new block for function
        let mut builder = FunctionBuilder::new(&mut context.func, &mut self.builder_context);
        let entry_block = builder.create_ebb();
        builder.append_ebb_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Add parameters to stack
        let mut values = HashMap::new();
        for (i, name) in parameters.iter().enumerate() {
            let val = builder.ebb_params(entry_block)[i];
            let variable = self.variable_builder.create_var(&mut builder, val);
            values.insert(name.clone(), variable);
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

        match generator.expr(function.body) {
            Ok(value) => value,
            Err(error) => {
                dbg!(&error);
                generator.builder.finalize();
                self.functions.remove(&function_name);
                return Err(error);
            }
        };

        generator.builder.finalize();
        // optimize(&mut context, &*self.module.isa())?;
        println!("{}", context.func.display(None).to_string());

        self.module.define_function(func_id, &mut context)?;
        self.module.clear_context(&mut context);
        self.module.finalize_definitions();
        Ok(())
    }
}
