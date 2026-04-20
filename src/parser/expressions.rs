use super::Parser;
use crate::ast::Expression;
use crate::lexer::JutsuToken;

impl<'a> Parser<'a> {
    /// Assigns a binding power (weight) to operators for correct mathematical precedence.
    pub(super) fn get_precedence(token: &Option<JutsuToken>) -> u8 {
        match token {
            Some(JutsuToken::Or) => 1,
            Some(JutsuToken::And) => 2,
            Some(JutsuToken::EqualEqual) | Some(JutsuToken::NotEqual) => 3,
            Some(JutsuToken::LessThan) | Some(JutsuToken::GreaterThan) | 
            Some(JutsuToken::LessEqual) | Some(JutsuToken::GreaterEqual) => 4,
            Some(JutsuToken::Plus) | Some(JutsuToken::Minus) => 5, // Sum/Sub
            Some(JutsuToken::Star) | Some(JutsuToken::Slash) | Some(JutsuToken::Modulo) => 6, // Mult/Div/Mod
            _ => 0,
        }
    }

    /// Public wrapper for parsing expressions starting from the lowest precedence
    pub(super) fn parse_expression(&mut self) -> Option<Expression> {
        self.parse_expression_with_precedence(0)
    }

    /// Core recursive expression parser using Pratt Parsing (Precedence Climbing)
    pub(super) fn parse_expression_with_precedence(&mut self, precedence: u8) -> Option<Expression> {
        let mut left_expr = match self.current_token {
            // UNARY OPERATORS (-x, !x)
            Some(JutsuToken::Minus) | Some(JutsuToken::Bang) => {
                let operator = self.lexer.slice().to_string();
                self.advance();
                let right = self.parse_expression_with_precedence(7)?; // Prefix has very high precedence
                Some(Expression::PrefixOp { operator, right: Box::new(right) })
            }
            // PARENTHESES FOR MATHEMATICAL GROUPING
            Some(JutsuToken::ParenOpen) => {
                self.advance();
                let expr = self.parse_expression_with_precedence(0)?;
                if let Some(JutsuToken::ParenClose) = self.current_token {
                    self.advance();
                    Some(expr)
                } else {
                    None // Missing closing parenthesis
                }
            }
            Some(JutsuToken::Recv) => { 
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::RecvCall)
            }
            // Literal values
            Some(JutsuToken::True) => { self.advance(); Some(Expression::BooleanLiteral(true)) }
            Some(JutsuToken::False) => { self.advance(); Some(Expression::BooleanLiteral(false)) }
            Some(JutsuToken::Number) => { 
                let n = self.lexer.slice().parse::<f32>().unwrap_or(0.0);
                self.advance(); Some(Expression::NumberLiteral(n))
            }
            Some(JutsuToken::StringLiteral) => {
                let raw_str = self.lexer.slice();
                let s;
                if raw_str.starts_with("\"\"\"") {
                    s = raw_str[3..raw_str.len() - 3].to_string();
                } else {
                    let trimmed = &raw_str[1..raw_str.len() - 1];
                    s = trimmed.replace("\\n", "\n").replace("\\t", "\t").replace("\\\"", "\"");
                }
                self.advance(); 
                Some(Expression::StringLiteral(s))
            }
            // THE 2D TENSOR PARSER
            Some(JutsuToken::Tensor) => { 
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                if let Some(JutsuToken::BracketOpen) = self.current_token { self.advance(); } else { return None; }
                
                let mut data = Vec::new();
                let shape;

                if let Some(JutsuToken::BracketOpen) = self.current_token {
                    let mut rows = 0;
                    let mut cols = 0;
                    
                    while self.current_token == Some(JutsuToken::BracketOpen) {
                        self.advance(); 
                        let mut current_cols = 0;
                        
                        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BracketClose) {
                            if let Some(expr) = self.parse_expression() {
                                data.push(expr);
                                current_cols += 1;
                            }
                            if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }
                        }
                        if rows == 0 { cols = current_cols; }
                        rows += 1;
                        if let Some(JutsuToken::BracketClose) = self.current_token { self.advance(); } 
                        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } 
                    }
                    shape = vec![rows, cols];
                } else {
                    let mut cols = 0;
                    while self.current_token.is_some() && self.current_token != Some(JutsuToken::BracketClose) {
                        if let Some(expr) = self.parse_expression() {
                            data.push(expr);
                            cols += 1;
                        }
                        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }
                    }
                    shape = vec![cols];
                }

                if let Some(JutsuToken::BracketClose) = self.current_token { self.advance(); } 
                if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }
                
                let mut requires_grad = false;
                if let Some(JutsuToken::Grad) = self.current_token {
                    self.advance();
                    if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
                    match self.current_token {
                        Some(JutsuToken::True) => { self.advance(); requires_grad = true; },
                        Some(JutsuToken::False) => { self.advance(); requires_grad = false; },
                        _ => {}
                    }
                }
                
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::TensorDeclaration { data, shape, requires_grad })
            }
            Some(JutsuToken::Infer) => {
                self.advance();
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                
                let model_name = if let Some(JutsuToken::Identifier) = self.current_token {
                    let m = self.lexer.slice().to_string(); self.advance(); m
                } else { return None; };

                if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } else { return None; }

                let prompt_var = if let Some(JutsuToken::Identifier) = self.current_token {
                    let p = self.lexer.slice().to_string(); self.advance(); p
                } else { return None; };

                let mut context_var = None;
                let mut grammar_var = None;

                if let Some(JutsuToken::Comma) = self.current_token {
                    self.advance();
                    if let Some(JutsuToken::Identifier) = self.current_token {
                        context_var = Some(self.lexer.slice().to_string());
                        self.advance();
                    }
                }
                
                if let Some(JutsuToken::Comma) = self.current_token {
                    self.advance();
                    if let Some(JutsuToken::Identifier) = self.current_token {
                        grammar_var = Some(self.lexer.slice().to_string());
                        self.advance();
                    }
                }

                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::InferCall { model_name, prompt_var, context_var, grammar_var })
            }
            Some(JutsuToken::Input) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                let prompt_expr = self.parse_expression()?; 
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::InputCall(Box::new(prompt_expr)))
            }
            Some(JutsuToken::ReadText) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                let filepath = if let Some(JutsuToken::StringLiteral) = self.current_token {
                    let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
                } else { return None; };
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::ReadTextCall(filepath))
            }
            Some(JutsuToken::Rag) => {
                self.advance();
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                let query_var = if let Some(JutsuToken::Identifier) = self.current_token {
                    let v = self.lexer.slice().to_string(); self.advance(); v
                } else { return None; };
                if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } else { return None; }
                let doc_var = if let Some(JutsuToken::Identifier) = self.current_token {
                    let v = self.lexer.slice().to_string(); self.advance(); v
                } else { return None; };
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::RagCall { query_var, doc_var })
            }
            Some(JutsuToken::Share) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                let inner_expr = self.parse_expression()?; 
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::Share { value: Box::new(inner_expr) })
            }
            Some(JutsuToken::BracketOpen) => {
                self.advance();
                let mut elements = Vec::new();
                let mut first = true;
                while self.current_token.is_some() && self.current_token != Some(JutsuToken::BracketClose) {
                    if !first {
                        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }
                        else { panic!("[Syntax Error] Expected ',' in array near: '{}'", self.lexer.slice()); }
                    }
                    if let Some(expr) = self.parse_expression() { elements.push(expr); }
                    first = false;
                }
                if let Some(JutsuToken::BracketClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing ']' in array"); }
                Some(Expression::Array(elements))
            }
            Some(JutsuToken::BraceOpen) => {
                self.advance();
                let mut pairs = Vec::new();
                let mut first = true;
                while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
                    if !first {
                        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }
                        else { panic!("[Syntax Error] Expected ',' in dictionary"); }
                    }
                    let key = if let Some(JutsuToken::StringLiteral) = self.current_token {
                        self.lexer.slice().trim_matches('"').to_string()
                    } else if let Some(JutsuToken::Identifier) = self.current_token {
                        self.lexer.slice().to_string()
                    } else { panic!("[Syntax Error] Dictionary key must be a string or identifier"); };
                    self.advance();

                    if let Some(JutsuToken::Colon) = self.current_token { self.advance(); }
                    else { panic!("[Syntax Error] Expected ':' after dictionary key"); }

                    if let Some(val) = self.parse_expression() { pairs.push((key, val)); }
                    first = false;
                }
                if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing '}}' in dictionary"); }
                Some(Expression::Dictionary(pairs))
            }
            Some(JutsuToken::SysExec) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                let expr = self.parse_expression()?; 
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::SysExecCall(Box::new(expr)))
            }
            Some(JutsuToken::HttpGet) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                let expr = self.parse_expression()?; 
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Expression::HttpGetCall(Box::new(expr)))
            }
            Some(JutsuToken::Identifier) => {
                let id = self.lexer.slice().to_string();
                self.advance(); 
                
                if let Some(JutsuToken::Dot) = self.current_token {
                    self.advance(); // We consume the period '.'
                    
                    if let Some(JutsuToken::Infer) = self.current_token {
                        self.advance(); 
                        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                        
                        let prompt_var = if let Some(JutsuToken::Identifier) = self.current_token {
                            let v = self.lexer.slice().to_string(); self.advance(); v
                        } else { return None; };
                        
                        let mut context_var = None;
                        let mut grammar_var = None;
                        
                        if let Some(JutsuToken::Comma) = self.current_token {
                            self.advance();
                            if let Some(JutsuToken::Identifier) = self.current_token {
                                context_var = Some(self.lexer.slice().to_string());
                                self.advance();
                            }
                        }

                        if let Some(JutsuToken::Comma) = self.current_token {
                            self.advance();
                            if let Some(JutsuToken::Identifier) = self.current_token {
                                grammar_var = Some(self.lexer.slice().to_string());
                                self.advance();
                            }
                        }
                        
                        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                        
                        Some(Expression::InferCall { model_name: id, prompt_var, context_var, grammar_var })
                    
                    } else if let Some(JutsuToken::CallTool) = self.current_token {
                        self.advance(); // We also consume 'call'
                        
                        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                        
                        // We extract the first argument: The name of the tool (It must be a String)
                        let tool_name = if let Some(JutsuToken::StringLiteral) = self.current_token {
                            let s = self.lexer.slice().trim_matches('"').to_string(); 
                            self.advance(); 
                            s
                        } else { 
                            panic!("[Syntax Error] Expected string literal for tool name in 'call'"); 
                        };

                        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Expected ',' after tool name"); }
                        
                        // The second argument is extracted: The dictionary/parameter object
                        // And we use self.parse_expression() which will recursively read the entire dictionary
                        let params = self.parse_expression().expect("[Syntax Error] Expected parameters (dictionary or expression) after tool name");
                        
                        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing ')' in 'call'"); }

                        Some(Expression::CallToolCall { vessel_name: id, tool_name, params: Box::new(params) })
                    } else {
                        Some(Expression::Variable(id))
                    }
                } else if let Some(JutsuToken::ParenOpen) = self.current_token {
                    self.advance(); 
                    let mut args = Vec::new();
                    let mut first = true;
                    while self.current_token.is_some() && self.current_token != Some(JutsuToken::ParenClose) {
                        if !first {
                            if let Some(JutsuToken::Comma) = self.current_token { 
                                self.advance(); 
                            } else { 
                                panic!("[Syntax Error] Expected ',' between arguments near: '{}'", self.lexer.slice()); 
                            }
                        }
                        if let Some(expr) = self.parse_expression() { 
                            args.push(expr); 
                        } else { 
                            panic!("[Syntax Error] Invalid argument expression near: '{}'", self.lexer.slice()); 
                        }
                        first = false;
                    }
                    if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing ')' in function call"); }
                    
                    Some(Expression::FunctionCall { name: id, args })
                } else if let Some(JutsuToken::BracketOpen) = self.current_token {
                    self.advance(); 
                    let index = self.parse_expression().expect("[Syntax Error] Expected index expression");
                    if let Some(JutsuToken::BracketClose) = self.current_token { 
                        self.advance(); 
                    } else { 
                        panic!("[Syntax Error] Missing ']' in index access near: '{}'", self.lexer.slice()); 
                    }
                    let left_expr = Expression::Variable(id.clone());
                    
                    Some(Expression::IndexAccess { left: Box::new(left_expr), index: Box::new(index) })
                } else {
                    Some(Expression::Variable(id))
                }
            }
            _ => { return None; }
        }?;
        
        // INFIX LOOP (Precedence Climbing)
        while self.current_token.is_some() {
            let current_prec = Self::get_precedence(&self.current_token);
            if precedence >= current_prec { break; } 

            match self.current_token {
                Some(JutsuToken::EqualEqual) | Some(JutsuToken::NotEqual) | 
                Some(JutsuToken::Star) | Some(JutsuToken::Plus) | Some(JutsuToken::Minus) | 
                Some(JutsuToken::Slash) | Some(JutsuToken::Modulo) |
                Some(JutsuToken::GreaterThan) | Some(JutsuToken::LessThan) | 
                Some(JutsuToken::GreaterEqual) | Some(JutsuToken::LessEqual) |
                Some(JutsuToken::And) | Some(JutsuToken::Or) => {
                    let operator = self.lexer.slice().to_string();
                    self.advance();
                    if let Some(right_expr) = self.parse_expression_with_precedence(current_prec) {
                        left_expr = Expression::InfixOp { left: Box::new(left_expr), operator, right: Box::new(right_expr) };
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        Some(left_expr)
    }
}