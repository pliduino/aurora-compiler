use std::fmt::{self, Debug, Formatter};
use std::{io, num::ParseFloatError, result};

use cranelift::codegen::CodegenError;
use cranelift_module::ModuleError;

use self::Error::*;

pub type Result<T> = result::Result<T, Error>;

pub enum Error {
    CraneliftCodegen(CodegenError),
    CraneliftModule(ModuleError),
    Io(io::Error),
    ParseFloat(ParseFloatError),
    UnknownChar(char),
    Undefined(&'static str),
    Unexpected(&'static str),
    WrongArgumentCount,
    FunctionRedef,
    FunctionRedefWithDifferentParams,
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match *self {
            Io(ref error) => error.fmt(formatter),
            ParseFloat(ref error) => error.fmt(formatter),
            UnknownChar(char) => write!(formatter, "unknown char `{}`", char),
            Undefined(msg) => write!(formatter, "undefined {}", msg),
            Unexpected(msg) => write!(formatter, "unexpected {}", msg),
            WrongArgumentCount => write!(formatter, "wrong argument count"),
            FunctionRedef => write!(formatter, "redefinition of function"),
            FunctionRedefWithDifferentParams => write!(
                formatter,
                "redefinition of function with different number of parameters"
            ),
            CraneliftModule(ref error) => error.fmt(formatter),
            CraneliftCodegen(ref error) => error.fmt(formatter),
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