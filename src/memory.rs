use std::ffi::CString;
use std::path::Path;
use tokio::task;

// ==========================================
// C++ FFI BINDINGS (llama.h)

/// Defines the C-compatible struct for quantization parameters.
/// Must match exactly the memory layout of 'llama_model_quantize_params' in llama.h
#[repr(C)]
pub struct LlamaModelQuantizeParams {
    pub nthread: i32,
    pub ftype: i32, // The target quantization type (e.g., 15 for Q4_K_M)
    pub padding: [u64; 32],
}

// ==========================================
// JUTSU HYPERQUAD IMPLEMENTATION

/// Parses the string target into the official llama_ftype integer
fn parse_quantization_type(compression: &str) -> i32 {
    match compression.to_uppercase().as_str() {
        "Q4_0" => 2,
        "Q4_1" => 3,
        "Q5_0" => 8,
        "Q5_1" => 9,
        "Q8_0" => 7,
        "Q4_K_M" => 15, 
        "Q5_K_M" => 17,
        "Q6_K" => 18,
        _ => 15, // Default a Q4_K_M
    }
}

/// Swarm Quantization: Physically compresses a GGUF model via C++ FFI
/// Returns the file path to the newly generated compressed model.

// ==========================================
// ROUTE A: HOT LOADING (DEFAULT / LIBLOADING)
// ==========================================
#[cfg(not(feature = "static_ffi"))]
pub async fn execute_hyper_quad(input_path: String, output_path: String, compression: String) -> Result<String, String> {
    use libloading::{Library, Symbol};

    println!("\n[Hardware] >>> HyperQuad Quantization Protocol Initiated (Dynamic FFI) <<<");
    println!("[Hardware] Target Model : {}", input_path);
    println!("[Hardware] Destination  : {}", output_path);
    println!("[Hardware] Compression  : {}", compression.to_uppercase());

    if !Path::new(&input_path).exists() {
        return Err(format!("Source model not found: {}", input_path));
    }

    let result = task::spawn_blocking(move || {
        let c_inp = CString::new(input_path.clone()).expect("CString::new failed");
        let c_out = CString::new(output_path.clone()).expect("CString::new failed");
        
        unsafe {
            // Dynamic loading depending on the Operating System
            let lib_name = if cfg!(windows) { "llama.dll" } else if cfg!(target_os = "macos") { "libllama.dylib" } else { "libllama.so" };
            
            let lib = match Library::new(lib_name) {
                Ok(l) => l,
                Err(e) => return Err(format!("Failed to dynamically load {}: {}", lib_name, e)),
            };

            let get_default_params: Symbol<unsafe extern "C" fn() -> LlamaModelQuantizeParams> = 
                match lib.get(b"llama_model_quantize_default_params\0") {
                    Ok(sym) => sym,
                    Err(e) => return Err(format!("Function llama_model_quantize_default_params not found: {}", e)),
                };

            let llama_model_quantize: Symbol<unsafe extern "C" fn(*const std::ffi::c_char, *const std::ffi::c_char, *const LlamaModelQuantizeParams) -> i32> = 
                match lib.get(b"llama_model_quantize\0") {
                    Ok(sym) => sym,
                    Err(e) => return Err(format!("Function llama_model_quantize not found: {}", e)),
                };

            let mut params = get_default_params();
            params.ftype = parse_quantization_type(&compression);
            params.nthread = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4); 

            println!("[Hardware] Engaging Dynamic C++ Tensor compression ({} cores). This may take a while...", params.nthread);

            let status = llama_model_quantize(c_inp.as_ptr(), c_out.as_ptr(), &params);

            if status == 0 { Ok(output_path) } else { Err(format!("llama_model_quantize failed with status code: {}", status)) }
        }
    }).await.map_err(|e| format!("Tokio blocking task panicked: {:?}", e))?;

    match result {
        Ok(out) => {
            println!("[Hardware] Tensor compression complete. Optimized vessel ready at: {}\n", out);
            Ok(out)
        },
        Err(e) => Err(e),
    }
}

// ==========================================
// ROUTE B: STATIC LINK (EXPERIMENTAL / EXTERNAL "C")
// ==========================================

// We link to the llama C library to access the native quantization functions
#[cfg(feature = "static_ffi")]
extern "C" {
    fn llama_model_quantize_default_params() -> LlamaModelQuantizeParams;
    
    fn llama_model_quantize(
        fname_inp: *const std::ffi::c_char,
        fname_out: *const std::ffi::c_char,
        params: *const LlamaModelQuantizeParams,
    ) -> i32; // Returns 0 on success
}

#[cfg(feature = "static_ffi")]
pub async fn execute_hyper_quad(input_path: String, output_path: String, compression: String) -> Result<String, String> {
    println!("\n[Hardware] >>> HyperQuad Quantization Protocol Initiated (Static FFI) <<<");
    println!("[Hardware] Target Model : {}", input_path);
    println!("[Hardware] Destination  : {}", output_path);
    println!("[Hardware] Compression  : {}", compression.to_uppercase());

    if !Path::new(&input_path).exists() {
        return Err(format!("Source model not found: {}", input_path));
    }

    let result = task::spawn_blocking(move || {
        let c_inp = CString::new(input_path.clone()).expect("CString::new failed");
        let c_out = CString::new(output_path.clone()).expect("CString::new failed");
        
        unsafe {
            let mut params = llama_model_quantize_default_params();
            params.ftype = parse_quantization_type(&compression);
            params.nthread = std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4); 

            println!("[Hardware] Engaging Static C++ Tensor compression ({} cores). This may take a while...", params.nthread);

            let status = llama_model_quantize(c_inp.as_ptr(), c_out.as_ptr(), &params);

            if status == 0 { Ok(output_path) } else { Err(format!("llama_model_quantize failed with status code: {}", status)) }
        }
    }).await.map_err(|e| format!("Tokio blocking task panicked: {:?}", e))?;

    match result {
        Ok(out) => {
            println!("[Hardware] Tensor compression complete. Optimized vessel ready at: {}\n", out);
            Ok(out)
        },
        Err(e) => Err(e),
    }
}