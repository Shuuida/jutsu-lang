use super::Parser;
use crate::ast::Statement;
use crate::lexer::JutsuToken;

impl<'a> Parser<'a> {
    pub(super) fn parse_backward_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        let target_var = if let Some(JutsuToken::Identifier) = self.current_token {
            let v = self.lexer.slice().to_string(); self.advance(); v
        } else { return None; };
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::BackwardStatement { target_var })
    }

    pub(super) fn parse_optim_statement(&mut self) -> Option<Statement> {
        self.advance(); 
        if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
        let target_var = if let Some(JutsuToken::Identifier) = self.current_token {
            let v = self.lexer.slice().to_string(); self.advance(); v
        } else { return None; };
        if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } else { return None; }
        let learning_rate = self.parse_expression()?;
        if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
        Some(Statement::OptimStatement { target_var, learning_rate })
    }

    pub(super) fn parse_vessel_declaration(&mut self) -> Option<Statement> {
        self.advance(); 
        
        let name = if let Some(JutsuToken::Identifier) = self.current_token {
            let n = self.lexer.slice().to_string(); self.advance(); n
        } else { return None; };

        if let Some(JutsuToken::Equal) = self.current_token { self.advance(); } else { return None; }

        match self.current_token {
            Some(JutsuToken::Absorb) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }

                let file_path = if let Some(JutsuToken::StringLiteral) = self.current_token {
                    let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
                } else { return None; };

                let mut tier = "bare_metal".to_string();
                let mut temp = 0.0; 
                let mut bind = 1.0; 
                let mut quantize = false; 

                while let Some(JutsuToken::Comma) = self.current_token {
                    self.advance();
                    match self.current_token {
                        Some(JutsuToken::TierBareMetal) => { tier = "bare_metal".to_string(); self.advance(); },
                        Some(JutsuToken::TierVramOnly) => { tier = "vram_only".to_string(); self.advance(); },
                        Some(JutsuToken::Temp) => {
                            self.advance(); 
                            if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
                            if let Some(JutsuToken::Number) = self.current_token {
                                temp = self.lexer.slice().parse::<f32>().unwrap_or(0.0); self.advance();
                            }
                        },
                        Some(JutsuToken::Bind) => {
                            self.advance(); 
                            if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
                            if let Some(JutsuToken::Number) = self.current_token {
                                bind = self.lexer.slice().parse::<f32>().unwrap_or(1.0); self.advance();
                            }
                        },
                        Some(JutsuToken::Quantize) => {
                            self.advance(); 
                            if let Some(JutsuToken::Equal) = self.current_token { self.advance(); }
                            if let Some(JutsuToken::True) = self.current_token {
                                quantize = true; self.advance();
                            } else if let Some(JutsuToken::False) = self.current_token {
                                quantize = false; self.advance();
                            }
                        },
                        _ => break,
                    }
                }
                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }
                Some(Statement::VesselDeclaration { name, file_path, tier, temp, bind, quantize })
            },
            
            Some(JutsuToken::HyperQuad) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                
                let model_ident = if let Some(JutsuToken::StringLiteral) = self.current_token {
                    let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
                } else { return None; };

                if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } else { return None; }
                
                let target = if let Some(JutsuToken::StringLiteral) = self.current_token {
                    let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
                } else { return None; };

                if let Some(JutsuToken::Comma) = self.current_token { self.advance(); } else { return None; }

                let compression = if let Some(JutsuToken::StringLiteral) = self.current_token {
                    let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
                } else { return None; };

                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }

                Some(Statement::HyperQuadDirective { name, model_ident, target, compression })
            },

            Some(JutsuToken::ConnectMcp) => {
                self.advance(); 
                if let Some(JutsuToken::ParenOpen) = self.current_token { self.advance(); } else { return None; }
                
                let url = if let Some(JutsuToken::StringLiteral) = self.current_token {
                    let s = self.lexer.slice().trim_matches('"').to_string(); self.advance(); s
                } else { return None; };

                if let Some(JutsuToken::ParenClose) = self.current_token { self.advance(); } else { return None; }

                Some(Statement::McpClientDeclaration { name, url })
            },
            
            _ => None
        }
    }
}