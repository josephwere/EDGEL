use crate::ast::{Item, Program, UiNode};
use crate::diagnostics::Diagnostic;
use crate::render::program_summary;
use std::io::{Read, Write};
use std::net::TcpStream;

pub fn explain_program(program: &Program) -> String {
    let mut lines = vec![
        "NEUROEDGE briefing".to_string(),
        program_summary(program),
    ];

    for item in &program.items {
        match item {
            Item::Import(import_decl) => {
                lines.push(format!("Imports module `{}`.", import_decl.module));
            }
            Item::App(app) => {
                lines.push(format!("App `{}` contains {} screen(s).", app.name, app.screens.len()));
                for screen in &app.screens {
                    let widgets = screen
                        .nodes
                        .iter()
                        .map(|node| match node {
                            UiNode::Text(_) => "text",
                            UiNode::Header(_) => "header",
                            UiNode::Paragraph(_) => "paragraph",
                            UiNode::Input { .. } => "input",
                            UiNode::Button { .. } => "button",
                            UiNode::Scene { .. } => "scene",
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!(
                        "Screen `{}` renders: {}.",
                        screen.name,
                        if widgets.is_empty() {
                            "no widgets".to_string()
                        } else {
                            widgets
                        }
                    ));
                }
            }
            Item::Web(web) => {
                lines.push(format!(
                    "Web project `{}` exposes {} page(s).",
                    web.name,
                    web.pages.len()
                ));
                if !web.apis.is_empty() {
                    let routes = web
                        .apis
                        .iter()
                        .map(|api| api.route.clone())
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("API routes: {routes}."));
                }
            }
            Item::Function(function) => {
                lines.push(format!(
                    "{}function `{}` accepts {} parameter(s).",
                    if function.is_async { "Async " } else { "" },
                    function.name,
                    function.params.len()
                ));
                if let Some(return_type) = &function.return_type {
                    lines.push(format!("Function `{}` returns `{}`.", function.name, return_type));
                }
            }
            Item::Test(test) => {
                lines.push(format!("Test `{}` validates runtime behavior.", test.name));
            }
            Item::Table(table) => {
                let columns = table
                    .columns
                    .iter()
                    .map(|column| format!("{}: {}", column.name, column.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("Table `{}` columns -> {}.", table.name, columns));
            }
            Item::Model(model) => {
                lines.push(format!(
                    "Model `{}` has {} property line(s) and can be mapped to NEUROEDGE tools.",
                    model.name,
                    model.properties.len()
                ));
            }
            Item::IdVerse(idverse) => {
                lines.push(format!(
                    "IDVerse block `{}` defines {} identity trait(s).",
                    idverse.name,
                    idverse.fields.len()
                ));
            }
            Item::Db(db) => lines.push(format!("Database connection target: `{}`.", db.name)),
            Item::Api(api) => lines.push(format!("Standalone API route: `{}`.", api.route)),
            Item::Statement(_) => {}
        }
    }

    lines.join("\n")
}

pub fn suggest_fixes(source: &str, diagnostic: &Diagnostic) -> String {
    let mut tips = vec![
        format!("NEUROEDGE detected: {diagnostic}"),
        "Common EDGEL fixes:".to_string(),
    ];

    if diagnostic.message.contains("expected `}`") {
        tips.push("Add a closing brace for the block you opened earlier.".to_string());
    }
    if diagnostic.message.contains("expected `)`") {
        tips.push("Finish the current call with a closing parenthesis.".to_string());
    }
    if diagnostic.message.contains("expected expression") {
        tips.push("Place a value after operators like `+`, `-`, `=`, or `await`.".to_string());
    }
    if diagnostic.message.contains("could not resolve module") {
        tips.push("Check that imported modules live beside the current file or under `src/`.".to_string());
    }
    if source.contains("button(") && !source.contains("navigate(") && !source.contains("alert(") {
        tips.push("Buttons become more useful when they call `navigate(...)`, `alert(...)`, or `print(...)`.".to_string());
    }
    if source.contains("try {") && !source.contains("catch ") {
        tips.push("Every `try { ... }` block should be followed by `catch err { ... }`.".to_string());
    }

    tips.push("EDGEL blocks always use `{ ... }` and string values use double quotes.".to_string());
    tips.join("\n")
}

pub fn assist_action(action: &str, prompt: &str) -> String {
    if let Ok(url) = std::env::var("NEUROEDGE_API_URL") {
        if let Ok(response) = call_remote_api(&url, action, prompt) {
            return response;
        }
    }

    match action {
        "generateLesson" => format!("NEUROEDGE lesson plan: {prompt}"),
        "createApp" => format!("NEUROEDGE app brief: {prompt}"),
        "ask" => format!("NEUROEDGE insight: {prompt}"),
        other => format!("NEUROEDGE action `{other}` completed for: {prompt}"),
    }
}

fn call_remote_api(url: &str, action: &str, prompt: &str) -> Result<String, Diagnostic> {
    let endpoint = ApiEndpoint::parse(url)?;
    let mut stream =
        TcpStream::connect((endpoint.host.as_str(), endpoint.port)).map_err(io_to_diagnostic)?;
    let body = format!(
        "{{\"action\":\"{}\",\"prompt\":\"{}\"}}",
        escape_json(action),
        escape_json(prompt)
    );

    write!(
        stream,
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        endpoint.path,
        endpoint.host,
        body.len(),
        body
    )
    .map_err(io_to_diagnostic)?;
    stream.flush().map_err(io_to_diagnostic)?;

    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(io_to_diagnostic)?;
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .map(str::trim)
        .unwrap_or_default();

    if body.is_empty() {
        return Err(Diagnostic::new(
            "NEUROEDGE API returned an empty response",
            0,
            0,
        ));
    }

    if let Some(value) = extract_json_string_field(body, "text")
        .or_else(|| extract_json_string_field(body, "message"))
        .or_else(|| extract_json_string_field(body, "result"))
    {
        Ok(value)
    } else {
        Ok(body.to_string())
    }
}

fn extract_json_string_field(body: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\":\"");
    let start = body.find(&pattern)? + pattern.len();
    let remainder = &body[start..];
    let end = remainder.find('"')?;
    Some(remainder[..end].replace("\\n", "\n").replace("\\\"", "\""))
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn io_to_diagnostic(error: std::io::Error) -> Diagnostic {
    Diagnostic::new(error.to_string(), 0, 0).with_context("neuroedge-api")
}

struct ApiEndpoint {
    host: String,
    port: u16,
    path: String,
}

impl ApiEndpoint {
    fn parse(url: &str) -> Result<Self, Diagnostic> {
        let stripped = url.strip_prefix("http://").ok_or_else(|| {
            Diagnostic::new(
                "NEUROEDGE_API_URL currently supports only http:// endpoints",
                0,
                0,
            )
        })?;
        let (host_port, path) = match stripped.split_once('/') {
            Some((host_port, path)) => (host_port, format!("/{}", path)),
            None => (stripped, "/".to_string()),
        };
        let (host, port) = match host_port.split_once(':') {
            Some((host, port)) => (
                host.to_string(),
                port.parse::<u16>().map_err(|_| {
                    Diagnostic::new("invalid NEUROEDGE API port", 0, 0)
                        .with_context("neuroedge-api")
                })?,
            ),
            None => (host_port.to_string(), 80),
        };
        Ok(Self { host, port, path })
    }
}
