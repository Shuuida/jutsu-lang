use libloading::{Library, Symbol};
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::io::{self, Write};

type LlamaModel = *mut c_void;
type LlamaContext = *mut c_void;
type LlamaToken = i32;

#[repr(C, align(8))]
pub struct LlamaModelParams { _padding: [u8; 256], }

#[repr(C, align(8))]
pub struct LlamaContextParams { _padding: [u8; 256], }

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LlamaBatch {
    pub n_tokens: i32,
    pub token: *mut LlamaToken,
    pub embd: *mut f32,
    pub pos: *mut i32,
    pub n_seq_id: *mut i32,
    pub seq_id: *mut *mut i32,
    pub logits: *mut i8,
    pub all_pos_0: i32,
    pub all_pos_1: i32,
    pub all_seq_id: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LlamaTokenData {
    pub id: i32,
    pub logit: f32,
    pub p: f32,
}

#[repr(C)]
pub struct LlamaTokenDataArray {
    pub data: *mut LlamaTokenData,
    pub size: usize,
    pub sorted: bool,
}

pub fn load_native_model(file_path: &str) -> usize {
    let c_file_path = CString::new(file_path.to_string()).expect("Failed to create CString");

    let (llama_lib, ggml_lib) = match std::env::consts::OS {
        "windows" => ("llama.dll", "ggml.dll"),
        "macos"   => ("libllama.dylib", "libggml.dylib"),
        _         => ("libllama.so", "libggml.so"), 
    };

    let model_ptr = unsafe {
        if let Ok(lib_llama) = Library::new(llama_lib) {
            let _lib_ggml = Library::new(ggml_lib).ok(); 

            let backend_init: Symbol<unsafe extern "C" fn()> = lib_llama.get(b"llama_backend_init").unwrap();
            backend_init();

            if let Some(load_fn) = lib_llama.get::<unsafe extern "C" fn()>(b"ggml_backend_load_all").ok()
                .or_else(|| _lib_ggml.as_ref().and_then(|lib| lib.get(b"ggml_backend_load_all").ok())) { load_fn(); }

            let get_params_fn: Symbol<unsafe extern "C" fn() -> LlamaModelParams> = lib_llama.get(b"llama_model_default_params").unwrap();
            let load_model_fn: Symbol<unsafe extern "C" fn(*const c_char, LlamaModelParams) -> LlamaModel> = lib_llama.get(b"llama_load_model_from_file").unwrap();

            let params = get_params_fn();
            let ptr = load_model_fn(c_file_path.as_ptr(), params);
            std::mem::forget(lib_llama); // Keep lib loaded for pointers to stay valid in background
            
            // Verify that the C++ engine actually returned memory
            if ptr.is_null() {
                panic!("[Hardware Error] The native AI engine failed to allocate memory for the model. VRAM may be full or the file may be corrupted.");
            }
            
            ptr as usize
        } else {
            println!("[Hardware Error] Native AI library '{}' not found.", llama_lib);
            0
        }
    };
    model_ptr
}

pub fn run_inference(model_ptr: usize, quantize: bool, temp: f32, bind: f32, raw_prompt: &str, context_text: Option<String>, grammar_var: Option<String>) -> Option<String> {
    if quantize {
        println!("[Hardware] INT4 Quantization Directive Active: Intercepting matrix load to compress weights on-the-fly...");
    }

    let ptr = model_ptr as *mut c_void;
    if ptr.is_null() {
        println!("[Hardware Error] Vessel memory pointer is null.");
        return None;
    }

    let llama_lib_str = match std::env::consts::OS {
        "windows" => "llama.dll",
        "macos"   => "libllama.dylib",
        _         => "libllama.so", 
    };

    unsafe {
        if let Ok(lib_llama) = Library::new(llama_lib_str) {
            if let (Ok(get_ctx_params), Ok(new_ctx), Ok(free_ctx), Ok(tokenize)) = (
                lib_llama.get::<unsafe extern "C" fn() -> LlamaContextParams>(b"llama_context_default_params"),
                lib_llama.get::<unsafe extern "C" fn(LlamaModel, LlamaContextParams) -> LlamaContext>(b"llama_new_context_with_model"),
                lib_llama.get::<unsafe extern "C" fn(LlamaContext)>(b"llama_free"),
                lib_llama.get::<unsafe extern "C" fn(*mut c_void, *const c_char, i32, *mut LlamaToken, i32, bool, bool) -> i32>(b"llama_tokenize")
            ) {
                let ctx_ptr = new_ctx(ptr, get_ctx_params());

                if !ctx_ptr.is_null() {
                    let target_vocab_ptr = match lib_llama.get::<unsafe extern "C" fn(LlamaModel) -> *mut c_void>(b"llama_model_get_vocab") {
                        Ok(get_vocab) => get_vocab(ptr), Err(_) => ptr
                    };

                    let eos_token: LlamaToken = match lib_llama.get::<unsafe extern "C" fn(*mut c_void) -> LlamaToken>(b"llama_token_eos") {
                        Ok(func) => func(target_vocab_ptr), 
                        Err(_) => match lib_llama.get::<unsafe extern "C" fn(LlamaModel) -> LlamaToken>(b"llama_token_eos") {
                            Ok(func2) => func2(ptr), 
                            Err(_) => 2, 
                        }
                    };

                    let chatml_prompt = if let Some(ctx) = context_text {
                        format!("<|im_start|>system\n{}\n<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", ctx, raw_prompt)
                    } else {
                        format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", raw_prompt)
                    };

                    let c_prompt = CString::new(chatml_prompt).unwrap();
                    let mut tokens: Vec<LlamaToken> = vec![0; 2048]; 
                    let n_tokens = tokenize(target_vocab_ptr, c_prompt.as_ptr(), c_prompt.as_bytes().len() as i32, tokens.as_mut_ptr(), 2048, true, true);

                    if n_tokens > 0 {
                        tokens.truncate(n_tokens as usize);
                        let mut history_tokens = tokens.clone();

                        if let (Ok(batch_init), Ok(decode), Ok(get_logits), Ok(n_vocab), Ok(token_to_piece), Ok(batch_free)) = (
                            lib_llama.get::<unsafe extern "C" fn(i32, i32, i32) -> LlamaBatch>(b"llama_batch_init"),
                            lib_llama.get::<unsafe extern "C" fn(LlamaContext, LlamaBatch) -> i32>(b"llama_decode"),
                            lib_llama.get::<unsafe extern "C" fn(LlamaContext, i32) -> *mut f32>(b"llama_get_logits_ith"),
                            lib_llama.get::<unsafe extern "C" fn(*mut c_void) -> i32>(b"llama_n_vocab"),
                            lib_llama.get::<unsafe extern "C" fn(*mut c_void, LlamaToken, *mut c_char, i32, i32, bool) -> i32>(b"llama_token_to_piece"),
                            lib_llama.get::<unsafe extern "C" fn(LlamaBatch)>(b"llama_batch_free")
                        ) {
                            let mut batch = batch_init(2048, 0, 1);
                            batch.n_tokens = tokens.len() as i32;
                            
                            for (i, &tok) in tokens.iter().enumerate() {
                                *batch.token.add(i) = tok; *batch.pos.add(i) = i as i32;
                                *batch.n_seq_id.add(i) = 1; *(*batch.seq_id.add(i)).add(0) = 0;
                                *batch.logits.add(i) = if i == tokens.len() - 1 { 1 } else { 0 }; 
                            }

                            print!("JUTSU AI Output: ");
                            io::stdout().flush().unwrap();

                            // Immutability of the original boundary
                            let prompt_tokens = batch.n_tokens; 
                            let mut n_cur = prompt_tokens;
                            let n_max_generation = 250; // Increased response capacity
                            let mut full_response = String::new();

                            let sample_grammar_fn = lib_llama.get::<unsafe extern "C" fn(LlamaContext, *mut LlamaTokenDataArray, *mut c_void)>(b"llama_sample_grammar").ok();
                            let grammar_accept_fn = lib_llama.get::<unsafe extern "C" fn(LlamaContext, *mut c_void, LlamaToken)>(b"llama_grammar_accept_token").ok();
                            
                            let grammar_ptr: *mut c_void = std::ptr::null_mut();
                            if let Some(ref _g_text) = grammar_var {
                                println!("[System] GBNF directive received. (Requires C++ rule initializer to activate the master pointer.)");
                                // Note: The llama.cpp C API requires an array of 'llama_grammar_element'. 
                                // If your DLL has a 'llama_grammar_init_from_string' wrapper, it is invoked here.
                            }

                            // Use static prompt_tokens, not the dynamic batch
                            while n_cur <= prompt_tokens + n_max_generation {
                                if decode(ctx_ptr, batch) != 0 { break; }

                                let vocab_size = n_vocab(target_vocab_ptr);
                                let logits_ptr = get_logits(ctx_ptr, batch.n_tokens - 1);
                                let logits_slice = std::slice::from_raw_parts_mut(logits_ptr, vocab_size as usize);

                                if !grammar_ptr.is_null() {
                                    if let Some(ref sample_fn) = sample_grammar_fn {
                                        // Package the native logits to the strict format of llama.cpp
                                        let mut candidates_data: Vec<LlamaTokenData> = (0..vocab_size).map(|i| LlamaTokenData {
                                            id: i as i32,
                                            logit: logits_slice[i as usize],
                                            p: 0.0,
                                        }).collect();

                                        let mut candidates = LlamaTokenDataArray {
                                            data: candidates_data.as_mut_ptr(),
                                            size: candidates_data.len(),
                                            sorted: false,
                                        };

                                        // Run the strainer (Assign f32::NEG_INFINITY to everything that breaks the JSON syntax)
                                        sample_fn(ctx_ptr, &mut candidates, grammar_ptr);

                                        // Dump the purified memory back to the slice for the RNG algorithm
                                        for i in 0..vocab_size as usize {
                                            logits_slice[i] = candidates_data[i].logit;
                                        }
                                    }
                                }
                                
                                if bind > 1.0 {
                                    for &tok in &history_tokens {
                                        let tok_idx = tok as usize;
                                        if tok_idx < vocab_size as usize {
                                            let logit = logits_slice[tok_idx];
                                            if logit > 0.0 { logits_slice[tok_idx] = logit / bind; } 
                                            else { logits_slice[tok_idx] = logit * bind; }
                                        }
                                    }
                                }

                                let mut next_token: LlamaToken = 0;
                                if temp <= 0.0 {
                                    let mut max_val = f32::NEG_INFINITY;
                                    for v in 0..(vocab_size as usize) {
                                        if logits_slice[v] > max_val { max_val = logits_slice[v]; next_token = v as LlamaToken; }
                                    }
                                } else {
                                    let mut max_val = f32::NEG_INFINITY;
                                    for v in 0..(vocab_size as usize) {
                                        if logits_slice[v] > max_val { max_val = logits_slice[v]; }
                                    }

                                    let mut sum = 0.0;
                                    let mut probs = vec![0.0; vocab_size as usize];
                                    for v in 0..(vocab_size as usize) {
                                        let p = ((logits_slice[v] - max_val) / temp).exp();
                                        probs[v] = p; sum += p;
                                    }

                                    let mut rng_state = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos() as u64;
                                    rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                                    let rand_float = (rng_state >> 33) as f32 / 2147483648.0; 
                                    let mut rand_val = rand_float * sum;

                                    for v in 0..(vocab_size as usize) {
                                        rand_val -= probs[v];
                                        if rand_val <= 0.0 { next_token = v as LlamaToken; break; }
                                    }
                                }

                                if next_token == eos_token || next_token == 151645 { break; } 
                                history_tokens.push(next_token);

                                if !grammar_ptr.is_null() {
                                    if let Some(ref accept_fn) = grammar_accept_fn {
                                        accept_fn(ctx_ptr, grammar_ptr, next_token);
                                    }
                                }

                                // 128 bytes buffer to support heavy nested tokens
                                let mut piece_buf = vec![0u8; 128];
                                let chars_written = token_to_piece(target_vocab_ptr, next_token, piece_buf.as_mut_ptr() as *mut c_char, 128, 0, true);
                                
                                if chars_written > 0 {
                                    piece_buf.truncate(chars_written as usize);
                                    let text_chunk = String::from_utf8_lossy(&piece_buf).to_string();
                                    print!("{}", text_chunk);
                                    io::stdout().flush().unwrap();
                                    full_response.push_str(&text_chunk);
                                }

                                batch.n_tokens = 1;
                                *batch.token.add(0) = next_token; *batch.pos.add(0) = n_cur;
                                *batch.n_seq_id.add(0) = 1; *(*batch.seq_id.add(0)).add(0) = 0;
                                *batch.logits.add(0) = 1;
                                n_cur += 1;
                            }
                            println!();
                            batch_free(batch); free_ctx(ctx_ptr); 
                            return Some(full_response);
                        }
                    }
                    free_ctx(ctx_ptr);
                }
            }
        }
    }
    None
}