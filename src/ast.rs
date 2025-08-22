#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum BinaryOp {
    LessThan,
    Minus,
    Plus,
    Times,
    Equal,
}

// TODO: Add types to expressions
#[derive(Debug)]
pub enum ExprType {
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Integer(i64),
    Boolean(bool),
    Float(f64),
    Let(String, Option<Box<Expr>>),
    Assign(String, Box<Expr>),
    Variable(String),
    Block(Vec<Expr>),
    Return(Option<Box<Expr>>),
    IfElse(Box<Expr>, Box<Expr>, Option<Box<Expr>>), // Condition -> if -> else]
    While(Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub struct Expr {
    pub expr_type: ExprType,
    pub type_: &'static str,
}

#[derive(Debug)]
pub struct Function {
    pub prototype: Prototype,
    pub body: Expr,
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub type_: &'static str,
}

#[derive(Debug)]
pub struct Prototype {
    pub function_name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: &'static str,
}
