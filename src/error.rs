use std::fmt::{self, Debug, Formatter};
use std::num::ParseIntError;
use std::{io, num::ParseFloatError, result};

use cranelift::codegen::CodegenError;
use cranelift_module::ModuleError;

use crate::lexer::Token;
use crate::typing::AuroraType;

use self::Error::*;

pub type Result<T> = result::Result<T, Error>;

pub enum Error {
    CraneliftCodegen(CodegenError),
    CraneliftModule(ModuleError),
    Io(io::Error),
    ParseFloat(ParseFloatError),
    ParseInt(ParseIntError),
    UnknownChar(char),
    Undefined(String),
    Unexpected(&'static str),
    UnexpectedToken(Token, Token),
    WrongArgumentCount,
    VariableRedef,
    FunctionRedef,
    FunctionRedefWithDifferentParams,
    MismatchedTypes(AuroraType, AuroraType),
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Io(ref error) => error.fmt(formatter),
            ParseFloat(ref error) => error.fmt(formatter),
            ParseInt(ref error) => error.fmt(formatter),
            UnknownChar(char) => write!(formatter, "unknown char `{}`", char),
            Undefined(msg) => write!(formatter, "undefined {}", msg),
            Unexpected(msg) => write!(formatter, "unexpected {}", msg),
            WrongArgumentCount => write!(formatter, "wrong argument count"),
            FunctionRedef => write!(formatter, "redefinition of function"),
            VariableRedef => write!(formatter, "redefinition of a variable"),
            FunctionRedefWithDifferentParams => write!(
                formatter,
                "redefinition of function with different number of parameters"
            ),
            CraneliftModule(ref error) => error.fmt(formatter),
            CraneliftCodegen(ref error) => error.fmt(formatter),
            UnexpectedToken(expected, got) => write!(
                formatter,
                "unexpected token, was expecting '{}' but got '{}'",
                expected, got,
            ),
            MismatchedTypes(expected, got) => write!(
                formatter,
                "mismatched type, was expecting '{}' but got '{}'",
                expected, got,
            ),
        }
    }
}

// Error conversions
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Io(error)
    }
}

impl From<ParseFloatError> for Error {
    fn from(error: ParseFloatError) -> Self {
        ParseFloat(error)
    }
}

impl From<ModuleError> for Error {
    fn from(error: ModuleError) -> Self {
        CraneliftModule(error)
    }
}

impl From<CodegenError> for Error {
    fn from(error: CodegenError) -> Self {
        CraneliftCodegen(error)
    }
}

impl From<ParseIntError> for Error {
    fn from(error: ParseIntError) -> Self {
        ParseInt(error)
    }
}
