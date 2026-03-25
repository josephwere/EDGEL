use crate::diagnostics::Diagnostic;
use crate::project::find_project_root;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectManifest {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PackageOperationReport {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    pub summary: String,
}

#[derive(Debug, Clone, Default)]
pub struct LockedPackage {
    pub requested: String,
    pub version: String,
    pub checksum: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ProjectLockfile {
    pub version: u32,
    pub packages: BTreeMap<String, LockedPackage>,
}

impl Default for ProjectLockfile {
    fn default() -> Self {
        Self {
            version: 1,
            packages: BTreeMap::new(),
        }
    }
}

pub fn load_manifest(start: &Path) -> Result<ProjectManifest, Diagnostic> {
    let root = project_root(start)?;
    let path = root.join("edgel.json");
    let source = fs::read_to_string(&path).map_err(io_to_diagnostic)?;
    Ok(parse_manifest(&source, &root))
}

pub fn save_manifest(start: &Path, manifest: &ProjectManifest) -> Result<PathBuf, Diagnostic> {
    let root = project_root(start)?;
    let path = root.join("edgel.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_to_diagnostic)?;
    }
    fs::write(&path, manifest_to_json(manifest)).map_err(io_to_diagnostic)?;
    Ok(path)
}

pub fn load_lockfile(start: &Path) -> Result<ProjectLockfile, Diagnostic> {
    let root = project_root(start)?;
    let path = root.join("edgel.lock");
    let source = fs::read_to_string(&path).map_err(io_to_diagnostic)?;
    Ok(parse_lockfile(&source))
}

pub fn save_lockfile(start: &Path, lockfile: &ProjectLockfile) -> Result<PathBuf, Diagnostic> {
    let root = project_root(start)?;
    let path = root.join("edgel.lock");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_to_diagnostic)?;
    }
    fs::write(&path, lockfile_to_json(lockfile)).map_err(io_to_diagnostic)?;
    Ok(path)
}

pub fn verify_lockfile(start: &Path) -> Result<ProjectLockfile, Diagnostic> {
    let root = project_root(start)?;
    let manifest = load_manifest_or_default(&root);
    let lock_path = root.join("edgel.lock");

    if manifest.dependencies.is_empty() && !lock_path.exists() {
        return Ok(ProjectLockfile::default());
    }

    if !lock_path.exists() {
        return Err(
            Diagnostic::new(
                format!(
                    "dependency lock missing for {} declared package(s)",
                    manifest.dependencies.len()
                ),
                0,
                0,
            )
            .with_context("lockfile")
            .with_note(format!("manifest: {}", root.join("edgel.json").display()))
            .with_note(format!("expected lockfile: {}", lock_path.display()))
            .with_note("Run `edgel update` to generate a fresh edgel.lock."),
        );
    }

    let lockfile = load_lockfile(&root)?;
    for (name, requested) in &manifest.dependencies {
        let Some(locked) = lockfile.packages.get(name) else {
            return Err(
                Diagnostic::new(format!("lockfile entry missing for dependency `{name}`"), 0, 0)
                    .with_context("lockfile")
                    .with_note(format!("manifest constraint: {requested}"))
                    .with_note("Run `edgel update` to resync the lockfile."),
            );
        };
        if locked.requested != *requested {
            return Err(
                Diagnostic::new(format!("lockfile entry for `{name}` is out of date"), 0, 0)
                    .with_context("lockfile")
                    .with_note(format!("manifest constraint: {requested}"))
                    .with_note(format!("locked constraint: {}", locked.requested))
                    .with_note("Run `edgel update` after editing dependencies."),
            );
        }

        let cache_root = root.join("packages").join(name);
        if !cache_root.exists() {
            return Err(
                Diagnostic::new(format!("installed package cache missing for `{name}`"), 0, 0)
                    .with_context("lockfile")
                    .with_note(format!("expected package path: {}", cache_root.display()))
                    .with_note("Run `edgel install` or `edgel update` to restore packages."),
            );
        }

        let actual_checksum = compute_package_checksum(&cache_root)?;
        if actual_checksum != locked.checksum {
            return Err(
                Diagnostic::new(format!("checksum mismatch for dependency `{name}`"), 0, 0)
                    .with_context("lockfile")
                    .with_note(format!("expected checksum: {}", locked.checksum))
                    .with_note(format!("actual checksum: {actual_checksum}"))
                    .with_note(format!("package path: {}", cache_root.display()))
                    .with_note("Run `edgel update` to restore package contents."),
            );
        }
    }

    let stale = lockfile
        .packages
        .keys()
        .filter(|name| !manifest.dependencies.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        return Err(
            Diagnostic::new(
                format!("lockfile contains undeclared package(s): {}", stale.join(", ")),
                0,
                0,
            )
            .with_context("lockfile")
            .with_note("Run `edgel update` to remove stale lockfile entries."),
        );
    }

    Ok(lockfile)
}

pub fn install_dependency(
    start: &Path,
    name: &str,
    version: Option<&str>,
) -> Result<PackageOperationReport, Diagnostic> {
    let root = project_root(start)?;
    let mut manifest = load_manifest_or_default(&root);
    let version = version.unwrap_or("^1.0.0").to_string();
    manifest
        .dependencies
        .insert(name.to_string(), version.clone());
    let mut files = vec![save_manifest(&root, &manifest)?];
    let sync = sync_dependencies(&root, &manifest)?;
    files.extend(sync.files);
    Ok(PackageOperationReport {
        root,
        files,
        summary: format!("Installed `{name}` at {version}."),
    })
}

pub fn update_dependencies(start: &Path) -> Result<PackageOperationReport, Diagnostic> {
    let root = project_root(start)?;
    let manifest = load_manifest_or_default(&root);
    let sync = sync_dependencies(&root, &manifest)?;
    Ok(PackageOperationReport {
        root,
        files: sync.files,
        summary: format!("Updated {} dependency cache entrie(s).", manifest.dependencies.len()),
    })
}

pub fn publish_package(start: &Path) -> Result<PackageOperationReport, Diagnostic> {
    let root = project_root(start)?;
    let manifest = load_manifest_or_default(&root);
    let registry_root = root
        .join(".edgel")
        .join("registry")
        .join(&manifest.name)
        .join(&manifest.version);
    let entry_path = root.join(&manifest.entry);

    if registry_root.exists() {
        fs::remove_dir_all(&registry_root).map_err(io_to_diagnostic)?;
    }
    let mut files = Vec::new();
    let package_json = registry_root.join("package.json");
    let registry_src = registry_root.join("src");
    fs::create_dir_all(&registry_src).map_err(io_to_diagnostic)?;
    fs::write(
        &package_json,
        format!(
            "{{\n  \"name\": \"{}\",\n  \"version\": \"{}\",\n  \"entry\": \"{}\",\n  \"source\": \"local-registry\"\n}}\n",
            manifest.name, manifest.version, manifest.entry
        ),
    )
    .map_err(io_to_diagnostic)?;
    files.push(package_json);

    if entry_path.exists() {
        let published_entry = registry_src.join("main.egl");
        fs::copy(&entry_path, &published_entry).map_err(io_to_diagnostic)?;
        files.push(published_entry);
    }

    let checksum = compute_package_checksum(&registry_root)?;
    let checksum_path = registry_root.join("package.sum");
    fs::write(&checksum_path, format!("{checksum}\n")).map_err(io_to_diagnostic)?;
    files.push(checksum_path);

    Ok(PackageOperationReport {
        root,
        files,
        summary: format!(
            "Published `{}` version `{}` to the local EDGEL registry.",
            manifest.name, manifest.version
        ),
    })
}

fn load_manifest_or_default(root: &Path) -> ProjectManifest {
    load_manifest(root).unwrap_or_else(|_| ProjectManifest {
        name: root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("edgel-app")
            .to_string(),
        version: "0.1.0".to_string(),
        entry: "src/main.egl".to_string(),
        dependencies: BTreeMap::new(),
    })
}

struct CacheMaterialization {
    files: Vec<PathBuf>,
    locked: LockedPackage,
}

struct SyncReport {
    files: Vec<PathBuf>,
}

fn sync_dependencies(root: &Path, manifest: &ProjectManifest) -> Result<SyncReport, Diagnostic> {
    let mut files = Vec::new();
    let mut lockfile = ProjectLockfile::default();

    for (name, requested) in &manifest.dependencies {
        let materialized = materialize_dependency_cache(root, name, requested)?;
        files.extend(materialized.files);
        lockfile.packages.insert(name.clone(), materialized.locked);
    }

    files.push(save_lockfile(root, &lockfile)?);
    Ok(SyncReport { files })
}

fn materialize_dependency_cache(
    root: &Path,
    name: &str,
    requested: &str,
) -> Result<CacheMaterialization, Diagnostic> {
    let cache_root = root.join("packages").join(name);
    let registry_root = root.join(".edgel").join("registry").join(name);
    let resolved_version = resolve_dependency_version(&registry_root, requested);
    let registry_package = registry_root.join(&resolved_version);

    if cache_root.exists() {
        fs::remove_dir_all(&cache_root).map_err(io_to_diagnostic)?;
    }
    fs::create_dir_all(&cache_root).map_err(io_to_diagnostic)?;

    let mut files = Vec::new();
    let source = if registry_package.join("package.json").exists() {
        verify_registry_package(&registry_package)?;
        copy_package_payload(&registry_package, &cache_root, &mut files)?;
        "local-registry".to_string()
    } else {
        let package_json = cache_root.join("package.json");
        fs::write(
            &package_json,
            format!(
                "{{\n  \"name\": \"{}\",\n  \"version\": \"{}\",\n  \"source\": \"local-cache\"\n}}\n",
                name, resolved_version
            ),
        )
        .map_err(io_to_diagnostic)?;
        files.push(package_json);
        "local-cache".to_string()
    };

    let checksum = compute_package_checksum(&cache_root)?;
    let checksum_path = cache_root.join("package.sum");
    fs::write(&checksum_path, format!("{checksum}\n")).map_err(io_to_diagnostic)?;
    files.push(checksum_path);

    let readme = cache_root.join("README.md");
    fs::write(
        &readme,
        format!(
            "# {name}\n\nInstalled by EDGEL package manager.\nRequested version: `{requested}`.\nResolved version: `{resolved_version}`.\nChecksum: `{checksum}`.\n"
        ),
    )
    .map_err(io_to_diagnostic)?;
    files.push(readme);

    Ok(CacheMaterialization {
        files,
        locked: LockedPackage {
            requested: requested.to_string(),
            version: resolved_version,
            checksum,
            source,
        },
    })
}

fn parse_manifest(source: &str, root: &Path) -> ProjectManifest {
    let mut manifest = ProjectManifest {
        name: root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("edgel-app")
            .to_string(),
        version: "0.1.0".to_string(),
        entry: "src/main.egl".to_string(),
        dependencies: BTreeMap::new(),
    };

    let mut in_dependencies = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"dependencies\"") {
            in_dependencies = true;
            continue;
        }
        if in_dependencies {
            if trimmed.starts_with('}') {
                in_dependencies = false;
                continue;
            }
            if let Some((name, version)) = parse_json_pair(trimmed) {
                manifest.dependencies.insert(name, version);
            }
            continue;
        }

        if let Some((key, value)) = parse_json_pair(trimmed) {
            match key.as_str() {
                "name" => manifest.name = value,
                "version" => manifest.version = value,
                "entry" => manifest.entry = value,
                _ => {}
            }
        }
    }

    manifest
}

fn parse_lockfile(source: &str) -> ProjectLockfile {
    let mut lockfile = ProjectLockfile::default();
    let mut in_packages = false;
    let mut current_package: Option<(String, LockedPackage)> = None;

    for line in source.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if current_package.is_some() {
            if trimmed.starts_with('}') {
                let (name, package) = current_package
                    .take()
                    .expect("current package should be present");
                lockfile.packages.insert(name, package);
                continue;
            }
            if let Some((key, value)) = parse_json_pair(trimmed) {
                if let Some((_, package)) = current_package.as_mut() {
                    match key.as_str() {
                        "requested" => package.requested = value,
                        "version" => package.version = value,
                        "checksum" => package.checksum = value,
                        "source" => package.source = value,
                        _ => {}
                    }
                }
            }
            continue;
        }

        if trimmed.starts_with("\"version\"") {
            if let Some((_, value)) = trimmed.split_once(':') {
                lockfile.version = value
                    .trim()
                    .trim_matches(',')
                    .parse::<u32>()
                    .unwrap_or(1);
            }
            continue;
        }

        if trimmed.starts_with("\"packages\"") {
            in_packages = true;
            continue;
        }
        if in_packages {
            if trimmed.starts_with('}') {
                in_packages = false;
                continue;
            }
            if let Some(name) = parse_object_key(trimmed) {
                current_package = Some((name, LockedPackage::default()));
            }
        }
    }

    if let Some((name, package)) = current_package {
        lockfile.packages.insert(name, package);
    }

    lockfile
}

fn manifest_to_json(manifest: &ProjectManifest) -> String {
    let dependencies = if manifest.dependencies.is_empty() {
        "    ".to_string()
    } else {
        manifest
            .dependencies
            .iter()
            .map(|(name, version)| format!("    \"{}\": \"{}\"", name, version))
            .collect::<Vec<_>>()
            .join(",\n")
    };

    format!(
        "{{\n  \"name\": \"{}\",\n  \"version\": \"{}\",\n  \"entry\": \"{}\",\n  \"dependencies\": {{\n{}\n  }}\n}}\n",
        manifest.name, manifest.version, manifest.entry, dependencies
    )
}

fn lockfile_to_json(lockfile: &ProjectLockfile) -> String {
    let packages = if lockfile.packages.is_empty() {
        "    ".to_string()
    } else {
        lockfile
            .packages
            .iter()
            .map(|(name, package)| {
                format!(
                    "    \"{}\": {{\n      \"requested\": \"{}\",\n      \"version\": \"{}\",\n      \"checksum\": \"{}\",\n      \"source\": \"{}\"\n    }}",
                    name, package.requested, package.version, package.checksum, package.source
                )
            })
            .collect::<Vec<_>>()
            .join(",\n")
    };

    format!(
        "{{\n  \"version\": {},\n  \"packages\": {{\n{}\n  }}\n}}\n",
        lockfile.version, packages
    )
}

fn parse_json_pair(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim().trim_end_matches(',');
    let (key, value) = trimmed.split_once(':')?;
    let key = key.trim().trim_matches('"').to_string();
    let value = value.trim().trim_matches('"').to_string();
    Some((key, value))
}

fn parse_object_key(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_end_matches(',');
    let (key, value) = trimmed.split_once(':')?;
    if !value.trim().starts_with('{') {
        return None;
    }
    Some(key.trim().trim_matches('"').to_string())
}

fn clean_version(version: &str) -> String {
    version.trim_start_matches('^').trim_start_matches('~').to_string()
}

fn resolve_dependency_version(registry_root: &Path, requested: &str) -> String {
    let requested_clean = clean_version(requested);
    if !registry_root.exists() {
        return requested_clean;
    }

    let available = fs::read_dir(registry_root)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            if entry.path().is_dir() {
                entry.file_name().into_string().ok()
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if available.is_empty() {
        return requested_clean;
    }

    let mut matched = available
        .into_iter()
        .filter(|candidate| version_matches(requested, candidate))
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return requested_clean;
    }
    matched.sort_by_key(|candidate| parse_version_triplet(candidate));
    matched.pop().unwrap_or(requested_clean)
}

fn version_matches(requested: &str, candidate: &str) -> bool {
    let requested_clean = clean_version(requested);
    let requested_parts = parse_version_triplet(&requested_clean);
    let candidate_parts = parse_version_triplet(candidate);
    match requested.chars().next() {
        Some('^') => {
            candidate_parts[0] == requested_parts[0] && candidate_parts >= requested_parts
        }
        Some('~') => {
            candidate_parts[0] == requested_parts[0]
                && candidate_parts[1] == requested_parts[1]
                && candidate_parts >= requested_parts
        }
        _ => candidate == requested_clean,
    }
}

fn parse_version_triplet(version: &str) -> [u32; 3] {
    let mut triplet = [0_u32; 3];
    for (index, part) in version.split('.').take(3).enumerate() {
        triplet[index] = part.parse::<u32>().unwrap_or(0);
    }
    triplet
}

fn verify_registry_package(registry_package: &Path) -> Result<(), Diagnostic> {
    let checksum_path = registry_package.join("package.sum");
    if !checksum_path.exists() {
        return Err(
            Diagnostic::new(
                format!(
                    "registry package `{}` is missing package.sum",
                    registry_package.display()
                ),
                0,
                0,
            )
            .with_context("manifest")
            .with_note("Republish the package with `edgel publish` before installing it."),
        );
    }
    let expected = fs::read_to_string(&checksum_path)
        .map_err(io_to_diagnostic)?
        .trim()
        .to_string();
    let actual = compute_package_checksum(registry_package)?;
    if expected != actual {
        return Err(
            Diagnostic::new(
                format!(
                    "registry package checksum mismatch for `{}`",
                    registry_package.display()
                ),
                0,
                0,
            )
            .with_context("manifest")
            .with_note(format!("expected checksum: {expected}"))
            .with_note(format!("actual checksum: {actual}"))
            .with_note("Republish the package to refresh registry metadata."),
        );
    }
    Ok(())
}

fn copy_package_payload(
    source: &Path,
    destination: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Diagnostic> {
    for entry in fs::read_dir(source).map_err(io_to_diagnostic)? {
        let entry = entry.map_err(io_to_diagnostic)?;
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if path.is_dir() {
            fs::create_dir_all(&target).map_err(io_to_diagnostic)?;
            copy_package_payload(&path, &target, files)?;
        } else {
            fs::copy(&path, &target).map_err(io_to_diagnostic)?;
            files.push(target);
        }
    }
    Ok(())
}

fn compute_package_checksum(root: &Path) -> Result<String, Diagnostic> {
    let mut files = Vec::new();
    collect_checksum_files(root, root, &mut files)?;
    files.sort();

    let mut hash = 0xcbf29ce484222325_u64;
    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap_or(file.as_path())
            .display()
            .to_string();
        fnv1a_update(&mut hash, relative.as_bytes());
        fnv1a_update(&mut hash, b"\0");
        let content = fs::read(&file).map_err(io_to_diagnostic)?;
        fnv1a_update(&mut hash, &content);
        fnv1a_update(&mut hash, b"\0");
    }

    Ok(format!("fnv1a64:{hash:016x}"))
}

fn collect_checksum_files(
    root: &Path,
    dir: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), Diagnostic> {
    for entry in fs::read_dir(dir).map_err(io_to_diagnostic)? {
        let entry = entry.map_err(io_to_diagnostic)?;
        let path = entry.path();
        if path.is_dir() {
            collect_checksum_files(root, &path, files)?;
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(path.as_path());
        let name = relative.file_name().and_then(|value| value.to_str());
        if matches!(name, Some("package.sum" | "README.md")) {
            continue;
        }
        files.push(path);
    }
    Ok(())
}

fn fnv1a_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= *byte as u64;
        *hash = hash.wrapping_mul(0x00000100000001b3);
    }
}

fn project_root(start: &Path) -> Result<PathBuf, Diagnostic> {
    find_project_root(start)
        .or_else(|| {
            if start.is_dir() {
                Some(start.to_path_buf())
            } else {
                start.parent().map(Path::to_path_buf)
            }
        })
        .ok_or_else(|| Diagnostic::new("could not find an EDGEL project root", 0, 0))
}

fn io_to_diagnostic(error: std::io::Error) -> Diagnostic {
    Diagnostic::new(error.to_string(), 0, 0).with_context("manifest")
}
