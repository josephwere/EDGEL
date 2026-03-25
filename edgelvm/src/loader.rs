use crate::ast::{Item, Program};
use crate::diagnostics::Diagnostic;
use crate::parse_source;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn load_program_from_file(path: &Path) -> Result<Program, Diagnostic> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(io_to_diagnostic)?
            .join(path)
    };
    let mut loader = ModuleLoader::default();
    loader.load_module(&path)
}

#[derive(Default)]
struct ModuleLoader {
    visited: BTreeSet<PathBuf>,
    stack: Vec<PathBuf>,
}

impl ModuleLoader {
    fn load_module(&mut self, path: &Path) -> Result<Program, Diagnostic> {
        let canonical = fs::canonicalize(path).map_err(io_to_diagnostic)?;
        if self.visited.contains(&canonical) {
            return Ok(Program { items: Vec::new() });
        }
        if self.stack.contains(&canonical) {
            return Err(Diagnostic::new(
                format!("cyclic import detected for `{}`", canonical.display()),
                0,
                0,
            ));
        }

        self.stack.push(canonical.clone());
        let source = fs::read_to_string(&canonical).map_err(io_to_diagnostic)?;
        let parsed = parse_source(&source)?;
        let base_dir = canonical
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut merged = Vec::new();
        for item in parsed.items {
            match item {
                Item::Import(import_decl) => {
                    let import_path = resolve_import(&base_dir, &import_decl.module).ok_or_else(|| {
                        Diagnostic::new(
                            format!(
                                "could not resolve module `{}` from `{}`",
                                import_decl.module,
                                canonical.display()
                            ),
                            0,
                            0,
                        )
                    })?;
                    merged.extend(self.load_module(&import_path)?.items);
                }
                other => merged.push(other),
            }
        }

        self.stack.pop();
        self.visited.insert(canonical);
        Ok(Program { items: merged })
    }
}

fn resolve_import(base_dir: &Path, module: &str) -> Option<PathBuf> {
    let raw = Path::new(module);
    let project_root = find_project_root(base_dir);
    let stdlib_root = workspace_root().join("stdlib");
    let module_path = module.replace('.', "/");
    let candidates = if module.ends_with(".egl") {
        vec![
            base_dir.join(raw),
            project_root
                .as_ref()
                .map(|root| root.join(raw))
                .unwrap_or_else(|| raw.to_path_buf()),
            raw.to_path_buf(),
        ]
    } else if let Some(native_module) = module.strip_prefix("rust:") {
        vec![
            stdlib_root.join("native").join(format!("{native_module}.egl")),
            stdlib_root.join(format!("{native_module}.egl")),
        ]
    } else if let Some(std_module) = module.strip_prefix("std.") {
        let std_module = std_module.replace('.', "/");
        vec![
            project_root
                .as_ref()
                .map(|root| root.join("stdlib").join(format!("{std_module}.egl")))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("stdlib").join(&std_module).join("main.egl"))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            stdlib_root.join(format!("{std_module}.egl")),
            stdlib_root.join(std_module).join("main.egl"),
        ]
    } else if let Some(plugin_module) = module.strip_prefix("plugins.") {
        let plugin_module = plugin_module.replace('.', "/");
        vec![
            project_root
                .as_ref()
                .map(|root| root.join("plugins").join(&plugin_module).join("plugin.egl"))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("plugins").join(format!("{plugin_module}.egl")))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
        ]
    } else {
        vec![
            base_dir.join(format!("{module}.egl")),
            base_dir.join(module).join("main.egl"),
            base_dir.join(format!("{module_path}.egl")),
            base_dir.join(&module_path).join("main.egl"),
            base_dir.join("src").join(format!("{module}.egl")),
            base_dir.join("src").join(module).join("main.egl"),
            base_dir.join("src").join(format!("{module_path}.egl")),
            base_dir.join("src").join(&module_path).join("main.egl"),
            project_root
                .as_ref()
                .map(|root| root.join("src").join(format!("{module}.egl")))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("src").join(format!("{module_path}.egl")))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("plugins").join(module).join("plugin.egl"))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("plugins").join(&module_path).join("plugin.egl"))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("stdlib").join(format!("{module}.egl")))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            project_root
                .as_ref()
                .map(|root| root.join("stdlib").join(format!("{module_path}.egl")))
                .unwrap_or_else(|| PathBuf::from("_missing_")),
            stdlib_root.join(format!("{module}.egl")),
            stdlib_root.join(module).join("main.egl"),
            stdlib_root.join(format!("{module_path}.egl")),
            stdlib_root.join(module_path).join("main.egl"),
        ]
    };

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cursor = start.to_path_buf();
    loop {
        if cursor.join("edgel.json").exists() {
            return Some(cursor);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn io_to_diagnostic(error: std::io::Error) -> Diagnostic {
    Diagnostic::new(error.to_string(), 0, 0)
}
