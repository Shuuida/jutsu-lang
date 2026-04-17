use super::Parser;
use crate::ast::Statement;
use crate::lexer::JutsuToken;

impl<'a> Parser<'a> {
    pub(super) fn parse_reply_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        let value = self.parse_expression()?;
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::ReplyStatement { value })
    }

    pub(super) fn parse_hextrace_block(&mut self) -> Option<Statement> {
        self.advance(); 
        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { return None; }
        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); } else { self.advance(); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::HexTraceBlock { body })
    }

    pub(super) fn parse_shield_block(&mut self) -> Option<Statement> {
        self.advance();
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::Identifier) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); } else { return None; }
        let max_vram = if let Some(JutsuToken::StringLiteral) = self.current_token {
            let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
        } else { return None; };
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { return None; }

        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); } else { self.advance(); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::ShieldBlock { max_vram, body })
    }

    pub(super) fn parse_mcp_server_block(&mut self) -> Option<Statement> {
        self.advance(); // Consume 'mcp_server'
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        
        // We consume the 'port' tag and the '=' (friendly syntax)
        if let Some(JutsuToken::Identifier) = self.current_token { self.advance(); } 
        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
        
        let port = self.parse_expression()?;
        
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { return None; }

        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); } else { self.advance(); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { return None; }
        
        Some(Statement::McpServerBlock { port, body })
    }

    pub(super) fn parse_expose_tool_statement(&mut self) -> Option<Statement> {
        self.advance(); // Consume 'expose_tool'
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        
        // We extract: name = "read_log"
        if let Some(JutsuToken::Identifier) = self.current_token { self.advance(); }
        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
        let name = if let Some(JutsuToken::StringLiteral) = self.current_token {
            let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
        } else { return None; };
        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }

        // We extract: desc = "Tool description"
        if let Some(JutsuToken::Identifier) = self.current_token { self.advance(); }
        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
        let description = if let Some(JutsuToken::StringLiteral) = self.current_token {
            let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
        } else { return None; };
        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); }

        // We extract: function = name_of_the_jutsu_function
        if let Some(JutsuToken::Identifier) = self.current_token { self.advance(); }
        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
        let function_name = if let Some(JutsuToken::Identifier) = self.current_token {
            let s = self.lexer.slice().to_string(); self.advance(); s
        } else { return None; };

        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        
        Some(Statement::ExposeToolStatement { name, description, function_name })
    }

    pub(super) fn parse_veil_block(&mut self) -> Option<Statement> {
        self.advance();
        let mut name = "backend_server".to_string(); 

        if let Some(JutsuToken::Identifier) = self.current_token {
            name = self.lexer.slice().to_string();
            self.advance();
        }

        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::Identifier) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); } else { return None; }
        
        let port = self.parse_expression()?; 
        
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { return None; }

        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); } else { self.advance(); }
        }
        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::VeilBlock { name, port, body })
    }

    pub(super) fn parse_worker_block(&mut self) -> Option<Statement> {
        self.advance(); 
        if let Some(JutsuToken::BraceOpen) = self.current_token { self.advance(); } else { return None; }

        let mut body = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(JutsuToken::BraceClose) {
            if let Some(stmt) = self.parse_statement() { 
                body.push(stmt); 
            } else { 
                self.advance(); 
            }
        }

        if let Some(JutsuToken::BraceClose) = self.current_token { self.advance(); } else { return None; }
        
        Some(Statement::WorkerBlock { body })
    }
}