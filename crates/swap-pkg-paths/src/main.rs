// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Rewrite `package.json` dependency specifiers by swapping between two states.
//!
//! The map file is a JSON object whose keys and values are both dependency
//! specifier strings (version ranges, `file:` paths, `workspace:*`, etc.).
//!
//! ```json
//! {
//!   "file:../foo": "^1.0.0",
//!   "file:../bar": "workspace:*"
//! }
//! ```
//!
//! Each entry defines a **pair**.  On first run `file:../foo` → `^1.0.0`; on
//! second run `^1.0.0` → `file:../foo` — returning to the original state.
//! Two runs is always a no-op.
//!
//! Replacements are collected for the whole file first and then applied in one
//! pass, so a specifier that is both a key and a value in the same file cannot
//! cascade into a second replacement.
//!
//! # Usage
//!
//! ```text
//! swap-pkg-paths [OPTIONS] <map-file> [roots...]
//! ```
//!
//! `map-file` — path to the JSON swap-map.
//! `roots`    — directories to search for `package.json` files (default: `.`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::Value;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "swap-pkg-paths",
    about = "Swap package.json dependency specifiers between two states (self-inverting)"
)]
struct Cli {
    /// Path to the JSON swap-map file.
    ///
    /// An object whose keys and values are dependency specifier strings.
    /// Each pair is treated bidirectionally: key↔value.  Running the tool
    /// twice restores every file to its original state.
    map_file: PathBuf,

    /// Directories to search for package.json files.  Defaults to `.`.
    #[arg(default_value = ".")]
    roots: Vec<PathBuf>,

    /// Print what would change without writing any files.
    #[arg(long)]
    dry_run: bool,
}

// ── Dependency object fields we rewrite ──────────────────────────────────────

const DEP_FIELDS: &[&str] = &[
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
    "bundledDependencies",
    "bundleDependencies",
];

const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", ".cache"];

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = run() {
        eprintln!("swap-pkg-paths: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let swap = load_swap_map(&cli.map_file)?;

    let mut pkg_files: Vec<PathBuf> = Vec::new();
    for root in &cli.roots {
        collect_package_jsons(root, &mut pkg_files);
    }
    pkg_files.sort();

    if pkg_files.is_empty() {
        eprintln!("swap-pkg-paths: no package.json files found");
        return Ok(());
    }

    let mut total_swaps = 0usize;
    let mut files_changed = 0usize;

    for path in &pkg_files {
        match process_file(path, &swap, cli.dry_run) {
            Ok(0) => {}
            Ok(n) => {
                let verb = if cli.dry_run { "would swap" } else { "swapped" };
                eprintln!(
                    "{}: {verb} {n} specifier{}",
                    path.display(),
                    if n == 1 { "" } else { "s" }
                );
                total_swaps += n;
                files_changed += 1;
            }
            Err(e) => eprintln!("swap-pkg-paths: warning: {e:#}"),
        }
    }

    if total_swaps == 0 {
        eprintln!("swap-pkg-paths: nothing to swap");
    } else {
        let verb = if cli.dry_run { "Would swap" } else { "Swapped" };
        eprintln!(
            "swap-pkg-paths: {verb} {total_swaps} specifier{} across {files_changed} file{}",
            if total_swaps == 1 { "" } else { "s" },
            if files_changed == 1 { "" } else { "s" },
        );
    }

    Ok(())
}

// ── Swap-map loading ──────────────────────────────────────────────────────────

/// Load the JSON map file and build a bidirectional lookup table.
///
/// For each explicit `(key → value)` pair the reverse `(value → key)` is also
/// added, unless the value already appears as an explicit key (in which case
/// the explicit mapping takes precedence).
///
/// Pairs where key == value are rejected — they would make the swap a no-op
/// and almost certainly indicate a mistake in the map file.
pub fn load_swap_map(path: &Path) -> Result<HashMap<String, String>> {
    let data = fs::read_to_string(path)
        .with_context(|| format!("reading map file {}", path.display()))?;

    let raw: HashMap<String, String> = serde_json::from_str(&data)
        .with_context(|| format!("parsing map file {} (expected a flat JSON object)", path.display()))?;

    // Validate: no pair may map a specifier to itself.
    for (k, v) in &raw {
        if k == v {
            bail!(
                "map file {}: key {:?} maps to itself — this would be a no-op",
                path.display(),
                k
            );
        }
    }

    // Step 1: seed implicit reverses (value → key) only where the value is not
    //         itself an explicit key.
    let mut swap: HashMap<String, String> = HashMap::new();
    for (k, v) in &raw {
        swap.entry(v.clone()).or_insert_with(|| k.clone());
    }

    // Step 2: explicit entries always win — insert them last so they override
    //         any implicit reverse that happened to collide.
    for (k, v) in raw {
        swap.insert(k, v);
    }

    Ok(swap)
}

// ── File processing ───────────────────────────────────────────────────────────

/// Apply the swap map to one `package.json`.
///
/// Returns the number of specifiers replaced.  If `dry_run` is true the file
/// is never written.
pub fn process_file(
    path: &Path,
    swap: &HashMap<String, String>,
    dry_run: bool,
) -> Result<usize> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let mut doc: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;

    let count = apply_swap(&mut doc, swap);

    if count > 0 && !dry_run {
        let out = serde_json::to_string_pretty(&doc)
            .with_context(|| format!("serialising {}", path.display()))?;
        // Always end with a newline.
        let out = if out.ends_with('\n') {
            out
        } else {
            format!("{out}\n")
        };
        fs::write(path, out)
            .with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(count)
}

/// Walk the dependency fields of a parsed `package.json` value and apply
/// `swap` to every matching specifier.  Returns the number of replacements.
pub fn apply_swap(doc: &mut Value, swap: &HashMap<String, String>) -> usize {
    let mut count = 0usize;

    for field in DEP_FIELDS {
        if let Some(deps) = doc.get_mut(*field).and_then(|v| v.as_object_mut()) {
            for spec in deps.values_mut() {
                if let Some(s) = spec.as_str() {
                    if let Some(replacement) = swap.get(s) {
                        *spec = Value::String(replacement.clone());
                        count += 1;
                    }
                }
            }
        }
    }

    count
}

// ── Directory walker ──────────────────────────────────────────────────────────

fn collect_package_jsons(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) || name.starts_with('.') {
                continue;
            }
            collect_package_jsons(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("package.json") {
            out.push(path);
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    // Build a bidirectional swap map from explicit pairs (mirrors load_swap_map
    // without touching the filesystem).
    fn make_swap(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        let mut swap: HashMap<String, String> = HashMap::new();
        for &(k, v) in pairs {
            swap.entry(v.to_string()).or_insert_with(|| k.to_string());
        }
        for &(k, v) in pairs {
            swap.insert(k.to_string(), v.to_string());
        }
        swap
    }

    fn pkg_json(deps: &[(&str, &str)]) -> Value {
        let mut obj = serde_json::Map::new();
        let mut dep_map = serde_json::Map::new();
        for &(name, spec) in deps {
            dep_map.insert(name.to_string(), Value::String(spec.to_string()));
        }
        obj.insert("name".to_string(), Value::String("test-pkg".to_string()));
        obj.insert("dependencies".to_string(), Value::Object(dep_map));
        Value::Object(obj)
    }

    // ── apply_swap ─────────────────────────────────────────────────────────

    #[test]
    fn swaps_matching_specifier() {
        let swap = make_swap(&[("file:../foo", "^1.0.0")]);
        let mut doc = pkg_json(&[("foo", "file:../foo"), ("bar", "^2.0.0")]);
        let n = apply_swap(&mut doc, &swap);
        assert_eq!(n, 1);
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("^1.0.0"));
        assert_eq!(doc["dependencies"]["bar"].as_str(), Some("^2.0.0"));
    }

    #[test]
    fn leaves_unmatched_specifiers_alone() {
        let swap = make_swap(&[("file:../foo", "^1.0.0")]);
        let mut doc = pkg_json(&[("bar", "^3.0.0")]);
        let n = apply_swap(&mut doc, &swap);
        assert_eq!(n, 0);
        assert_eq!(doc["dependencies"]["bar"].as_str(), Some("^3.0.0"));
    }

    #[test]
    fn self_inverting_two_runs_restore_original() {
        let swap = make_swap(&[("file:../foo", "^1.0.0")]);

        let mut doc = pkg_json(&[("foo", "file:../foo")]);
        let original = doc.clone();

        // First run: local → published.
        let n1 = apply_swap(&mut doc, &swap);
        assert_eq!(n1, 1);
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("^1.0.0"));

        // Second run: published → local (restored).
        let n2 = apply_swap(&mut doc, &swap);
        assert_eq!(n2, 1);
        assert_eq!(doc, original);
    }

    #[test]
    fn multiple_pairs_each_self_inverting() {
        let swap = make_swap(&[
            ("file:../foo", "^1.0.0"),
            ("file:../bar", "workspace:*"),
        ]);

        let mut doc = pkg_json(&[("foo", "file:../foo"), ("bar", "file:../bar")]);
        let original = doc.clone();

        apply_swap(&mut doc, &swap);
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("^1.0.0"));
        assert_eq!(doc["dependencies"]["bar"].as_str(), Some("workspace:*"));

        apply_swap(&mut doc, &swap);
        assert_eq!(doc, original);
    }

    #[test]
    fn no_cascade_within_single_pass() {
        // If "A" → "B" and "B" → "A" are both in the swap map,
        // a dep already set to "B" must become "A" (not flip twice to "B").
        let swap = make_swap(&[("A", "B")]);
        assert_eq!(swap.get("A").map(String::as_str), Some("B"));
        assert_eq!(swap.get("B").map(String::as_str), Some("A"));

        let mut doc = pkg_json(&[("x", "B")]);
        apply_swap(&mut doc, &swap);
        // "B" → "A" — one replacement, not two.
        assert_eq!(doc["dependencies"]["x"].as_str(), Some("A"));
    }

    #[test]
    fn touches_all_dep_fields() {
        let swap = make_swap(&[("file:../x", "^0.1.0")]);
        let mut doc: Value = serde_json::from_str(r#"{
            "name": "test",
            "dependencies":         { "x": "file:../x" },
            "devDependencies":      { "x": "file:../x" },
            "peerDependencies":     { "x": "file:../x" },
            "optionalDependencies": { "x": "file:../x" }
        }"#).unwrap();

        let n = apply_swap(&mut doc, &swap);
        assert_eq!(n, 4);
        for field in &["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
            assert_eq!(doc[field]["x"].as_str(), Some("^0.1.0"), "field {field}");
        }
    }

    // ── load_swap_map ──────────────────────────────────────────────────────

    #[test]
    fn load_swap_map_bidirectional() {
        let dir = TempDir::new().unwrap();
        let map_path = dir.path().join("swap.json");
        fs::write(&map_path, r#"{"file:../foo": "^1.0.0"}"#).unwrap();

        let swap = load_swap_map(&map_path).unwrap();
        assert_eq!(swap.get("file:../foo").map(String::as_str), Some("^1.0.0"));
        assert_eq!(swap.get("^1.0.0").map(String::as_str), Some("file:../foo"));
    }

    #[test]
    fn load_swap_map_rejects_self_mapping() {
        let dir = TempDir::new().unwrap();
        let map_path = dir.path().join("swap.json");
        fs::write(&map_path, r#"{"foo": "foo"}"#).unwrap();

        let err = load_swap_map(&map_path).unwrap_err();
        assert!(err.to_string().contains("maps to itself"), "{err}");
    }

    #[test]
    fn load_swap_map_explicit_wins_over_implicit_reverse() {
        // "A" → "B" (explicit) and "B" → "C" (explicit).
        // Implicit reverse of A→B would be B→A, but B→C is explicit and wins.
        let dir = TempDir::new().unwrap();
        let map_path = dir.path().join("swap.json");
        fs::write(&map_path, r#"{"A": "B", "B": "C"}"#).unwrap();

        let swap = load_swap_map(&map_path).unwrap();
        assert_eq!(swap.get("A").map(String::as_str), Some("B"));
        assert_eq!(swap.get("B").map(String::as_str), Some("C"));
        // "C" gets the implicit reverse of B→C.
        assert_eq!(swap.get("C").map(String::as_str), Some("B"));
    }

    // ── process_file ───────────────────────────────────────────────────────

    #[test]
    fn process_file_writes_changed_file() {
        let dir = TempDir::new().unwrap();
        let pkg_path = dir.path().join("package.json");
        fs::write(&pkg_path, r#"{"name":"t","dependencies":{"foo":"file:../foo"}}"#).unwrap();

        let swap = make_swap(&[("file:../foo", "^1.0.0")]);
        let n = process_file(&pkg_path, &swap, false).unwrap();
        assert_eq!(n, 1);

        let written: Value = serde_json::from_str(&fs::read_to_string(&pkg_path).unwrap()).unwrap();
        assert_eq!(written["dependencies"]["foo"].as_str(), Some("^1.0.0"));
    }

    #[test]
    fn process_file_dry_run_does_not_write() {
        let dir = TempDir::new().unwrap();
        let pkg_path = dir.path().join("package.json");
        let original = r#"{"name":"t","dependencies":{"foo":"file:../foo"}}"#;
        fs::write(&pkg_path, original).unwrap();

        let swap = make_swap(&[("file:../foo", "^1.0.0")]);
        let n = process_file(&pkg_path, &swap, /* dry_run */ true).unwrap();
        assert_eq!(n, 1);

        // File on disk must be unchanged.
        assert_eq!(fs::read_to_string(&pkg_path).unwrap(), original);
    }

    #[test]
    fn process_file_written_file_is_self_inverting() {
        let dir = TempDir::new().unwrap();
        let pkg_path = dir.path().join("package.json");
        fs::write(
            &pkg_path,
            r#"{"name":"t","dependencies":{"foo":"file:../foo"}}"#,
        ).unwrap();

        let swap = make_swap(&[("file:../foo", "^1.0.0")]);

        process_file(&pkg_path, &swap, false).unwrap();
        process_file(&pkg_path, &swap, false).unwrap();

        let restored: Value =
            serde_json::from_str(&fs::read_to_string(&pkg_path).unwrap()).unwrap();
        assert_eq!(restored["dependencies"]["foo"].as_str(), Some("file:../foo"));
    }
}
