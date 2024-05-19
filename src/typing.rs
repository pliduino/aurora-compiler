use cranelift_codegen::ir::{types, Type};

pub fn get_type_from_str(str: &str) -> Option<Type> {
    match str {
        I8 => Some(types::I8),
        I16 => Some(types::I16),
        I32 => Some(types::I32),
        I64 => Some(types::I64),
        F32 => Some(types::F32),
        F64 => Some(types::F64),
        VOID => None,
        _ => None, // TODO: Trigger error
    }
}

pub fn get_const_str_from_string(str: String) -> &'static str {
    match str.as_str() {
        VOID => VOID,

        I8 => I8,
        I16 => I16,
        I32 => I32,
        I64 => I64,

        F32 => F32,
        F64 => F64,

        _ => unimplemented!(),
    }
}

pub const ANY: &str = "";
pub const VOID: &str = "void";

pub const I8: &str = "i8";
pub const I16: &str = "i16";
pub const I32: &str = "i32";
pub const I64: &str = "i64";

pub const F32: &str = "f32";
pub const F64: &str = "f64";
