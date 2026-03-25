use crate::ast::{BinaryOp, UnaryOp};
use crate::compiler::{BytecodeProgram, Chunk, DebugPoint, FunctionBytecode, Instruction};
use crate::diagnostics::Diagnostic;
use crate::ir::lower_to_ir;
use crate::lexer::{lex, TokenKind};
use crate::neuroedge;
use crate::parser::parse;
use crate::value::Value;
use std::collections::BTreeMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct VmOptions {
    pub debug: bool,
    pub profile: bool,
    pub trace: bool,
    pub max_instructions: Option<u64>,
    pub breakpoints: Vec<DebugBreakpoint>,
}

#[derive(Debug, Clone, Default)]
pub struct VmProfile {
    pub instruction_count: u64,
    pub function_calls: u64,
    pub builtin_calls: u64,
    pub caught_errors: u64,
    pub max_stack_depth: usize,
    pub elapsed_ms: u128,
    pub function_hits: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
pub struct VmOutput {
    pub console: Vec<String>,
    pub globals: BTreeMap<String, Value>,
    pub database: BTreeMap<String, Vec<BTreeMap<String, Value>>>,
    pub profile: Option<VmProfile>,
    pub trace: Vec<String>,
    pub debug: Option<DebugRecord>,
}

#[derive(Debug, Clone)]
pub enum DebugBreakpoint {
    Line(usize),
    Function(String),
}

#[derive(Debug, Clone)]
pub struct DebugFrame {
    pub function: String,
    pub line: usize,
    pub summary: String,
    pub locals: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct DebugSnapshot {
    pub index: usize,
    pub line: usize,
    pub summary: String,
    pub instruction: String,
    pub stack: Vec<String>,
    pub globals: BTreeMap<String, Value>,
    pub frames: Vec<DebugFrame>,
    pub pause_reason: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DebugRecord {
    pub snapshots: Vec<DebugSnapshot>,
}

#[derive(Debug, Clone, Copy)]
pub enum DebugAction {
    StepInto,
    StepOver,
    StepOut,
    Continue,
}

pub fn execute(program: &BytecodeProgram) -> Result<VmOutput, Diagnostic> {
    execute_with_options(program, VmOptions::default())
}

pub fn execute_with_options(
    program: &BytecodeProgram,
    options: VmOptions,
) -> Result<VmOutput, Diagnostic> {
    let (output, _) = execute_internal(program, None, Vec::new(), options)?;
    Ok(output)
}

pub fn execute_function_with_options(
    program: &BytecodeProgram,
    function_name: &str,
    args: Vec<Value>,
    options: VmOptions,
) -> Result<(VmOutput, Value), Diagnostic> {
    execute_internal(program, Some(function_name), args, options)
}

pub fn debug_step_index(record: &DebugRecord, current: usize, action: DebugAction) -> usize {
    if record.snapshots.is_empty() {
        return 0;
    }
    let current = current.min(record.snapshots.len().saturating_sub(1));
    let current_snapshot = &record.snapshots[current];
    match action {
        DebugAction::StepInto => (current + 1).min(record.snapshots.len().saturating_sub(1)),
        DebugAction::StepOver => {
            let depth = current_snapshot.frames.len();
            let line = current_snapshot.line;
            record
                .snapshots
                .iter()
                .enumerate()
                .skip(current + 1)
                .find(|(_, snapshot)| {
                    snapshot.frames.len() < depth
                        || (snapshot.frames.len() == depth
                            && (snapshot.line != line
                                || snapshot.summary != current_snapshot.summary))
                })
                .map(|(index, _)| index)
                .unwrap_or(record.snapshots.len().saturating_sub(1))
        }
        DebugAction::StepOut => {
            let depth = current_snapshot.frames.len();
            record
                .snapshots
                .iter()
                .enumerate()
                .skip(current + 1)
                .find(|(_, snapshot)| snapshot.frames.len() < depth)
                .map(|(index, _)| index)
                .unwrap_or(record.snapshots.len().saturating_sub(1))
        }
        DebugAction::Continue => record
            .snapshots
            .iter()
            .enumerate()
            .skip(current + 1)
            .find(|(_, snapshot)| snapshot.pause_reason.is_some())
            .map(|(index, _)| index)
            .unwrap_or(record.snapshots.len().saturating_sub(1)),
    }
}

pub fn inspect_debug_snapshot(snapshot: &DebugSnapshot, expr: &str, frame: usize) -> Value {
    if expr.trim().is_empty() {
        return Value::Object(snapshot.globals.clone());
    }
    let mut segments = expr
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty());
    let Some(root) = segments.next() else {
        return Value::Null;
    };

    let mut value = snapshot
        .frames
        .get(frame)
        .and_then(|frame| frame.locals.get(root).cloned())
        .or_else(|| snapshot.globals.get(root).cloned())
        .or_else(|| builtin_placeholder(root))
        .unwrap_or(Value::Null);

    for segment in segments {
        value = match value {
            Value::Object(map) => map.get(segment).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        };
    }
    value
}

fn execute_internal(
    program: &BytecodeProgram,
    function_name: Option<&str>,
    args: Vec<Value>,
    options: VmOptions,
) -> Result<(VmOutput, Value), Diagnostic> {
    let started = Instant::now();
    let mut vm = VirtualMachine::new(program, options);
    vm.execute_entry().map_err(|error| vm.enrich_error(error))?;
    let return_value = if let Some(function_name) = function_name {
        vm.call_function(function_name, args)
            .map_err(|error| vm.enrich_error(error))?
    } else {
        Value::Null
    };
    let mut profile = vm.profile.clone();
    profile.elapsed_ms = started.elapsed().as_millis();

    if vm.options.debug {
        if vm.debug_record.snapshots.is_empty() {
            vm.debug_record.snapshots.push(DebugSnapshot {
                index: 0,
                line: 0,
                summary: "no executable statements".to_string(),
                instruction: "Idle".to_string(),
                stack: Vec::new(),
                globals: vm.globals.clone(),
                frames: vec![DebugFrame {
                    function: "<main>".to_string(),
                    line: 0,
                    summary: "no executable statements".to_string(),
                    locals: BTreeMap::new(),
                }],
                pause_reason: None,
            });
        }
        vm.console.push(format!(
            "[debug] instructions={}, functions={}, builtins={}, max_stack={}, caught_errors={}, elapsed_ms={}",
            profile.instruction_count,
            profile.function_calls,
            profile.builtin_calls,
            profile.max_stack_depth,
            profile.caught_errors,
            profile.elapsed_ms
        ));
    }

    Ok((
        VmOutput {
            console: vm.console,
            globals: vm.globals,
            database: vm.database,
            profile: vm.options.profile.then_some(profile),
            trace: vm.trace_log,
            debug: vm.options.debug.then_some(vm.debug_record),
        },
        return_value,
    ))
}

struct VirtualMachine<'a> {
    program: &'a BytecodeProgram,
    options: VmOptions,
    stack: Vec<Value>,
    globals: BTreeMap<String, Value>,
    console: Vec<String>,
    database: BTreeMap<String, Vec<BTreeMap<String, Value>>>,
    profile: VmProfile,
    trace_log: Vec<String>,
    call_stack: Vec<String>,
    frame_locals: Vec<BTreeMap<String, Value>>,
    frame_locations: Vec<Option<DebugPoint>>,
    debug_record: DebugRecord,
}

impl<'a> VirtualMachine<'a> {
    fn new(program: &'a BytecodeProgram, options: VmOptions) -> Self {
        Self {
            program,
            options,
            stack: Vec::with_capacity(64),
            globals: BTreeMap::new(),
            console: Vec::new(),
            database: BTreeMap::new(),
            profile: VmProfile::default(),
            trace_log: Vec::new(),
            call_stack: Vec::new(),
            frame_locals: Vec::new(),
            frame_locations: Vec::new(),
            debug_record: DebugRecord::default(),
        }
    }

    fn execute_entry(&mut self) -> Result<(), Diagnostic> {
        let mut locals = BTreeMap::new();
        self.call_stack.push("<main>".to_string());
        self.frame_locals.push(locals.clone());
        self.frame_locations.push(None);
        let result = self.execute_chunk(&self.program.entry, &mut locals, true).map(|_| ());
        self.call_stack.pop();
        self.frame_locals.pop();
        self.frame_locations.pop();
        result
    }

    fn execute_chunk(
        &mut self,
        chunk: &Chunk,
        locals: &mut BTreeMap<String, Value>,
        is_global_scope: bool,
    ) -> Result<Option<Value>, Diagnostic> {
        for (ip, instruction) in chunk.instructions.iter().enumerate() {
            self.profile.instruction_count += 1;
            if let Some(limit) = self.options.max_instructions {
                if self.profile.instruction_count > limit {
                    return Err(Diagnostic::new(
                        format!("instruction limit exceeded ({limit})"),
                        0,
                        0,
                    )
                    .with_context("vm"));
                }
            }
            if let Some(location) = chunk.debug_points.get(&ip) {
                if let Some(current) = self.frame_locations.last_mut() {
                    *current = Some(location.clone());
                }
            }
            let pause_reason = self.pause_reason(chunk, ip, instruction);
            match instruction {
                Instruction::Push(value) => self.push(value.clone()),
                Instruction::Load(name) => {
                    let value = locals
                        .get(name)
                        .cloned()
                        .or_else(|| self.globals.get(name).cloned())
                        .or_else(|| builtin_placeholder(name))
                        .unwrap_or(Value::Null);
                    self.push(value);
                }
                Instruction::Store(name) => {
                    let value = self.pop()?;
                    if is_global_scope {
                        self.globals.insert(name.clone(), value);
                    } else {
                        locals.insert(name.clone(), value);
                    }
                }
                Instruction::GetProperty(property) => {
                    let object = self.pop()?;
                    let value = match object {
                        Value::Object(map) => map.get(property).cloned().unwrap_or(Value::Null),
                        _ => Value::Null,
                    };
                    self.push(value);
                }
                Instruction::Binary(op) => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    self.push(eval_binary(*op, left, right)?);
                }
                Instruction::Unary(op) => {
                    let value = self.pop()?;
                    self.push(eval_unary(*op, value)?);
                }
                Instruction::Print => {
                    let value = self.pop()?;
                    self.console.push(value.to_string());
                }
                Instruction::Pop => {
                    let _ = self.pop()?;
                }
                Instruction::Branch {
                    then_chunk,
                    else_chunk,
                } => {
                    let condition = self.pop()?;
                    let chosen = if condition.truthy() {
                        then_chunk
                    } else {
                        else_chunk
                    };
                    if let Some(value) = self.execute_chunk(chosen, locals, is_global_scope)? {
                        return Ok(Some(value));
                    }
                }
                Instruction::TryCatch {
                    error_name,
                    try_chunk,
                    catch_chunk,
                } => match self.execute_chunk(try_chunk, locals, is_global_scope) {
                    Ok(Some(value)) => return Ok(Some(value)),
                    Ok(None) => {}
                    Err(error) => {
                        self.profile.caught_errors += 1;
                        locals.insert(error_name.clone(), diagnostic_to_value(&error));
                        if let Some(value) =
                            self.execute_chunk(catch_chunk, locals, is_global_scope)?
                        {
                            return Ok(Some(value));
                        }
                    }
                },
                Instruction::RangeLoop { name, body } => {
                    let end = self.pop()?;
                    let start = self.pop()?;
                    let start = start.as_f64().unwrap_or(0.0) as i64;
                    let end = end.as_f64().unwrap_or(0.0) as i64;
                    for value in start..=end {
                        locals.insert(name.clone(), Value::Number(value as f64));
                        if let Some(result) = self.execute_chunk(body, locals, is_global_scope)? {
                            return Ok(Some(result));
                        }
                    }
                }
                Instruction::EachLoop { name, body } => {
                    let iterable = self.pop()?;
                    if let Value::List(items) = iterable {
                        for item in items {
                            locals.insert(name.clone(), item);
                            if let Some(result) = self.execute_chunk(body, locals, is_global_scope)? {
                                return Ok(Some(result));
                            }
                        }
                    }
                }
                Instruction::CallFunction(name, argc) => {
                    let mut args = self.take_args(*argc)?;
                    args.reverse();
                    let value = self.call_function(name, args)?;
                    self.push(value);
                }
                Instruction::CallMethod(method, argc) => {
                    let mut args = self.take_args(*argc)?;
                    args.reverse();
                    let object = self.pop()?;
                    let result = self.call_method(object, method, args)?;
                    self.push(result);
                }
                Instruction::MakeList(count) => {
                    let mut items = self.take_args(*count)?;
                    items.reverse();
                    self.push(Value::List(items));
                }
                Instruction::MakeObject(keys) => {
                    let mut values = self.take_args(keys.len())?;
                    values.reverse();
                    let map = keys
                        .iter()
                        .cloned()
                        .zip(values.into_iter())
                        .collect::<BTreeMap<_, _>>();
                    self.push(Value::Object(map));
                }
                Instruction::BuiltinFetch => {
                    self.profile.builtin_calls += 1;
                    let url = self.pop()?.to_string();
                    let mut response = BTreeMap::new();
                    response.insert("source".to_string(), Value::String("mock-fetch".to_string()));
                    response.insert("url".to_string(), Value::String(url));
                    response.insert(
                        "message".to_string(),
                        Value::String("Offline EDGEL sandbox response".to_string()),
                    );
                    self.push(Value::Object(response));
                }
                Instruction::BuiltinNow => {
                    self.profile.builtin_calls += 1;
                    let now = std::env::var("EDGEL_DETERMINISTIC_NOW").unwrap_or_else(|_| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                            .to_string()
                    });
                    self.push(Value::String(now));
                }
                Instruction::Insert(table, keys) => {
                    let mut values = self.take_args(keys.len())?;
                    values.reverse();
                    let row = keys
                        .iter()
                        .cloned()
                        .zip(values.into_iter())
                        .collect::<BTreeMap<_, _>>();
                    self.database.entry(table.clone()).or_default().push(row);
                    self.push(Value::Null);
                }
                Instruction::Query(table) => {
                    let filter = self.pop()?;
                    let rows = self.database.get(table).cloned().unwrap_or_default();
                    let mut result = Vec::new();
                    for row in rows {
                        if filter.truthy() {
                            result.push(Value::Object(row));
                        }
                    }
                    self.push(Value::List(result));
                }
                Instruction::Return => {
                    let value = self.pop()?;
                    self.sync_current_frame(locals);
                    self.record_snapshot(instruction, locals, pause_reason);
                    return Ok(Some(value));
                }
            }
            self.sync_current_frame(locals);
            self.record_snapshot(instruction, locals, pause_reason);
        }
        Ok(None)
    }

    fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value, Diagnostic> {
        let Some(function) = self.program.functions.get(name) else {
            return match name {
                "alert" => {
                    self.profile.builtin_calls += 1;
                    if let Some(value) = args.first() {
                        self.console.push(format!("ALERT: {value}"));
                    }
                    Ok(Value::Null)
                }
                "navigate" => {
                    self.profile.builtin_calls += 1;
                    Ok(args
                        .first()
                        .cloned()
                        .unwrap_or(Value::String("Main".to_string())))
                }
                "assert" => {
                    self.profile.builtin_calls += 1;
                    let condition = args.first().map(Value::truthy).unwrap_or(false);
                    if condition {
                        Ok(Value::Null)
                    } else {
                        let message = args
                            .get(1)
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "Assertion failed".to_string());
                        Err(Diagnostic::new(message, 0, 0).with_context("assert"))
                    }
                }
                "breakpoint" => {
                    self.profile.builtin_calls += 1;
                    Ok(Value::Null)
                }
                "coreCompilerTokens" => {
                    self.profile.builtin_calls += 1;
                    Ok(Value::String(compiler_tokens(args.first())?))
                }
                "coreCompilerIr" => {
                    self.profile.builtin_calls += 1;
                    Ok(Value::String(compiler_ir(args.first())?))
                }
                "coreCompilerBytecode" => {
                    self.profile.builtin_calls += 1;
                    Ok(Value::String(compiler_bytecode(args.first())?))
                }
                _ => Ok(Value::Null),
            };
        };
        self.invoke_function(name, function, args)
    }

    fn invoke_function(
        &mut self,
        name: &str,
        function: &FunctionBytecode,
        args: Vec<Value>,
    ) -> Result<Value, Diagnostic> {
        self.profile.function_calls += 1;
        *self.profile.function_hits.entry(name.to_string()).or_insert(0) += 1;
        let mut locals = BTreeMap::new();
        for (name, value) in function.params.iter().cloned().zip(args.into_iter()) {
            locals.insert(name, value);
        }
        let _ = function.is_async;
        self.call_stack.push(name.to_string());
        self.frame_locals.push(locals.clone());
        self.frame_locations.push(None);
        let result = self
            .execute_chunk(&function.chunk, &mut locals, false)
            .map(|value| value.unwrap_or(Value::Null));
        self.call_stack.pop();
        self.frame_locals.pop();
        self.frame_locations.pop();
        result
    }

    fn call_method(
        &mut self,
        object: Value,
        method: &str,
        args: Vec<Value>,
    ) -> Result<Value, Diagnostic> {
        match method {
            "generateLesson" | "createApp" | "ask" => {
                let prompt = args
                    .first()
                    .cloned()
                    .unwrap_or(Value::String("Untitled".to_string()))
                    .to_string();
                Ok(Value::String(neuroedge::assist_action(method, &prompt)))
            }
            _ => Ok(match object {
                Value::Object(mut map) => map.remove(method).unwrap_or(Value::Null),
                _ => Value::Null,
            }),
        }
    }

    fn take_args(&mut self, count: usize) -> Result<Vec<Value>, Diagnostic> {
        if self.stack.len() < count {
            return Err(Diagnostic::new("stack underflow", 0, 0).with_context("vm"));
        }
        let split_at = self.stack.len() - count;
        let mut args = self.stack.split_off(split_at);
        args.reverse();
        Ok(args)
    }

    fn push(&mut self, value: Value) {
        self.stack.push(value);
        self.profile.max_stack_depth = self.profile.max_stack_depth.max(self.stack.len());
    }

    fn pop(&mut self) -> Result<Value, Diagnostic> {
        self.stack
            .pop()
            .ok_or_else(|| Diagnostic::new("stack underflow", 0, 0))
    }

    fn sync_current_frame(&mut self, locals: &BTreeMap<String, Value>) {
        if let Some(frame) = self.frame_locals.last_mut() {
            *frame = locals.clone();
        }
    }

    fn pause_reason(
        &self,
        chunk: &Chunk,
        ip: usize,
        instruction: &Instruction,
    ) -> Option<String> {
        if !self.options.debug {
            return None;
        }
        if matches!(instruction, Instruction::CallFunction(name, _) if name == "breakpoint") {
            return Some("breakpoint()".to_string());
        }
        if ip == 0 {
            if let Some(function) = self.call_stack.last() {
                for breakpoint in &self.options.breakpoints {
                    if matches!(breakpoint, DebugBreakpoint::Function(name) if name == function) {
                        return Some(format!("function breakpoint `{function}`"));
                    }
                }
            }
        }
        if let Some(location) = chunk.debug_points.get(&ip) {
            for breakpoint in &self.options.breakpoints {
                if matches!(breakpoint, DebugBreakpoint::Line(line) if *line == location.line) {
                    return Some(format!("line breakpoint {}", location.line));
                }
            }
        }
        None
    }

    fn current_location(&self) -> Option<&DebugPoint> {
        self.frame_locations.last().and_then(|location| location.as_ref())
    }

    fn record_snapshot(
        &mut self,
        instruction: &Instruction,
        locals: &BTreeMap<String, Value>,
        pause_reason: Option<String>,
    ) {
        if !(self.options.trace || self.options.debug) {
            return;
        }

        let stack_preview = self
            .stack
            .iter()
            .rev()
            .take(3)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let locals_preview = locals
            .iter()
            .take(4)
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(", ");
        let location = self.current_location();
        let line = location.map(|value| value.line).unwrap_or(0);
        let summary = location
            .map(|value| value.summary.clone())
            .unwrap_or_else(|| instruction_label(instruction));

        let trace_line = if line > 0 {
            format!(
                "[DEBUG] Line {} -> {} | op={instruction:?} | stack=[{}] | vars: {}",
                line,
                summary,
                stack_preview,
                if locals_preview.is_empty() {
                    "<none>".to_string()
                } else {
                    locals_preview.clone()
                }
            )
        } else {
            format!(
                "[DEBUG] {} | op={instruction:?} | stack=[{}] | vars: {}",
                summary,
                stack_preview,
                if locals_preview.is_empty() {
                    "<none>".to_string()
                } else {
                    locals_preview.clone()
                }
            )
        };
        self.trace_log.push(trace_line);

        if self.options.debug {
            let frames = self
                .call_stack
                .iter()
                .zip(self.frame_locals.iter())
                .zip(self.frame_locations.iter())
                .rev()
                .map(|((function, frame_locals), location)| DebugFrame {
                    function: function.clone(),
                    line: location.as_ref().map(|value| value.line).unwrap_or(0),
                    summary: location
                        .as_ref()
                        .map(|value| value.summary.clone())
                        .unwrap_or_else(|| function.clone()),
                    locals: frame_locals.clone(),
                })
                .collect::<Vec<_>>();
            self.debug_record.snapshots.push(DebugSnapshot {
                index: self.debug_record.snapshots.len(),
                line,
                summary,
                instruction: format!("{instruction:?}"),
                stack: self
                    .stack
                    .iter()
                    .rev()
                    .take(8)
                    .map(ToString::to_string)
                    .collect(),
                globals: self.globals.clone(),
                frames,
                pause_reason,
            });
        }
    }

    fn enrich_error(&self, mut error: Diagnostic) -> Diagnostic {
        for frame in self.call_stack.iter().rev() {
            error = error.with_stack_frame(frame.clone());
        }
        if !self.trace_log.is_empty() {
            let tail = self
                .trace_log
                .iter()
                .rev()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" || ");
            error = error.with_note(format!("recent trace: {tail}"));
        }
        if !self.globals.is_empty() {
            let globals = self
                .globals
                .iter()
                .take(5)
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", ");
            error = error.with_note(format!("globals: {globals}"));
        }
        error
    }
}

fn builtin_placeholder(name: &str) -> Option<Value> {
    match name {
        "user" => {
            let mut user = BTreeMap::new();
            user.insert("name".to_string(), Value::String("EDGEL User".to_string()));
            user.insert("id".to_string(), Value::String("idv-local-001".to_string()));
            Some(Value::Object(user))
        }
        _ => None,
    }
}

fn instruction_label(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Push(_) => "push value".to_string(),
        Instruction::Load(name) => format!("load {name}"),
        Instruction::Store(name) => format!("store {name}"),
        Instruction::GetProperty(name) => format!("get property {name}"),
        Instruction::Binary(_) => "binary operation".to_string(),
        Instruction::Unary(_) => "unary operation".to_string(),
        Instruction::Print => "print".to_string(),
        Instruction::Pop => "discard value".to_string(),
        Instruction::Branch { .. } => "branch".to_string(),
        Instruction::TryCatch { .. } => "try/catch".to_string(),
        Instruction::RangeLoop { name, .. } => format!("range loop {name}"),
        Instruction::EachLoop { name, .. } => format!("each loop {name}"),
        Instruction::CallFunction(name, _) => format!("call {name}"),
        Instruction::CallMethod(name, _) => format!("call method {name}"),
        Instruction::MakeList(_) => "make list".to_string(),
        Instruction::MakeObject(_) => "make object".to_string(),
        Instruction::BuiltinFetch => "fetch".to_string(),
        Instruction::BuiltinNow => "now".to_string(),
        Instruction::Insert(table, _) => format!("insert {table}"),
        Instruction::Query(table) => format!("query {table}"),
        Instruction::Return => "return".to_string(),
    }
}

fn diagnostic_to_value(diagnostic: &Diagnostic) -> Value {
    let mut error = BTreeMap::new();
    error.insert(
        "message".to_string(),
        Value::String(diagnostic.message.clone()),
    );
    error.insert("line".to_string(), Value::Number(diagnostic.line as f64));
    error.insert(
        "column".to_string(),
        Value::Number(diagnostic.column as f64),
    );
    if let Some(context) = &diagnostic.context {
        error.insert("context".to_string(), Value::String(context.clone()));
    }
    if !diagnostic.notes.is_empty() {
        error.insert(
            "notes".to_string(),
            Value::List(
                diagnostic
                    .notes
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if !diagnostic.stack.is_empty() {
        error.insert(
            "stack".to_string(),
            Value::List(
                diagnostic
                    .stack
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if !diagnostic.related.is_empty() {
        error.insert(
            "related".to_string(),
            Value::List(
                diagnostic
                    .related
                    .iter()
                    .map(diagnostic_to_value)
                    .collect(),
            ),
        );
    }
    Value::Object(error)
}

fn eval_unary(op: UnaryOp, value: Value) -> Result<Value, Diagnostic> {
    Ok(match op {
        UnaryOp::Negate => Value::Number(-value.as_f64().unwrap_or(0.0)),
        UnaryOp::Not => Value::Bool(!value.truthy()),
    })
}

fn eval_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, Diagnostic> {
    Ok(match op {
        BinaryOp::Add => match (&left, &right) {
            (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
            _ => Value::String(format!("{left}{right}")),
        },
        BinaryOp::Subtract => {
            Value::Number(left.as_f64().unwrap_or(0.0) - right.as_f64().unwrap_or(0.0))
        }
        BinaryOp::Multiply => {
            Value::Number(left.as_f64().unwrap_or(0.0) * right.as_f64().unwrap_or(0.0))
        }
        BinaryOp::Divide => {
            Value::Number(left.as_f64().unwrap_or(0.0) / right.as_f64().unwrap_or(1.0))
        }
        BinaryOp::Modulo => {
            Value::Number(left.as_f64().unwrap_or(0.0) % right.as_f64().unwrap_or(1.0))
        }
        BinaryOp::Equal => Value::Bool(left == right),
        BinaryOp::NotEqual => Value::Bool(left != right),
        BinaryOp::Greater => {
            Value::Bool(left.as_f64().unwrap_or(0.0) > right.as_f64().unwrap_or(0.0))
        }
        BinaryOp::GreaterEqual => {
            Value::Bool(left.as_f64().unwrap_or(0.0) >= right.as_f64().unwrap_or(0.0))
        }
        BinaryOp::Less => Value::Bool(left.as_f64().unwrap_or(0.0) < right.as_f64().unwrap_or(0.0)),
        BinaryOp::LessEqual => {
            Value::Bool(left.as_f64().unwrap_or(0.0) <= right.as_f64().unwrap_or(0.0))
        }
        BinaryOp::And => Value::Bool(left.truthy() && right.truthy()),
        BinaryOp::Or => Value::Bool(left.truthy() || right.truthy()),
    })
}

fn compiler_source(value: Option<&Value>) -> Result<String, Diagnostic> {
    match value {
        Some(Value::String(source)) => Ok(source.clone()),
        Some(other) => Ok(other.to_string()),
        None => Err(Diagnostic::new("compiler bridge expects source text", 0, 0)
            .with_context("compiler")),
    }
}

fn compiler_tokens(value: Option<&Value>) -> Result<String, Diagnostic> {
    let source = compiler_source(value)?;
    let tokens = lex(&source)?;
    let rendered = tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof))
        .map(|token| token_name(&token.kind))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!("tokens[{rendered}]"))
}

fn compiler_ir(value: Option<&Value>) -> Result<String, Diagnostic> {
    let source = compiler_source(value)?;
    let program = parse(&lex(&source)?)?;
    let ir = lower_to_ir(&program);
    Ok(format!(
        "ir entry={} functions={}",
        ir.entry.len(),
        ir.functions.len()
    ))
}

fn compiler_bytecode(value: Option<&Value>) -> Result<String, Diagnostic> {
    let source = compiler_source(value)?;
    let program = parse(&lex(&source)?)?;
    let bytecode = crate::compile_program(&program)?;
    Ok(crate::compiler::serialize_bytecode(&bytecode))
}

fn token_name(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Keyword(keyword) => format!("keyword:{keyword:?}"),
        TokenKind::Identifier(value) => format!("identifier:{value}"),
        TokenKind::String(value) => format!("string:{value}"),
        TokenKind::Number(value) => format!("number:{value}"),
        TokenKind::LBrace => "{".to_string(),
        TokenKind::RBrace => "}".to_string(),
        TokenKind::LParen => "(".to_string(),
        TokenKind::RParen => ")".to_string(),
        TokenKind::LBracket => "[".to_string(),
        TokenKind::RBracket => "]".to_string(),
        TokenKind::Comma => ",".to_string(),
        TokenKind::Colon => ":".to_string(),
        TokenKind::Dot => ".".to_string(),
        TokenKind::Plus => "+".to_string(),
        TokenKind::Minus => "-".to_string(),
        TokenKind::Star => "*".to_string(),
        TokenKind::Slash => "/".to_string(),
        TokenKind::Percent => "%".to_string(),
        TokenKind::Eq => "=".to_string(),
        TokenKind::EqEq => "==".to_string(),
        TokenKind::Bang => "!".to_string(),
        TokenKind::BangEq => "!=".to_string(),
        TokenKind::Gt => ">".to_string(),
        TokenKind::Gte => ">=".to_string(),
        TokenKind::Lt => "<".to_string(),
        TokenKind::Lte => "<=".to_string(),
        TokenKind::Range => "..".to_string(),
        TokenKind::Newline => "\\n".to_string(),
        TokenKind::Eof => "eof".to_string(),
    }
}
