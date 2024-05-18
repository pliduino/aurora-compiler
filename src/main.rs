use std::{
    fs::File,
    io::{stdin, stdout, Write},
};

use cranelift_module::Linkage;
use error::Result;
use lexer::Lexer;
use parser::Parser;

use crate::{gen::Generator, lexer::Token};

mod ast;
mod error;
mod gen;
mod lexer;
mod parser;

fn main() -> Result<()> {
    let file = File::open("test.au")?;
    let lexer = Lexer::new(file);
    let mut parser = Parser::new(lexer);
    let mut generator = Generator::new();
    loop {
        let token = match parser.lexer.peek() {
            Ok(ref token) => *token,
            Err(error) => {
                eprintln!("Error: {:?}", error);
                continue;
            }
        };
        match token {
            Token::Eof => break,
            Token::Semicolon => {
                parser.lexer.next_token()?;
                continue;
            }
            Token::Def => {
                match parser
                    .definition()
                    .and_then(|definition| generator.function(definition))
                {
                    Ok(_definition) => (),
                    Err(error) => {
                        parser.lexer.next_token()?;
                        eprintln!("Error: {:?}", error);
                    }
                }
            }
            Token::Extern => {
                match parser
                    .extern_()
                    .and_then(|prototype| generator.prototype(&prototype, Linkage::Import))
                {
                    Ok(prototype) => println!("{}", prototype),
                    Err(error) => {
                        parser.lexer.next_token()?;
                        eprintln!("Error: {:?}", error);
                    }
                }
            }
            _ => match parser.toplevel().and_then(|expr| generator.function(expr)) {
                Ok(function) => println!("{}", function()),
                Err(error) => {
                    parser.lexer.next_token()?;
                    eprintln!("Error: {:?}", error);
                }
            },
        }
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn putchard(char: f64) -> f64 {
    println!("{}", char as u8 as char);
    0.0
}
