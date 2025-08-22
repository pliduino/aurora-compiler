use std::{fs::File, io::Write};

use cranelift_module::Linkage;
use error::{Error, Result};
use lexer::Lexer;
use parser::Parser;

use crate::{gen::Generator, lexer::Token};

mod ast;
mod error;
mod gen;
mod lexer;
mod parser;
mod typing;

fn main() -> Result<()> {
    let filename = "example/test.au";
    let exe_filename = "test.exe";

    let file = File::open(filename)?;
    let lexer = Lexer::new(file);
    let mut parser = Parser::new(lexer);
    let mut generator = Generator::new();

    generator.init_essential_lib()?;

    loop {
        let token = match parser.lexer.peek(0) {
            Ok(ref token) => *token,
            Err(error) => {
                eprintln!(
                    "\x1b[91m[{}:{}:{}] Error: {:?}\x1b[0m",
                    filename,
                    parser.lexer.get_line(),
                    parser.lexer.get_pos(),
                    error
                );
                continue;
            }
        };
        match token {
            Token::Eof => break,
            Token::SemiColon => {
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
                        eprintln!(
                            "\x1b[91m[{}:{}:{}] Error: {:?}\x1b[0m",
                            filename,
                            parser.lexer.get_line(),
                            parser.lexer.get_pos(),
                            error
                        );
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
                        eprintln!(
                            "\x1b[91m[{}:{}:{}] Error: {:?}\x1b[0m",
                            filename,
                            parser.lexer.get_line(),
                            parser.lexer.get_pos(),
                            error
                        );
                    }
                }
            }
            _ => return Err(Error::Unexpected("top level token")),
        }
    }

    let mut object = generator.module.finish().emit().unwrap();

    {
        let mut output_file = File::create("./build/test.o")?;
        output_file.write_all(&mut object)?;
    }

    _ = std::process::Command::new("gcc")
        .args(&[
            "./example/lib.c",
            "./build/test.o",
            "-lm",
            "-o",
            &format!("./build/{}", exe_filename),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    Ok(())
}

#[no_mangle]
pub extern "C" fn putfloatd(float: f64) {
    println!("{}", float);
}
