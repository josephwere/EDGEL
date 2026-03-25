mod learn;

use edgelvm::{
    available_project_templates, build_apk_bundle_from_file, build_bytecode_bundle_from_file,
    build_web_bundle_from_file, collect_tests, compile_file, compile_source, debug_step_index,
    default_entry_file, discover_plugins, explain_file, explain_source, find_project_root,
    fix_file, fix_source, init_project, init_project_with_template, inspect_debug_snapshot,
    install_dependency, load_manifest, lower_file, lower_source, parse_file, parse_source,
    publish_package, remove_plugin, run_plugin_hooks, run_project_file, run_source, run_test_file,
    scaffold_plugin, serve, update_dependencies, verify_lockfile, DebugAction, DebugBreakpoint,
    DebugSnapshot, PluginHook, Value, VmOptions,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("{} {}", color("error", 31), error);
        std::process::exit(1);
    }
}

fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("run") => handle_run(&args),
        Some("build") => handle_build(&args),
        Some("serve") => {
            serve("127.0.0.1", 4040)?;
            Ok(())
        }
        Some("plugin") => handle_plugin(&args),
        Some("repl") => repl(),
        Some("test") => handle_test(&args),
        Some("doctor") => handle_doctor(),
        Some("debug") => handle_debug(&args),
        Some("info") => handle_info(&args),
        Some("optimize") => handle_optimize(&args),
        Some("profile") => handle_profile(&args),
        Some("install") => handle_install(&args),
        Some("update") => handle_update(&args),
        Some("publish") => handle_publish(&args),
        Some("new") => handle_new(&args),
        Some("init") => handle_init(&args),
        Some("learn") => learn::handle_learn(&args),
        Some("ai") => handle_ai(&args),
        Some("parse") => {
            let file = resolve_input_file(first_positional(&args, 1))?;
            println!("{:#?}", parse_file(&file)?);
            Ok(())
        }
        Some("ir") => {
            let file = resolve_input_file(first_positional(&args, 1))?;
            println!("{:#?}", lower_file(&file)?);
            Ok(())
        }
        Some("bytecode") => {
            let file = resolve_input_file(first_positional(&args, 1))?;
            println!("{:#?}", compile_file(&file)?);
            Ok(())
        }
        Some("help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`").into()),
    }
}

fn handle_run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = resolve_input_file(first_positional(args, 1))?;
    let project_root = plugin_root_for(&file);
    emit_cli_command_hook("run", &project_root, &file, args);
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnStart,
        event_map([
            ("command", Value::String("run".to_string())),
            ("file", Value::String(file.display().to_string())),
        ]),
    );
    let options = VmOptions {
        debug: has_flag(args, "--debug"),
        profile: has_flag(args, "--profile"),
        trace: false,
        max_instructions: None,
        breakpoints: Vec::new(),
    };
    let report = match run_project_file(&file, options) {
        Ok(report) => report,
        Err(error) => {
            emit_plugin_hooks_best_effort(
                &project_root,
                PluginHook::OnError,
                event_map([
                    ("command", Value::String("run".to_string())),
                    ("file", Value::String(file.display().to_string())),
                    ("error", Value::String(error.to_string())),
                ]),
            );
            return Err(error.into());
        }
    };
    let console_is_empty = report.console.is_empty();
    println!("{} {}", color("run", 32), file.display());
    for line in report.console {
        println!("{line}");
    }
    if console_is_empty {
        println!("{}", report.summary);
    }
    if let Some(profile) = report.profile {
        println!(
            "{} instructions={} calls={} builtins={} caught_errors={} max_stack={} elapsed_ms={}",
            color("profile", 36),
            profile.instruction_count,
            profile.function_calls,
            profile.builtin_calls,
            profile.caught_errors,
            profile.max_stack_depth,
            profile.elapsed_ms
        );
    }
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnExecute,
        event_map([
            ("command", Value::String("run".to_string())),
            ("file", Value::String(file.display().to_string())),
            ("summary", Value::String(report.summary.clone())),
        ]),
    );
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnRun,
        event_map([
            ("command", Value::String("run".to_string())),
            ("file", Value::String(file.display().to_string())),
            ("summary", Value::String(report.summary)),
        ]),
    );
    Ok(())
}

fn handle_build(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mode = args.get(1).map(String::as_str);
    let (target, file_arg_index) = match mode {
        Some("--web") => ("web", 2),
        Some("--apk") => ("apk", 2),
        Some("--bytecode") => ("bytecode", 2),
        Some(value) if !value.starts_with("--") => ("bytecode", 1),
        None => ("bytecode", 1),
        _ => return Err("unknown build target; use --web, --apk, or --bytecode".into()),
    };
    let file = resolve_input_file(first_positional(args, file_arg_index))?;
    let project_root = plugin_root_for(&file);
    emit_cli_command_hook("build", &project_root, &file, args);
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnStart,
        event_map([
            ("command", Value::String("build".to_string())),
            ("target", Value::String(target.to_string())),
            ("file", Value::String(file.display().to_string())),
        ]),
    );
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnCompile,
        event_map([
            ("command", Value::String("build".to_string())),
            ("target", Value::String(target.to_string())),
            ("file", Value::String(file.display().to_string())),
        ]),
    );
    let report = match target {
        "web" => build_web_bundle_from_file(&file, None)?,
        "apk" => build_apk_bundle_from_file(&file, None)?,
        "bytecode" => build_bytecode_bundle_from_file(&file, None)?,
        _ => return Err("unknown build target; use --web, --apk, or --bytecode".into()),
    };

    println!("{} {}", color("build", 32), report.summary);
    println!("Output: {}", report.output_dir.display());
    for file in report.files {
        println!(" - {}", file.display());
    }
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnBuild,
        event_map([
            ("command", Value::String("build".to_string())),
            ("target", Value::String(target.to_string())),
            ("summary", Value::String(report.summary)),
        ]),
    );
    Ok(())
}

fn handle_ai(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = resolve_input_file(first_positional(args, 2))?;
    match args.get(1).map(String::as_str) {
        Some("explain") => {
            println!("{}", explain_file(&file)?);
            Ok(())
        }
        Some("fix") => {
            println!("{}", fix_file(&file)?);
            Ok(())
        }
        _ => Err("use `edgel ai explain <file.egl>` or `edgel ai fix <file.egl>`".into()),
    }
}

fn handle_test(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let target = first_positional(args, 1)
        .map(PathBuf::from)
        .unwrap_or_else(default_test_root);
    let write_report = has_flag(args, "--report");
    let show_coverage = has_flag(args, "--coverage");
    let project_root = plugin_root_for(&target);
    emit_cli_command_hook("test", &project_root, &target, args);
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnStart,
        event_map([
            ("command", Value::String("test".to_string())),
            ("target", Value::String(target.display().to_string())),
        ]),
    );
    let files = discover_tests(&target)?;
    if files.is_empty() {
        return Err(format!("no `.test.egl` files found under {}", target.display()).into());
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut report_cases = Vec::new();
    for file in files {
        match run_test_file(&file, VmOptions::default()) {
            Ok(results) => {
                for result in results {
                    let edgelvm::TestRunResult {
                        name,
                        console,
                        profile,
                    } = result;
                    passed += 1;
                    println!("{} {} :: {}", color("pass", 32), file.display(), name);
                    for line in console {
                        println!("  {line}");
                    }
                    if show_coverage {
                        if let Some(profile) = &profile {
                            let functions = profile
                                .function_hits
                                .keys()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ");
                            println!("  coverage: {}", if functions.is_empty() { "<none>" } else { &functions });
                        }
                    }
                    report_cases.push(TestCaseReport {
                        file: file.display().to_string(),
                        name,
                        ok: true,
                        function_hits: profile
                            .map(|profile| profile.function_hits.keys().cloned().collect())
                            .unwrap_or_default(),
                    });
                }
            }
            Err(error) => {
                failed += 1;
                println!("{} {} -> {}", color("fail", 31), file.display(), error);
                report_cases.push(TestCaseReport {
                    file: file.display().to_string(),
                    name: "file execution".to_string(),
                    ok: false,
                    function_hits: Vec::new(),
                });
            }
        }
    }

    println!(
        "{} passed={}, failed={}",
        color("test-summary", if failed == 0 { 32 } else { 31 }),
        passed,
        failed
    );

    if write_report {
        let report_path = write_test_report(&project_root, &report_cases, passed, failed)?;
        println!("{} {}", color("report", 36), report_path.display());
    }

    if failed == 0 {
        Ok(())
    } else {
        emit_plugin_hooks_best_effort(
            &project_root,
            PluginHook::OnError,
            event_map([
                ("command", Value::String("test".to_string())),
                ("error", Value::String(format!("{failed} test(s) failed"))),
            ]),
        );
        Err(format!("{failed} test(s) failed").into())
    }
}

fn handle_doctor() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let rustc = command_version("rustc", "--version");
    let cargo = command_version("cargo", "--version");
    let project_root = find_project_root(&cwd);
    let entry = default_entry_file(&cwd);
    let neuroedge_api = env::var("NEUROEDGE_API_URL").ok();

    println!("{} {}", color("doctor", 36), cwd.display());
    println!(" - rustc: {}", rustc.unwrap_or_else(|| "missing".to_string()));
    println!(" - cargo: {}", cargo.unwrap_or_else(|| "missing".to_string()));
    println!(
        " - project root: {}",
        project_root
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not detected".to_string())
    );
    println!(
        " - default entry: {}",
        entry
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "src/main.egl not found".to_string())
    );
    println!(
        " - neuroedge api: {}",
        neuroedge_api.unwrap_or_else(|| "local fallback mode".to_string())
    );
    println!(
        " - browser assets: {}",
        if cwd.join("frontend/index.html").exists() || cwd.join("../frontend/index.html").exists() {
            "present"
        } else {
            "missing"
        }
    );
    if let Ok(manifest) = load_manifest(&cwd) {
        println!(" - manifest: {}@{}", manifest.name, manifest.version);
        println!(" - dependencies: {}", manifest.dependencies.len());
        match verify_lockfile(&cwd) {
            Ok(lockfile) => println!(" - lockfile: verified ({})", lockfile.packages.len()),
            Err(error) => println!(" - lockfile: invalid ({})", error.message),
        }
    }
    Ok(())
}

fn handle_debug(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = resolve_input_file(first_positional(args, 1))?;
    let project_root = plugin_root_for(&file);
    emit_cli_command_hook("debug", &project_root, &file, args);
    let breakpoints = parse_debug_breakpoints(args)?;

    let report = run_project_file(
        &file,
        VmOptions {
            debug: true,
            profile: has_flag(args, "--profile"),
            trace: true,
            max_instructions: None,
            breakpoints,
        },
    )?;
    let Some(debug) = report.debug.clone() else {
        return Err("debug session did not capture any snapshots".into());
    };
    let mut cursor = debug
        .snapshots
        .iter()
        .position(|snapshot| snapshot.pause_reason.is_some())
        .unwrap_or(0);
    let mut selected_frame = 0usize;

    println!("{} {}", color("debug", 36), file.display());
    println!("{}", report.summary);
    render_debug_snapshot(&debug.snapshots[cursor], selected_frame);
    println!("Commands: step, next, out, continue, stack, locals, globals, frame <n>, print <expr>, inspect <expr>, help, exit");

    let mut line = String::new();
    loop {
        line.clear();
        print!("debug> ");
        io::stdout().flush()?;
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let command = line.trim();
        if command.is_empty() {
            continue;
        }
        match command {
            "step" | "s" | "into" => {
                cursor = debug_step_index(&debug, cursor, DebugAction::StepInto);
                selected_frame =
                    selected_frame.min(debug.snapshots[cursor].frames.len().saturating_sub(1));
                render_debug_snapshot(&debug.snapshots[cursor], selected_frame);
            }
            "next" | "n" | "over" => {
                cursor = debug_step_index(&debug, cursor, DebugAction::StepOver);
                selected_frame =
                    selected_frame.min(debug.snapshots[cursor].frames.len().saturating_sub(1));
                render_debug_snapshot(&debug.snapshots[cursor], selected_frame);
            }
            "out" | "finish" => {
                cursor = debug_step_index(&debug, cursor, DebugAction::StepOut);
                selected_frame = selected_frame.min(
                    debug.snapshots[cursor].frames.len().saturating_sub(1),
                );
                render_debug_snapshot(&debug.snapshots[cursor], selected_frame);
            }
            "continue" | "c" => {
                cursor = debug_step_index(&debug, cursor, DebugAction::Continue);
                selected_frame =
                    selected_frame.min(debug.snapshots[cursor].frames.len().saturating_sub(1));
                render_debug_snapshot(&debug.snapshots[cursor], selected_frame);
            }
            "stack" => render_debug_stack(&debug.snapshots[cursor]),
            "locals" => render_debug_locals(&debug.snapshots[cursor], selected_frame),
            "globals" => render_debug_globals(&debug.snapshots[cursor]),
            "help" => {
                println!("step|s|into      move to the next recorded instruction");
                println!("next|n|over      step over function calls");
                println!("out|finish       run until the current frame returns");
                println!("continue|c       run until the next breakpoint or the end");
                println!("stack            show the current call stack");
                println!("locals           show locals for the selected frame");
                println!("globals          show global values");
                println!("frame <n>        select a frame for inspection");
                println!("print <expr>     inspect a variable or dotted path");
                println!("inspect <expr>   same as print");
                println!("exit             leave the debugger");
            }
            "exit" | "quit" => break,
            _ if command.starts_with("frame ") => {
                let index = command
                    .trim_start_matches("frame ")
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "frame expects a numeric index")?;
                if index >= debug.snapshots[cursor].frames.len() {
                    println!("{} frame {index} is out of range", color("debug", 33));
                } else {
                    selected_frame = index;
                    render_debug_locals(&debug.snapshots[cursor], selected_frame);
                }
            }
            _ if command.starts_with("print ") || command.starts_with("inspect ") => {
                let expr = command
                    .split_once(' ')
                    .map(|(_, value)| value.trim())
                    .unwrap_or("");
                let value = inspect_debug_snapshot(&debug.snapshots[cursor], expr, selected_frame);
                println!(" - {} = {}", expr, value);
            }
            _ => println!("{} unknown debug command", color("debug", 33)),
        }
    }
    if let Some(profile) = report.profile {
        println!(" - instructions: {}", profile.instruction_count);
        println!(" - functions: {}", profile.function_calls);
        println!(" - builtins: {}", profile.builtin_calls);
        println!(" - max stack: {}", profile.max_stack_depth);
    }
    Ok(())
}

fn handle_optimize(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = resolve_input_file(first_positional(args, 1))?;
    let project_root = plugin_root_for(&file);
    emit_cli_command_hook("optimize", &project_root, &file, args);

    let lowered = lower_file(&file)?;
    let optimized = edgelvm::optimizer::optimize_ir(&lowered);
    let raw_program = edgelvm::compiler::compile_unoptimized(&lowered)?;
    let optimized_program = edgelvm::compiler::compile_unoptimized(&optimized)?;
    let raw_instructions = instruction_count(&raw_program.entry)
        + raw_program
            .functions
            .values()
            .map(|function| instruction_count(&function.chunk))
            .sum::<usize>();
    let optimized_instructions = instruction_count(&optimized_program.entry)
        + optimized_program
            .functions
            .values()
            .map(|function| instruction_count(&function.chunk))
            .sum::<usize>();

    let output_dir = optimize_output_dir(&file);
    fs::create_dir_all(&output_dir)?;
    let output_file = output_dir.join(
        file.file_stem()
            .and_then(|value| value.to_str())
            .map(|value| format!("{value}.eglc"))
            .unwrap_or_else(|| "optimized.eglc".to_string()),
    );
    fs::write(
        &output_file,
        edgelvm::compiler::serialize_bytecode(&optimized_program),
    )?;

    println!("{} {}", color("optimize", 32), file.display());
    println!(" - raw instructions: {}", raw_instructions);
    println!(" - optimized instructions: {}", optimized_instructions);
    println!(
        " - delta: {}",
        raw_instructions as isize - optimized_instructions as isize
    );
    println!(" - output: {}", output_file.display());
    Ok(())
}

fn handle_info(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = resolve_input_file(first_positional(args, 1))?;
    let project_root = plugin_root_for(&file);
    emit_cli_command_hook("info", &project_root, &file, args);

    let program = parse_file(&file)?;
    let bytecode = compile_file(&file)?;
    let tests = collect_tests(&program);
    let plugins = discover_plugins(&project_root)?;
    let instruction_count = instruction_count(&bytecode.entry)
        + bytecode
            .functions
            .values()
            .map(|function| instruction_count(&function.chunk))
            .sum::<usize>();

    println!("{} {}", color("info", 36), file.display());
    println!(" - summary: {}", edgelvm::render::program_summary(&program));
    println!(
        " - project root: {}",
        find_project_root(&file)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not detected".to_string())
    );
    println!(" - functions: {}", bytecode.functions.len());
    println!(" - tests: {}", tests.len());
    println!(" - instructions: {}", instruction_count);
    println!(" - plugins: {}", plugins.len());
    if let Ok(manifest) = load_manifest(&project_root) {
        println!(" - package: {}@{}", manifest.name, manifest.version);
        println!(" - dependencies: {}", manifest.dependencies.len());
        match verify_lockfile(&project_root) {
            Ok(lockfile) => println!(" - lockfile: verified ({})", lockfile.packages.len()),
            Err(error) => println!(" - lockfile: invalid ({})", error.message),
        }
    }
    Ok(())
}

fn handle_profile(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = resolve_input_file(first_positional(args, 1))?;
    let project_root = plugin_root_for(&file);
    emit_cli_command_hook("profile", &project_root, &file, args);
    let options = VmOptions {
        debug: has_flag(args, "--debug"),
        profile: true,
        trace: false,
        max_instructions: None,
        breakpoints: Vec::new(),
    };
    let report = run_project_file(&file, options)?;
    println!("{} {}", color("profile", 36), file.display());
    println!("{}", report.summary);
    if let Some(profile) = report.profile {
        println!(" - instructions: {}", profile.instruction_count);
        println!(" - functions: {}", profile.function_calls);
        println!(" - builtins: {}", profile.builtin_calls);
        println!(" - caught errors: {}", profile.caught_errors);
        println!(" - max stack: {}", profile.max_stack_depth);
        println!(" - elapsed ms: {}", profile.elapsed_ms);
        if !profile.function_hits.is_empty() {
            println!(
                " - function hits: {}",
                profile
                    .function_hits
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    for line in report.console {
        println!(" - console: {line}");
    }
    emit_plugin_hooks_best_effort(
        &project_root,
        PluginHook::OnExecute,
        event_map([
            ("command", Value::String("profile".to_string())),
            ("file", Value::String(file.display().to_string())),
        ]),
    );
    Ok(())
}

fn handle_init(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let root = match first_positional(args, 1) {
        Some(path) => PathBuf::from(path),
        None => env::current_dir()?.join("edgel-project"),
    };
    let template = option_value(args, "--template").unwrap_or_else(|| "app".to_string());
    scaffold_project(&root, &template, "init")
}

fn handle_new(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(
        first_positional(args, 1).unwrap_or_else(|| "my-app".to_string()),
    );
    let template = option_value(args, "--template").unwrap_or_else(|| "app".to_string());
    scaffold_project(&root, &template, "new")
}

fn handle_install(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let package = first_positional(args, 1).ok_or("use `edgel install <package> [version]`")?;
    let version = first_positional(args, 2);
    let cwd = env::current_dir()?;
    let report = install_dependency(&cwd, &package, version.as_deref())?;
    println!("{} {}", color("install", 32), report.summary);
    for file in report.files {
        println!(" - {}", file.display());
    }
    Ok(())
}

fn handle_update(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let report = update_dependencies(&cwd)?;
    println!("{} {}", color("update", 32), report.summary);
    for file in report.files {
        println!(" - {}", file.display());
    }
    Ok(())
}

fn handle_publish(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let report = publish_package(&cwd)?;
    println!("{} {}", color("publish", 32), report.summary);
    for file in report.files {
        println!(" - {}", file.display());
    }
    Ok(())
}

fn handle_plugin(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.get(1).map(String::as_str) {
        Some("list") | None => {
            let root = env::current_dir()?;
            let plugins = discover_plugins(&root)?;
            if plugins.is_empty() {
                println!("{} no plugins discovered", color("plugin", 36));
            } else {
                println!("{} {}", color("plugin", 36), root.display());
                for plugin in plugins {
                    let hooks = plugin
                        .hooks
                        .iter()
                        .map(|hook| hook.label())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let permissions = plugin.permissions.join(", ");
                    println!(
                        " - {} [{}] perms=[{}] order={}",
                        plugin.name, hooks, permissions, plugin.order
                    );
                }
            }
            Ok(())
        }
        Some("inspect") => {
            let name = args.get(2).ok_or("use `edgel plugin inspect <name>`")?;
            let root = env::current_dir()?;
            let Some(plugin) = discover_plugins(&root)?
                .into_iter()
                .find(|plugin| &plugin.name == name)
            else {
                return Err(format!("plugin `{name}` not found").into());
            };
            println!("{} {}", color("plugin", 36), plugin.name);
            println!(" - path: {}", plugin.path.display());
            println!(
                " - hooks: {}",
                plugin
                    .hooks
                    .iter()
                    .map(|hook| hook.label())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(" - permissions: {}", plugin.permissions.join(", "));
            println!(
                " - version: {}",
                plugin.version.unwrap_or_else(|| "unspecified".to_string())
            );
            println!(" - order: {}", plugin.order);
            println!(
                " - channel: {}",
                plugin.channel.unwrap_or_else(|| "default".to_string())
            );
            Ok(())
        }
        Some("init") | Some("add") => {
            let name = args.get(2).ok_or("use `edgel plugin init <name>`")?;
            let root = env::current_dir()?;
            let files = scaffold_plugin(&root, name)?;
            println!("{} {}", color("plugin", 32), name);
            for file in files {
                println!(" - {}", file.display());
            }
            Ok(())
        }
        Some("remove") => {
            let name = args.get(2).ok_or("use `edgel plugin remove <name>`")?;
            let root = env::current_dir()?;
            remove_plugin(&root, name)?;
            println!("{} removed {}", color("plugin", 32), name);
            Ok(())
        }
        _ => Err("use `edgel plugin list`, `edgel plugin inspect <name>`, `edgel plugin init <name>`, or `edgel plugin remove <name>`".into()),
    }
}

fn repl() -> Result<(), Box<dyn std::error::Error>> {
    println!("EDGEL REPL. Type `exit` to leave.");
    let mut line = String::new();
    loop {
        line.clear();
        print!("edgel> ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut line)?;
        if line.trim() == "exit" {
            break;
        }
        match run_source(&line) {
            Ok(output) => {
                if output.console.is_empty() {
                    println!("No console output.");
                } else {
                    for item in output.console {
                        println!("{item}");
                    }
                }
            }
            Err(error) => println!("{error}"),
        }
    }
    Ok(())
}

fn resolve_input_file(maybe_path: Option<String>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = maybe_path {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("file not found: {}", path.display()).into());
    }

    let cwd = env::current_dir()?;
    default_entry_file(&cwd)
        .ok_or_else(|| {
            "missing .egl source file and no `src/main.egl` project entry was found. Start with `edgel new my-app`."
                .into()
        })
}

fn default_test_root() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(root) = find_project_root(&cwd) {
        root.join("tests")
    } else {
        cwd.join("tests")
    }
}

fn discover_tests(path: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut files = Vec::new();
    walk_tests(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_tests(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_tests(&path, files)?;
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.ends_with(".test.egl"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn plugin_root_for(path: &Path) -> PathBuf {
    find_project_root(path).unwrap_or_else(|| {
        if path.is_file() {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            path.to_path_buf()
        }
    })
}

fn emit_plugin_hooks_best_effort(root: &Path, hook: PluginHook, event: BTreeMap<String, Value>) {
    match run_plugin_hooks(root, hook, event, plugin_vm_options()) {
        Ok(executions) => {
            for execution in executions {
                for line in execution.console {
                    println!("{} {}: {}", color("plugin", 35), execution.plugin, line);
                }
                if let Some(value) = execution.return_value {
                    println!("{} {} -> {}", color("plugin", 35), execution.plugin, value);
                }
            }
        }
        Err(error) => {
            eprintln!("{} {}", color("plugin-warning", 33), error);
        }
    }
}

fn emit_cli_command_hook(command: &str, root: &Path, target: &Path, args: &[String]) {
    emit_plugin_hooks_best_effort(
        root,
        PluginHook::OnCliCommand,
        event_map([
            ("command", Value::String(command.to_string())),
            ("target", Value::String(target.display().to_string())),
            (
                "args",
                Value::List(args.iter().cloned().map(Value::String).collect()),
            ),
        ]),
    );
}

fn event_map<const N: usize>(entries: [(&str, Value); N]) -> BTreeMap<String, Value> {
    entries
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn instruction_count(chunk: &edgelvm::compiler::Chunk) -> usize {
    chunk.instructions.len()
        + chunk
            .instructions
            .iter()
            .map(|instruction| match instruction {
                edgelvm::compiler::Instruction::Branch {
                    then_chunk,
                    else_chunk,
                } => instruction_count(then_chunk) + instruction_count(else_chunk),
                edgelvm::compiler::Instruction::TryCatch {
                    try_chunk,
                    catch_chunk,
                    ..
                } => instruction_count(try_chunk) + instruction_count(catch_chunk),
                edgelvm::compiler::Instruction::RangeLoop { body, .. }
                | edgelvm::compiler::Instruction::EachLoop { body, .. } => instruction_count(body),
                _ => 0,
            })
            .sum::<usize>()
}

fn optimize_output_dir(file: &Path) -> PathBuf {
    find_project_root(file).unwrap_or_else(|| {
        if file.is_file() {
            file.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            file.to_path_buf()
        }
    })
    .join("dist")
    .join("optimized")
}

fn parse_debug_breakpoints(
    args: &[String],
) -> Result<Vec<DebugBreakpoint>, Box<dyn std::error::Error>> {
    let mut breakpoints = Vec::new();
    let mut index = 0usize;
    while index < args.len() {
        if args[index] == "--breakpoint" {
            let spec = args
                .get(index + 1)
                .ok_or("`--breakpoint` expects a line number or `function:<name>`")?;
            if let Ok(line) = spec.parse::<usize>() {
                breakpoints.push(DebugBreakpoint::Line(line));
            } else if let Some(name) = spec.strip_prefix("function:") {
                breakpoints.push(DebugBreakpoint::Function(name.to_string()));
            } else {
                breakpoints.push(DebugBreakpoint::Function(spec.to_string()));
            }
            index += 1;
        }
        index += 1;
    }
    Ok(breakpoints)
}

fn render_debug_snapshot(snapshot: &DebugSnapshot, selected_frame: usize) {
    let pause = snapshot
        .pause_reason
        .as_ref()
        .map(|reason| format!(" pause={reason}"))
        .unwrap_or_default();
    if snapshot.line > 0 {
        println!(
            "[DEBUG] line {} -> {} | {}{}",
            snapshot.line, snapshot.summary, snapshot.instruction, pause
        );
    } else {
        println!("[DEBUG] {} | {}{}", snapshot.summary, snapshot.instruction, pause);
    }
    render_debug_locals(snapshot, selected_frame);
}

fn render_debug_stack(snapshot: &DebugSnapshot) {
    if snapshot.frames.is_empty() {
        println!(" - stack: <empty>");
        return;
    }
    println!(" - stack:");
    for (index, frame) in snapshot.frames.iter().enumerate() {
        if frame.line > 0 {
            println!("   {}: {} at line {} -> {}", index, frame.function, frame.line, frame.summary);
        } else {
            println!("   {}: {} -> {}", index, frame.function, frame.summary);
        }
    }
}

fn render_debug_locals(snapshot: &DebugSnapshot, selected_frame: usize) {
    let Some(frame) = snapshot.frames.get(selected_frame) else {
        println!(" - locals: frame {} not available", selected_frame);
        return;
    };
    println!(" - frame {}: {}", selected_frame, frame.function);
    if frame.locals.is_empty() {
        println!("   <no locals>");
    } else {
        for (name, value) in &frame.locals {
            println!("   {} = {}", name, value);
        }
    }
}

fn render_debug_globals(snapshot: &DebugSnapshot) {
    if snapshot.globals.is_empty() {
        println!(" - globals: <empty>");
        return;
    }
    println!(" - globals:");
    for (name, value) in &snapshot.globals {
        println!("   {} = {}", name, value);
    }
}

#[derive(Debug)]
struct TestCaseReport {
    file: String,
    name: String,
    ok: bool,
    function_hits: Vec<String>,
}

fn write_test_report(
    root: &Path,
    cases: &[TestCaseReport],
    passed: usize,
    failed: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = root.join("dist").join("test-report.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = format!(
        "{{\n  \"passed\": {},\n  \"failed\": {},\n  \"cases\": [\n{}\n  ]\n}}\n",
        passed,
        failed,
        cases
            .iter()
            .map(|case| format!(
                "    {{\"file\":\"{}\",\"name\":\"{}\",\"ok\":{},\"function_hits\":[{}]}}",
                json_escape(&case.file),
                json_escape(&case.name),
                case.ok,
                case
                    .function_hits
                    .iter()
                    .map(|name| format!("\"{}\"", json_escape(name)))
                    .collect::<Vec<_>>()
                    .join(",")
            ))
            .collect::<Vec<_>>()
            .join(",\n")
    );
    fs::write(&path, body)?;
    Ok(path)
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

fn command_version(command: &str, arg: &str) -> Option<String> {
    Command::new(command)
        .arg(arg)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn first_positional(args: &[String], start: usize) -> Option<String> {
    args.iter()
        .skip(start)
        .find(|arg| !arg.starts_with("--"))
        .cloned()
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn color(label: &str, code: u8) -> String {
    format!("\u{1b}[{}m{}\u{1b}[0m", code, label)
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

fn print_help() {
    println!("edgel run <file.egl> [--debug] [--profile]");
    println!("edgel debug <file.egl> [--profile] [--breakpoint <line|function:name>]");
    println!("edgel build <file.egl>");
    println!("edgel build --web [file.egl]");
    println!("edgel build --apk [file.egl]");
    println!("edgel build --bytecode [file.egl]");
    println!("edgel optimize <file.egl>");
    println!("edgel serve");
    println!("edgel plugin list");
    println!("edgel plugin inspect <name>");
    println!("edgel plugin init <name>");
    println!("edgel plugin remove <name>");
    println!("edgel repl");
    println!("edgel test [tests|file.test.egl] [--report] [--coverage]");
    println!("edgel doctor");
    println!("edgel info [file.egl]");
    println!("edgel profile [file.egl] [--debug]");
    println!("edgel install <package> [version]");
    println!("edgel update");
    println!("edgel publish");
    println!("edgel new <project-dir> [--template app|web|api]");
    println!("edgel init [project-dir] [--template app|web|api]");
    println!("edgel learn [lesson-number]");
    println!("edgel ai explain <file.egl>");
    println!("edgel ai fix <file.egl>");
    println!("edgel parse <file.egl>");
    println!("edgel ir <file.egl>");
    println!("edgel bytecode <file.egl>");
    let _ = (
        compile_source as fn(&str) -> _,
        lower_source as fn(&str) -> _,
        parse_source as fn(&str) -> _,
        explain_source as fn(&str) -> _,
        fix_source as fn(&str) -> _,
        init_project as fn(&Path) -> _,
    );
}

fn scaffold_project(
    root: &Path,
    template: &str,
    command: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = init_project_with_template(root, template)?;
    println!("{} {}", color(command, 32), report.root.display());
    println!(" - template: {}", template);
    for file in report.files {
        println!(" - {}", file.display());
    }
    println!("Next:");
    println!(" - cd {}", report.root.display());
    println!(" - edgel run");
    println!(" - edgel test");
    println!(" - edgel learn");
    println!(
        "Templates available: {}",
        available_project_templates().join(", ")
    );
    Ok(())
}
