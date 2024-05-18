use std::{collections::HashMap, io::Read};

use crate::{
    ast::{BinaryOp, Expr, Function, Prototype},
    error::{Error, Result},
    lexer::{Lexer, Token},
};

pub struct Parser<R: Read> {
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
            bin_precedence,
            lexer,
        };
    }

    pub fn definition(&mut self) -> Result<Function> {
        self.eat(Token::Def)?;
        let prototype = self.prototype()?;

        let body;

        body = self.block()?;

        Ok(Function { prototype, body })
    }

    fn block(&mut self) -> Result<Expr> {
        let mut exprs: Vec<Expr> = vec![];
        self.eat(Token::OpenBracket)?;
        loop {
            let peek = self.lexer.peek()?;
            if *peek == Token::Return {
                self.eat(Token::Return)?;
                let peek = self.lexer.peek()?;
                if *peek == Token::Semicolon {
                    exprs.push(Expr::Return(None));
                } else {
                    exprs.push(Expr::Return(Some(Box::new(self.expr()?))));
                }
            } else {
                exprs.push(self.expr()?);
            }
            self.eat(Token::Semicolon)?;
            let peek = self.lexer.peek()?;
            if *peek == Token::CloseBracket {
                self.eat(Token::CloseBracket)?;
                break;
            }
        }
        Ok(Expr::Block(exprs))
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
        let return_type = match self.lexer.peek()? {
            Token::Identifier(_) => self.identifier()?,
            _ => "void".to_string(),
        };

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

    fn parameters(&mut self) -> Result<Vec<String>> {
        self.eat(Token::OpenParen)?;
        let mut params = vec![];
        let mut accept_more = true;
        loop {
            match self.lexer.peek()? {
                Token::Identifier(_) => {
                    if !accept_more {
                        return Err(Error::Unexpected("operator, expected ','"));
                    }
                    accept_more = false;

                    let ident = match self.lexer.next_token()? {
                        Token::Identifier(ident) => ident,
                        _ => unreachable!(),
                    };
                    params.push(ident);
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
        match *self.lexer.peek()? {
            Token::Number(number) => {
                self.lexer.next_token()?;
                Ok(Expr::Number(number))
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
        let ast = match self.lexer.peek()? {
            Token::OpenParen => {
                self.eat(Token::OpenParen)?;
                let args = self.args()?;
                self.eat(Token::CloseParen)?;
                Expr::Call(name, args)
            }
            _ => Expr::Variable(name),
        };
        Ok(ast)
    }

    fn args(&mut self) -> Result<Vec<Expr>> {
        if *self.lexer.peek()? == Token::CloseParen {
            return Ok(vec![]);
        }
        let mut args = vec![self.expr()?];
        while *self.lexer.peek()? == Token::Comma {
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
                    let left = Expr::Binary(op, Box::new(left), Box::new(right));
                    self.binary_right(expr_precedence, left)
                }
            }
            None => Ok(left),
        }
    }

    fn binary_op(&mut self) -> Result<Option<BinaryOp>> {
        let op = match self.lexer.peek()? {
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
            None => Err(Error::Undefined("operator")),
        }
    }
}
