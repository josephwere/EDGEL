pub mod ast;
pub mod compiler;
pub mod diagnostics;
pub mod ir;
pub mod lexer;
pub mod loader;
pub mod manifest;
pub mod neuroedge;
pub mod optimizer;
pub mod parser;
pub mod plugins;
pub mod project;
pub mod render;
pub mod server;
pub mod telemetry;
pub mod value;
pub mod vm;

use crate::compiler::{compile, BytecodeProgram};
use crate::diagnostics::Diagnostic;
use crate::ir::{lower_to_ir, IrProgram};
use crate::lexer::lex;
use crate::loader::load_program_from_file;
use crate::parser::parse;
use crate::vm::{execute_with_options, VmOutput};
use ast::Program;
use std::path::Path;

pub fn parse_source(source: &str) -> Result<Program, Diagnostic> {
    let tokens = lex(source)?;
    parse(&tokens)
}

pub fn lower_source(source: &str) -> Result<IrProgram, Diagnostic> {
    let program = parse_source(source)?;
    Ok(lower_to_ir(&program))
}

pub fn compile_source(source: &str) -> Result<BytecodeProgram, Diagnostic> {
    let program = parse_source(source)?;
    compile_program(&program)
}

pub fn parse_file(path: &Path) -> Result<Program, Diagnostic> {
    load_program_from_file(path)
}

pub fn lower_file(path: &Path) -> Result<IrProgram, Diagnostic> {
    let program = parse_file(path)?;
    Ok(lower_to_ir(&program))
}

pub fn compile_file(path: &Path) -> Result<BytecodeProgram, Diagnostic> {
    let program = parse_file(path)?;
    compile_program(&program)
}

pub fn compile_program(program: &Program) -> Result<BytecodeProgram, Diagnostic> {
    let ir = lower_to_ir(program);
    compile(&ir)
}

pub fn run_source(source: &str) -> Result<VmOutput, Diagnostic> {
    run_source_with_options(source, VmOptions::default())
}

pub fn run_source_with_options(source: &str, options: VmOptions) -> Result<VmOutput, Diagnostic> {
    let program = parse_source(source)?;
    run_program_with_options(&program, options)
}

pub fn run_file(path: &Path) -> Result<VmOutput, Diagnostic> {
    run_file_with_options(path, VmOptions::default())
}

pub fn run_file_with_options(path: &Path, options: VmOptions) -> Result<VmOutput, Diagnostic> {
    let program = parse_file(path)?;
    run_program_with_options(&program, options)
}

pub fn run_program(program: &Program) -> Result<VmOutput, Diagnostic> {
    run_program_with_options(program, VmOptions::default())
}

pub fn run_program_with_options(program: &Program, options: VmOptions) -> Result<VmOutput, Diagnostic> {
    let bytecode = compile_program(program)?;
    execute_with_options(&bytecode, options)
}

pub use project::{
    build_apk_bundle, build_apk_bundle_from_file, build_bytecode_bundle,
    build_bytecode_bundle_from_file,
    build_web_bundle, build_web_bundle_from_file, collect_tests, default_entry_file,
    explain_file, explain_source, find_project_root, fix_file, fix_source,
    init_project, init_project_with_template, available_project_templates,
    run_project, run_project_file, run_project_with_options, run_test_file, BuildReport,
    ProjectInitReport, RunReport, TestCase, TestRunResult,
};
pub use plugins::{
    discover_plugins, remove_plugin, run_plugin_hooks, scaffold_plugin, PluginDescriptor,
    PluginExecution, PluginHook,
};
pub use manifest::{
    install_dependency, load_lockfile, load_manifest, publish_package, save_lockfile,
    save_manifest, update_dependencies, verify_lockfile, LockedPackage,
    PackageOperationReport, ProjectLockfile, ProjectManifest,
};
pub use server::serve;
pub use value::Value;
pub use vm::{
    debug_step_index, inspect_debug_snapshot, DebugAction, DebugBreakpoint, DebugFrame,
    DebugRecord, DebugSnapshot, VmOptions, VmProfile,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn runs_basic_logic_program() {
        let source = r#"
function add(a, b) {
    return a + b
}

let result = add(5, 10)
print("Result: " + result)
"#;
        let output = run_source(source).expect("program should run");
        assert_eq!(output.console, vec!["Result: 15"]);
    }

    #[test]
    fn renders_app_preview() {
        let source = r#"
app Hello {
    screen Main {
        text("Hello")
    }
}
"#;
        let program = parse_source(source).expect("program should parse");
        let html = render::render_preview_document(&program).expect("preview");
        assert!(html.contains("Hello"));
        assert!(html.contains("GoldEdge Browser Preview"));
    }

    #[test]
    fn builds_web_bundle() {
        let output_dir = PathBuf::from("/tmp/edgelvm-test-web");
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).expect("clean dir");
        }
        let source = r#"
app Demo {
    screen Main {
        text("Hi")
    }
}
"#;
        let report = build_web_bundle(source, &output_dir).expect("build should work");
        assert!(report.output_dir.exists());
        assert!(output_dir.join("index.html").exists());
    }

    #[test]
    fn supports_try_catch_and_assertions() {
        let source = r#"
function fail() {
    assert(false, "boom")
}

try {
    fail()
} catch err {
    print(err.message)
}
"#;
        let output = run_source(source).expect("try/catch should recover");
        assert_eq!(output.console, vec!["boom"]);
    }

    #[test]
    fn loads_imported_modules_from_files() {
        let root = PathBuf::from("/tmp/edgelvm-import-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean import test dir");
        }
        fs::create_dir_all(root.join("support")).expect("support dir");
        fs::write(
            root.join("support/math.egl"),
            "function add(a: number, b: number): number { return a + b }\n",
        )
        .expect("write support module");
        fs::write(
            root.join("main.egl"),
            "import \"support/math.egl\"\nprint(add(2, 3))\n",
        )
        .expect("write main module");

        let output = run_file(&root.join("main.egl")).expect("imported file should run");
        assert_eq!(output.console, vec!["5"]);
    }

    #[test]
    fn loads_workspace_stdlib_modules() {
        let root = PathBuf::from("/tmp/edgelvm-stdlib-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean stdlib test dir");
        }
        fs::create_dir_all(&root).expect("stdlib test dir");
        fs::write(
            root.join("main.egl"),
            "import ui\nprint(screenTitle(\"Hello Stdlib\"))\n",
        )
        .expect("write stdlib main file");

        let output = run_file(&root.join("main.egl")).expect("stdlib import should run");
        assert_eq!(output.console, vec!["Hello Stdlib"]);
    }

    #[test]
    fn loads_namespaced_stdlib_modules() {
        let root = PathBuf::from("/tmp/edgelvm-stdlib-namespace-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean stdlib namespace test dir");
        }
        fs::create_dir_all(&root).expect("stdlib namespace test dir");
        fs::write(
            root.join("main.egl"),
            "import std.ui\nprint(screenTitle(\"Hello Namespace\"))\n",
        )
        .expect("write namespaced stdlib main file");

        let output = run_file(&root.join("main.egl")).expect("namespaced stdlib import should run");
        assert_eq!(output.console, vec!["Hello Namespace"]);
    }

    #[test]
    fn runs_named_test_blocks() {
        let root = PathBuf::from("/tmp/edgelvm-test-blocks");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean named test dir");
        }
        fs::create_dir_all(&root).expect("named test dir");
        fs::write(
            root.join("math.test.egl"),
            r#"function add(a, b) {
    return a + b
}

test "sum works" {
    assert(add(2, 3) == 5, "sum should match")
}
"#,
        )
        .expect("write named test file");

        let results = run_test_file(&root.join("math.test.egl"), VmOptions::default())
            .expect("named test blocks should run");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "sum works");
    }

    #[test]
    fn parser_reports_recovered_errors() {
        let error = parse_source(
            r#"
function broken( {
print("oops")
let x =
"#,
        )
        .expect_err("parser should report invalid syntax");
        assert!(error
            .notes
            .iter()
            .any(|note| note.contains("parser recovered")));
        assert!(!error.related.is_empty());
    }

    #[test]
    fn parser_uses_beginner_friendly_messages() {
        let error = parse_source(
            r#"
function broken() {
    print("hello")
"#,
        )
        .expect_err("parser should flag the missing closing brace");
        assert!(error.message.contains("missed a closing bracket"));
    }

    #[test]
    fn debug_trace_records_steps() {
        let output = run_source_with_options(
            "let x = 1\nprint(x)\n",
            VmOptions {
                debug: true,
                profile: true,
                trace: true,
                max_instructions: None,
                breakpoints: Vec::new(),
            },
        )
        .expect("debug execution should succeed");
        assert!(!output.trace.is_empty());
        assert!(output
            .trace
            .iter()
            .any(|line| line.contains("Store(\"x\")")));
    }

    #[test]
    fn debug_records_breakpoints_and_supports_inspection() {
        let output = run_source_with_options(
            "let user = { name: \"Asha\" }\nbreakpoint()\nprint(user.name)\n",
            VmOptions {
                debug: true,
                profile: true,
                trace: true,
                max_instructions: None,
                breakpoints: Vec::new(),
            },
        )
        .expect("debug execution should succeed");
        let record = output.debug.expect("debug record");
        let snapshot = record
            .snapshots
            .iter()
            .find(|snapshot| snapshot.pause_reason.as_deref() == Some("breakpoint()"))
            .expect("breakpoint snapshot");
        assert_eq!(
            inspect_debug_snapshot(snapshot, "user.name", 0),
            Value::String("Asha".to_string())
        );
    }

    #[test]
    fn debug_respects_line_breakpoints() {
        let output = run_source_with_options(
            "let x = 1\nlet y = 2\nprint(y)\n",
            VmOptions {
                debug: true,
                profile: false,
                trace: true,
                max_instructions: None,
                breakpoints: vec![DebugBreakpoint::Line(2)],
            },
        )
        .expect("debug execution should succeed");
        let record = output.debug.expect("debug record");
        assert!(record.snapshots.iter().any(|snapshot| {
            snapshot.pause_reason.as_deref() == Some("line breakpoint 2")
        }));
    }

    #[test]
    fn scaffolds_web_template_projects() {
        let root = PathBuf::from("/tmp/edgelvm-web-template");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean template dir");
        }

        let report = init_project_with_template(&root, "web").expect("template scaffold");
        assert!(report.root.join("src/main.egl").exists());
        let main = fs::read_to_string(report.root.join("src/main.egl")).expect("read main");
        assert!(main.contains("web "));
        let readme = fs::read_to_string(report.root.join("README.md")).expect("read readme");
        assert!(readme.contains("Template: `web`"));
    }

    #[test]
    fn rejects_unknown_project_templates() {
        let root = PathBuf::from("/tmp/edgelvm-bad-template");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean bad template dir");
        }

        let error = init_project_with_template(&root, "desktop")
            .expect_err("unknown templates should fail");
        assert!(error.message.contains("unknown template"));
        assert!(error
            .notes
            .iter()
            .any(|note| note.contains("Available templates")));
    }

    #[test]
    fn installs_and_publishes_local_package_metadata() {
        let root = PathBuf::from("/tmp/edgelvm-package-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean package dir");
        }
        fs::create_dir_all(root.join("src")).expect("src dir");
        fs::write(root.join("src/main.egl"), "print(\"hello\")\n").expect("entry");
        fs::write(
            root.join("edgel.json"),
            "{\n  \"name\": \"pkg-test\",\n  \"version\": \"1.2.0\",\n  \"entry\": \"src/main.egl\",\n  \"dependencies\": {\n  }\n}\n",
        )
        .expect("manifest");

        let install = install_dependency(&root, "logger", Some("^1.0.0")).expect("install");
        assert!(install
            .files
            .iter()
            .any(|path| path.ends_with("packages/logger/package.json")));
        assert!(install
            .files
            .iter()
            .any(|path| path.ends_with("edgel.lock")));
        let lockfile = load_lockfile(&root).expect("lockfile");
        assert_eq!(
            lockfile
                .packages
                .get("logger")
                .map(|package| package.requested.as_str()),
            Some("^1.0.0")
        );

        let publish = publish_package(&root).expect("publish");
        assert!(publish
            .files
            .iter()
            .any(|path| path.ends_with(".edgel/registry/pkg-test/1.2.0/package.json")));
        assert!(publish
            .files
            .iter()
            .any(|path| path.ends_with(".edgel/registry/pkg-test/1.2.0/package.sum")));
    }

    #[test]
    fn rejects_tampered_package_cache_until_update_repairs_it() {
        let root = PathBuf::from("/tmp/edgelvm-lockfile-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean lockfile dir");
        }
        fs::create_dir_all(root.join("src")).expect("src dir");
        fs::write(root.join("src/main.egl"), "print(\"secure\")\n").expect("entry");
        fs::write(
            root.join("edgel.json"),
            "{\n  \"name\": \"lock-test\",\n  \"version\": \"0.1.0\",\n  \"entry\": \"src/main.egl\",\n  \"dependencies\": {\n    \"logger\": \"^1.0.0\"\n  }\n}\n",
        )
        .expect("manifest");

        install_dependency(&root, "logger", Some("^1.0.0")).expect("install");
        verify_lockfile(&root).expect("fresh lock should verify");
        let initial = run_project_file(&root.join("src/main.egl"), VmOptions::default())
            .expect("project should run with verified lock");
        assert_eq!(initial.console, vec!["secure"]);

        fs::write(
            root.join("packages/logger/package.json"),
            "{\n  \"name\": \"logger\",\n  \"version\": \"9.9.9\",\n  \"source\": \"tampered\"\n}\n",
        )
        .expect("tamper package");
        let error = run_project_file(&root.join("src/main.egl"), VmOptions::default())
            .expect_err("tampered package should be rejected");
        assert!(error.message.contains("checksum mismatch"));

        update_dependencies(&root).expect("update should restore package cache and lock");
        verify_lockfile(&root).expect("lock should verify after update");
        let repaired = run_project_file(&root.join("src/main.egl"), VmOptions::default())
            .expect("project should run after update repairs tampering");
        assert_eq!(repaired.console, vec!["secure"]);
    }

    #[test]
    fn installs_from_local_registry_and_locks_resolved_version() {
        let root = PathBuf::from("/tmp/edgelvm-registry-lock-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean registry dir");
        }
        fs::create_dir_all(root.join("src")).expect("src dir");
        fs::write(root.join("src/main.egl"), "print(\"registry\")\n").expect("entry");
        fs::write(
            root.join("edgel.json"),
            "{\n  \"name\": \"logger\",\n  \"version\": \"1.4.0\",\n  \"entry\": \"src/main.egl\",\n  \"dependencies\": {\n  }\n}\n",
        )
        .expect("manifest");

        publish_package(&root).expect("publish");
        install_dependency(&root, "logger", Some("^1.0.0")).expect("install from registry");

        let lockfile = load_lockfile(&root).expect("lockfile");
        let package = lockfile.packages.get("logger").expect("locked package");
        assert_eq!(package.version, "1.4.0");
        assert_eq!(package.source, "local-registry");
    }

    #[test]
    fn discovers_and_runs_plugin_hooks() {
        let root = PathBuf::from("/tmp/edgelvm-plugin-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean plugin dir");
        }
        fs::create_dir_all(root.join("plugins/logger")).expect("plugin dir");
        fs::write(root.join("edgel.json"), "{ \"name\": \"plugin-test\" }\n").expect("manifest");
        fs::write(
            root.join("plugins/logger/plugin.egl"),
            r#"function onRun(event) {
    print("plugin command " + event.command)
}
"#,
        )
        .expect("plugin file");

        let plugins = discover_plugins(&root).expect("plugins should load");
        assert_eq!(plugins.len(), 1);
        let executions = run_plugin_hooks(
            &root,
            PluginHook::OnRun,
            BTreeMap::from([("command".to_string(), Value::String("run".to_string()))]),
            VmOptions::default(),
        )
        .expect("hook execution");
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].console, vec!["plugin command run"]);
    }

    #[test]
    fn chains_plugin_return_values_across_hooks() {
        let root = PathBuf::from("/tmp/edgelvm-plugin-chain-test");
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean plugin chain dir");
        }
        fs::create_dir_all(root.join("plugins/logger")).expect("logger dir");
        fs::create_dir_all(root.join("plugins/auditor")).expect("auditor dir");
        fs::write(root.join("edgel.json"), "{ \"name\": \"plugin-chain-test\" }\n")
            .expect("manifest");
        fs::write(
            root.join("plugins/logger/plugin.egl"),
            r#"model plugin {
    order: 10
    permissions: ["run"]
}

function onRun(event) {
    return { lastCommand: event.command }
}
"#,
        )
        .expect("logger file");
        fs::write(
            root.join("plugins/auditor/plugin.egl"),
            r#"model plugin {
    order: 20
    permissions: ["run"]
}

function onRun(event) {
    print(event.plugins.logger.lastCommand)
}
"#,
        )
        .expect("auditor file");

        let executions = run_plugin_hooks(
            &root,
            PluginHook::OnRun,
            BTreeMap::from([("command".to_string(), Value::String("run".to_string()))]),
            VmOptions::default(),
        )
        .expect("plugin chain execution");
        assert_eq!(executions.len(), 2);
        assert!(executions.iter().any(|execution| execution.console == vec!["run"]));
    }
}
