#![allow(dead_code)]

use std::os::raw::{c_char, c_void};
use crate::inference::{LlamaToken, LlamaTokenDataArray};

// =====================================================================
// FFI MODERN FIRMS: THE SAMPLER API (LATE 2024 / 2025)
// =====================================================================
pub type LlamaSamplerInitGrammarFn = unsafe extern "C" fn(
    model: *mut c_void,
    grammar_str: *const c_char,
    grammar_root: *const c_char,
) -> *mut c_void;

pub type LlamaSamplerApplyFn = unsafe extern "C" fn(
    smpl: *mut c_void,
    cur_p: *mut LlamaTokenDataArray,
);

pub type LlamaSamplerAcceptFn = unsafe extern "C" fn(
    smpl: *mut c_void,
    token: LlamaToken,
);

pub type LlamaSamplerFreeFn = unsafe extern "C" fn(
    smpl: *mut c_void,
);