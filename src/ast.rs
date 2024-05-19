#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum BinaryOp {
    LessThan,
    Minus,
    Plus,
    Times,
}

// TODO: Add types to expressions
#[derive(Debug)]
pub enum Expr {
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Integer(i64),
    Float(f64),
    Let(String, Option<Box<Expr>>),
    Assign(String, Box<Expr>),
    Variable(String),
    Block(Vec<Expr>),
    Return(Option<Box<Expr>>),
}

#[derive(Debug)]
pub struct Function {
    pub prototype: Prototype,
    pub body: Expr,
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub type_: String,
}

#[derive(Debug)]
pub struct Prototype {
    pub function_name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: String,
}
