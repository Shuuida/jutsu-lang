use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

// Third-party crates for HTTP and Progress UI
use reqwest::blocking::Client;
use indicatif::{ProgressBar, ProgressStyle};

// Helper function to get the global cache directory (~/.tengen/models)
pub fn get_cache_dir() -> PathBuf {
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
        
    let mut path = PathBuf::from(home);
    path.push(".tengen");
    path.push("models");
    path
}

pub fn absorb_model(model_url: &str) {
    // Basic validation to ensure it's a URL
    if !model_url.starts_with("http") {
        println!("Fatal Error: 'absorb' requires a direct HTTP/HTTPS URL.");
        println!("Example: tgn absorb https://huggingface.co/user/repo/resolve/main/model.gguf");
        return;
    }

    // Extract the filename from the URL
    let file_name = model_url.split('/').last().unwrap_or("downloaded_model.gguf");
    let cache_dir = get_cache_dir();
    
    // Create the global directory if it doesn't exist
    if !cache_dir.exists() {
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            println!("Fatal Error: Could not create global cache directory. Details: {}", e);
            return;
        }
    }

    let mut model_path = cache_dir.clone();
    model_path.push(file_name);

    if model_path.exists() {
        println!(">>> Tengen Hub: Model '{}' is already in the global cache. <<<", file_name);
        println!("Path: {}", model_path.display());
        return;
    }

    println!(">>> Tengen Hub: Initiating secure TLS handshake and connecting to target server... <<<");

    let client = match Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent("tengen-cli/0.1.0-alpha")
        .build() 
    {
        Ok(c) => c,
        Err(e) => {
            println!("Network Error: Failed to build HTTP client. Details: {}", e);
            return;
        }
    };

    let mut response = match client.get(model_url).send() {
        Ok(res) => {
            if res.status().is_success() {
                res
            } else {
                println!("Network Error: Server returned status code: {}", res.status());
                return;
            }
        }
        Err(e) => {
            println!("Network Error: Failed to connect to URL. Details: {}", e);
            return;
        }
    };

    // Get total file size for the progress bar
    let total_size = response.content_length().unwrap_or(0);
    
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("#>-"));

    println!(">>> Absorbing '{}' into global cache... <<<", file_name);

    // Create the local file
    let mut file = match File::create(&model_path) {
        Ok(f) => f,
        Err(e) => {
            println!("File System Error: Could not create file. Details: {}", e);
            return;
        }
    };

    // Stream the download in chunks to prevent RAM overflow
    let mut buffer = [0; 8192]; // 8KB buffer
    let mut downloaded: u64 = 0;

    loop {
        match response.read(&mut buffer) {
            Ok(0) => break, // End of stream
            Ok(n) => {
                if let Err(e) = file.write_all(&buffer[..n]) {
                    println!("\nFile System Error: Failed to write chunk to disk. Details: {}", e);
                    return;
                }
                downloaded += n as u64;
                pb.set_position(downloaded);
            }
            Err(e) => {
                println!("\nNetwork Error: Stream interrupted. Details: {}", e);
                return;
            }
        }
    }

    pb.finish_with_message("Download complete!");
    println!();
    println!(">>> Tengen Hub: Model successfully secured at: {} <<<", model_path.display());
}

// Resolves the path of a model for the Jutsu runtime
pub fn resolve_model_path(file_path: &str) -> String {
    let local_path = Path::new(file_path);
    
    // Check local relative path
    if local_path.exists() {
        return file_path.to_string();
    }
    
    // Check global cache
    let mut cache_path = get_cache_dir();
    cache_path.push(file_path);
    
    if cache_path.exists() {
        return cache_path.to_string_lossy().to_string();
    }

    panic!("[File Error] The model file '{}' was not found locally or in the Tengen cache (~/.tengen/models/). Did you run 'tgn absorb'?", file_path);
}