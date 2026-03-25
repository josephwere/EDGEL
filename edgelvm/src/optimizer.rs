use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::ir::IrProgram;

pub fn optimize_ir(ir: &IrProgram) -> IrProgram {
    let mut functions = ir.functions.clone();
    for function in functions.values_mut() {
        function.body = optimize_block(&function.body);
    }

    IrProgram {
        entry: optimize_block(&ir.entry),
        functions,
    }
}

fn optimize_block(statements: &[Stmt]) -> Vec<Stmt> {
    let mut optimized = Vec::new();
    for statement in statements {
        let lowered = optimize_stmt(statement);
        let stop_after = lowered
            .iter()
            .any(|stmt| matches!(stmt, Stmt::Return { .. }));
        optimized.extend(lowered);
        if stop_after {
            break;
        }
    }
    optimized
}

fn optimize_stmt(statement: &Stmt) -> Vec<Stmt> {
    match statement {
        Stmt::Let { line, name, ty, expr } => vec![Stmt::Let {
            line: *line,
            name: name.clone(),
            ty: ty.clone(),
            expr: fold_expr(expr),
        }],
        Stmt::Print { line, expr } => vec![Stmt::Print {
            line: *line,
            expr: fold_expr(expr),
        }],
        Stmt::Expr { line, expr } => vec![Stmt::Expr {
            line: *line,
            expr: fold_expr(expr),
        }],
        Stmt::If {
            line,
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = fold_expr(condition);
            match literal_truthy(&condition) {
                Some(true) => optimize_block(then_branch),
                Some(false) => optimize_block(else_branch),
                None => vec![Stmt::If {
                    line: *line,
                    condition,
                    then_branch: optimize_block(then_branch),
                    else_branch: optimize_block(else_branch),
                }],
            }
        }
        Stmt::TryCatch {
            line,
            try_branch,
            error_name,
            catch_branch,
        } => vec![Stmt::TryCatch {
            line: *line,
            try_branch: optimize_block(try_branch),
            error_name: error_name.clone(),
            catch_branch: optimize_block(catch_branch),
        }],
        Stmt::ForRange {
            line,
            name,
            start,
            end,
            body,
        } => vec![Stmt::ForRange {
            line: *line,
            name: name.clone(),
            start: fold_expr(start),
            end: fold_expr(end),
            body: optimize_block(body),
        }],
        Stmt::ForEach {
            line,
            name,
            iterable,
            body,
        } => vec![Stmt::ForEach {
            line: *line,
            name: name.clone(),
            iterable: fold_expr(iterable),
            body: optimize_block(body),
        }],
        Stmt::Return { line, expr } => vec![Stmt::Return {
            line: *line,
            expr: expr.as_ref().map(fold_expr),
        }],
        Stmt::Insert { line, table, fields } => vec![Stmt::Insert {
            line: *line,
            table: table.clone(),
            fields: fields
                .iter()
                .map(|(name, expr)| (name.clone(), fold_expr(expr)))
                .collect(),
        }],
        Stmt::Query { line, table, filter } => vec![Stmt::Query {
            line: *line,
            table: table.clone(),
            filter: filter.as_ref().map(fold_expr),
        }],
    }
}

fn fold_expr(expr: &Expr) -> Expr {
    match expr {
        Expr::Binary { left, op, right } => {
            let left = fold_expr(left);
            let right = fold_expr(right);
            fold_binary(left, *op, right)
        }
        Expr::Unary { op, expr } => {
            let expr = fold_expr(expr);
            fold_unary(*op, expr)
        }
        Expr::Call { callee, args } => Expr::Call {
            callee: Box::new(fold_expr(callee)),
            args: args.iter().map(fold_expr).collect(),
        },
        Expr::Property { object, name } => Expr::Property {
            object: Box::new(fold_expr(object)),
            name: name.clone(),
        },
        Expr::List(items) => Expr::List(items.iter().map(fold_expr).collect()),
        Expr::Object(entries) => Expr::Object(
            entries
                .iter()
                .map(|(name, expr)| (name.clone(), fold_expr(expr)))
                .collect(),
        ),
        Expr::Await(expr) => Expr::Await(Box::new(fold_expr(expr))),
        Expr::Group(expr) => {
            let folded = fold_expr(expr);
            match folded {
                Expr::Number(_) | Expr::String(_) | Expr::Bool(_) => folded,
                other => Expr::Group(Box::new(other)),
            }
        }
        other => other.clone(),
    }
}

fn fold_binary(left: Expr, op: BinaryOp, right: Expr) -> Expr {
    match (&left, &right) {
        (Expr::Number(a), Expr::Number(b)) => match op {
            BinaryOp::Add => Expr::Number(a + b),
            BinaryOp::Subtract => Expr::Number(a - b),
            BinaryOp::Multiply => Expr::Number(a * b),
            BinaryOp::Divide => Expr::Number(a / b),
            BinaryOp::Modulo => Expr::Number(a % b),
            BinaryOp::Equal => Expr::Bool((a - b).abs() < f64::EPSILON),
            BinaryOp::NotEqual => Expr::Bool((a - b).abs() >= f64::EPSILON),
            BinaryOp::Greater => Expr::Bool(a > b),
            BinaryOp::GreaterEqual => Expr::Bool(a >= b),
            BinaryOp::Less => Expr::Bool(a < b),
            BinaryOp::LessEqual => Expr::Bool(a <= b),
            BinaryOp::And => Expr::Bool(*a != 0.0 && *b != 0.0),
            BinaryOp::Or => Expr::Bool(*a != 0.0 || *b != 0.0),
        },
        (Expr::String(a), Expr::String(b)) if matches!(op, BinaryOp::Add) => {
            Expr::String(format!("{a}{b}"))
        }
        (Expr::Bool(a), Expr::Bool(b)) => match op {
            BinaryOp::Equal => Expr::Bool(a == b),
            BinaryOp::NotEqual => Expr::Bool(a != b),
            BinaryOp::And => Expr::Bool(*a && *b),
            BinaryOp::Or => Expr::Bool(*a || *b),
            _ => Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
            },
        },
        _ => Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
        },
    }
}

fn fold_unary(op: UnaryOp, expr: Expr) -> Expr {
    match (&op, &expr) {
        (UnaryOp::Negate, Expr::Number(value)) => Expr::Number(-value),
        (UnaryOp::Not, Expr::Bool(value)) => Expr::Bool(!value),
        _ => Expr::Unary {
            op,
            expr: Box::new(expr),
        },
    }
}

fn literal_truthy(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Bool(value) => Some(*value),
        Expr::Number(value) => Some(*value != 0.0),
        Expr::String(value) => Some(!value.is_empty()),
        _ => None,
    }
}
