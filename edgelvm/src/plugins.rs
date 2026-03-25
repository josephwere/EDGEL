use crate::ast::{Expr, Item, ModelDecl, Program};
use crate::compile_program;
use crate::diagnostics::Diagnostic;
use crate::loader::load_program_from_file;
use crate::project::find_project_root;
use crate::value::Value;
use crate::vm::{execute_function_with_options, VmOptions};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PluginDescriptor {
    pub name: String,
    pub path: PathBuf,
    pub hooks: Vec<PluginHook>,
    pub permissions: Vec<String>,
    pub version: Option<String>,
    pub channel: Option<String>,
    pub order: i64,
}

#[derive(Debug, Clone)]
pub struct PluginExecution {
    pub plugin: String,
    pub hook: PluginHook,
    pub console: Vec<String>,
    pub return_value: Option<Value>,
    pub shared_state: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginHook {
    OnStart,
    OnRun,
    OnBuild,
    OnError,
    OnCompile,
    OnExecute,
    OnApiRequest,
    OnCliCommand,
}

impl PluginHook {
    pub fn function_name(self) -> &'static str {
        match self {
            Self::OnStart => "onStart",
            Self::OnRun => "onRun",
            Self::OnBuild => "onBuild",
            Self::OnError => "onError",
            Self::OnCompile => "onCompile",
            Self::OnExecute => "onExecute",
            Self::OnApiRequest => "onApiRequest",
            Self::OnCliCommand => "onCliCommand",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OnStart => "onStart",
            Self::OnRun => "onRun",
            Self::OnBuild => "onBuild",
            Self::OnError => "onError",
            Self::OnCompile => "onCompile",
            Self::OnExecute => "onExecute",
            Self::OnApiRequest => "onApiRequest",
            Self::OnCliCommand => "onCliCommand",
        }
    }

    pub fn required_permission(self) -> &'static str {
        match self {
            Self::OnStart => "start",
            Self::OnRun => "run",
            Self::OnBuild => "build",
            Self::OnError => "error",
            Self::OnCompile => "compile",
            Self::OnExecute => "execute",
            Self::OnApiRequest => "api",
            Self::OnCliCommand => "cli",
        }
    }
}

pub fn discover_plugins(root: &Path) -> Result<Vec<PluginDescriptor>, Diagnostic> {
    let plugin_root = plugin_root(root);
    if !plugin_root.exists() {
        return Ok(Vec::new());
    }

    let mut descriptors = Vec::new();
    for entry in fs::read_dir(&plugin_root).map_err(io_to_diagnostic)? {
        let entry = entry.map_err(io_to_diagnostic)?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let plugin_file = path.join("plugin.egl");
        if !plugin_file.exists() {
            continue;
        }
        let program = load_program_from_file(&plugin_file)?;
        let metadata = extract_plugin_metadata(&program, path.file_name().and_then(|value| value.to_str()));
        let hooks = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(function) if function.name == PluginHook::OnStart.function_name() => {
                    Some(PluginHook::OnStart)
                }
                Item::Function(function) if function.name == PluginHook::OnRun.function_name() => {
                    Some(PluginHook::OnRun)
                }
                Item::Function(function) if function.name == PluginHook::OnBuild.function_name() => {
                    Some(PluginHook::OnBuild)
                }
                Item::Function(function) if function.name == PluginHook::OnError.function_name() => {
                    Some(PluginHook::OnError)
                }
                Item::Function(function)
                    if function.name == PluginHook::OnCompile.function_name() =>
                {
                    Some(PluginHook::OnCompile)
                }
                Item::Function(function)
                    if function.name == PluginHook::OnExecute.function_name() =>
                {
                    Some(PluginHook::OnExecute)
                }
                Item::Function(function)
                    if function.name == PluginHook::OnApiRequest.function_name() =>
                {
                    Some(PluginHook::OnApiRequest)
                }
                Item::Function(function)
                    if function.name == PluginHook::OnCliCommand.function_name() =>
                {
                    Some(PluginHook::OnCliCommand)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let permissions = if metadata.permissions.is_empty() {
            hooks.iter()
                .map(|hook| hook.required_permission().to_string())
                .collect()
        } else {
            metadata.permissions
        };
        descriptors.push(PluginDescriptor {
            name: metadata.name.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("plugin")
                    .to_string()
            }),
            path: plugin_file,
            hooks,
            permissions,
            version: metadata.version,
            channel: metadata.channel,
            order: metadata.order,
        });
    }

    descriptors.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(descriptors)
}

pub fn run_plugin_hooks(
    root: &Path,
    hook: PluginHook,
    event: BTreeMap<String, Value>,
    options: VmOptions,
) -> Result<Vec<PluginExecution>, Diagnostic> {
    let mut executions = Vec::new();
    let mut shared = BTreeMap::new();
    for plugin in discover_plugins(root)? {
        if !plugin.hooks.contains(&hook) || !plugin_allows_hook(&plugin, hook) {
            continue;
        }
        let program = load_program_from_file(&plugin.path)?;
        let bytecode = compile_program(&program)?;
        let mut scoped_event = event.clone();
        scoped_event.insert("hook".to_string(), Value::String(hook.label().to_string()));
        scoped_event.insert("plugin".to_string(), Value::String(plugin.name.clone()));
        scoped_event.insert(
            "permissions".to_string(),
            Value::List(
                plugin
                    .permissions
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
        scoped_event.insert("plugins".to_string(), Value::Object(shared.clone()));
        scoped_event.insert("shared".to_string(), Value::Object(shared.clone()));
        if let Some(version) = &plugin.version {
            scoped_event.insert("version".to_string(), Value::String(version.clone()));
        }
        if let Some(channel) = &plugin.channel {
            scoped_event.insert("channel".to_string(), Value::String(channel.clone()));
        }
        let (output, return_value) = execute_function_with_options(
            &bytecode,
            hook.function_name(),
            vec![Value::Object(scoped_event)],
            options.clone(),
        )?;
        let return_value = (!matches!(return_value, Value::Null)).then_some(return_value);
        if let Some(value) = &return_value {
            shared.insert(plugin.name.clone(), value.clone());
        }
        executions.push(PluginExecution {
            plugin: plugin.name,
            hook,
            console: output.console,
            return_value,
            shared_state: shared.clone(),
        });
    }
    Ok(executions)
}

pub fn scaffold_plugin(root: &Path, name: &str) -> Result<Vec<PathBuf>, Diagnostic> {
    let plugin_root = plugin_root(root).join(name);
    fs::create_dir_all(&plugin_root).map_err(io_to_diagnostic)?;
    let plugin_file = plugin_root.join("plugin.egl");
    fs::write(&plugin_file, plugin_template(name)).map_err(io_to_diagnostic)?;
    Ok(vec![plugin_file])
}

pub fn remove_plugin(root: &Path, name: &str) -> Result<(), Diagnostic> {
    let target = plugin_root(root).join(name);
    if target.exists() {
        fs::remove_dir_all(target).map_err(io_to_diagnostic)?;
    }
    Ok(())
}

fn plugin_root(root: &Path) -> PathBuf {
    find_project_root(root)
        .unwrap_or_else(|| root.to_path_buf())
        .join("plugins")
}

fn plugin_template(name: &str) -> String {
    format!(
        r#"model plugin {{
    name: "{name}"
    version: "0.1.0"
    order: 100
    permissions: ["run", "build", "cli"]
    channel: "observability"
}}

function onCliCommand(event) {{
    print("plugin {name} saw cli command " + event.command)
}}

function onRun(event) {{
    print("plugin {name} saw command " + event.command)
    return {{
        channel: event.channel,
        lastCommand: event.command
    }}
}}

function onBuild(event) {{
    print("plugin {name} saw build target " + event.target)
}}
"#
    )
}

fn plugin_allows_hook(plugin: &PluginDescriptor, hook: PluginHook) -> bool {
    if plugin.permissions.is_empty() {
        return true;
    }
    plugin
        .permissions
        .iter()
        .any(|permission| permission == "all" || permission == hook.required_permission())
}

#[derive(Default)]
struct PluginMetadata {
    name: Option<String>,
    version: Option<String>,
    channel: Option<String>,
    order: i64,
    permissions: Vec<String>,
}

fn extract_plugin_metadata(program: &Program, default_name: Option<&str>) -> PluginMetadata {
    let mut metadata = PluginMetadata {
        name: default_name.map(ToString::to_string),
        order: 100,
        ..PluginMetadata::default()
    };

    let Some(model) = program.items.iter().find_map(|item| match item {
        Item::Model(model) if model.name == "plugin" => Some(model),
        _ => None,
    }) else {
        return metadata;
    };

    apply_model_properties(model, &mut metadata);
    metadata
}

fn apply_model_properties(model: &ModelDecl, metadata: &mut PluginMetadata) {
    for (key, value) in &model.properties {
        match key.as_str() {
            "name" => metadata.name = string_from_expr(value),
            "version" => metadata.version = string_from_expr(value),
            "channel" => metadata.channel = string_from_expr(value),
            "order" => metadata.order = number_from_expr(value).unwrap_or(100.0) as i64,
            "permissions" => {
                metadata.permissions = string_list_from_expr(value);
            }
            _ => {}
        }
    }
}

fn string_from_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::String(value) => Some(value.clone()),
        Expr::Identifier(value) => Some(value.clone()),
        _ => None,
    }
}

fn string_list_from_expr(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::List(values) => values.iter().filter_map(string_from_expr).collect(),
        Expr::String(value) => vec![value.clone()],
        Expr::Identifier(value) => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn number_from_expr(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Number(value) => Some(*value),
        _ => None,
    }
}

fn io_to_diagnostic(error: std::io::Error) -> Diagnostic {
    Diagnostic::new(error.to_string(), 0, 0).with_context("plugins")
}
