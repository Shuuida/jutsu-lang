use super::{Evaluator, JutsuValue, serde_to_jutsu};
use crate::ast::Expression;
use std::collections::HashMap;
use std::io::{self, Write};
/// use tokio::io::AsyncReadExt;
use async_recursion::async_recursion;

impl Evaluator {
    #[async_recursion]
    pub async fn evaluate_expression(&mut self, expr: &Expression) -> JutsuValue {
        let expr_clone = expr.clone(); 
        match expr_clone {
            Expression::StringLiteral(s) => JutsuValue::Text(s),
            Expression::NumberLiteral(n) => JutsuValue::Number(n),
            Expression::BooleanLiteral(b) => JutsuValue::Boolean(b),

            Expression::Array(elements) => {
                let mut eval_elements = Vec::new();
                for e in elements { eval_elements.push(self.evaluate_expression(&e).await); }
                JutsuValue::Array(eval_elements)
            }
            Expression::Dictionary(pairs) => {
                let mut dict = HashMap::new();
                for (k, v) in pairs { dict.insert(k, self.evaluate_expression(&v).await); }
                JutsuValue::Dictionary(dict)
            }
            Expression::IndexAccess { left, index } => {
                let left_val = self.evaluate_expression(&left).await;
                let index_val = self.evaluate_expression(&index).await;
                
                match left_val {
                    JutsuValue::Array(arr) => {
                        if let JutsuValue::Number(n) = index_val {
                            let idx = n as usize;
                            if idx < arr.len() { arr[idx].clone() } else { JutsuValue::Null }
                        } else { panic!("[Runtime Error] Array index must be a number"); }
                    }
                    JutsuValue::Dictionary(dict) => {
                        if let JutsuValue::Text(k) = index_val {
                            dict.get(&k).cloned().unwrap_or(JutsuValue::Null)
                        } else { panic!("[Runtime Error] Dictionary key must be text"); }
                    }
                    _ => panic!("[Runtime Error] Index access only supported on Arrays and Dictionaries"),
                }
            }
            
            Expression::RecvCall => {
                let mut queue = self.global_queue.lock().await;
                if let Some(val) = queue.pop_front() {
                    val
                } else {
                    JutsuValue::Null
                }
            }

            Expression::TensorDeclaration { data, shape, requires_grad } => {
                let mut evaluated_data = Vec::new();
                for expr in data {
                    let val = self.evaluate_expression(&expr).await;
                    match val {
                        JutsuValue::Number(n) => evaluated_data.push(n),
                        _ => panic!("[Runtime Error] Tensors can only contain numeric values."),
                    }
                }
                JutsuValue::Tensor { data: evaluated_data, shape, requires_grad, grad: None, id: None, parents: vec![], backward_op: None, backward_val: None }
            }

            Expression::Variable(name) => {
                let mut val = self.get_variable(&name).unwrap_or_else(|| panic!("[Runtime Error] Variable '{}' is undefined.", name));
                if let JutsuValue::Tensor { ref mut id, .. } = val { *id = Some(name.clone()); }
                val
            }

            Expression::InputCall(prompt_expr) => {
                // Dynamically evaluate what's inside the parentheses (variable, string, sum...)
                // We use Box::pin because it is an asynchronous recursive function
                let evaluated_prompt = Box::pin(self.evaluate_expression(&prompt_expr)).await;
                
                print!("{}", evaluated_prompt.to_string()); 
                io::stdout().flush().unwrap();
                // Encarculate the keyboard wait in a blocking thread to not paralyze the network
                let input_text = tokio::task::spawn_blocking(|| {
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf
                }).await.expect("Input task panicked");
                
                JutsuValue::Text(input_text.trim().to_string())
            }

            Expression::ReadTextCall(filepath) => {
                match tokio::fs::read_to_string(&filepath).await { 
                    Ok(content) => JutsuValue::Text(content), 
                    Err(_) => panic!("[File Error] Could not read file '{}'", filepath) 
                }
            }

            Expression::RagCall { query_var, doc_var } => {
                if let (Some(JutsuValue::Text(query_text)), Some(JutsuValue::Text(doc_text))) = (self.get_variable(&query_var), self.get_variable(&doc_var)) {
                    JutsuValue::Text(self.native_vector_search(&query_text, &doc_text))
                } else { panic!("[Runtime Error] RAG requires Text queries and Text documents."); }
            }

            Expression::FunctionCall { name, args } => {
                if name == "json_extract" && args.len() == 2 {
                    let json_val = self.evaluate_expression(&args[0]).await;
                    let key_val = self.evaluate_expression(&args[1]).await;
                    
                    if let (JutsuValue::Text(json_str), JutsuValue::Text(key_str)) = (json_val, key_val) {
                        let search_key = format!("\"{}\":", key_str);
                        if let Some(key_idx) = json_str.find(&search_key) {
                            let rest = &json_str[key_idx + search_key.len()..];
                            let value_str = rest.trim_start().split(|c| c == ',' || c == '}').next().unwrap_or("").trim();
                            let clean_val = value_str.trim_matches('"');
                            return JutsuValue::Text(clean_val.to_string());
                        }
                        return JutsuValue::Null;
                    }
                }

                if name == "parse_json" && args.len() == 1 {
                    let arg_val = self.evaluate_expression(&args[0]).await;
                    if let JutsuValue::Text(json_str) = arg_val {
                        let clean_str = json_str.trim()
                            .trim_start_matches("```json")
                            .trim_start_matches("```")
                            .trim_end_matches("```")
                            .trim();
                        
                        match serde_json::from_str::<serde_json::Value>(clean_str) {
                            Ok(parsed) => return serde_to_jutsu(parsed),
                            Err(_) => panic!("[Runtime Error] The model returned invalid or corrupted JSON: {}", clean_str),
                        }
                    } else {
                        panic!("[Runtime Error] parse_json requires a Text argument.");
                    }
                }

                if name == "write" && args.len() == 2 {
                    let path_val = self.evaluate_expression(&args[0]).await;
                    let content_val = self.evaluate_expression(&args[1]).await;
                    
                    if let (JutsuValue::Text(path), JutsuValue::Text(content)) = (path_val, content_val) {
                        match std::fs::write(&path, content) {
                            Ok(_) => return JutsuValue::Boolean(true),
                            Err(e) => panic!("[Runtime Error] Stdlib failed to write file '{}': {}", path, e),
                        }
                    } else {
                        panic!("[Type Error] 'write' requires (Text, Text) arguments.");
                    }
                }

                if name == "to_up" && args.len() == 1 {
                    let text_val = self.evaluate_expression(&args[0]).await;
                    if let JutsuValue::Text(t) = text_val {
                        return JutsuValue::Text(t.to_uppercase());
                    } else { panic!("[Type Error] 'to_up' requires a Text argument."); }
                }

                if name == "to_low" && args.len() == 1 {
                    let text_val = self.evaluate_expression(&args[0]).await;
                    if let JutsuValue::Text(t) = text_val {
                        return JutsuValue::Text(t.to_lowercase());
                    } else { panic!("[Type Error] 'to_low' requires a Text argument."); }
                }

                if name == "type_of" && args.len() == 1 {
                    let val = self.evaluate_expression(&args[0]).await;
                    let type_name = match val {
                        JutsuValue::Text(_) => "Text",
                        JutsuValue::Number(_) => "Number",
                        JutsuValue::Boolean(_) => "Boolean",
                        JutsuValue::Dictionary(_) => "Dictionary",
                        JutsuValue::Array(_) => "Array",
                        JutsuValue::Null => "Null",
                        _ => "Unknown", 
                    };
                    return JutsuValue::Text(type_name.to_string());
                }

                if name == "clean" && args.len() == 1 {
                    let text_val = self.evaluate_expression(&args[0]).await;
                    if let JutsuValue::Text(t) = text_val {
                        let mut clean = t.trim().to_string();
                        if clean.starts_with("```") {
                            if let Some(end_first_line) = clean.find('\n') { clean = clean[end_first_line + 1..].to_string(); }
                        }
                        if clean.ends_with("```") {
                            clean = clean[..clean.len() - 3].trim().to_string();
                        }
                        return JutsuValue::Text(clean);
                    } else { panic!("[Type Error] 'clean' requires a Text argument."); }
                }

                if name == "queue_push" && args.len() == 1 {
                    let val = self.evaluate_expression(&args[0]).await;
                    let mut queue = self.global_queue.lock().await;
                    queue.push_back(val);
                    return JutsuValue::Boolean(true);
                }

                if name == "queue_pop" && args.len() == 0 {
                    let mut queue = self.global_queue.lock().await;
                    if let Some(val) = queue.pop_front() {
                        return val;
                    } else {
                        return JutsuValue::Null;
                    }
                }
                
                if name == "str_replace" && args.len() == 3 {
                    let base_val = self.evaluate_expression(&args[0]).await;
                    let old_val = self.evaluate_expression(&args[1]).await;
                    let new_val = self.evaluate_expression(&args[2]).await;
                    
                    if let (JutsuValue::Text(b), JutsuValue::Text(o), JutsuValue::Text(n)) = (base_val, old_val, new_val) {
                        return JutsuValue::Text(b.replace(&o, &n));
                    }
                }

                if name == "sleep" && args.len() == 1 {
                    let arg_val = self.evaluate_expression(&args[0]).await;
                    
                    let sec = match arg_val {
                        JutsuValue::Number(n) => {
                            if n < 0.0 { 0 } else { n as u64 } // We prevent negative numbers
                        },
                        _ => 1,
                    };
                    
                    // Pause the Tokio thread, allowing other Workers and the Router to continue functioning
                    tokio::time::sleep(std::time::Duration::from_secs(sec)).await;
                    
                    return JutsuValue::Null;
                }

                let func_def = self.functions.get(&name).cloned();
                if let Some((params, body)) = func_def {
                    let mut eval_args = Vec::new();
                    for arg in args { eval_args.push(self.evaluate_expression(&arg).await); }
                    
                    let mut func_env = HashMap::new();
                    for (param_name, arg_val) in params.iter().zip(eval_args) { func_env.insert(param_name.clone(), arg_val); }
                    
                    self.env_stack.push(func_env);
                    let mut return_value = JutsuValue::Null;
                    for s in body { 
                        if let super::ExecResult::Return(val) = self.execute_statement(&s).await { return_value = val; break; } 
                    }
                    self.env_stack.pop();
                    return_value
                } else { panic!("[Runtime Error] Calling undefined function '{}'", name); }
            }

            Expression::InferCall { model_name, prompt_var, context_var, grammar_var } => {
                if let Some(config) = self.models.get(&model_name) {
                    if let Some(JutsuValue::Text(prompt_text)) = self.get_variable(&prompt_var) {
                        
                        let context_text = if let Some(ctx_name) = context_var {
                            if let Some(JutsuValue::Text(c)) = self.get_variable(&ctx_name) { Some(c.clone()) } else { None }
                        } else { None };

                        let grammar_var = if let Some(grm_name) = grammar_var {
                            if let Some(JutsuValue::Text(g)) = self.get_variable(&grm_name) { Some(g.clone()) } else { None }
                        } else { None };
                        
                        let m_ptr = config.model_ptr; let q = config.quantize; let t = config.temp; let b = config.bind;

                        let _optional_lock = if !self.is_shielded { Some(self.hardware_lock.lock().await) } else { None }; 
                        // From this point on, if another thread attempts to make inferences, it will wait patiently
                        // until this thread ends, thus protecting RAM, VRAM and Context.

                        let res = tokio::task::spawn_blocking(move || {
                            crate::inference::run_inference(m_ptr, q, t, b, &prompt_text, context_text, grammar_var)
                        }).await.expect("Thread panic during inference");

                        // Upon exiting this block, _hw_lock is destroyed and the next thread in the queue can use the GPU.
                        match res {
                            Some(result_str) => JutsuValue::Text(result_str),
                            None => panic!("[Hardware Error] The Inference Engine failed to generate a response.")
                        }
                    } else { panic!("[Runtime Error] Inference prompt must be a Text variable."); }
                } else { panic!("[Runtime Error] Vessel '{}' is not defined.", model_name); }
            }

            Expression::PrefixOp { operator, right } => {
                let right_val = self.evaluate_expression(&right).await;
                match operator.as_str() {
                    "-" => match right_val {
                        JutsuValue::Number(n) => JutsuValue::Number(-n),
                        JutsuValue::Tensor { data, shape, requires_grad, .. } => {
                            let new_data = data.iter().map(|&v| -v).collect();
                            JutsuValue::Tensor { data: new_data, shape, requires_grad, grad: None, id: None, parents: vec![], backward_op: None, backward_val: None }
                        },
                        _ => panic!("[Runtime Error] Cannot apply unary minus to non-numeric value."),
                    },
                    "!" => match right_val {
                        JutsuValue::Boolean(b) => JutsuValue::Boolean(!b),
                        _ => panic!("[Runtime Error] Cannot apply logical NOT to non-boolean value."),
                    },
                    _ => panic!("[Runtime Error] Unknown prefix operator '{}'", operator),
                }
            }

            Expression::InfixOp { left, operator, right } => {
                let left_val_raw = self.evaluate_expression(&left).await;
                let right_val_raw = self.evaluate_expression(&right).await;
                
                let left_val = match left_val_raw { JutsuValue::Shared(ref domain) => domain.0.lock().unwrap().as_ref().clone(), other => other };
                let right_val = match right_val_raw { JutsuValue::Shared(ref domain) => domain.0.lock().unwrap().as_ref().clone(), other => other };
                
                match operator.as_str() {
                    "+" => match (left_val, right_val) {
                        (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Number(l + r),
                        (JutsuValue::Text(l), JutsuValue::Text(r)) => JutsuValue::Text(format!("{}{}", l, r)),
                        (JutsuValue::Text(l), r) => JutsuValue::Text(format!("{}{}", l, r.to_string())),
                        (l, JutsuValue::Text(r)) => JutsuValue::Text(format!("{}{}", l.to_string(), r)),
                        (JutsuValue::Tensor { data: d1, shape: s1, requires_grad: r1, .. }, JutsuValue::Tensor { data: d2, shape: s2, .. }) => {
                            if s1 != s2 { panic!("[Runtime Error] Tensor addition shape mismatch."); }
                            let new_data = d1.iter().zip(d2.iter()).map(|(a, b)| a + b).collect();
                            JutsuValue::Tensor { data: new_data, shape: s1, requires_grad: r1, grad: None, id: None, parents: vec![], backward_op: Some("add".to_string()), backward_val: None }
                        }
                        _ => panic!("[Runtime Error] Type mismatch in addition ('+')."),
                    },
                    "-" => match (left_val, right_val) {
                        (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Number(l - r),
                        (JutsuValue::Tensor { data: d1, shape: s1, requires_grad: r1, .. }, JutsuValue::Tensor { data: d2, shape: s2, .. }) => {
                            if s1 != s2 { panic!("[Runtime Error] Tensor subtraction shape mismatch."); }
                            let new_data = d1.iter().zip(d2.iter()).map(|(a, b)| a - b).collect();
                            JutsuValue::Tensor { data: new_data, shape: s1, requires_grad: r1, grad: None, id: None, parents: vec![], backward_op: Some("sub".to_string()), backward_val: None }
                        }
                        _ => panic!("[Runtime Error] Type mismatch in subtraction ('-')."),
                    },
                    "/" => match (left_val, right_val) {
                        (JutsuValue::Number(l), JutsuValue::Number(r)) => {
                            if r == 0.0 { panic!("[Runtime Error] Division by zero."); } JutsuValue::Number(l / r)
                        },
                        _ => panic!("[Runtime Error] Type mismatch in division ('/')."),
                    },
                    "%" => match (left_val, right_val) {
                        (JutsuValue::Number(l), JutsuValue::Number(r)) => {
                            if r == 0.0 { panic!("[Runtime Error] Modulo by zero."); } JutsuValue::Number(l % r)
                        },
                        _ => panic!("[Runtime Error] Modulo requires numeric values."),
                    },
                    "*" => match (left_val, right_val) {
                        (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Number(l * r),
                        (JutsuValue::Text(s), JutsuValue::Number(n)) | (JutsuValue::Number(n), JutsuValue::Text(s)) => {
                            if n < 0.0 { panic!("[Runtime Error] Cannot multiply string by a negative number."); }
                            JutsuValue::Text(s.repeat(n as usize))
                        },
                        (JutsuValue::Tensor { data, shape, requires_grad, id, .. }, JutsuValue::Number(n)) => {
                            let new_data: Vec<f32> = data.iter().map(|v| v * n).collect();
                            let mut parents = Vec::new(); if let Some(parent_name) = id { parents.push(parent_name); }
                            JutsuValue::Tensor { data: new_data, shape, requires_grad, grad: None, id: None, parents, backward_op: Some("multiply".to_string()), backward_val: Some(n) }
                        },
                        (JutsuValue::Tensor { data: d1, shape: s1, requires_grad: r1, id: id1, .. }, JutsuValue::Tensor { data: d2, shape: s2, id: id2, .. }) => {
                            if s1.len() == 2 && s2.len() == 2 && s1[1] == s2[0] {
                                let rows1 = s1[0]; let cols1 = s1[1]; let cols2 = s2[1];
                                let mut new_data = vec![0.0; rows1 * cols2];
                                for i in 0..rows1 {
                                    for j in 0..cols2 {
                                        let mut sum = 0.0; for k in 0..cols1 { sum += d1[i * cols1 + k] * d2[k * cols2 + j]; }
                                        new_data[i * cols2 + j] = sum;
                                    }
                                }
                                let mut parents = Vec::new(); if let Some(p) = id1 { parents.push(p); } if let Some(p) = id2 { parents.push(p); }
                                JutsuValue::Tensor { data: new_data, shape: vec![rows1, cols2], requires_grad: r1, grad: None, id: None, parents, backward_op: Some("matmul".to_string()), backward_val: None }
                            } else if s1 == s2 {
                                let new_data: Vec<f32> = d1.iter().zip(d2.iter()).map(|(a, b)| a * b).collect();
                                let mut parents = Vec::new(); if let Some(p) = id1 { parents.push(p); } if let Some(p) = id2 { parents.push(p); }
                                JutsuValue::Tensor { data: new_data, shape: s1, requires_grad: r1, grad: None, id: None, parents, backward_op: Some("elem_mul".to_string()), backward_val: None }
                            } else { panic!("[Runtime Error] Tensor multiplication shape mismatch."); }
                        }
                        _ => panic!("[Runtime Error] Type mismatch in multiplication ('*')."),
                    },
                    ">" => match (left_val, right_val) { (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Boolean(l > r), _ => panic!("[Runtime Error] '>' requires numeric values.") },
                    "<" => match (left_val, right_val) { (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Boolean(l < r), _ => panic!("[Runtime Error] '<' requires numeric values.") },
                    ">=" => match (left_val, right_val) { (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Boolean(l >= r), _ => panic!("[Runtime Error] '>=' requires numeric values.") },
                    "<=" => match (left_val, right_val) { (JutsuValue::Number(l), JutsuValue::Number(r)) => JutsuValue::Boolean(l <= r), _ => panic!("[Runtime Error] '<=' requires numeric values.") },
                    "==" => JutsuValue::Boolean(left_val == right_val),
                    "!=" => JutsuValue::Boolean(left_val != right_val),
                    "&&" => {
                        let l = match left_val { JutsuValue::Boolean(b) => b, _ => panic!("[Runtime Error] '&&' requires booleans") };
                        let r = match right_val { JutsuValue::Boolean(b) => b, _ => panic!("[Runtime Error] '&&' requires booleans") };
                        JutsuValue::Boolean(l && r)
                    },
                    "||" => {
                        let l = match left_val { JutsuValue::Boolean(b) => b, _ => panic!("[Runtime Error] '||' requires booleans") };
                        let r = match right_val { JutsuValue::Boolean(b) => b, _ => panic!("[Runtime Error] '||' requires booleans") };
                        JutsuValue::Boolean(l || r)
                    },
                    _ => panic!("[Runtime Error] Unknown operator: {}", operator),
                }
            }

            Expression::Share { value } => {
                let val = Box::pin(self.evaluate_expression(&value)).await;
                let mut queue = self.global_queue.lock().await;
                queue.push_back(val);
                
                JutsuValue::Boolean(true)
            }

            Expression::SysExecCall(expr) => {
                let cmd_val = Box::pin(self.evaluate_expression(&expr)).await;
                if let JutsuValue::Text(cmd_str) = cmd_val {
                    let output = if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd").args(["/C", &cmd_str]).output()
                    } else {
                        std::process::Command::new("sh").arg("-c").arg(&cmd_str).output()
                    };

                    match output {
                        Ok(out) => {
                            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                            if out.status.success() { JutsuValue::Text(stdout.trim().to_string()) } else { JutsuValue::Text(format!("Error: {}", stderr.trim())) }
                        }
                        Err(e) => panic!("[Runtime Error] Execution of sys_exec failed: {}", e),
                    }
                } else { panic!("[Type Error] sys_exec expects a string command."); }
            }

            Expression::HttpGetCall(expr) => {
                let url_val = Box::pin(self.evaluate_expression(&expr)).await;
                if let JutsuValue::Text(url_str) = url_val {
                    match reqwest::get(&url_str).await {
                        Ok(response) => {
                            match response.text().await {
                                Ok(text) => JutsuValue::Text(text),
                                Err(e) => panic!("[Runtime Error] http_get failed to read text body: {}", e),
                            }
                        }
                        Err(e) => JutsuValue::Text(format!("Error: Could not connect to API. {}", e)),
                    }
                } else { panic!("[Type Error] http_get expects a string URL."); }
            }
        }
    }

    #[async_recursion]
    pub async fn evaluate_condition(&mut self, expr: &Expression) -> bool {
        let mut val = self.evaluate_expression(expr).await;
        if let JutsuValue::Shared(domain) = val { val = domain.0.lock().unwrap().as_ref().clone(); }
        match val {
            JutsuValue::Boolean(b) => b,
            JutsuValue::Array(_) | JutsuValue::Dictionary(_) | JutsuValue::Tensor { .. } | JutsuValue::Shared(_) => true,
            JutsuValue::Text(s) => !s.is_empty() && s != "false" && s != "EMPTY_PING",
            JutsuValue::Number(n) => n != 0.0,
            JutsuValue::Null => false,
        }
    }
}