use std::{collections::HashMap, mem, str::FromStr};

use cranelift::{
    codegen::{
        entity::EntityRef,
        ir::{condcodes::FloatCC, types, AbiParam, InstBuilder, Value},
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
}

pub struct FunctionGenerator<'a> {
    builder: FunctionBuilder<'a>,
    functions: &'a HashMap<String, CompiledFunction>,
    module: &'a mut Module<SimpleJITBackend>,
    values: HashMap<String, Variable>,
}

struct ParseExpr {
    pub value: Value,
    pub is_return: bool,
}

impl ParseExpr {
    pub fn new(value: Value) -> Self {
        Self {
            value,
            is_return: false,
        }
    }

    pub fn new_return(value: Value) -> Self {
        Self {
            value,
            is_return: true,
        }
    }
}

impl<'a> FunctionGenerator<'a> {
    fn expr(&mut self, expr: Expr) -> Result<ParseExpr> {
        let value = match expr {
            Expr::Number(num) => ParseExpr::new(self.builder.ins().f64const(num)),
            Expr::Variable(name) => match self.values.get(&name) {
                Some(&variable) => ParseExpr::new(self.builder.use_var(variable)),
                None => return Err(Error::Undefined("variable")),
            },
            Expr::Binary(op, left, right) => {
                let left = self.expr(*left)?.value;
                let right = self.expr(*right)?.value;
                match op {
                    BinaryOp::Plus => ParseExpr::new(self.builder.ins().fadd(left, right)),
                    BinaryOp::Minus => ParseExpr::new(self.builder.ins().fsub(left, right)),
                    BinaryOp::Times => ParseExpr::new(self.builder.ins().fmul(left, right)),
                    BinaryOp::LessThan => {
                        let boolean = self.builder.ins().fcmp(FloatCC::LessThan, left, right);
                        let int = self.builder.ins().bint(types::I32, boolean);
                        ParseExpr::new(self.builder.ins().fcvt_from_sint(types::F64, int))
                    }
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
                    let arguments: Vec<Value> =
                        arguments?.into_iter().map(|arg| arg.value).collect();

                    let call = self.builder.ins().call(local_func, &arguments);
                    ParseExpr::new(self.builder.inst_results(call)[0])
                }
                None => return Err(Error::Undefined("function")),
            },
            Expr::Block(exprs) => {
                let mut value: Option<ParseExpr> = None;
                for expr in exprs {
                    value = Some(self.expr(expr)?);
                }
                match value {
                    Some(value) => value,
                    None => ParseExpr::new_return(self.builder.ins().f64const(0.0)),
                }
            }
            Expr::Return(expr) => {
                let value = self.expr(*expr)?;
                ParseExpr::new_return(value.value)
            }
        };
        Ok(value)
    }
}

pub struct Generator {
    builder_context: FunctionBuilderContext,
    functions: HashMap<String, CompiledFunction>,
    module: Module<SimpleJITBackend>,
    variable_builder: VariableBuilder,
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

    pub fn get_function_executable<T>(&mut self, func_name: String) -> Option<fn() -> T> {
        match self.functions.get(&func_name) {
            Some(func) => unsafe {
                Some(mem::transmute(self.module.get_finalized_function(func.id)))
            },
            None => None,
        }
    }

    pub fn prototype(&mut self, prototype: &Prototype, linkage: Linkage) -> Result<FuncId> {
        let function_name = &prototype.function_name;
        let parameters = &prototype.parameters;
        match self.functions.get(function_name) {
            None => {
                let mut signature = self.module.make_signature();
                for _parameter in parameters {
                    signature.params.push(AbiParam::new(types::F64));
                }
                let return_type = match prototype.return_type.as_str() {
                    "f64" => Some(types::F64),
                    _ => None,
                };
                if let Some(x) = return_type {
                    signature.returns.push(AbiParam::new(x));
                }

                let id = self
                    .module
                    .declare_function(&function_name, linkage, &signature)?;
                self.functions.insert(
                    function_name.to_string(),
                    CompiledFunction {
                        defined: false,
                        id,
                        param_count: parameters.len(),
                    },
                );
                Ok(id)
            }
            Some(function) => {
                if function.defined {
                    return Err(Error::FunctionRedef);
                }
                if function.param_count != parameters.len() {
                    return Err(Error::FunctionRedefWithDifferentParams);
                }
                Ok(function.id)
            }
        }
    }

    pub fn function(&mut self, function: Function) -> Result<()> {
        let mut context = self.module.make_context();
        let signature = &mut context.func.signature;
        let parameters = &function.prototype.parameters;

        // Parameters types
        for _parameter in parameters {
            signature.params.push(AbiParam::new(types::F64));
        }
        // Return type
        signature.returns.push(AbiParam::new(types::F64));

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
        };

        let return_value = match generator.expr(function.body) {
            Ok(value) => value,
            Err(error) => {
                generator.builder.finalize();
                self.functions.remove(&function_name);
                return Err(error);
            }
        };

        if return_value.is_return {
            generator.builder.ins().return_(&[return_value.value]);
        } else {
            let empty_return_value = generator.builder.ins().f64const(0.0);
            generator.builder.ins().return_(&[empty_return_value]);
        }

        generator.builder.finalize();
        // optimize(&mut context, &*self.module.isa())?;
        println!("{}", context.func.display(None).to_string());

        self.module.define_function(func_id, &mut context)?;
        self.module.clear_context(&mut context);
        self.module.finalize_definitions();
        Ok(())
    }
}
