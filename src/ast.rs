#![allow(dead_code)]

#[derive(Debug, Clone)]
pub enum Expression {
    StringLiteral(String),
    NumberLiteral(f32), 
    BooleanLiteral(bool),
    Variable(String),
    InputCall(Box<Expression>), 
    ReadTextCall(String), 
    RagCall { query_var: String, doc_var: String },
    RecvCall, 
    FunctionCall { name: String, args: Vec<Expression> }, 
    InferCall { model_name: String, prompt_var: String, context_var: Option<String>, grammar_var: Option<String> },
    TensorDeclaration { data: Vec<Expression>, shape: Vec<usize>, requires_grad: bool },
    PrefixOp { operator: String, right: Box<Expression> }, 
    InfixOp { left: Box<Expression>, operator: String, right: Box<Expression> },
    Array(Vec<Expression>),
    Dictionary(Vec<(String, Expression)>),
    IndexAccess { left: Box<Expression>, index: Box<Expression> },
    Share { value: Box<Expression> },
    SysExecCall(Box<Expression>),
    HttpGetCall(Box<Expression>),
}

#[derive(Debug, Clone)]
pub enum Statement {
    VesselDeclaration { name: String, file_path: String, tier: String, temp: f32, bind: f32, quantize: bool },
    LetStatement { name: String, value: Expression },
    AssignmentStatement { name: String, value: Expression },    
    PrintStatement { value: Expression },
    ImportStatement { path: String },
    InferStatement { model_name: String, prompt_var: String, context_var: Option<String> },
    ReplyStatement { value: Expression }, 
    IfStatement { condition: Expression, consequence: Vec<Statement>, alternative: Option<Vec<Statement>> },
    WhileStatement { condition: Expression, body: Vec<Statement> }, 
    FunctionDeclaration { name: String, params: Vec<String>, body: Vec<Statement> }, 
    ReturnStatement { value: Expression },
    HexTraceBlock { body: Vec<Statement> },
    BackwardStatement { target_var: String },
    OptimStatement { target_var: String, learning_rate: Expression }, 
    ShieldBlock { max_vram: String, body: Vec<Statement> },
    McpServerBlock { port: Expression, body: Vec<Statement> },
    ExposeToolStatement { name: String, description: String, function_name: String },
    VeilBlock { name: String, port: Expression, body: Vec<Statement> },
    WorkerBlock { body: Vec<Statement> },
    /// Swarm Quantization directive: Moves/Compresses tensors between RAM and VRAM
    HyperQuadDirective { name: String, model_ident: String, target: String, compression: String },
    /// Node to evaluate expressions for their side effects and discard the value
    ExpressionStatement(Expression),
}

#[derive(Debug)]
pub struct Program {
    pub statements: Vec<Statement>,
}