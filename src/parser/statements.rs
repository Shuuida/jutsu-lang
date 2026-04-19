use super::Parser;
use crate::ast::{Expression, Statement};
use crate::lexer::JutsuToken;

impl<'a> Parser<'a> {
    pub(super) fn parse_import_statement(&mut self) -> Option<Statement> {
        self.advance(); // Consume 'import'
        let path = if let Some(JutsuToken::StringLiteral) = self.current_token {
            let p = self.lexer.slice().trim_matches('"').to_string();
            self.advance();
            p
        } else {
            return None;
        };

        Some(Statement::ImportStatement { path })
    }

    pub(super) fn parse_if_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        let condition = self.parse_expression()?;

        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Expected '{{' after if condition near: '{}'", self.lexer.slice()); }
        let mut consequence = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { consequence.push(stmt); } else { panic!("[Syntax Error] Invalid syntax inside if block near: '{}'", self.lexer.slice()); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing '}}' to close if block"); }

        let mut alternative = None;
        if let Some(JutsuToken::Else) = self.current_token {
            self.advance(); 
            
            if let Some(JutsuToken::If) = self.current_token {
                if let Some(else_if_stmt) = self.parse_if_statement() {
                    alternative = Some(vec![else_if_stmt]);
                }
            } else {
                if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Expected '{{' after else near: '{}'", self.lexer.slice()); }
                let mut alt_body = Vec::new();
                while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
                    if let Some(stmt) = self.parse_statement() { alt_body.push(stmt); } else { panic!("[Syntax Error] Invalid syntax inside else block near: '{}'", self.lexer.slice()); }
                }
                if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing '}}' to close else block"); }
                alternative = Some(alt_body);
            }
        }
        Some(Statement::IfStatement { condition, consequence, alternative })
    }

    pub(super) fn parse_while_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        let condition = self.parse_expression()?;
        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { return None; }
        
        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); } else { self.advance(); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::WhileStatement { condition, body })
    }

    pub(super) fn parse_function_declaration(&mut self) -> Option<Statement> {
        self.advance(); 
        let name = if let Some(JutsuToken::Identifier) = self.current_token {
            let n = self.lexer.slice().to_string(); self.advance(); n
        } else { return None; };

        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        
        let mut params = Vec::new();
        let mut first = true;
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::ParenClose) {
            if !first {
                if let Some(JutsuToken::Comma) = self.current_token { 
                    self.advance(); 
                } else { 
                    panic!("[Syntax Error] Expected ',' between parameters near: '{}'", self.lexer.slice()); 
                }
            }
            if let Some(JutsuToken::Identifier) = self.current_token {
                params.push(self.lexer.slice().to_string());
                self.advance();
            } else {
                panic!("[Syntax Error] Expected parameter name near: '{}'", self.lexer.slice());
            }
            first = false;
        }
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing ')' in function declaration"); }

        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Expected '{{' to start function body"); }
        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); } else { panic!("[Syntax Error] Invalid syntax inside function body near: '{}'", self.lexer.slice()); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { panic!("[Syntax Error] Missing '}}' to close function body"); }
        
        Some(Statement::FunctionDeclaration { name, params, body })
    }

    pub(super) fn parse_return_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        let value = self.parse_expression()?;
        Some(Statement::ReturnStatement { value })
    }

    pub(super) fn parse_let_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        let name = if let Some(JutsuToken::Identifier) = self.current_token {
            let n = self.lexer.slice().to_string(); self.advance(); n
        } else { return None; };

        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); } else { return None; }
        let value = self.parse_expression()?;
        Some(Statement::LetStatement { name, value })
    }

    pub(super) fn parse_print_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        let expr = self.parse_expression()?;
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::PrintStatement { value: expr })
    }

    /// Can parse a re-assignment (x = 5) or an inference method call (x.infer)
    pub(super) fn parse_identifier_driven_statement(&mut self) -> Option<Statement> {
        let name = self.lexer.slice().to_string();
        self.advance(); 

        match self.current_token {
            Some(JutsuToken::Equal) => {
                self.advance(); // Consume the '='
                let value = self.parse_expression()?;
                Some(Statement::AssignmentStatement { name, value })
            }
            Some(JutsuToken::ParenOpen) => {
                self.advance(); // Consume the '('
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
                
                let call_expr = Expression::FunctionCall { name: name.clone(), args };
                Some(Statement::LetStatement { name: "_".to_string(), value: call_expr })
            }
            Some(JutsuToken::Dot) => {
                self.advance(); // Consume the '.'
                if let Some(JutsuToken::Infer) = self.current_token { self.advance(); } else { return None; }
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }

                let prompt_var = if let Some(JutsuToken::Identifier) = self.current_token {
                    let var = self.lexer.slice().to_string(); self.advance(); var
                } else { return None; };

                let mut context_var = None;
                if let Some(JutsuToken::Comma) = self.current_token {
                    self.advance();
                    if let Some(JutsuToken::Identifier) = self.current_token {
                        context_var = Some(self.lexer.slice().to_string());
                        self.advance();
                    }
                }
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Statement::InferStatement { model_name: name, prompt_var, context_var })
            }
            _ => {
                None
            }
        }
    }
}