use crate::project::{
    build_apk_bundle_from_file, build_bytecode_bundle_from_file, build_web_bundle_from_file,
    list_project_files, run_project_file, sanitize_relative_path, BuildReport,
};
use crate::plugins::{
    discover_plugins, remove_plugin, run_plugin_hooks, scaffold_plugin, PluginHook,
};
use crate::telemetry::{recent_logs, record_log};
use crate::value::Value;
use crate::vm::{
    debug_step_index, inspect_debug_snapshot, DebugAction, DebugBreakpoint, DebugRecord,
    DebugSnapshot, VmOptions,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn serve(host: &str, port: u16) -> std::io::Result<()> {
    let listener = TcpListener::bind((host, port))?;
    println!("GoldEdge Browser running at http://{host}:{port}");
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(error) = handle_connection(&mut stream) {
                    let _ = write_response(
                        &mut stream,
                        500,
                        "text/plain; charset=utf-8",
                        &format!("Server error: {error}"),
                    );
                }
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut request = read_request(stream)?;
    request.remote_addr = stream
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "local".to_string());
    record_log(format!("{} {} from {}", request.method, request.path, request.remote_addr));
    if request.method == "OPTIONS" && request.path.starts_with("/api/") {
        return write_api_empty_response(stream, &request, 204);
    }
    if request.path.starts_with("/api/") && !allow_request(&request.remote_addr) {
        record_log(format!("rate limited {}", request.remote_addr));
        return write_response_with_headers(
            stream,
            429,
            "application/json; charset=utf-8",
            &error_json("rate limit exceeded"),
            &api_response_headers(&request),
        );
    }
    if request.method == "GET" && request.path != "/" {
        if let Some(response) = try_serve_frontend_asset(stream, &request.path) {
            return response;
        }
    }
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => serve_static(stream, "index.html", "text/html; charset=utf-8"),
        ("GET", "/health") => write_response(stream, 200, "text/plain; charset=utf-8", "ok"),
        ("POST", "/api/run") => write_api_json_response(stream, &request, &run_api_response(&request, false)),
        ("POST", "/api/profile") => {
            write_api_json_response(stream, &request, &run_api_response(&request, true))
        }
        ("POST", "/api/debug/start") => {
            write_api_json_response(stream, &request, &debug_start_response(&request))
        }
        ("POST", "/api/debug/step") => {
            write_api_json_response(stream, &request, &debug_step_response(&request))
        }
        ("GET", "/api/debug/inspect") | ("POST", "/api/debug/inspect") => {
            write_api_json_response(stream, &request, &debug_inspect_response(&request))
        }
        ("POST", "/api/build") => {
            let target = request.query.get("target").map(String::as_str).unwrap_or("web");
            let mut logs =
                api_hook_logs(PluginHook::OnApiRequest, api_event(&request, [("target", Value::String(target.to_string()))]));
            logs.extend(api_hook_logs(
                PluginHook::OnCompile,
                event_map([
                    ("command", Value::String("build".to_string())),
                    ("origin", Value::String("api".to_string())),
                    ("target", Value::String(target.to_string())),
                ]),
            ));
            let response = match run_build(&request) {
                Ok(report) => format!(
                    "{{\"ok\":true,\"summary\":\"{}\",\"output\":\"{}\",\"files\":[{}],\"pluginLogs\":[{}]}}",
                    escape_json(&report.summary),
                    escape_json(&report.output_dir.display().to_string()),
                    report
                        .files
                        .iter()
                        .map(|file| format!("\"{}\"", escape_json(&file.display().to_string())))
                        .collect::<Vec<_>>()
                        .join(","),
                    {
                        logs.extend(api_hook_logs(
                            PluginHook::OnBuild,
                            event_map([
                                ("command", Value::String("build".to_string())),
                                ("origin", Value::String("api".to_string())),
                                ("target", Value::String(target.to_string())),
                            ]),
                        ));
                        logs.join(",")
                    }
                ),
                Err(error) => {
                    record_log(format!("build error: {}", error));
                    diagnostic_json(&error)
                }
            };
            write_api_json_response(stream, &request, &response)
        }
        ("POST", "/api/build/web") => {
            let target = workspace_root().join("output/web");
            let response = build_response(
                materialize_request_source(&request)
                    .and_then(|path| build_web_bundle_from_file(&path, Some(&target))),
            );
            write_api_json_response(stream, &request, &response)
        }
        ("POST", "/api/build/apk") => {
            let target = workspace_root().join("output/android");
            let response = build_response(
                materialize_request_source(&request)
                    .and_then(|path| build_apk_bundle_from_file(&path, Some(&target))),
            );
            write_api_json_response(stream, &request, &response)
        }
        ("POST", "/api/build/bytecode") => {
            let target = workspace_root().join("output/bytecode");
            let response = build_response(
                materialize_request_source(&request)
                    .and_then(|path| build_bytecode_bundle_from_file(&path, Some(&target))),
            );
            write_api_json_response(stream, &request, &response)
        }
        ("GET", "/api/plugins") | ("POST", "/api/plugins") => {
            let response = handle_plugins_api(&request);
            write_api_json_response(stream, &request, &response)
        }
        ("GET", "/api/logs") => {
            let response = logs_api_response();
            write_api_json_response(stream, &request, &response)
        }
        ("GET", "/api/project") | ("POST", "/api/project") => {
            let response = handle_project_api(&request);
            write_api_json_response(stream, &request, &response)
        }
        ("POST", "/api/ai/explain") => {
            let response = match crate::explain_source(&request.body) {
                Ok(text) => format!("{{\"ok\":true,\"text\":\"{}\"}}", escape_json(&text)),
                Err(error) => diagnostic_json(&error),
            };
            write_api_json_response(stream, &request, &response)
        }
        ("POST", "/api/ai/fix") => {
            let text = crate::fix_source(&request.body);
            let response = format!("{{\"ok\":true,\"text\":\"{}\"}}", escape_json(&text));
            write_api_json_response(stream, &request, &response)
        }
        _ => write_response(stream, 404, "text/plain; charset=utf-8", "Not found"),
    }
}

fn run_build(request: &Request) -> Result<BuildReport, crate::diagnostics::Diagnostic> {
    let target = request.query.get("target").map(String::as_str).unwrap_or("web");
    let output = workspace_root().join("output").join(target);
    let request_file = materialize_request_source(request)?;
    match target {
        "web" => build_web_bundle_from_file(&request_file, Some(&output)),
        "apk" | "android" => build_apk_bundle_from_file(&request_file, Some(&output)),
        "bytecode" => build_bytecode_bundle_from_file(&request_file, Some(&output)),
        other => Err(crate::diagnostics::Diagnostic::new(
            format!("unknown build target `{other}`"),
            0,
            0,
        )),
    }
}

fn handle_project_api(request: &Request) -> String {
    let root = workspace_root();
    let action = request.query.get("action").map(String::as_str).unwrap_or("list");
    match (request.method.as_str(), action) {
        ("GET", "list") => match list_project_files(&root) {
            Ok(files) => format!(
                "{{\"ok\":true,\"root\":\"{}\",\"files\":[{}]}}",
                escape_json(&root.display().to_string()),
                files
                    .iter()
                    .map(|path| format!("\"{}\"", escape_json(&path.display().to_string())))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Err(error) => error_json(&error.to_string()),
        },
        ("GET", "plugins") => match discover_plugins(&root) {
            Ok(plugins) => format!(
                "{{\"ok\":true,\"plugins\":[{}]}}",
                plugin_inventory_json(&plugins)
            ),
            Err(error) => error_json(&error.to_string()),
        },
        ("GET", "read") => {
            let Some(path) = request.query.get("path") else {
                return error_json("missing project path");
            };
            let Some(path) = sanitize_relative_path(&root, path) else {
                return error_json("invalid project path");
            };
            match fs::read_to_string(&path) {
                Ok(content) => format!(
                    "{{\"ok\":true,\"path\":\"{}\",\"content\":\"{}\"}}",
                    escape_json(
                        &path.strip_prefix(&root)
                            .unwrap_or(path.as_path())
                            .display()
                            .to_string()
                    ),
                    escape_json(&content)
                ),
                Err(error) => error_json(&error.to_string()),
            }
        }
        ("POST", "write") => {
            let Some(path) = request.query.get("path") else {
                return error_json("missing project path");
            };
            let Some(path) = sanitize_relative_path(&root, path) else {
                return error_json("invalid project path");
            };
            match path.parent() {
                Some(parent) => {
                    if let Err(error) = fs::create_dir_all(parent) {
                        return error_json(&error.to_string());
                    }
                }
                None => return error_json("invalid project path"),
            }
            match fs::write(&path, &request.body) {
                Ok(()) => format!(
                    "{{\"ok\":true,\"path\":\"{}\"}}",
                    escape_json(
                        &path.strip_prefix(&root)
                            .unwrap_or(path.as_path())
                            .display()
                            .to_string()
                    )
                ),
                Err(error) => error_json(&error.to_string()),
            }
        }
        ("POST", "rename") => {
            let Some(path) = request.query.get("path") else {
                return error_json("missing project path");
            };
            let Some(next_path) = request.query.get("to") else {
                return error_json("missing destination path");
            };
            let Some(path) = sanitize_relative_path(&root, path) else {
                return error_json("invalid project path");
            };
            let Some(next_path) = sanitize_relative_path(&root, next_path) else {
                return error_json("invalid destination path");
            };
            if next_path.exists() {
                return error_json("destination path already exists");
            }
            if let Some(parent) = next_path.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    return error_json(&error.to_string());
                }
            }
            match fs::rename(&path, &next_path) {
                Ok(()) => format!(
                    "{{\"ok\":true,\"path\":\"{}\",\"to\":\"{}\"}}",
                    escape_json(
                        &path.strip_prefix(&root)
                            .unwrap_or(path.as_path())
                            .display()
                            .to_string()
                    ),
                    escape_json(
                        &next_path
                            .strip_prefix(&root)
                            .unwrap_or(next_path.as_path())
                            .display()
                            .to_string()
                    )
                ),
                Err(error) => error_json(&error.to_string()),
            }
        }
        ("POST", "delete") => {
            let Some(path) = request.query.get("path") else {
                return error_json("missing project path");
            };
            let Some(path) = sanitize_relative_path(&root, path) else {
                return error_json("invalid project path");
            };
            let result = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            match result {
                Ok(()) => format!(
                    "{{\"ok\":true,\"path\":\"{}\"}}",
                    escape_json(
                        &path.strip_prefix(&root)
                            .unwrap_or(path.as_path())
                            .display()
                            .to_string()
                    )
                ),
                Err(error) => error_json(&error.to_string()),
            }
        }
        _ => error_json("unsupported project action"),
    }
}

fn run_api_response(request: &Request, force_profile: bool) -> String {
    let options = VmOptions {
        debug: request.query_bool("debug"),
        profile: force_profile || request.query_bool("profile"),
        trace: request.query_bool("trace"),
        max_instructions: request
            .query
            .get("maxInstructions")
            .and_then(|value| value.parse::<u64>().ok())
            .or(Some(20_000)),
        breakpoints: Vec::new(),
    };
    let mut logs = api_hook_logs(
        PluginHook::OnApiRequest,
        api_event(
            request,
            [(
                "mode",
                Value::String(if force_profile {
                    "profile".to_string()
                } else {
                    "run".to_string()
                }),
            )],
        ),
    );
    logs.extend(api_hook_logs(
        PluginHook::OnStart,
        event_map([
            (
                "command",
                Value::String(if force_profile {
                    "profile".to_string()
                } else {
                    "run".to_string()
                }),
            ),
            ("origin", Value::String("api".to_string())),
        ]),
    ));

    let request_file = match materialize_request_source(request) {
        Ok(path) => path,
        Err(error) => return error_json(&error.to_string()),
    };

    match run_project_file(&request_file, options) {
        Ok(report) => {
            let command = if force_profile { "profile" } else { "run" };
            logs.extend(api_hook_logs(
                PluginHook::OnExecute,
                event_map([
                    ("command", Value::String(command.to_string())),
                    ("origin", Value::String("api".to_string())),
                    ("summary", Value::String(report.summary.clone())),
                ]),
            ));
            logs.extend(api_hook_logs(
                PluginHook::OnRun,
                event_map([
                    ("command", Value::String(command.to_string())),
                    ("origin", Value::String("api".to_string())),
                    ("summary", Value::String(report.summary.clone())),
                ]),
            ));
            record_log(format!("{} ok: {}", command, report.summary));
            format!(
                "{{\"ok\":true,\"summary\":\"{}\",\"console\":[{}],\"html\":{},\"profile\":{},\"trace\":[{}],\"pluginLogs\":[{}]}}",
                escape_json(&report.summary),
                report
                    .console
                    .iter()
                    .map(|line| format!("\"{}\"", escape_json(line)))
                    .collect::<Vec<_>>()
                    .join(","),
                report
                    .html_preview
                    .as_ref()
                    .map(|html| format!("\"{}\"", escape_json(html)))
                    .unwrap_or_else(|| "null".to_string()),
                report
                    .profile
                    .map(profile_json)
                    .unwrap_or_else(|| "null".to_string()),
                if request.query_bool("trace") || request.query_bool("debug") {
                    report
                        .trace
                        .iter()
                        .map(|line| format!("\"{}\"", escape_json(line)))
                        .collect::<Vec<_>>()
                        .join(",")
                } else {
                    String::new()
                },
                logs.join(",")
            )
        }
        Err(error) => {
            record_log(format!("run error: {}", error));
            let plugin_logs = api_hook_logs(
                PluginHook::OnError,
                event_map([
                    (
                        "command",
                        Value::String(if force_profile {
                            "profile".to_string()
                        } else {
                            "run".to_string()
                        }),
                    ),
                    ("origin", Value::String("api".to_string())),
                    ("error", Value::String(error.to_string())),
                ]),
            )
            .join(",");
            let mut response = diagnostic_json(&error);
            if response.ends_with('}') {
                response.pop();
            }
            format!("{response},\"pluginLogs\":[{plugin_logs}]}}")
        }
    }
}

#[derive(Debug, Clone)]
struct DebugSessionState {
    record: DebugRecord,
    cursor: usize,
    selected_frame: usize,
}

fn debug_start_response(request: &Request) -> String {
    let options = VmOptions {
        debug: true,
        profile: true,
        trace: true,
        max_instructions: request
            .query
            .get("maxInstructions")
            .and_then(|value| value.parse::<u64>().ok())
            .or(Some(20_000)),
        breakpoints: parse_debug_breakpoints(request.query.get("breakpoints").map(String::as_str)),
    };
    let request_file = match materialize_request_source(request) {
        Ok(path) => path,
        Err(error) => return diagnostic_json(&error),
    };
    match run_project_file(&request_file, options) {
        Ok(report) => {
            let Some(record) = report.debug.clone() else {
                return error_json("debug session did not capture any snapshots");
            };
            let cursor = record
                .snapshots
                .iter()
                .position(|snapshot| snapshot.pause_reason.is_some())
                .unwrap_or(0);
            let session_id = register_debug_session(DebugSessionState {
                record: record.clone(),
                cursor,
                selected_frame: 0,
            });
            let snapshot = record.snapshots.get(cursor).cloned();
            format!(
                "{{\"ok\":true,\"session\":\"{}\",\"summary\":\"{}\",\"cursor\":{},\"done\":{},\"snapshot\":{},\"console\":[{}],\"html\":{},\"profile\":{}}}",
                escape_json(&session_id),
                escape_json(&report.summary),
                cursor,
                cursor + 1 >= record.snapshots.len(),
                snapshot
                    .as_ref()
                    .map(debug_snapshot_json)
                    .unwrap_or_else(|| "null".to_string()),
                report
                    .console
                    .iter()
                    .map(|line| format!("\"{}\"", escape_json(line)))
                    .collect::<Vec<_>>()
                    .join(","),
                report
                    .html_preview
                    .as_ref()
                    .map(|html| format!("\"{}\"", escape_json(html)))
                    .unwrap_or_else(|| "null".to_string()),
                report
                    .profile
                    .map(profile_json)
                    .unwrap_or_else(|| "null".to_string())
            )
        }
        Err(error) => diagnostic_json(&error),
    }
}

fn debug_step_response(request: &Request) -> String {
    let Some(session_id) = request.query.get("session") else {
        return error_json("missing debug session id");
    };
    let action = match request.query.get("action").map(String::as_str).unwrap_or("into") {
        "into" | "step" => DebugAction::StepInto,
        "over" | "next" => DebugAction::StepOver,
        "out" | "finish" => DebugAction::StepOut,
        "continue" | "run" => DebugAction::Continue,
        other => return error_json(&format!("unknown debug action `{other}`")),
    };

    let store = debug_sessions();
    let Ok(mut store) = store.lock() else {
        return error_json("debug session store unavailable");
    };
    let Some(session) = store.get_mut(session_id) else {
        return error_json("debug session not found");
    };
    session.cursor = debug_step_index(&session.record, session.cursor, action);
    let frame_count = session
        .record
        .snapshots
        .get(session.cursor)
        .map(|snapshot| snapshot.frames.len())
        .unwrap_or(0);
    session.selected_frame = session.selected_frame.min(frame_count.saturating_sub(1));

    let snapshot = session.record.snapshots.get(session.cursor).cloned();
    format!(
        "{{\"ok\":true,\"session\":\"{}\",\"cursor\":{},\"done\":{},\"snapshot\":{},\"selectedFrame\":{}}}",
        escape_json(session_id),
        session.cursor,
        session.cursor + 1 >= session.record.snapshots.len(),
        snapshot
            .as_ref()
            .map(debug_snapshot_json)
            .unwrap_or_else(|| "null".to_string()),
        session.selected_frame
    )
}

fn debug_inspect_response(request: &Request) -> String {
    let Some(session_id) = request.query.get("session") else {
        return error_json("missing debug session id");
    };
    let expression = request
        .query
        .get("expr")
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    let store = debug_sessions();
    let Ok(mut store) = store.lock() else {
        return error_json("debug session store unavailable");
    };
    let Some(session) = store.get_mut(session_id) else {
        return error_json("debug session not found");
    };
    if let Some(frame) = request
        .query
        .get("frame")
        .and_then(|value| value.parse::<usize>().ok())
    {
        session.selected_frame = frame;
    }
    let Some(snapshot) = session.record.snapshots.get(session.cursor) else {
        return error_json("debug snapshot not found");
    };
    let selected_frame = session.selected_frame.min(snapshot.frames.len().saturating_sub(1));
    session.selected_frame = selected_frame;
    let value = inspect_debug_snapshot(snapshot, expression, selected_frame);
    format!(
        "{{\"ok\":true,\"session\":\"{}\",\"frame\":{},\"expr\":\"{}\",\"value\":{},\"snapshot\":{}}}",
        escape_json(session_id),
        selected_frame,
        escape_json(expression),
        value_json(&value),
        debug_snapshot_json(snapshot)
    )
}

fn register_debug_session(session: DebugSessionState) -> String {
    let store = debug_sessions();
    let Ok(mut store) = store.lock() else {
        return "debug-unavailable".to_string();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let session_id = format!("dbg-{now}-{}", store.len() + 1);
    store.insert(session_id.clone(), session);
    session_id
}

fn debug_sessions() -> &'static Mutex<BTreeMap<String, DebugSessionState>> {
    static DEBUG_SESSIONS: OnceLock<Mutex<BTreeMap<String, DebugSessionState>>> = OnceLock::new();
    DEBUG_SESSIONS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn parse_debug_breakpoints(spec: Option<&str>) -> Vec<DebugBreakpoint> {
    spec.unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if let Ok(line) = value.parse::<usize>() {
                DebugBreakpoint::Line(line)
            } else if let Some(name) = value.strip_prefix("function:") {
                DebugBreakpoint::Function(name.to_string())
            } else {
                DebugBreakpoint::Function(value.to_string())
            }
        })
        .collect()
}

fn debug_snapshot_json(snapshot: &DebugSnapshot) -> String {
    format!(
        "{{\"index\":{},\"line\":{},\"summary\":\"{}\",\"instruction\":\"{}\",\"stack\":[{}],\"globals\":{},\"frames\":[{}],\"pauseReason\":{}}}",
        snapshot.index,
        snapshot.line,
        escape_json(&snapshot.summary),
        escape_json(&snapshot.instruction),
        snapshot
            .stack
            .iter()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .collect::<Vec<_>>()
            .join(","),
        value_json(&Value::Object(snapshot.globals.clone())),
        snapshot
            .frames
            .iter()
            .map(debug_frame_json)
            .collect::<Vec<_>>()
            .join(","),
        snapshot
            .pause_reason
            .as_ref()
            .map(|reason| format!("\"{}\"", escape_json(reason)))
            .unwrap_or_else(|| "null".to_string())
    )
}

fn debug_frame_json(frame: &crate::vm::DebugFrame) -> String {
    format!(
        "{{\"function\":\"{}\",\"line\":{},\"summary\":\"{}\",\"locals\":{}}}",
        escape_json(&frame.function),
        frame.line,
        escape_json(&frame.summary),
        value_json(&Value::Object(frame.locals.clone()))
    )
}

fn handle_plugins_api(request: &Request) -> String {
    let root = workspace_root();
    let action = request.query.get("action").map(String::as_str).unwrap_or("list");
    match (request.method.as_str(), action) {
        ("GET", "list") => match discover_plugins(&root) {
            Ok(plugins) => {
                let plugins = plugins
                    .into_iter()
                    .filter(|plugin| {
                        request
                            .query
                            .get("name")
                            .map(|needle| needle == &plugin.name)
                            .unwrap_or(true)
                    })
                    .collect::<Vec<_>>();
                format!("{{\"ok\":true,\"plugins\":[{}]}}", plugin_inventory_json(&plugins))
            }
            Err(error) => error_json(&error.to_string()),
        },
        ("POST", "install") | ("POST", "scaffold") => {
            let Some(name) = request.query.get("name") else {
                return error_json("missing plugin name");
            };
            match scaffold_plugin(&root, name) {
                Ok(files) => format!(
                    "{{\"ok\":true,\"summary\":\"{}\",\"files\":[{}]}}",
                    escape_json(&format!("Plugin `{name}` scaffolded.")),
                    files
                        .iter()
                        .map(|file| format!("\"{}\"", escape_json(&file.display().to_string())))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Err(error) => diagnostic_json(&error),
            }
        }
        ("POST", "remove") | ("POST", "uninstall") => {
            let Some(name) = request.query.get("name") else {
                return error_json("missing plugin name");
            };
            match remove_plugin(&root, name) {
                Ok(()) => format!(
                    "{{\"ok\":true,\"summary\":\"{}\"}}",
                    escape_json(&format!("Plugin `{name}` removed."))
                ),
                Err(error) => diagnostic_json(&error),
            }
        }
        _ => error_json("unsupported plugin action"),
    }
}

fn logs_api_response() -> String {
    format!(
        "{{\"ok\":true,\"logs\":[{}]}}",
        recent_logs()
            .into_iter()
            .map(|line| format!("\"{}\"", escape_json(&line)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn plugin_inventory_json(plugins: &[crate::plugins::PluginDescriptor]) -> String {
    plugins
        .iter()
        .map(|plugin| {
            format!(
                "{{\"name\":\"{}\",\"path\":\"{}\",\"hooks\":[{}],\"permissions\":[{}],\"version\":{},\"channel\":{},\"order\":{}}}",
                escape_json(&plugin.name),
                escape_json(&plugin.path.display().to_string()),
                plugin
                    .hooks
                    .iter()
                    .map(|hook| format!("\"{}\"", hook.label()))
                    .collect::<Vec<_>>()
                    .join(","),
                plugin
                    .permissions
                    .iter()
                    .map(|permission| format!("\"{}\"", escape_json(permission)))
                    .collect::<Vec<_>>()
                    .join(","),
                plugin
                    .version
                    .as_ref()
                    .map(|value| format!("\"{}\"", escape_json(value)))
                    .unwrap_or_else(|| "null".to_string()),
                plugin
                    .channel
                    .as_ref()
                    .map(|value| format!("\"{}\"", escape_json(value)))
                    .unwrap_or_else(|| "null".to_string()),
                plugin.order
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn build_response(result: Result<BuildReport, crate::diagnostics::Diagnostic>) -> String {
    match result {
        Ok(report) => format!(
            "{{\"ok\":true,\"summary\":\"{}\",\"output\":\"{}\",\"files\":[{}]}}",
            escape_json(&report.summary),
            escape_json(&report.output_dir.display().to_string()),
            report
                .files
                .iter()
                .map(|file| format!("\"{}\"", escape_json(&file.display().to_string())))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Err(error) => diagnostic_json(&error),
    }
}

fn materialize_request_source(request: &Request) -> Result<PathBuf, crate::diagnostics::Diagnostic> {
    let root = workspace_root();
    let path = request
        .query
        .get("path")
        .and_then(|raw| sanitize_relative_path(&root, raw))
        .unwrap_or_else(|| root.join("output/.api-session/main.egl"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_to_diagnostic)?;
    }
    fs::write(&path, &request.body).map_err(io_to_diagnostic)?;
    Ok(path)
}

fn api_hook_logs(hook: PluginHook, event: BTreeMap<String, Value>) -> Vec<String> {
    plugin_logs(run_plugin_hooks(
        &workspace_root(),
        hook,
        event,
        plugin_vm_options(),
    ))
}

fn api_event<const N: usize>(request: &Request, extra: [(&str, Value); N]) -> BTreeMap<String, Value> {
    let mut event = event_map([
        ("origin", Value::String("api".to_string())),
        ("method", Value::String(request.method.clone())),
        ("path", Value::String(request.path.clone())),
    ]);
    for (key, value) in extra {
        event.insert(key.to_string(), value);
    }
    event
}

fn plugin_vm_options() -> VmOptions {
    VmOptions {
        debug: false,
        profile: false,
        trace: false,
        max_instructions: Some(5_000),
        breakpoints: Vec::new(),
    }
}

fn serve_static(stream: &mut TcpStream, file: &str, content_type: &str) -> std::io::Result<()> {
    let path = workspace_root().join("frontend").join(file);
    let body = fs::read_to_string(path)?;
    write_response(stream, 200, content_type, &body)
}

fn try_serve_frontend_asset(
    stream: &mut TcpStream,
    request_path: &str,
) -> Option<std::io::Result<()>> {
    let relative = request_path.strip_prefix('/')?;
    if relative.is_empty() || relative.contains("..") || relative.contains('\\') {
        return None;
    }
    let content_type = content_type_for_asset(relative)?;
    let path = workspace_root().join("frontend").join(relative);
    if !path.is_file() {
        return None;
    }
    Some(serve_static_path(stream, &path, content_type))
}

fn serve_static_path(
    stream: &mut TcpStream,
    path: &Path,
    content_type: &str,
) -> std::io::Result<()> {
    let body = fs::read_to_string(path)?;
    write_response(stream, 200, content_type, &body)
}

fn content_type_for_asset(path: &str) -> Option<&'static str> {
    match Path::new(path).extension().and_then(|value| value.to_str()) {
        Some("html") => Some("text/html; charset=utf-8"),
        Some("css") => Some("text/css; charset=utf-8"),
        Some("js") => Some("application/javascript; charset=utf-8"),
        Some("json") => Some("application/json; charset=utf-8"),
        Some("svg") => Some("image/svg+xml"),
        _ => None,
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Request> {
    let mut buffer = Vec::new();
    let mut temp = [0_u8; 4096];

    loop {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let header_end = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .unwrap_or(buffer.len());

    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines.next().unwrap_or("GET / HTTP/1.1");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let raw_path = parts.next().unwrap_or("/").to_string();
    let (path, query) = split_path_and_query(&raw_path);
    let header_map = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let content_length = header_map
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    while buffer.len().saturating_sub(header_end) < content_length {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
    }

    let body = String::from_utf8_lossy(&buffer[header_end..]).to_string();
    Ok(Request {
        method,
        path,
        query,
        headers: header_map,
        body,
        remote_addr: "local".to_string(),
    })
}

fn split_path_and_query(raw_path: &str) -> (String, BTreeMap<String, String>) {
    let (path, query_string) = match raw_path.split_once('?') {
        Some((path, query)) => (path.to_string(), Some(query)),
        None => (raw_path.to_string(), None),
    };
    let mut query = BTreeMap::new();
    if let Some(query_string) = query_string {
        for pair in query_string.split('&') {
            if pair.is_empty() {
                continue;
            }
            match pair.split_once('=') {
                Some((key, value)) => {
                    query.insert(key.to_string(), percent_decode(value));
                }
                None => {
                    query.insert(pair.to_string(), String::new());
                }
            }
        }
    }
    (path, query)
}

fn percent_decode(value: &str) -> String {
    value.replace("%2F", "/").replace("%20", " ").replace('+', " ")
}

fn write_api_json_response(
    stream: &mut TcpStream,
    request: &Request,
    body: &str,
) -> std::io::Result<()> {
    write_response_with_headers(
        stream,
        200,
        "application/json; charset=utf-8",
        body,
        &api_response_headers(request),
    )
}

fn write_api_empty_response(
    stream: &mut TcpStream,
    request: &Request,
    status: u16,
) -> std::io::Result<()> {
    write_response_with_headers(
        stream,
        status,
        "text/plain; charset=utf-8",
        "",
        &api_response_headers(request),
    )
}

fn api_response_headers(request: &Request) -> Vec<(String, String)> {
    vec![
        (
            "Access-Control-Allow-Origin".to_string(),
            allowed_origin(request),
        ),
        (
            "Access-Control-Allow-Methods".to_string(),
            "GET, POST, OPTIONS".to_string(),
        ),
        (
            "Access-Control-Allow-Headers".to_string(),
            "Content-Type".to_string(),
        ),
        ("Access-Control-Max-Age".to_string(), "86400".to_string()),
        ("Vary".to_string(), "Origin".to_string()),
    ]
}

fn allowed_origin(request: &Request) -> String {
    let configured = env::var("EDGEL_ALLOWED_ORIGIN").unwrap_or_else(|_| "*".to_string());
    if configured.trim() == "*" {
        return "*".to_string();
    }

    let origin = request
        .headers
        .get("origin")
        .cloned()
        .unwrap_or_default();
    let allowed = configured
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if !origin.is_empty() && allowed.iter().any(|candidate| *candidate == origin) {
        origin
    } else {
        allowed.first().copied().unwrap_or("*").to_string()
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    write_response_with_headers(stream, status, content_type, body, &[])
}

fn write_response_with_headers(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
    extra_headers: &[(String, String)],
) -> std::io::Result<()> {
    let status_text = match status {
        200 => "OK",
        204 => "No Content",
        429 => "Too Many Requests",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };

    let mut header_block = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status,
        status_text,
        content_type,
        body.len()
    );
    for (name, value) in extra_headers {
        header_block.push_str(&format!("{name}: {value}\r\n"));
    }
    header_block.push_str("\r\n");

    write!(stream, "{}{}", header_block, body)?;
    stream.flush()
}

fn profile_json(profile: crate::vm::VmProfile) -> String {
    format!(
        "{{\"instruction_count\":{},\"function_calls\":{},\"builtin_calls\":{},\"caught_errors\":{},\"max_stack_depth\":{},\"elapsed_ms\":{},\"function_hits\":[{}]}}",
        profile.instruction_count,
        profile.function_calls,
        profile.builtin_calls,
        profile.caught_errors,
        profile.max_stack_depth,
        profile.elapsed_ms,
        profile
            .function_hits
            .keys()
            .map(|name| format!("\"{}\"", escape_json(name)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn error_json(message: &str) -> String {
    format!("{{\"ok\":false,\"error\":\"{}\"}}", escape_json(message))
}

fn value_json(value: &Value) -> String {
    match value {
        Value::Number(number) => {
            if number.fract() == 0.0 {
                format!("{}", *number as i64)
            } else {
                number.to_string()
            }
        }
        Value::String(text) => format!("\"{}\"", escape_json(text)),
        Value::Bool(flag) => flag.to_string(),
        Value::List(items) => format!(
            "[{}]",
            items
                .iter()
                .map(value_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!("\"{}\":{}", escape_json(key), value_json(value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Null => "null".to_string(),
    }
}

fn diagnostic_json(diagnostic: &crate::diagnostics::Diagnostic) -> String {
    fn diagnostic_body_json(diagnostic: &crate::diagnostics::Diagnostic) -> String {
        format!(
            "{{\"error\":\"{}\",\"line\":{},\"column\":{},\"context\":{},\"notes\":[{}],\"stack\":[{}],\"related\":[{}]}}",
            escape_json(&diagnostic.message),
            diagnostic.line,
            diagnostic.column,
            diagnostic
                .context
                .as_ref()
                .map(|value| format!("\"{}\"", escape_json(value)))
                .unwrap_or_else(|| "null".to_string()),
            diagnostic
                .notes
                .iter()
                .map(|note| format!("\"{}\"", escape_json(note)))
                .collect::<Vec<_>>()
                .join(","),
            diagnostic
                .stack
                .iter()
                .map(|frame| format!("\"{}\"", escape_json(frame)))
                .collect::<Vec<_>>()
                .join(","),
            diagnostic
                .related
                .iter()
                .map(diagnostic_body_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    format!(
        "{{\"ok\":false,{}}}",
        diagnostic_body_json(diagnostic)
            .trim_start_matches('{')
            .trim_end_matches('}')
    )
}

fn io_to_diagnostic(error: std::io::Error) -> crate::diagnostics::Diagnostic {
    crate::diagnostics::Diagnostic::new(error.to_string(), 0, 0).with_context("server")
}

fn plugin_logs(
    result: Result<Vec<crate::plugins::PluginExecution>, crate::diagnostics::Diagnostic>,
) -> Vec<String> {
    match result {
        Ok(executions) => executions
            .into_iter()
            .flat_map(|execution| {
                let mut lines = execution
                    .console
                    .into_iter()
                    .map(|line| {
                        format!(
                            "\"{}\"",
                            escape_json(&format!("{}:{} {}", execution.plugin, execution.hook.label(), line))
                        )
                    })
                    .collect::<Vec<_>>();
                if let Some(value) = execution.return_value {
                    lines.push(format!(
                        "\"{}\"",
                        escape_json(&format!("{}:{} {}", execution.plugin, execution.hook.label(), value))
                    ));
                }
                lines
            })
            .collect(),
        Err(error) => vec![format!("\"{}\"", escape_json(&format!("plugin-error: {error}")))],
    }
}

fn event_map<const N: usize>(entries: [(&str, Value); N]) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

struct Request {
    method: String,
    path: String,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    body: String,
    remote_addr: String,
}

impl Request {
    fn query_bool(&self, key: &str) -> bool {
        matches!(
            self.query.get(key).map(String::as_str),
            Some("1" | "true" | "yes" | "on")
        )
    }
}

fn allow_request(remote_addr: &str) -> bool {
    static RATE_LIMITER: OnceLock<Mutex<BTreeMap<String, Vec<u64>>>> = OnceLock::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let limiter = RATE_LIMITER.get_or_init(|| Mutex::new(BTreeMap::new()));
    let Ok(mut limiter) = limiter.lock() else {
        return true;
    };
    let history = limiter.entry(remote_addr.to_string()).or_default();
    history.retain(|timestamp| now.saturating_sub(*timestamp) < 60);
    if history.len() >= 60 {
        return false;
    }
    history.push(now);
    true
}
