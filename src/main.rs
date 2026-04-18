mod lexer;
mod ast;
mod parser;
mod evaluator;
mod cli;
mod tgn_pm;
mod memory;
mod inference;
mod grammar;

#[tokio::main]
async fn main() {
    cli::execute().await;
}