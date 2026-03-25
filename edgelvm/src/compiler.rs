use crate::ast::{BinaryOp, Expr, FunctionDecl, Stmt, UnaryOp};
use crate::diagnostics::Diagnostic;
use crate::ir::IrProgram;
use crate::optimizer::optimize_ir;
use crate::value::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    pub entry: Chunk,
    pub functions: BTreeMap<String, FunctionBytecode>,
}

#[derive(Debug, Clone)]
pub struct FunctionBytecode {
    pub is_async: bool,
    pub params: Vec<String>,
    pub chunk: Chunk,
}

#[derive(Debug, Clone, Default)]
pub struct Chunk {
    pub instructions: Vec<Instruction>,
    pub debug_points: BTreeMap<usize, DebugPoint>,
}

#[derive(Debug, Clone)]
pub struct DebugPoint {
    pub line: usize,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Push(Value),
    Load(String),
    Store(String),
    GetProperty(String),
    Binary(BinaryOp),
    Unary(UnaryOp),
    Print,
    Pop,
    Branch {
        then_chunk: Box<Chunk>,
        else_chunk: Box<Chunk>,
    },
    TryCatch {
        error_name: String,
        try_chunk: Box<Chunk>,
        catch_chunk: Box<Chunk>,
    },
    RangeLoop {
        name: String,
        body: Box<Chunk>,
    },
    EachLoop {
        name: String,
        body: Box<Chunk>,
    },
    CallFunction(String, usize),
    CallMethod(String, usize),
    MakeList(usize),
    MakeObject(Vec<String>),
    BuiltinFetch,
    BuiltinNow,
    Insert(String, Vec<String>),
    Query(String),
    Return,
}

pub fn compile(ir: &IrProgram) -> Result<BytecodeProgram, Diagnostic> {
    let optimized = optimize_ir(ir);
    compile_unoptimized(&optimized)
}

pub fn compile_unoptimized(ir: &IrProgram) -> Result<BytecodeProgram, Diagnostic> {
    let mut compiler = Compiler;
    let entry = compiler.compile_chunk(&ir.entry)?;
    let mut functions: BTreeMap<String, FunctionBytecode> = BTreeMap::new();
    for (name, function) in &ir.functions {
        functions.insert(name.clone(), compiler.compile_function(function)?);
    }

    Ok(BytecodeProgram { entry, functions })
}

pub fn serialize_bytecode(program: &BytecodeProgram) -> String {
    let mut lines = vec!["[entry]".to_string()];
    render_chunk(&program.entry, 0, &mut lines);
    for (name, function) in &program.functions {
        lines.push(format!("[function {name}]"));
        render_chunk(&function.chunk, 0, &mut lines);
    }
    lines.join("\n")
}

struct Compiler;

impl Compiler {
    fn compile_function(&mut self, function: &FunctionDecl) -> Result<FunctionBytecode, Diagnostic> {
        Ok(FunctionBytecode {
            is_async: function.is_async,
            params: function.params.iter().map(|param| param.name.clone()).collect(),
            chunk: self.compile_chunk(&function.body)?,
        })
    }

    fn compile_chunk(&mut self, statements: &[Stmt]) -> Result<Chunk, Diagnostic> {
        let mut chunk = Chunk::default();
        for statement in statements {
            chunk.debug_points.insert(
                chunk.instructions.len(),
                DebugPoint {
                    line: statement.line(),
                    summary: statement.summary(),
                },
            );
            self.compile_stmt(statement, &mut chunk)?;
        }
        Ok(chunk)
    }

    fn compile_stmt(&mut self, statement: &Stmt, chunk: &mut Chunk) -> Result<(), Diagnostic> {
        match statement {
            Stmt::Let { name, expr, .. } => {
                self.compile_expr(expr, chunk)?;
                chunk.instructions.push(Instruction::Store(name.clone()));
            }
            Stmt::Print { expr, .. } => {
                self.compile_expr(expr, chunk)?;
                chunk.instructions.push(Instruction::Print);
            }
            Stmt::Expr { expr, .. } => {
                self.compile_expr(expr, chunk)?;
                chunk.instructions.push(Instruction::Pop);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.compile_expr(condition, chunk)?;
                let then_chunk = Box::new(self.compile_chunk(then_branch)?);
                let else_chunk = Box::new(self.compile_chunk(else_branch)?);
                chunk.instructions.push(Instruction::Branch {
                    then_chunk,
                    else_chunk,
                });
            }
            Stmt::TryCatch {
                try_branch,
                error_name,
                catch_branch,
                ..
            } => {
                chunk.instructions.push(Instruction::TryCatch {
                    error_name: error_name.clone(),
                    try_chunk: Box::new(self.compile_chunk(try_branch)?),
                    catch_chunk: Box::new(self.compile_chunk(catch_branch)?),
                });
            }
            Stmt::ForRange {
                name,
                start,
                end,
                body,
                ..
            } => {
                self.compile_expr(start, chunk)?;
                self.compile_expr(end, chunk)?;
                chunk.instructions.push(Instruction::RangeLoop {
                    name: name.clone(),
                    body: Box::new(self.compile_chunk(body)?),
                });
            }
            Stmt::ForEach {
                name,
                iterable,
                body,
                ..
            } => {
                self.compile_expr(iterable, chunk)?;
                chunk.instructions.push(Instruction::EachLoop {
                    name: name.clone(),
                    body: Box::new(self.compile_chunk(body)?),
                });
            }
            Stmt::Return { expr, .. } => {
                if let Some(expr) = expr {
                    self.compile_expr(expr, chunk)?;
                } else {
                    chunk.instructions.push(Instruction::Push(Value::Null));
                }
                chunk.instructions.push(Instruction::Return);
            }
            Stmt::Insert { table, fields, .. } => {
                let mut keys = Vec::new();
                for (key, value) in fields {
                    self.compile_expr(value, chunk)?;
                    keys.push(key.clone());
                }
                chunk
                    .instructions
                    .push(Instruction::Insert(table.clone(), keys));
            }
            Stmt::Query { table, filter, .. } => {
                if let Some(filter) = filter {
                    self.compile_expr(filter, chunk)?;
                } else {
                    chunk.instructions.push(Instruction::Push(Value::Bool(true)));
                }
                chunk.instructions.push(Instruction::Query(table.clone()));
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr, chunk: &mut Chunk) -> Result<(), Diagnostic> {
        match expr {
            Expr::Number(value) => chunk.instructions.push(Instruction::Push(Value::Number(*value))),
            Expr::String(value) => {
                chunk.instructions.push(Instruction::Push(Value::String(value.clone())))
            }
            Expr::Bool(value) => chunk.instructions.push(Instruction::Push(Value::Bool(*value))),
            Expr::Identifier(name) => chunk.instructions.push(Instruction::Load(name.clone())),
            Expr::List(items) => {
                for item in items {
                    self.compile_expr(item, chunk)?;
                }
                chunk.instructions.push(Instruction::MakeList(items.len()));
            }
            Expr::Object(fields) => {
                let mut keys = Vec::new();
                for (key, value) in fields {
                    self.compile_expr(value, chunk)?;
                    keys.push(key.clone());
                }
                chunk.instructions.push(Instruction::MakeObject(keys));
            }
            Expr::Property { object, name } => {
                self.compile_expr(object, chunk)?;
                chunk.instructions.push(Instruction::GetProperty(name.clone()));
            }
            Expr::Call { callee, args } => match callee.as_ref() {
                Expr::Identifier(name) if name == "fetch" => {
                    if args.len() != 1 {
                        return Err(Diagnostic::new("fetch expects one argument", 0, 0));
                    }
                    self.compile_expr(&args[0], chunk)?;
                    chunk.instructions.push(Instruction::BuiltinFetch);
                }
                Expr::Identifier(name) if name == "now" => {
                    chunk.instructions.push(Instruction::BuiltinNow);
                }
                Expr::Identifier(name) => {
                    for arg in args {
                        self.compile_expr(arg, chunk)?;
                    }
                    chunk
                        .instructions
                        .push(Instruction::CallFunction(name.clone(), args.len()));
                }
                Expr::Property { object, name } => {
                    self.compile_expr(object, chunk)?;
                    for arg in args {
                        self.compile_expr(arg, chunk)?;
                    }
                    chunk
                        .instructions
                        .push(Instruction::CallMethod(name.clone(), args.len()));
                }
                _ => return Err(Diagnostic::new("unsupported call target", 0, 0)),
            },
            Expr::Binary { left, op, right } => {
                self.compile_expr(left, chunk)?;
                self.compile_expr(right, chunk)?;
                chunk.instructions.push(Instruction::Binary(*op));
            }
            Expr::Unary { op, expr } => {
                self.compile_expr(expr, chunk)?;
                chunk.instructions.push(Instruction::Unary(*op));
            }
            Expr::Await(expr) => self.compile_expr(expr, chunk)?,
            Expr::Group(expr) => self.compile_expr(expr, chunk)?,
        }
        Ok(())
    }
}

fn render_chunk(chunk: &Chunk, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    for instruction in &chunk.instructions {
        match instruction {
            Instruction::Branch {
                then_chunk,
                else_chunk,
            } => {
                lines.push(format!("{indent}Branch"));
                lines.push(format!("{indent}  [then]"));
                render_chunk(then_chunk, depth + 2, lines);
                lines.push(format!("{indent}  [else]"));
                render_chunk(else_chunk, depth + 2, lines);
            }
            Instruction::TryCatch {
                error_name,
                try_chunk,
                catch_chunk,
            } => {
                lines.push(format!("{indent}TryCatch {error_name}"));
                lines.push(format!("{indent}  [try]"));
                render_chunk(try_chunk, depth + 2, lines);
                lines.push(format!("{indent}  [catch]"));
                render_chunk(catch_chunk, depth + 2, lines);
            }
            Instruction::RangeLoop { name, body } => {
                lines.push(format!("{indent}RangeLoop {name}"));
                render_chunk(body, depth + 1, lines);
            }
            Instruction::EachLoop { name, body } => {
                lines.push(format!("{indent}EachLoop {name}"));
                render_chunk(body, depth + 1, lines);
            }
            other => lines.push(format!("{indent}{other:?}")),
        }
    }
}
