use crate::ast::{Item, Program};
use crate::compiler::serialize_bytecode;
use crate::diagnostics::Diagnostic;
use crate::ir::test_function_name;
use crate::loader::load_program_from_file;
use crate::manifest::verify_lockfile;
use crate::neuroedge::{explain_program, suggest_fixes};
use crate::parse_source;
use crate::render::{program_summary, render_preview_document};
use crate::value::Value;
use crate::vm::{execute_function_with_options, DebugRecord, VmOptions, VmProfile};
use crate::run_program_with_options;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RunReport {
    pub console: Vec<String>,
    pub html_preview: Option<String>,
    pub summary: String,
    pub profile: Option<VmProfile>,
    pub globals: std::collections::BTreeMap<String, Value>,
    pub trace: Vec<String>,
    pub debug: Option<DebugRecord>,
}

#[derive(Debug, Clone)]
pub struct BuildReport {
    pub output_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct ProjectInitReport {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub function_name: String,
}

#[derive(Debug, Clone)]
pub struct TestRunResult {
    pub name: String,
    pub console: Vec<String>,
    pub profile: Option<VmProfile>,
}

pub fn run_project(source: &str) -> Result<RunReport, Diagnostic> {
    run_project_with_options(source, VmOptions::default())
}

pub fn run_project_with_options(source: &str, options: VmOptions) -> Result<RunReport, Diagnostic> {
    let program = parse_source(source)?;
    let runtime = run_program_with_options(&program, options)?;
    Ok(RunReport {
        console: runtime.console,
        html_preview: render_preview_document(&program),
        summary: program_summary(&program),
        profile: runtime.profile,
        globals: runtime.globals,
        trace: runtime.trace,
        debug: runtime.debug,
    })
}

pub fn run_project_file(path: &Path, options: VmOptions) -> Result<RunReport, Diagnostic> {
    verify_project_dependencies(path)?;
    let program = load_program_from_file(path)?;
    let runtime = run_program_with_options(&program, options)?;
    Ok(RunReport {
        console: runtime.console,
        html_preview: render_preview_document(&program),
        summary: program_summary(&program),
        profile: runtime.profile,
        globals: runtime.globals,
        trace: runtime.trace,
        debug: runtime.debug,
    })
}

pub fn explain_source(source: &str) -> Result<String, Diagnostic> {
    let program = parse_source(source)?;
    Ok(explain_program(&program))
}

pub fn explain_file(path: &Path) -> Result<String, Diagnostic> {
    let program = load_program_from_file(path)?;
    Ok(explain_program(&program))
}

pub fn fix_source(source: &str) -> String {
    match parse_source(source) {
        Ok(program) => format!(
            "NEUROEDGE did not find syntax errors.\n{}\nSuggestion: expand this into more screens, APIs, or tests next.",
            explain_program(&program)
        ),
        Err(diagnostic) => suggest_fixes(source, &diagnostic),
    }
}

pub fn fix_file(path: &Path) -> Result<String, Diagnostic> {
    let source = fs::read_to_string(path).map_err(to_diagnostic)?;
    Ok(fix_source(&source))
}

pub fn build_web_bundle(source: &str, output_dir: &Path) -> Result<BuildReport, Diagnostic> {
    let program = parse_source(source)?;
    build_web_from_program(&program, source, output_dir)
}

pub fn build_web_bundle_from_file(
    path: &Path,
    output_dir: Option<&Path>,
) -> Result<BuildReport, Diagnostic> {
    verify_project_dependencies(path)?;
    let source = fs::read_to_string(path).map_err(to_diagnostic)?;
    let program = load_program_from_file(path)?;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_build_dir(path, "web"));
    build_web_from_program(&program, &source, &output_dir)
}

pub fn build_apk_bundle(source: &str, output_dir: &Path) -> Result<BuildReport, Diagnostic> {
    let program = parse_source(source)?;
    build_apk_from_program(&program, source, output_dir)
}

pub fn build_apk_bundle_from_file(
    path: &Path,
    output_dir: Option<&Path>,
) -> Result<BuildReport, Diagnostic> {
    verify_project_dependencies(path)?;
    let source = fs::read_to_string(path).map_err(to_diagnostic)?;
    let program = load_program_from_file(path)?;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_build_dir(path, "android"));
    build_apk_from_program(&program, &source, &output_dir)
}

pub fn build_bytecode_bundle(source: &str, output_dir: &Path) -> Result<BuildReport, Diagnostic> {
    let program = parse_source(source)?;
    let bytecode = crate::compile_program(&program)?;
    build_bytecode_from_program(
        &program,
        source,
        &serialize_bytecode(&bytecode),
        output_dir,
        Path::new("app.egl"),
    )
}

pub fn build_bytecode_bundle_from_file(
    path: &Path,
    output_dir: Option<&Path>,
) -> Result<BuildReport, Diagnostic> {
    verify_project_dependencies(path)?;
    let source = fs::read_to_string(path).map_err(to_diagnostic)?;
    let program = load_program_from_file(path)?;
    let bytecode = crate::compile_program(&program)?;
    let output_dir = output_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_build_dir(path, "bytecode"));
    build_bytecode_from_program(&program, &source, &serialize_bytecode(&bytecode), &output_dir, path)
}

pub fn init_project(root: &Path) -> Result<ProjectInitReport, Diagnostic> {
    init_project_with_template(root, "app")
}

pub fn init_project_with_template(
    root: &Path,
    template: &str,
) -> Result<ProjectInitReport, Diagnostic> {
    if root.exists() {
        let mut entries = fs::read_dir(root).map_err(to_diagnostic)?;
        if entries.next().transpose().map_err(to_diagnostic)?.is_some() {
            return Err(
                Diagnostic::new(
                    format!("project directory `{}` already exists and is not empty", root.display()),
                    0,
                    0,
                )
                .with_note("Choose a new directory name or scaffold into an empty folder."),
            );
        }
    }
    fs::create_dir_all(root).map_err(to_diagnostic)?;
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("edgel-project");
    let template = normalize_template(template)?;

    let mut files = Vec::new();
    write(root.join("src/main.egl"), starter_main(name, template), &mut files)?;
    write(root.join("tests/basic.test.egl"), starter_test(template), &mut files)?;
    write(root.join("config/.edgelconfig"), starter_config(template), &mut files)?;
    write(root.join("plugins/README.md"), starter_plugins_readme(), &mut files)?;
    write(root.join("README.md"), starter_readme(name, template), &mut files)?;
    write(root.join(".gitignore"), starter_gitignore(), &mut files)?;
    write(root.join("edgel.json"), &starter_manifest(name, template), &mut files)?;
    fs::create_dir_all(root.join("assets")).map_err(to_diagnostic)?;
    fs::create_dir_all(root.join("build")).map_err(to_diagnostic)?;
    fs::create_dir_all(root.join("dist")).map_err(to_diagnostic)?;

    Ok(ProjectInitReport {
        root: root.to_path_buf(),
        files,
    })
}

pub fn available_project_templates() -> &'static [&'static str] {
    &["app", "web", "api"]
}

pub fn default_entry_file(start: &Path) -> Option<PathBuf> {
    let base = if start.is_file() {
        start.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        start.to_path_buf()
    };

    if let Some(root) = find_project_root(&base) {
        let entry = root.join("src/main.egl");
        if entry.exists() {
            return Some(entry);
        }
    }

    let direct = base.join("src/main.egl");
    if direct.exists() {
        Some(direct)
    } else {
        None
    }
}

pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cursor = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if cursor.join("edgel.json").exists() {
            return Some(cursor);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

pub fn sanitize_relative_path(root: &Path, raw: &str) -> Option<PathBuf> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return None;
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return None;
    }
    Some(root.join(path))
}

pub fn list_project_files(root: &Path) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files).map_err(to_diagnostic)?;
    files.sort();
    Ok(files)
}

pub fn collect_tests(program: &Program) -> Vec<TestCase> {
    let mut cases = Vec::new();
    let mut index = 0usize;
    for item in &program.items {
        if let Item::Test(test) = item {
            cases.push(TestCase {
                name: test.name.clone(),
                function_name: test_function_name(index, &test.name),
            });
            index += 1;
        }
    }
    cases
}

pub fn run_test_file(path: &Path, options: VmOptions) -> Result<Vec<TestRunResult>, Diagnostic> {
    verify_project_dependencies(path)?;
    let program = load_program_from_file(path)?;
    let tests = collect_tests(&program);
    if tests.is_empty() {
        let report = run_project_file(path, options)?;
        return Ok(vec![TestRunResult {
            name: "top-level assertions".to_string(),
            console: report.console,
            profile: report.profile,
        }]);
    }

    let bytecode = crate::compile_program(&program)?;
    let mut results = Vec::new();
    for test in tests {
        let (output, _) = execute_function_with_options(
            &bytecode,
            &test.function_name,
            Vec::new(),
            options.clone(),
        )?;
        results.push(TestRunResult {
            name: test.name,
            console: output.console,
            profile: output.profile,
        });
    }
    Ok(results)
}

fn verify_project_dependencies(path: &Path) -> Result<(), Diagnostic> {
    let Some(root) = find_project_root(path) else {
        return Ok(());
    };
    verify_lockfile(&root).map(|_| ())
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().and_then(|value| value.to_str()) == Some("target") {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            files.push(relative.to_path_buf());
        }
    }
    Ok(())
}

fn build_web_from_program(
    program: &crate::ast::Program,
    source: &str,
    output_dir: &Path,
) -> Result<BuildReport, Diagnostic> {
    let html = render_preview_document(program).unwrap_or_else(|| {
        "<!doctype html><html><body><pre>No UI preview generated. Use console output.</pre></body></html>"
            .to_string()
    });

    let mut files = Vec::new();
    write(output_dir.join("index.html"), &html, &mut files)?;
    write(output_dir.join("app.egl"), source, &mut files)?;
    write(
        output_dir.join("manifest.json"),
        &format!(
            "{{\n  \"name\": \"EDGEL Web Export\",\n  \"summary\": \"{}\"\n}}\n",
            json_escape(&program_summary(program))
        ),
        &mut files,
    )?;

    Ok(BuildReport {
        output_dir: output_dir.to_path_buf(),
        files,
        summary: "Web bundle exported from EDGEL source.".to_string(),
    })
}

fn build_apk_from_program(
    program: &crate::ast::Program,
    source: &str,
    output_dir: &Path,
) -> Result<BuildReport, Diagnostic> {
    let html = render_preview_document(program).unwrap_or_else(|| {
        "<!doctype html><html><body><pre>No UI preview generated. Use console output.</pre></body></html>"
            .to_string()
    });
    let package = "labs.goldedge.edgel";

    let mut files = Vec::new();
    write(
        output_dir.join("README.md"),
        &format!(
            "# Android Export\n\nThis EDGEL export creates a Gradle-ready Android WebView shell.\n\nPackage: `{package}`\n\nSummary: {}\n",
            program_summary(program)
        ),
        &mut files,
    )?;
    write(
        output_dir.join("settings.gradle.kts"),
        "rootProject.name = \"EDGELAndroidExport\"\ninclude(\":app\")\n",
        &mut files,
    )?;
    write(output_dir.join("build.gradle.kts"), "plugins {}\n", &mut files)?;
    write(
        output_dir.join("app/build.gradle.kts"),
        r#"plugins {
    id("com.android.application")
}

android {
    namespace = "labs.goldedge.edgel"
    compileSdk = 34

    defaultConfig {
        applicationId = "labs.goldedge.edgel"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    signingConfigs {
        create("release") {
            storeFile = file("../signing/debug.keystore")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }
}
"#,
        &mut files,
    )?;
    write(
        output_dir.join("app/src/main/AndroidManifest.xml"),
        r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <application android:label="EDGEL App" android:usesCleartextTraffic="true">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
        </activity>
    </application>
</manifest>
"#,
        &mut files,
    )?;
    write(
        output_dir.join("app/src/main/java/labs/goldedge/edgel/MainActivity.java"),
        r#"package labs.goldedge.edgel;

import android.app.Activity;
import android.os.Bundle;
import android.webkit.WebView;

public class MainActivity extends Activity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        WebView view = new WebView(this);
        view.getSettings().setJavaScriptEnabled(true);
        view.loadUrl("file:///android_asset/index.html");
        setContentView(view);
    }
}
"#,
        &mut files,
    )?;
    write(output_dir.join("app/src/main/assets/index.html"), &html, &mut files)?;
    write(output_dir.join("app/src/main/assets/app.egl"), source, &mut files)?;

    Ok(BuildReport {
        output_dir: output_dir.to_path_buf(),
        files,
        summary: "Android project scaffold exported with signing placeholders. Final native APK assembly still requires Android SDK tooling.".to_string(),
    })
}

fn build_bytecode_from_program(
    program: &crate::ast::Program,
    source: &str,
    bytecode: &str,
    output_dir: &Path,
    source_path: &Path,
) -> Result<BuildReport, Diagnostic> {
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("app");

    let mut files = Vec::new();
    write(output_dir.join(format!("{stem}.eglc")), bytecode, &mut files)?;
    write(output_dir.join(format!("{stem}.egl")), source, &mut files)?;
    write(
        output_dir.join("manifest.json"),
        &format!(
            "{{\n  \"name\": \"{}\",\n  \"summary\": \"{}\",\n  \"artifact\": \"{}.eglc\"\n}}\n",
            json_escape(stem),
            json_escape(&program_summary(program)),
            json_escape(stem)
        ),
        &mut files,
    )?;

    Ok(BuildReport {
        output_dir: output_dir.to_path_buf(),
        files,
        summary: "Bytecode bundle exported from EDGEL source.".to_string(),
    })
}

fn default_build_dir(entry_path: &Path, target: &str) -> PathBuf {
    if let Some(root) = find_project_root(entry_path) {
        return root.join("dist").join(target);
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("output")
        .join(target)
}

fn starter_main(name: &str, template: &str) -> String {
    match template {
        "app" => format!(
            r#"import std.ui

app {} {{
    screen Main {{
        header(screenTitle("{}"))
        text(helperText("Welcome to EDGEL production projects."))

        button("Run Check") {{
            print("System ready")
        }}
    }}
}}
"#,
            to_pascal_case(name),
            name
        ),
        "web" => format!(
            r#"web {} {{
    page "/" {{
        h1("Welcome to {}")
        p("This web template is ready for EDGEL launch builds.")
    }}

    api "/health" {{
        return {{ status: "ok", app: "{}" }}
    }}
}}
"#,
            to_pascal_case(name),
            name,
            name
        ),
        "api" => format!(
            r#"function add(a: number, b: number): number {{
    return a + b
}}

api "/hello" {{
    return {{ message: "Hello from {}", sample: add(2, 3) }}
}}

print("API template ready")
"#,
            name
        ),
        _ => unreachable!("template already normalized"),
    }
}

fn starter_test(template: &str) -> &'static str {
    match template {
        "app" | "web" | "api" => {
            r#"function add(a: number, b: number): number {
    return a + b
}

test "addition works" {
    assert(add(2, 3) == 5, "add should sum values")
}
"#
        }
        _ => unreachable!("template already normalized"),
    }
}

fn starter_config(template: &str) -> &'static str {
    match template {
        "app" => "theme = goldedge-sunrise\npreview_device = mobile\nai_mode = api-neuroedge\n",
        "web" => "theme = goldedge-sunrise\npreview_device = desktop\nai_mode = api-neuroedge\n",
        "api" => "theme = goldedge-sunrise\npreview_device = terminal\nai_mode = api-neuroedge\n",
        _ => unreachable!("template already normalized"),
    }
}

fn starter_plugins_readme() -> &'static str {
    "# Plugins\n\nPlace runtime extensions in `plugins/<name>/plugin.egl`, declare metadata with `model plugin { ... }`, and expose hooks like `onCliCommand(event)`, `onRun(event)`, or `onBuild(event)`.\n"
}

fn starter_readme(name: &str, template: &str) -> String {
    format!(
        "# {}\n\nTemplate: `{}`\n\n## Quick Start\n\n```bash\nedgel run\nedgel test\nedgel info\n```\n\n## Build Targets\n\n```bash\nedgel build --web\nedgel build --bytecode\n```\n\n## Learn Next\n\n- `edgel learn`\n- `edgel ai explain src/main.egl`\n- `edgel debug src/main.egl --profile`\n",
        name, template
    )
}

fn starter_gitignore() -> &'static str {
    "build/\ndist/\noutput/\n"
}

fn starter_manifest(name: &str, template: &str) -> String {
    format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"0.1.0\",\n  \"entry\": \"src/main.egl\",\n  \"template\": \"{}\",\n  \"dependencies\": {{\n  }}\n}}\n",
        json_escape(name),
        json_escape(template)
    )
}

fn normalize_template(template: &str) -> Result<&'static str, Diagnostic> {
    match template.trim().to_ascii_lowercase().as_str() {
        "" | "app" => Ok("app"),
        "web" => Ok("web"),
        "api" => Ok("api"),
        other => Err(
            Diagnostic::new(format!("unknown template `{other}`"), 0, 0)
                .with_note(format!(
                    "Available templates: {}",
                    available_project_templates().join(", ")
                )),
        ),
    }
}

fn to_pascal_case(value: &str) -> String {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

fn write(path: PathBuf, content: impl AsRef<str>, files: &mut Vec<PathBuf>) -> Result<(), Diagnostic> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(to_diagnostic)?;
    }
    fs::write(&path, content.as_ref()).map_err(to_diagnostic)?;
    files.push(path);
    Ok(())
}

fn to_diagnostic(error: std::io::Error) -> Diagnostic {
    Diagnostic::new(error.to_string(), 0, 0)
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
