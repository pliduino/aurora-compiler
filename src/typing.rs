use std::fmt::Display;

use cranelift_codegen::ir::{types, Type};

// pub fn get_const_str_from_string(str: String) -> &'static str {
//     match str.as_str() {
//         VOID => VOID,

//         BOOL => BOOL,

//         I8 => I8,
//         I16 => I16,
//         I32 => I32,
//         I64 => I64,

//         F32 => F32,
//         F64 => F64,

//         _ => unimplemented!(),
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuroraType {
    Any,
    Void,
    Bool,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

impl AuroraType {
    pub fn get_type(&self) -> Option<Type> {
        match self {
            AuroraType::Void | AuroraType::Any => None,
            AuroraType::Bool => Some(types::I8),
            AuroraType::I8 => Some(types::I8),
            AuroraType::I16 => Some(types::I16),
            AuroraType::I32 => Some(types::I32),
            AuroraType::I64 => Some(types::I64),
            AuroraType::F32 => Some(types::F32),
            AuroraType::F64 => Some(types::F64),
        }
    }

    pub fn from_string(str: &String) -> Self {
        match str.as_str() {
            "void" => AuroraType::Void,
            "bool" => AuroraType::Bool,
            "i8" => AuroraType::I8,
            "i16" => AuroraType::I16,
            "i32" => AuroraType::I32,
            "i64" => AuroraType::I64,
            "f32" => AuroraType::F32,
            "f64" => AuroraType::F64,
            _ => unimplemented!(),
        }
    }
}

impl Display for AuroraType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuroraType::Any => write!(f, "any"),
            AuroraType::Void => write!(f, "void"),
            AuroraType::Bool => write!(f, "bool"),
            AuroraType::I8 => write!(f, "i8"),
            AuroraType::I16 => write!(f, "i16"),
            AuroraType::I32 => write!(f, "i32"),
            AuroraType::I64 => write!(f, "i64"),
            AuroraType::F32 => write!(f, "f32"),
            AuroraType::F64 => write!(f, "f64"),
        }
    }
}
