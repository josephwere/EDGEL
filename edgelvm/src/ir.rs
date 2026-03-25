use crate::ast::{FunctionDecl, Item, Parameter, Program, Stmt, TestDecl};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct IrProgram {
    pub entry: Vec<Stmt>,
    pub functions: BTreeMap<String, FunctionDecl>,
}

pub fn lower_to_ir(program: &Program) -> IrProgram {
    let mut entry = Vec::new();
    let mut functions = BTreeMap::new();
    let mut test_index = 0usize;

    for item in &program.items {
        match item {
            Item::Statement(stmt) => entry.push(stmt.clone()),
            Item::Function(function) => {
                functions.insert(function.name.clone(), function.clone());
            }
            Item::Test(test) => {
                let function = lower_test_to_function(test_index, test);
                functions.insert(function.name.clone(), function);
                test_index += 1;
            }
            _ => {}
        }
    }

    IrProgram { entry, functions }
}

pub fn test_function_name(index: usize, name: &str) -> String {
    let slug = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase();
    format!(
        "__test_{}_{}",
        index + 1,
        if slug.is_empty() { "case" } else { &slug }
    )
}

fn lower_test_to_function(index: usize, test: &TestDecl) -> FunctionDecl {
    FunctionDecl {
        is_async: false,
        name: test_function_name(index, &test.name),
        params: Vec::<Parameter>::new(),
        return_type: None,
        body: test.body.clone(),
    }
}
