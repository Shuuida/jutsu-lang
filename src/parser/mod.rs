pub mod expressions;
pub mod statements;
pub mod system;
pub mod ai;

#[cfg(test)]
mod tests;

use crate::lexer::JutsuToken;
use crate::ast::{Program, Statement};
use logos::{Lexer, Logos};

/// The `Parser` struct maintains the state of the parsing process.
pub struct Parser<'a> {
    // 'Super' visibility so that submodules can read and advance tokens
    pub(super) lexer: Lexer<'a, JutsuToken>,
    pub(super) current_token: Option<JutsuToken>,
}

impl<'a> Parser<'a> {
    /// Initializes the parser, priming it with the very first token.
    pub fn new(source: &'a str) -> Self {
        let mut lexer = JutsuToken::lexer(source);
        let current_token = loop {
            match lexer.next() {
                Some(Ok(token)) => break Some(token),
                Some(Err(_)) => continue, // Silently skip lexical errors for now
                None => break None,
            }
        };
        Parser { lexer, current_token }
    }

    pub(super) fn advance(&mut self) {
        self.current_token = loop {
            match self.lexer.next() {
                Some(Ok(token)) => break Some(token),
                Some(Err(_)) => continue,
                None => break None,
            }
        };
    }

    /// The main entry point. Parses the entire source code file into a `Program` AST node.
    pub fn parse(&mut self) -> Program {
        let mut statements = Vec::new();
        while self.current_token.is_some() {
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            } else { 
                panic!("[Syntax Error] Unrecognized token or invalid syntax near: '{}'", self.lexer.slice());
            }
        }
        Program { statements }
    }

    /// Statement Router: Looks at the current token and delegates to the specific parsing function.
    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_token {
            Some(JutsuToken::Vessel) => self.parse_vessel_declaration(),
            Some(JutsuToken::Let) => self.parse_let_statement(),
            Some(JutsuToken::Print) => self.parse_print_statement(),
            Some(JutsuToken::Import) => self.parse_import_statement(),
            Some(JutsuToken::If) => self.parse_if_statement(),
            Some(JutsuToken::While) => self.parse_while_statement(),
            Some(JutsuToken::Def) => self.parse_function_declaration(), 
            Some(JutsuToken::Return) => self.parse_return_statement(),
            Some(JutsuToken::HexTrace) => self.parse_hextrace_block(),
            Some(JutsuToken::Backward) => self.parse_backward_statement(),
            Some(JutsuToken::Optim) => self.parse_optim_statement(),
            Some(JutsuToken::Reply) => self.parse_reply_statement(), 
            Some(JutsuToken::Identifier) => self.parse_identifier_driven_statement(),
            Some(JutsuToken::Shield) => self.parse_shield_block(),
            Some(JutsuToken::McpServer) => self.parse_mcp_server_block(),
            Some(JutsuToken::ExposeTool) => self.parse_expose_tool_statement(),
            Some(JutsuToken::Veil) => self.parse_veil_block(),
            Some(JutsuToken::Worker) => self.parse_worker_block(),
            _ => {
                // If it doesn't start with let, if, etc., we try to read the line as an expression
                if let Some(expr) = self.parse_expression() {
                    Some(Statement::ExpressionStatement(expr))
                } else {
                    None
                }
            }
        }
    }
}