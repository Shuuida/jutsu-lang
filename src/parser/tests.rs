use super::Parser;
use crate::ast::{Expression, Statement};

#[test]
fn test_parse_let_statement() {
    let input = "let dog = 100";
    let mut parser = Parser::new(input);
    
    let program = parser.parse();

    assert_eq!(program.statements.len(), 1, "There should be exactly 1 instruction");

    match &program.statements[0] {
        Statement::LetStatement { name, value } => {
            assert_eq!(name, "dog", "The variable name should be 'dog'");
            
            match value {
                Expression::NumberLiteral(val) => assert_eq!(*val, 100.0, "The value should be 100"),
                _ => panic!("The value was not parsed as a literal number"),
            }
        }
        _ => panic!("The instruction was not recognized as a LetStatement"),
    }
}

#[test]
fn test_syntax_error_input() {
    let input = "input("; // The parenthesis needs to be closed
    
    let result = std::panic::catch_unwind(|| {
        let mut parser = Parser::new(input);
        parser.parse();
    });
    
    assert!(result.is_err(), "The parser should have triggered a panic due to a syntax error.");
}