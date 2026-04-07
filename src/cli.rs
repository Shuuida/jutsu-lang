use std::env;
use std::fs;
use std::panic;
use std::path::Path;
use std::collections::HashMap;
use std::io::{self, Write};
use crate::parser::Parser;
use crate::evaluator::{Evaluator, JutsuValue}; 

pub async fn execute() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        start_repl().await;
        return;
    }

    let command = &args[1];

    match command.as_str() {
        "run" => {
            if args.len() < 3 {
                println!("Error: You must specify a file. Example: tgn run api/main.ju");
                return;
            }
            let file_path = &args[2];
            
            let mut env_vars = HashMap::new();
            let mut i = 3;
            while i < args.len() {
                if args[i].starts_with("--") && i + 1 < args.len() {
                    let key = args[i][2..].to_uppercase();
                    let val_str = &args[i + 1];
                    let val = if let Ok(n) = val_str.parse::<f32>() { JutsuValue::Number(n) } else { JutsuValue::Text(val_str.to_string()) };
                    env_vars.insert(key, val);
                    i += 2;
                } else { i += 1; }
            }
            execute_file(file_path, env_vars).await;
        }
        "init" => {
            let mut target_dir = ".";
            let mut project_name = "jutsu_api";
            let mut i = 2;
            while i < args.len() {
                if args[i] == "--folder" && i + 1 < args.len() {
                    target_dir = &args[i + 1];
                    project_name = target_dir;
                    break;
                }
                i += 1;
            }
            init_project(target_dir, project_name);
        }
        "absorb" => {
            if args.len() < 3 {
                println!("Error: You must specify a URL. Example: tgn absorb https://.../model.gguf");
                return;
            }
            let model = &args[2];
            crate::tgn_pm::absorb_model(model);
        }
        "help" => {
            show_help();
        }
        _ => {
            println!("Error: Unknown command '{}'", command);
            show_help();
        }
    }
}

async fn start_repl() {
    println!("Tengen Interactive Console (REPL) - Jutsu v0.1.0-alpha");
    println!("Type 'exit' or 'quit' to close.\n");

    let mut runtime = Evaluator::new();

    loop {
        print!("jutsu> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();
        if input == "exit" || input == "quit" { break; }
        if input.is_empty() { continue; }

        // Silence standard Rust memory dump
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(|_| {})); 

        // Catch compiler panic in mid-air
        let parse_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let mut parser = Parser::new(input);
            parser.parse()
        }));

        panic::set_hook(default_hook); // Restore normal behavior

        match parse_result {
            Ok(ast_program) => {
                runtime.evaluate(&ast_program).await;
            }
            Err(err) => {
                let msg = if let Some(s) = err.downcast_ref::<String>() { s.clone() } 
                          else if let Some(s) = err.downcast_ref::<&str>() { s.to_string() } 
                          else { "Syntax Error".to_string() };
                println!("\x1b[31m{}\x1b[0m", msg);
            }
        }
    }
}

fn init_project(target_dir: &str, project_name: &str) {
    println!(">>> Tengen: Scaffolding new Jutsu API project '{}'... <<<", project_name);

    // Format the base path depending on whether a custom folder was provided
    let base_path = if target_dir == "." { "".to_string() } else { format!("{}/", target_dir) };
    
    let api_dir = format!("{}api", base_path);

    if let Err(e) = fs::create_dir_all(&api_dir) {
        println!("Fatal Error: Could not create directories. Details: {}", e);
        return;
    }

    // Generate tgn.toml (Configuration File)
    let toml_content = format!(r#"[package]
name = "{}"
version = "0.1.0"
author = "Jutsu Developer"
description = "A native AI inference API powered by Jutsu Language"

[models]
# Define the AI models required for this project
# Tengen will download them into the global cache using 'tgn absorb'
# qwen = "qwen1_5-0_5b-chat-q4_0.gguf"
"#, project_name);

    let toml_path = format!("{}tgn.toml", base_path);
    if let Err(e) = fs::write(&toml_path, toml_content) {
        println!("Error writing tgn.toml: {}", e);
    }

    let gitignore_content = r#"# Tengen cache and AI models
.tgn_cache/
*.gguf
*.log
"#;
    let gitignore_path = format!("{}.gitignore", base_path);
    if let Err(e) = fs::write(&gitignore_path, gitignore_content) {
        println!("Error writing .gitignore: {}", e);
    }

    // Generate api/main.ju (Sample API Entry Point)
    let main_ju_content = r#"// Tengen API Entry Point
print("====================================")
print("  Jutsu Engine: API Initialized     ")
print("====================================")

// Example of a 2D Tensor
let weights = tensor([
    [0.5, 0.1],
    [-0.2, 0.9]
], grad=true)

print("Base Matrix:")
print(weights)

// Dynamic port injection from Tengen CLI (defaults to 8080 if not provided)
let server_port = PORT

// To deploy a concurrent HTTP AI Server, uncomment the veil block:
// veil(port = server_port) {
//     let req = recv()
//     if req != "EMPTY_PING" {
//         reply("Hello from Jutsu API!")
//     }
// }
"#;
    let main_ju_path = format!("{}/main.ju", api_dir);
    if let Err(e) = fs::write(&main_ju_path, main_ju_content) {
        println!("Error writing main.ju: {}", e);
    }

    println!("\nProject '{}' created successfully!", project_name);
    println!("Next steps:");
    if target_dir != "." {
        println!("  cd {}", target_dir);
    }
    println!("  tgn run api/main.ju --port 3000");
}

async fn execute_file(path: &str, env_vars: HashMap<String, JutsuValue>) {
    if !Path::new(path).exists() {
        println!("Error: The file '{}' does not exist.", path);
        return;
    }

    println!(">>> Tengen Engine: Deploying '{}' <<<\n", path);
    
    // Read the source code directly from the hard drive
    match fs::read_to_string(path) {
        Ok(source_code) => {
            // Silence standard Rust memory dump (The same logic as the REPL)
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {})); 

            // Catch compiler panic (Syntax Errors) in mid-air (The same logic as the REPL)
            let parse_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut parser = Parser::new(&source_code);
                parser.parse()
            }));

            std::panic::set_hook(default_hook); // Restore normal behavior

            match parse_result {
                Ok(ast_program) => {
                    let mut runtime = Evaluator::new();
                    
                    // GLOBAL VARIABLE INJECTION FROM CLI
                    for (k, v) in env_vars {
                        runtime.set_global_variable(k, v);
                    }

                    // Async execution
                    runtime.evaluate(&ast_program).await;
                }
                Err(err) => {
                    // Extract and print the panic! message cleanly
                    let msg = if let Some(s) = err.downcast_ref::<String>() { s.clone() } 
                              else if let Some(s) = err.downcast_ref::<&str>() { s.to_string() } 
                              else { "Syntax Error".to_string() };
                    
                    println!("\x1b[31m{}\x1b[0m", msg); // Print error in red ANSI
                    println!("\x1b[33m[Tengen] Execution aborted safely due to syntax errors.\x1b[0m");
                }
            }
        }
        Err(e) => {
            println!("Fatal Error: Could not read file '{}'. Details: {}", path, e);
        }
    }
}

fn show_help() {
    println!("Tengen (tgn) - Jutsu Language Package Manager & Compiler");
    println!("Usage:");
    println!("  tgn run <file.ju> [flags]            Executes a Jutsu script (e.g., --port 3000)");
    println!("  tgn init                             Initializes project in current directory");
    println!("  tgn init [--folder <name>]             Creates a new isolated project folder");
    println!("  tgn absorb <url>                   Downloads a GGUF model to the global cache");
}