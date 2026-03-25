#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub enum Item {
    Import(ImportDecl),
    Statement(Stmt),
    Function(FunctionDecl),
    Test(TestDecl),
    App(AppDecl),
    Web(WebDecl),
    Api(ApiDecl),
    Db(DbDecl),
    Table(TableDecl),
    Model(ModelDecl),
    IdVerse(IdVerseDecl),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub ty: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub is_async: bool,
    pub name: String,
    pub params: Vec<Parameter>,
    pub return_type: Option<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct TestDecl {
    pub name: String,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct AppDecl {
    pub name: String,
    pub screens: Vec<ScreenDecl>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct WebDecl {
    pub name: String,
    pub pages: Vec<PageDecl>,
    pub apis: Vec<ApiDecl>,
}

#[derive(Debug, Clone)]
pub struct ScreenDecl {
    pub name: String,
    pub nodes: Vec<UiNode>,
}

#[derive(Debug, Clone)]
pub struct PageDecl {
    pub route: String,
    pub nodes: Vec<UiNode>,
}

#[derive(Debug, Clone)]
pub struct ApiDecl {
    pub route: String,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct DbDecl {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct TableDecl {
    pub name: String,
    pub columns: Vec<TableColumn>,
}

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct ModelDecl {
    pub name: String,
    pub properties: Vec<(String, Expr)>,
}

#[derive(Debug, Clone)]
pub struct IdVerseDecl {
    pub name: String,
    pub fields: Vec<IdVerseField>,
}

#[derive(Debug, Clone)]
pub struct IdVerseField {
    pub name: String,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub enum UiNode {
    Text(Expr),
    Header(Expr),
    Paragraph(Expr),
    Input {
        name: String,
        prompt: Option<Expr>,
        input_type: Option<String>,
    },
    Button {
        label: Expr,
        actions: Vec<Stmt>,
    },
    Scene {
        commands: Vec<SceneCommand>,
    },
}

#[derive(Debug, Clone)]
pub struct SceneCommand {
    pub name: String,
    pub label: Option<String>,
    pub args: Vec<Expr>,
    pub children: Vec<SceneCommand>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        line: usize,
        name: String,
        ty: Option<String>,
        expr: Expr,
    },
    Print {
        line: usize,
        expr: Expr,
    },
    Expr {
        line: usize,
        expr: Expr,
    },
    If {
        line: usize,
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Vec<Stmt>,
    },
    ForRange {
        line: usize,
        name: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    TryCatch {
        line: usize,
        try_branch: Vec<Stmt>,
        error_name: String,
        catch_branch: Vec<Stmt>,
    },
    ForEach {
        line: usize,
        name: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    Return {
        line: usize,
        expr: Option<Expr>,
    },
    Insert {
        line: usize,
        table: String,
        fields: Vec<(String, Expr)>,
    },
    Query {
        line: usize,
        table: String,
        filter: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    String(String),
    Bool(bool),
    Identifier(String),
    List(Vec<Expr>),
    Object(Vec<(String, Expr)>),
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Property {
        object: Box<Expr>,
        name: String,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Await(Box<Expr>),
    Group(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

impl Stmt {
    pub fn line(&self) -> usize {
        match self {
            Stmt::Let { line, .. }
            | Stmt::Print { line, .. }
            | Stmt::Expr { line, .. }
            | Stmt::If { line, .. }
            | Stmt::ForRange { line, .. }
            | Stmt::TryCatch { line, .. }
            | Stmt::ForEach { line, .. }
            | Stmt::Return { line, .. }
            | Stmt::Insert { line, .. }
            | Stmt::Query { line, .. } => *line,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Stmt::Let { name, .. } => format!("let {name} = ..."),
            Stmt::Print { .. } => "print(...)".to_string(),
            Stmt::Expr { expr, .. } => match expr {
                Expr::Call { callee, .. } => format!("{}(...)", expr_label(callee)),
                Expr::Identifier(name) => name.clone(),
                _ => "expression".to_string(),
            },
            Stmt::If { .. } => "if ...".to_string(),
            Stmt::ForRange { name, .. } => format!("for {name} in start..end"),
            Stmt::TryCatch { .. } => "try/catch".to_string(),
            Stmt::ForEach { name, .. } => format!("for {name} in iterable"),
            Stmt::Return { .. } => "return".to_string(),
            Stmt::Insert { table, .. } => format!("insert {table}"),
            Stmt::Query { table, .. } => format!("query {table}"),
        }
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Identifier(name) => name.clone(),
        Expr::Property { object, name } => format!("{}.{}", expr_label(object), name),
        _ => "call".to_string(),
    }
}
