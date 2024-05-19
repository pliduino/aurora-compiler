use std::{collections::HashMap, io::Read};

use crate::{
    ast::{BinaryOp, Expr, ExprType, Function, Parameter, Prototype},
    error::{Error, Result},
    lexer::{Lexer, Token},
    typing,
};

pub struct Parser<R: Read> {
    type_map: HashMap<String, &'static str>,
    bin_precedence: HashMap<BinaryOp, i32>,
    pub lexer: Lexer<R>,
}

impl<R: Read> Parser<R> {
    pub fn new(lexer: Lexer<R>) -> Self {
        let mut bin_precedence = HashMap::new();
        bin_precedence.insert(BinaryOp::LessThan, 10);
        bin_precedence.insert(BinaryOp::Plus, 20);
        bin_precedence.insert(BinaryOp::Minus, 20);
        bin_precedence.insert(BinaryOp::Times, 40);
        return Self {
            type_map: HashMap::new(),
            bin_precedence,
            lexer,
        };
    }

    pub fn definition(&mut self) -> Result<Function> {
        self.eat(Token::Def)?;
        let prototype = self.prototype()?;

        for par in &prototype.parameters {
            self.type_map.insert(par.name.clone(), par.type_);
        }

        let body = self.block(prototype.return_type)?;

        for par in &prototype.parameters {
            self.type_map.remove(&par.name);
        }

        Ok(Function { prototype, body })
    }

    fn block(&mut self, type_: &'static str) -> Result<Expr> {
        let mut exprs: Vec<Expr> = vec![];
        self.eat(Token::OpenBracket)?;
        loop {
            let peek = (*self.lexer.peek(0)?).clone();
            match peek {
                Token::Return => {
                    self.eat(Token::Return)?;
                    let peek = self.lexer.peek(0)?;
                    if *peek == Token::SemiColon {
                        exprs.push(Expr {
                            expr_type: ExprType::Return(None),
                            type_: typing::VOID,
                        })
                    } else {
                        let expr = Box::new(self.expr()?);
                        exprs.push(Expr {
                            type_: expr.type_,
                            expr_type: ExprType::Return(Some(expr)),
                        })
                    }
                }
                Token::Let => exprs.push(self.let_()?),
                Token::Identifier(_) if *self.lexer.peek(1)? == Token::Equal => {
                    exprs.push(self.assign()?)
                }
                _ => exprs.push(self.expr()?),
            }
            self.eat(Token::SemiColon)?;
            let peek = self.lexer.peek(0)?;
            if *peek == Token::CloseBracket {
                self.eat(Token::CloseBracket)?;
                break;
            }
        }
        Ok(Expr {
            expr_type: ExprType::Block(exprs),
            type_,
        })
    }

    fn let_(&mut self) -> Result<Expr> {
        self.eat(Token::Let)?;
        let name = self.identifier()?;
        let token = self.lexer.peek(0)?;
        let mut type_: &str = typing::ANY;
        if *token == Token::Colon {
            self.eat(Token::Colon)?;
            type_ = typing::get_const_str_from_string(self.identifier()?);
        }

        let peek = self.lexer.peek(0)?;
        match peek {
            Token::Equal => {
                self.eat(Token::Equal)?;
                let expr = self.expr()?;
                if type_.is_empty() || expr.type_ == type_ {
                    if let Some(_) = self.type_map.insert(name.clone(), expr.type_) {
                        return Err(Error::VariableRedef);
                    }
                    Ok(Expr {
                        type_: expr.type_,
                        expr_type: ExprType::Let(name, Some(Box::new(expr))),
                    })
                } else {
                    dbg!(type_, &expr);
                    Err(Error::MismatchedTypes(type_, expr.type_))
                }
            }
            Token::SemiColon => {
                if let Some(_) = self.type_map.insert(name.clone(), type_) {
                    return Err(Error::VariableRedef);
                }
                if type_ == typing::ANY {
                    return Err(Error::Undefined("type".to_string())); //TODO: Remove this and try to infer it later via type_map
                }
                Ok(Expr {
                    expr_type: ExprType::Let(name, None),
                    type_,
                })
            }
            _ => Err(Error::Unexpected("Expected ';' or '='")),
        }
    }

    fn assign(&mut self) -> Result<Expr> {
        let name = self.identifier()?;
        self.eat(Token::Equal)?;
        let expr = self.expr()?;
        Ok(Expr {
            type_: expr.type_,
            expr_type: ExprType::Assign(name, Box::new(expr)),
        })
    }

    fn eat(&mut self, token: Token) -> Result<()> {
        let current_token = self.lexer.next_token()?;
        if current_token != token {
            return Err(Error::UnexpectedToken(token, current_token));
        }
        Ok(())
    }

    fn prototype(&mut self) -> Result<Prototype> {
        let function_name = self.identifier()?;
        let parameters = self.parameters()?;
        let return_type = match self.lexer.peek(0)? {
            Token::Identifier(_) => typing::get_const_str_from_string(self.identifier()?),
            _ => typing::VOID,
        };

        if let Some(_) = self.type_map.insert(function_name.clone(), return_type) {
            return Err(Error::FunctionRedef);
        }

        Ok(Prototype {
            function_name,
            parameters,
            return_type,
        })
    }

    pub fn extern_(&mut self) -> Result<Prototype> {
        self.eat(Token::Extern)?;
        self.prototype()
    }

    fn identifier(&mut self) -> Result<String> {
        match self.lexer.next_token()? {
            Token::Identifier(identifier) => Ok(identifier),
            _ => Err(Error::Unexpected("token, expecting identifier")),
        }
    }

    fn parameters(&mut self) -> Result<Vec<Parameter>> {
        self.eat(Token::OpenParen)?;
        let mut params = vec![];
        let mut accept_more = true;
        loop {
            match self.lexer.peek(0)? {
                Token::Identifier(_) => {
                    if !accept_more {
                        return Err(Error::Unexpected("operator, expected ','"));
                    }
                    accept_more = false;

                    let name = match self.lexer.next_token()? {
                        Token::Identifier(id) => id,
                        _ => unreachable!(),
                    };
                    self.eat(Token::Colon)?;
                    let type_ =
                        typing::get_const_str_from_string(match self.lexer.next_token()? {
                            Token::Identifier(t) => t,
                            _ => return Err(Error::Unexpected("type token")),
                        });
                    params.push(Parameter { name, type_ });
                }
                Token::CloseParen => {
                    self.eat(Token::CloseParen)?;
                    break;
                }
                Token::Comma => {
                    self.eat(Token::Comma)?;
                    accept_more = true;
                }
                x => {
                    dbg!(x);
                    return Err(Error::Unexpected("token"));
                }
            }
        }

        Ok(params)
    }

    fn primary(&mut self) -> Result<Expr> {
        match *self.lexer.peek(0)? {
            Token::Float(f) => {
                self.lexer.next_token()?;
                Ok(Expr {
                    expr_type: ExprType::Float(f),
                    type_: typing::F64,
                })
            }
            Token::Integer(i) => {
                self.lexer.next_token()?;
                Ok(Expr {
                    expr_type: ExprType::Integer(i),
                    type_: typing::I64,
                })
            }
            Token::OpenParen => {
                self.eat(Token::OpenParen)?;
                let expr = self.expr()?;
                self.eat(Token::CloseParen)?;
                Ok(expr)
            }
            Token::Identifier(_) => self.ident_expr(),
            _ => Err(Error::Unexpected("token when expecting an expression")),
        }
    }

    fn ident_expr(&mut self) -> Result<Expr> {
        let name = self.identifier()?;
        let type_ = match self.type_map.get(&name) {
            Some(t) => *t,
            None => return Err(Error::Undefined(format!("identifier {}", name))),
        };
        let ast = match self.lexer.peek(0)? {
            Token::OpenParen => {
                self.eat(Token::OpenParen)?;
                let args = self.args()?;
                self.eat(Token::CloseParen)?;
                Expr {
                    expr_type: ExprType::Call(name, args),
                    type_,
                }
            }
            _ => Expr {
                expr_type: ExprType::Variable(name),
                type_,
            },
        };
        Ok(ast)
    }

    fn args(&mut self) -> Result<Vec<Expr>> {
        if *self.lexer.peek(0)? == Token::CloseParen {
            return Ok(vec![]);
        }
        let mut args = vec![self.expr()?];
        while *self.lexer.peek(0)? == Token::Comma {
            self.eat(Token::Comma)?;
            args.push(self.expr()?);
        }

        Ok(args)
    }

    fn expr(&mut self) -> Result<Expr> {
        let left = self.primary()?;
        self.binary_right(0, left)
    }

    fn binary_right(&mut self, expr_precedence: i32, left: Expr) -> Result<Expr> {
        match self.binary_op()? {
            Some(op) => {
                let token_precedence = self.precedence(op)?;
                if token_precedence < expr_precedence {
                    Ok(left)
                } else {
                    self.lexer.next_token()?;
                    let right = self.primary()?;
                    let right = match self.binary_op()? {
                        Some(op) => {
                            if token_precedence < self.precedence(op)? {
                                self.binary_right(token_precedence + 1, right)?
                            } else {
                                right
                            }
                        }
                        None => right,
                    };
                    let left = Expr {
                        type_: left.type_,
                        expr_type: ExprType::Binary(op, Box::new(left), Box::new(right)),
                    };
                    self.binary_right(expr_precedence, left)
                }
            }
            None => Ok(left),
        }
    }

    fn binary_op(&mut self) -> Result<Option<BinaryOp>> {
        let op = match self.lexer.peek(0)? {
            Token::LessThan => BinaryOp::LessThan,
            Token::Minus => BinaryOp::Minus,
            Token::Plus => BinaryOp::Plus,
            Token::Star => BinaryOp::Times,
            _ => return Ok(None),
        };
        Ok(Some(op))
    }

    fn precedence(&mut self, op: BinaryOp) -> Result<i32> {
        match self.bin_precedence.get(&op) {
            Some(&precedence) => Ok(precedence),
            None => Err(Error::Undefined("operator".to_string())),
        }
    }
}
