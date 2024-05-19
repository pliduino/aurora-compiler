use std::{
    collections::VecDeque,
    fmt::Display,
    io::{Bytes, Read},
    iter::Peekable,
};

use crate::error::Error::UnknownChar;
use crate::error::Result;

pub struct Lexer<R: Read> {
    bytes: Peekable<Bytes<R>>,
    lookahead: VecDeque<Token>,
    line: usize,
    pos: usize,
}

impl<R: Read> Lexer<R> {
    pub fn new(reader: R) -> Self {
        Self {
            bytes: reader.bytes().peekable(),
            lookahead: VecDeque::new(),
            line: 1,
            pos: 1,
        }
    }

    pub fn get_line(&self) -> usize {
        self.line
    }

    pub fn get_pos(&self) -> usize {
        self.pos
    }

    pub fn next_token(&mut self) -> Result<Token> {
        if let Some(lookahead) = self.lookahead.pop_front() {
            return Ok(lookahead);
        }

        self.next_token_no_check()
    }

    fn next_token_no_check(&mut self) -> Result<Token> {
        if let Some(&Ok(byte)) = self.bytes.peek() {
            self.pos += 1;
            return match byte {
                // Skips empty spaces
                b' ' | b'\r' | b'\t' => {
                    self.bytes.next();
                    self.next_token_no_check()
                }
                b'\n' => {
                    self.line += 1;
                    self.pos = 1;
                    self.bytes.next();
                    self.next_token_no_check()
                }
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.identifier(),
                b'0'..=b'9' | b'.' => self.number(),
                b'#' => self.comment(),
                _ => {
                    self.bytes.next();
                    let token = match byte {
                        b'<' => Token::LessThan,
                        b'+' => Token::Plus,
                        b'-' => Token::Minus,
                        b'*' => Token::Star,
                        b';' => Token::SemiColon,
                        b',' => Token::Comma,
                        b'(' => Token::OpenParen,
                        b')' => Token::CloseParen,
                        b'{' => Token::OpenBracket,
                        b'}' => Token::CloseBracket,
                        b'=' => Token::Equal,
                        b':' => Token::Colon,
                        _ => return Err(UnknownChar(byte as char)),
                    };

                    Ok(token)
                }
            };
        }
        Ok(Token::Eof)
    }

    pub fn peek(&mut self, dist: usize) -> Result<&Token> {
        loop {
            if self.lookahead.len() >= dist + 1 {
                return Ok(self.lookahead.get(dist).unwrap());
            }

            let token = self.next_token_no_check()?;
            self.lookahead.push_back(token);
        }
    }

    fn identifier(&mut self) -> Result<Token> {
        let mut identifier = String::new();
        loop {
            if let Some(char) = self.peek_char()? {
                if char.is_ascii_alphanumeric() || char == '_' {
                    self.bytes.next();
                    identifier.push(char);
                    continue;
                }
            }
            break;
        }

        let token = match identifier.as_str() {
            "fn" => Token::Def,
            "extern" => Token::Extern,
            "return" => Token::Return,
            "let" => Token::Let,
            _ => Token::Identifier(identifier),
        };

        Ok(token)
    }

    fn peek_char(&mut self) -> Result<Option<char>> {
        if let Some(&Ok(byte)) = self.bytes.peek() {
            self.pos += 1;
            return Ok(Some(byte as char));
        }

        match self.bytes.next() {
            Some(Ok(_)) => unreachable!(),
            Some(Err(error)) => Err(error.into()),
            None => Ok(None),
        }
    }

    fn number(&mut self) -> Result<Token> {
        let integral = self.digits()?;
        if let Some('.') = self.peek_char()? {
            self.bytes.next();
            let decimals = self.digits()?;
            Ok(Token::Float(format!("{}.{}", integral, decimals).parse()?))
        } else {
            Ok(Token::Integer(integral.parse()?))
        }
    }

    fn digits(&mut self) -> Result<String> {
        let mut buffer = String::new();
        loop {
            if let Some(char) = self.peek_char()? {
                if char.is_numeric() {
                    self.bytes.next();
                    buffer.push(char);
                    continue;
                }
            }
            break;
        }

        Ok(buffer)
    }

    fn comment(&mut self) -> Result<Token> {
        loop {
            if let Some(char) = self.peek_char()? {
                self.bytes.next();
                if char == '\n' {
                    break;
                }
            } else {
                return Ok(Token::Eof);
            }
        }
        self.next_token()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Eof,

    // Commands
    Def,
    Extern,
    Let,

    // Primary
    Identifier(String),
    Integer(i64),
    Float(f64),

    // Operators
    LessThan,
    Minus,
    Plus,
    Star,

    Equal,

    // Other
    Colon,
    SemiColon,
    OpenParen,
    CloseParen,
    Comma,
    OpenBracket,
    CloseBracket,
    Return,
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Eof => todo!(),
            Token::Def => write!(f, "fn"),
            Token::Extern => write!(f, "extern"),
            Token::Identifier(id) => write!(f, "identifier: {}", id),
            Token::Float(_) => todo!(),
            Token::Integer(_) => todo!(),
            Token::LessThan => write!(f, "<"),
            Token::Minus => write!(f, "-"),
            Token::Plus => write!(f, "+"),
            Token::Star => write!(f, "*"),
            Token::SemiColon => write!(f, ";"),
            Token::OpenParen => write!(f, "("),
            Token::CloseParen => write!(f, ")"),
            Token::Comma => write!(f, ","),
            Token::OpenBracket => write!(f, "{{"),
            Token::CloseBracket => write!(f, "}}"),
            Token::Return => write!(f, "return"),
            Token::Equal => write!(f, "="),
            Token::Let => write!(f, "let"),
            Token::Colon => write!(f, ":"),
        }
    }
}
