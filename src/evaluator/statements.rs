use super::{Evaluator, JutsuValue, ExecResult, VesselConfig};
use crate::ast::{Statement, Expression};
/// use std::collections::HashMap;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex as AsyncMutex;
use std::sync::Arc;
use async_recursion::async_recursion;
use std::path::Path;
use crate::evaluator::{serde_to_jutsu, jutsu_to_serde};

impl Evaluator {
    #[async_recursion]
    pub async fn execute_statement(&mut self, statement: &Statement) -> ExecResult {
        let stmt_clone = statement.clone(); 
        match stmt_clone {
            Statement::AssignmentStatement { name, value } => {
                let evaluated_val = self.evaluate_expression(&value).await;
                let mut found = false;
                for env in self.env_stack.iter_mut().rev() {
                    if let Some(existing_val) = env.get_mut(&name) {
                        if let JutsuValue::Shared(shared_domain) = existing_val {
                            let mut locked_val = shared_domain.0.lock().unwrap();
                            *locked_val = Box::new(evaluated_val.clone());
                        } else {
                            *existing_val = evaluated_val.clone();
                        }
                        found = true; break;
                    }
                }
                if !found { panic!("[Runtime Error] Attempted to assign to undefined variable '{}'. Use 'let' to declare it.", name); }
                ExecResult::Normal
            }

            Statement::LetStatement { name, value } => {
                let evaluated_val = self.evaluate_expression(&value).await; 
                if let Some(env) = self.env_stack.last_mut() { env.insert(name, evaluated_val); }
                ExecResult::Normal
            }

            Statement::ReplyStatement { value } => {
                let val = self.evaluate_expression(&value).await;
                if let Some(stream_arc) = &self.tcp_stream {
                    let mut stream = stream_arc.lock().await;
                    let response_body = val.to_string();
                    let http_response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(), response_body
                    );
                    let _ = stream.write_all(http_response.as_bytes()).await;
                    let _ = stream.flush().await;
                    println!("[Server] Reply OK send.");
                } else { panic!("[Network Error] 'reply' can only be used inside a veil."); }
                ExecResult::Normal
            }

            Statement::ExposeToolStatement { name: _, description: _, function_name: _ } => {
                // In the normal evaluator, we do nothing.
                // The tools are extracted directly by the McpServerBlock.
                ExecResult::Normal
            }

            Statement::McpServerBlock { port, body } => {
                let port_val = self.evaluate_expression(&port).await;
                let port_num = match port_val {
                    JutsuValue::Number(n) => n as u16,
                    _ => 8080, 
                };

                // We dynamically extract the tools from the block
                let mut mcp_tools = Vec::new();
                for stmt in &body {
                    if let Statement::ExposeToolStatement { name, description, function_name } = stmt {
                        mcp_tools.push((name.clone(), description.clone(), function_name.clone()));
                    }
                }

                println!("[MCP Engine] Lifting native Model Context Protocol Server on 0.0.0.0:{}...", port_num);
                println!("[MCP Engine] Registered Tools: {}", mcp_tools.len());
                let listener = TcpListener::bind(format!("0.0.0.0:{}", port_num)).await.expect("Failed to bind MCP port.");
                println!("[MCP Engine] MCP Server ACTIVE. Ready for JSON-RPC connections.");

                // Asynchronous infinite loop to accept MCP clients
                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let mut thread_evaluator = self.clone();
                            let thread_tools = mcp_tools.clone();
                            
                            // Spawn: A light thread from Tokyo for every MCP connection
                            tokio::spawn(async move {
                                let mut reader = BufReader::new(stream);
                                let mut line = String::new();

                                // We read packet by packet (JSON-Lines)
                                loop {
                                    line.clear();
                                    match reader.read_line(&mut line).await {
                                        Ok(0) => break, // The client closed the connection
                                        Ok(_) => {
                                            if line.trim().is_empty() { continue; }
                                            
                                            // We parse the JSON-RPC 2.0
                                            if let Ok(json_req) = serde_json::from_str::<serde_json::Value>(&line) {
                                                let response = thread_evaluator.handle_mcp_request(json_req, &thread_tools).await;
                                                
                                                // We respond to the MCP client
                                                let mut res_str = serde_json::to_string(&response).unwrap();
                                                res_str.push('\n'); // Vital line break for the protocol
                                                let _ = reader.get_mut().write_all(res_str.as_bytes()).await;
                                            } else {
                                                println!("[MCP Error] Received invalid JSON payload.");
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }
                        Err(e) => { println!("[MCP Network Error] Failed Connection: {}", e); }
                    }
                }
            }

            Statement::VeilBlock { name: _, port, body } => {
                let port_val = self.evaluate_expression(&port).await;
                let port_num = match port_val {
                    JutsuValue::Number(n) => n as u16,
                    JutsuValue::Text(s) => s.parse::<u16>().unwrap_or(8080),
                    _ => 8080, 
                };

                println!("[Network] Lifting Veil (Async TCP Listener) on 0.0.0.0:{}...", port_num);
                let listener = TcpListener::bind(format!("0.0.0.0:{}", port_num)).await.expect("Failed to bind port.");
                println!("[Network] Veil ACTIVE. Listening to concurrent async requests...");

                loop {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let mut thread_evaluator = self.clone();
                            thread_evaluator.tcp_stream = Some(Arc::new(AsyncMutex::new(stream)));
                            let thread_body = body.clone();

                            tokio::spawn(async move {
                                thread_evaluator.execute_block(&thread_body).await;
                            });
                        }
                        Err(e) => { println!("[Network Error] Failed Connection: {}", e); }
                    }
                }
            }

            Statement::VesselDeclaration { name, file_path, tier: _, temp, bind, quantize } => {
                let resolved_path = crate::tgn_pm::resolve_model_path(&file_path);
                let model_ptr = crate::inference::load_native_model(&resolved_path);

                if model_ptr == 0 { panic!("[Hardware Error] Failed to load Vessel '{}'.", name); }

                println!("[Hardware] Vessel '{}' loaded into RAM successfully (Shared Mode).", name);
                self.models.insert(name, VesselConfig { file_path, temp, bind, quantize, model_ptr });
                ExecResult::Normal
            }

            Statement::HexTraceBlock { body } => {
                let start_time = Instant::now();
                let block_result = self.execute_block(&body).await;
                let elapsed = start_time.elapsed();
                println!("[HexTrace] Block execution Time: {:.2} ms | Stable", elapsed.as_secs_f64() * 1000.0);
                block_result
            }

            Statement::BackwardStatement { target_var } => {
                let tensor_val = self.get_variable(&target_var);
                if let Some(JutsuValue::Tensor { data, requires_grad, parents, backward_op, backward_val, .. }) = tensor_val {
                    if requires_grad {
                        let mut current_grad: Vec<f32> = data.iter().map(|_| 1.0).collect();
                        if backward_op.as_deref() == Some("multiply") {
                            let multiplier = backward_val.unwrap_or(1.0);
                            current_grad = current_grad.iter().map(|g| g * multiplier).collect();
                        }
                        if !parents.is_empty() {
                            for parent_id in parents {
                                for env in self.env_stack.iter_mut().rev() {
                                    if let Some(JutsuValue::Tensor { grad: ref mut parent_grad, .. }) = env.get_mut(&parent_id) {
                                        *parent_grad = Some(current_grad.clone()); break;
                                    }
                                }
                            }
                        }
                    } 
                } 
                ExecResult::Normal
            }

            Statement::OptimStatement { target_var, learning_rate } => {
                let lr_val = self.evaluate_expression(&learning_rate).await;
                let lr = match lr_val { JutsuValue::Number(n) => n, _ => 0.01 };
                
                for env in self.env_stack.iter_mut().rev() {
                    if let Some(JutsuValue::Tensor { ref mut data, ref mut grad, .. }) = env.get_mut(&target_var) {
                        if let Some(g) = grad.as_ref() {
                            for i in 0..data.len() { data[i] -= lr * g[i]; }
                        }
                        *grad = None; break;
                    }
                }
                ExecResult::Normal
            }

            Statement::FunctionDeclaration { name, params, body } => {
                self.functions.insert(name, (params, body)); ExecResult::Normal
            }

            Statement::ReturnStatement { value } => {
                let val = self.evaluate_expression(&value).await; ExecResult::Return(val)
            }

            Statement::PrintStatement { value } => {
                let evaluated_val = self.evaluate_expression(&value).await; 
                let _lock = super::CONSOLE_LOCK.lock().unwrap();
                println!("{}", evaluated_val.to_string()); 
                ExecResult::Normal
            }

            Statement::ImportStatement { path } => {
                let resolved_path = Path::new(&path);
                match tokio::fs::read_to_string(&resolved_path).await {
                    Ok(content) => {
                        let mut parser = crate::parser::Parser::new(&content);
                        let ast = parser.parse();
                        Box::pin(self.evaluate(&ast)).await;
                    }
                    Err(e) => panic!("[Import Error] Failed to read module '{}': {}", path, e),
                }
                ExecResult::Normal
            }

            Statement::InferStatement { model_name, prompt_var, context_var } => {
                let expr = Expression::InferCall { model_name, prompt_var, context_var, grammar_var: None }; 
                self.evaluate_expression(&expr).await; 
                ExecResult::Normal
            }

            Statement::IfStatement { condition, consequence, alternative } => {
                if self.evaluate_condition(&condition).await { return self.execute_block(&consequence).await; } 
                else if let Some(alt_body) = alternative { return self.execute_block(&alt_body).await; }
                ExecResult::Normal
            }

            Statement::WhileStatement { condition, body } => {
                while self.evaluate_condition(&condition).await {
                    if let ExecResult::Return(val) = self.execute_block(&body).await { return ExecResult::Return(val); }
                }
                ExecResult::Normal
            }

            crate::ast::Statement::ShieldBlock { max_vram: _, body } => {
                // We requested the GPU master lock
                let local_lock_arc = self.hardware_lock.clone();
                let _hw_lock = local_lock_arc.lock().await;
                // We activate the privilege so that 'infer' does not self-block
                self.is_shielded = true;
                // Execute all the critical agent code
                // While we are here, no other worker will be able to use the GPU
                self.execute_block(&body).await;
                // This turn off privilege and release the lock
                self.is_shielded = false;
                return ExecResult::None;
            }

            crate::ast::Statement::WorkerBlock { body } => {
                let local_env_stack = self.env_stack.clone(); 
                let local_models = self.models.clone(); 
                let local_functions = self.functions.clone();
                let local_tcp_stream = self.tcp_stream.clone();
                let local_global_queue = self.global_queue.clone(); 
                let local_hardware_lock = self.hardware_lock.clone();
                let thread_body = body.clone(); 
                
                println!("[Concurrency] Shadow Worker initialized. Spawning Tokio Async Task...");
                tokio::spawn(async move {
                    let mut isolated_runtime = Evaluator::new();
                    isolated_runtime.env_stack = local_env_stack;
                    isolated_runtime.models = local_models;
                    isolated_runtime.functions = local_functions;
                    isolated_runtime.tcp_stream = local_tcp_stream;
                    isolated_runtime.global_queue = local_global_queue;
                    isolated_runtime.hardware_lock = local_hardware_lock;
                    
                    isolated_runtime.execute_block(&thread_body).await;
                    println!("[Concurrency] Shadow Worker task completed. Thread gracefully killed.");
                });
                
                return ExecResult::None;
            }

            crate::ast::Statement::HyperQuadDirective { name, model_ident, target, compression } => {
                match crate::memory::execute_hyper_quad(model_ident.clone(), target.clone(), compression.clone()).await {
                    Ok(new_model_path) => {
                        println!("[Hardware] Auto-absorbing quantized model into Vessel memory...");
                        let resolved_path = crate::tgn_pm::resolve_model_path(&new_model_path);
                        let model_ptr = crate::inference::load_native_model(&resolved_path);

                        if model_ptr == 0 { 
                            println!("[Hardware Error] Failed to auto-load quantized Vessel '{}'.", new_model_path); 
                        } else {
                            self.models.insert(name.clone(), VesselConfig { 
                                file_path: new_model_path, 
                                temp: 0.0,            
                                bind: 1.0,          
                                quantize: false,      
                                model_ptr 
                            });
                            
                            println!("[Hardware] Vessel '{}' auto-injected and ready for inference.", name);
                        }
                    },
                    Err(e) => {
                        println!("[Hardware Error] HyperQuad Failed: {}", e);
                    }
                }
                return ExecResult::None;
            }

            Statement::ExpressionStatement(expr) => {
                Box::pin(self.evaluate_expression(&expr)).await;
                ExecResult::Normal
            }
        }
    }
    #[async_recursion]
    pub async fn handle_mcp_request(&mut self, req: serde_json::Value, tools: &[(String, String, String)]) -> serde_json::Value {
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let req_id = req.get("id").unwrap_or(&serde_json::Value::Null);

        match method {
            "initialize" => {
                // The initial handshake of the MCP protocol
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "Tengen-MCP-Engine",
                            "version": "0.2.0-alpha"
                        }
                    }
                })
            }
            "tools/list" => {
                // We present Jutsu's tools to the client
                let json_tools: Vec<serde_json::Value> = tools.iter().map(|(name, desc, _)| {
                    serde_json::json!({
                        "name": name,
                        "description": desc,
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "params": {
                                    "type": "object",
                                    "description": "Dynamic parameters mapped to Jutsu Dictionary"
                                }
                            }
                        }
                    })
                }).collect();

                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "tools": json_tools
                    }
                })
            }
            "tools/call" => {
                let empty_json = serde_json::json!({});
                // Native execution of the tool
                let params = req.get("params").unwrap_or(&empty_json);
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args_json = params.get("arguments").unwrap_or(&empty_json);

                // We are looking for the Jutsu function mapped to this tool
                if let Some((_, _, func_name)) = tools.iter().find(|t| t.0 == name) {
                    if let Some((params_names, body)) = self.functions.get(func_name).cloned() {
                        
                        // Convert the JSON arguments to a native Jutsu Dictionary
                        let jutsu_args = serde_to_jutsu(args_json.clone());
                        let mut func_env = std::collections::HashMap::new();
                        
                        // Inject the JSON as the FIRST parameter of the function in Jutsu
                        if let Some(first_param) = params_names.first() {
                            func_env.insert(first_param.clone(), jutsu_args);
                        }

                        // Execute the Jutsu function in isolated memory
                        self.env_stack.push(func_env);
                        let mut return_value = JutsuValue::Null;
                        for s in body { 
                            if let ExecResult::Return(val) = self.execute_statement(&s).await { 
                                return_value = val; 
                                break; 
                            } 
                        }
                        self.env_stack.pop();

                        // We respond using the strict MCP format
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": req_id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": jutsu_to_serde(&return_value).to_string()
                                    }
                                ]
                            }
                        })
                    } else {
                        serde_json::json!({ "jsonrpc": "2.0", "id": req_id, "error": { "code": -32601, "message": "Jutsu function not found" } })
                    }
                } else {
                    serde_json::json!({ "jsonrpc": "2.0", "id": req_id, "error": { "code": -32601, "message": "Tool not exposed" } })
                }
            }
            _ => {
                // Unsupported notifications or methods
                serde_json::json!({ "jsonrpc": "2.0", "id": req_id, "error": { "code": -32601, "message": "Method not found" } })
            }
        }
    }
}