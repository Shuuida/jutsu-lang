#![allow(dead_code)]

pub mod expressions;
pub mod statements;
pub mod rag;

use crate::ast::{Program, Statement};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;
use async_recursion::async_recursion;

#[derive(Clone)]
pub struct VesselConfig {
    pub file_path: String,
    pub temp: f32,
    pub bind: f32,
    pub quantize: bool,
    pub model_ptr: usize, 
}

#[derive(Clone, Debug)]
pub struct SharedDomain(pub Arc<Mutex<Box<JutsuValue>>>);

impl PartialEq for SharedDomain {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) 
    }
}

pub static CONSOLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug, PartialEq)]
pub enum JutsuValue {
    Text(String),
    Number(f32),
    Boolean(bool),
    Array(Vec<JutsuValue>),
    Dictionary(HashMap<String, JutsuValue>),
    Shared(SharedDomain),
    Tensor { 
        data: Vec<f32>, shape: Vec<usize>, requires_grad: bool, grad: Option<Vec<f32>>, 
        id: Option<String>, parents: Vec<String>, backward_op: Option<String>, backward_val: Option<f32> 
    },
    Null,
}

impl JutsuValue {
    pub fn to_string(&self) -> String {
        match self {
            JutsuValue::Text(s) => s.clone(),
            JutsuValue::Number(n) => n.to_string(),
            JutsuValue::Boolean(b) => b.to_string(),
            JutsuValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| {
                    if let JutsuValue::Text(s) = v { format!("\"{}\"", s) } else { v.to_string() }
                }).collect();
                format!("[{}]", items.join(", "))
            },
            JutsuValue::Dictionary(dict) => {
                let mut items: Vec<String> = dict.iter().map(|(k, v)| {
                    let v_str = if let JutsuValue::Text(s) = v { format!("\"{}\"", s) } else { v.to_string() };
                    format!("\"{}\": {}", k, v_str)
                }).collect();
                items.sort();
                format!("{{{}}}", items.join(", "))
            },
            JutsuValue::Shared(domain) => {
                let inner = domain.0.lock().unwrap();
                format!("Shared({})", inner.to_string())
            },
            JutsuValue::Tensor { data, shape, requires_grad, grad, .. } => {
                let mut data_str = String::new();
                if shape.len() == 2 {
                    let cols = shape[1];
                    data_str.push_str("[\n");
                    for (i, val) in data.iter().enumerate() {
                        if i % cols == 0 { data_str.push_str("    ["); }
                        data_str.push_str(&val.to_string());
                        if (i + 1) % cols == 0 { 
                            data_str.push_str("]"); 
                            if i < data.len() - 1 { data_str.push_str(",\n"); }
                        } else {
                            data_str.push_str(", ");
                        }
                    }
                    data_str.push_str("\n  ]");
                } else {
                    data_str = format!("{:?}", data); 
                }

                if let Some(g) = grad { format!("Tensor(data={}, shape={:?}, grad={:?})", data_str, shape, g) } 
                else { format!("Tensor(data={}, shape={:?}, grad={})", data_str, shape, requires_grad) }
            },
            JutsuValue::Null => "null".to_string(),
        }
    }
}

pub enum ExecResult {
    Normal,
    Return(JutsuValue),
    None, 
}

pub fn serde_to_jutsu(val: serde_json::Value) -> JutsuValue {
    match val {
        serde_json::Value::Null => JutsuValue::Null,
        serde_json::Value::Bool(b) => JutsuValue::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() { JutsuValue::Number(f as f32) } else { JutsuValue::Number(0.0) }
        },
        serde_json::Value::String(s) => JutsuValue::Text(s),
        serde_json::Value::Array(arr) => {
            JutsuValue::Array(arr.into_iter().map(serde_to_jutsu).collect())
        },
        serde_json::Value::Object(obj) => {
            let mut dict = HashMap::new();
            for (k, v) in obj { dict.insert(k, serde_to_jutsu(v)); }
            JutsuValue::Dictionary(dict)
        }
    }
}

#[derive(Clone)]
pub struct Evaluator {
    pub env_stack: Vec<HashMap<String, JutsuValue>>, 
    pub models: HashMap<String, VesselConfig>,
    pub functions: HashMap<String, (Vec<String>, Vec<Statement>)>, 
    pub tcp_stream: Option<Arc<AsyncMutex<TcpStream>>>,
    pub global_queue: Arc<AsyncMutex<VecDeque<JutsuValue>>>,
    pub hardware_lock: Arc<AsyncMutex<()>>,
    pub is_shielded: bool,
}

impl Evaluator {
    pub fn new() -> Self { 
        Evaluator {
            env_stack: vec![HashMap::new()], 
            models: HashMap::new(),
            functions: HashMap::new(),
            tcp_stream: None,
            global_queue: Arc::new(AsyncMutex::new(VecDeque::new())),
            hardware_lock: Arc::new(AsyncMutex::new(())),
            is_shielded: false,
        } 
    }

    pub fn get_variable(&self, name: &str) -> Option<JutsuValue> {
        for env in self.env_stack.iter().rev() {
            if let Some(val) = env.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn set_global_variable(&mut self, name: String, value: JutsuValue) {
        if let Some(global_env) = self.env_stack.first_mut() {
            global_env.insert(name, value);
        }
    }

    pub async fn evaluate(&mut self, program: &Program) {
        for statement in &program.statements { 
            if let ExecResult::Return(val) = self.execute_statement(statement).await {
                println!("[Runtime] Execution halted by top-level return: {}", val.to_string());
                break;
            }
        }
    }

    #[async_recursion]
    pub async fn execute_block(&mut self, statements: &[Statement]) -> ExecResult {
        self.env_stack.push(HashMap::new()); 
        let mut result = ExecResult::Normal;
        
        for s in statements {
            result = self.execute_statement(s).await;
            if let ExecResult::Return(_) = result { break; }
        }
        
        self.env_stack.pop(); 
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser; // Import the parser so that it translates the text

    #[tokio::test] 
    async fn test_assignment_and_mathematics() {
        let input = "let dog = 100 + 50";
        let mut parser = Parser::new(input);
        let program = parser.parse();

        let mut evaluator = Evaluator::new();
        
        // We simulate the execution of the block
        for stmt in program.statements {
            evaluator.execute_statement(&stmt).await;
        }

        // Check if the variable 'dog' was actually saved in the environment (env_stack)
        let saved_value = evaluator.get_variable("dog").expect("The variable 'dog' should exist in memory");
        
        match saved_value {
            JutsuValue::Number(n) => assert_eq!(n, 150.0, "100 + 50 should be 150"),
            _ => panic!("The variable was not saved as a number"),
        }
    }
}