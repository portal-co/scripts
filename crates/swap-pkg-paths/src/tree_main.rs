// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Swap `package.json` values at arbitrary object paths using a tree-shaped
//! config — the generic companion to `swap-pkg-paths`.
//!
//! # Config format
//!
//! The config is a JSON object tree.  Interior nodes are objects; leaf nodes
//! are arrays of strings that name a *second* object path.  For every leaf
//! the tool swaps the value at the leaf's own path (the keys traversed to
//! reach it) with the value at the path named by the array — provided **both**
//! values exist in the target `package.json`.  If either side is absent the
//! pair is silently skipped.
//!
//! ```json
//! {
//!   "dependencies": {
//!     "foo": ["_local", "foo"],
//!     "bar": ["_local", "bar"]
//!   }
//! }
//! ```
//!
//! Given a `package.json`:
//! ```json
//! {
//!   "dependencies": { "foo": "^1.0.0",      "bar": "^2.0.0"      },
//!   "_local":       { "foo": "file:../foo",  "bar": "file:../bar" }
//! }
//! ```
//!
//! After one run `dependencies.foo` holds `"file:../foo"` and `_local.foo`
//! holds `"^1.0.0"`.  After a second run the original state is restored.
//!
//! # Use cases
//!
//! * **Development** — a `dev.json` config swaps registry specifiers in
//!   `dependencies` with local `file:` paths stored in a `_local` section.
//!
//! * **Publishing** — a `git.json` config swaps the same registry specifiers
//!   with git sources stored in a `_git` section, for a publish target that
//!   cannot use the registry.
//!
//! Both the registry version and the alternative specifier live permanently
//! inside `package.json` so nothing is ever lost between runs.
//!
//! # Usage
//!
//! ```text
//! swap-pkg-tree [OPTIONS] <config> [roots...]
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde_json::Value;
use swap_pkg_paths::{collect_package_jsons, swap_json_paths};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "swap-pkg-tree",
    about = "Swap package.json values at object paths using a tree-shaped config (self-inverting)"
)]
struct Cli {
    /// Path to the JSON tree-config file.
    ///
    /// Interior nodes are objects; leaf nodes are string arrays that name the
    /// partner path.  Each leaf describes one swap: the value at the leaf's
    /// own path is exchanged with the value at the named path, provided both
    /// exist.  Running the tool twice restores all files to their original
    /// state.
    config: PathBuf,

    /// Directories to search for package.json files.  Defaults to `.`.
    #[arg(default_value = ".")]
    roots: Vec<PathBuf>,

    /// Print what would change without writing any files.
    #[arg(long)]
    dry_run: bool,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = run() {
        eprintln!("swap-pkg-tree: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let config_raw = fs::read_to_string(&cli.config)
        .with_context(|| format!("reading config {}", cli.config.display()))?;
    let config: Value = serde_json::from_str(&config_raw)
        .with_context(|| format!("parsing config {}", cli.config.display()))?;

    let mut pairs: Vec<(Vec<String>, Vec<String>)> = Vec::new();
    collect_pairs(&config, &mut Vec::new(), &mut pairs)
        .with_context(|| format!("validating config {}", cli.config.display()))?;

    if pairs.is_empty() {
        eprintln!("swap-pkg-tree: config contains no swap pairs");
        return Ok(());
    }

    let mut pkg_files: Vec<PathBuf> = Vec::new();
    for root in &cli.roots {
        collect_package_jsons(root, &mut pkg_files);
    }
    pkg_files.sort();

    if pkg_files.is_empty() {
        eprintln!("swap-pkg-tree: no package.json files found");
        return Ok(());
    }

    let mut total_swaps = 0usize;
    let mut files_changed = 0usize;

    for path in &pkg_files {
        match process_file(path, &pairs, cli.dry_run) {
            Ok(0) => {}
            Ok(n) => {
                let verb = if cli.dry_run { "would swap" } else { "swapped" };
                eprintln!(
                    "{}: {verb} {n} pair{}",
                    path.display(),
                    if n == 1 { "" } else { "s" }
                );
                total_swaps += n;
                files_changed += 1;
            }
            Err(e) => eprintln!("swap-pkg-tree: warning: {e:#}"),
        }
    }

    if total_swaps == 0 {
        eprintln!("swap-pkg-tree: nothing to swap");
    } else {
        let verb = if cli.dry_run { "Would swap" } else { "Swapped" };
        eprintln!(
            "swap-pkg-tree: {verb} {total_swaps} pair{} across {files_changed} file{}",
            if total_swaps == 1 { "" } else { "s" },
            if files_changed == 1 { "" } else { "s" },
        );
    }

    Ok(())
}

// ── Config walking ────────────────────────────────────────────────────────────

/// Recursively walk the config tree and collect all `(path_a, path_b)` swap
/// pairs into `out`.
///
/// * Interior nodes must be JSON objects — keys extend the current path.
/// * Leaf nodes must be non-empty JSON arrays of strings — the array is
///   interpreted as `path_b`; the accumulated key sequence is `path_a`.
/// * Any other JSON type at a leaf position is an error.
/// * A pair where `path_a == path_b` is an error (self-swap is always a
///   no-op and almost certainly a config mistake).
pub fn collect_pairs(
    node: &Value,
    current_path: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, Vec<String>)>,
) -> Result<()> {
    match node {
        Value::Object(map) => {
            for (key, child) in map {
                current_path.push(key.clone());
                collect_pairs(child, current_path, out)?;
                current_path.pop();
            }
        }

        Value::Array(items) => {
            if items.is_empty() {
                bail!(
                    "path {:?}: target path array must not be empty",
                    current_path
                );
            }

            // Every element must be a string.
            let path_b: Vec<String> = items
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    v.as_str().map(String::from).with_context(|| {
                        format!(
                            "path {:?}: element {i} is not a string (got {v})",
                            current_path
                        )
                    })
                })
                .collect::<Result<_>>()?;

            let path_a = current_path.clone();

            if path_a == path_b {
                bail!(
                    "path {:?}: a path cannot name itself as its swap partner",
                    current_path
                );
            }

            out.push((path_a, path_b));
        }

        other => {
            bail!(
                "path {:?}: expected object or string array, got {}",
                current_path,
                other
            );
        }
    }
    Ok(())
}

// ── File processing ───────────────────────────────────────────────────────────

/// Apply all swap pairs to one `package.json`.
///
/// Pairs where either path is absent in the document are silently skipped —
/// this lets a single config work across a monorepo where not every package
/// has every dependency.
///
/// Returns the number of pairs that were actually swapped.  If `dry_run` is
/// true the file is never written.
pub fn process_file(
    path: &Path,
    pairs: &[(Vec<String>, Vec<String>)],
    dry_run: bool,
) -> Result<usize> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;

    let mut doc: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing {}", path.display()))?;

    let mut count = 0usize;
    for (path_a, path_b) in pairs {
        if swap_json_paths(&mut doc, path_a, path_b) {
            count += 1;
        }
    }

    if count > 0 && !dry_run {
        let out = serde_json::to_string_pretty(&doc)
            .with_context(|| format!("serialising {}", path.display()))?;
        let out = if out.ends_with('\n') { out } else { format!("{out}\n") };
        fs::write(path, out)
            .with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(count)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    // ── collect_pairs ──────────────────────────────────────────────────────

    #[test]
    fn collect_pairs_single_leaf() {
        let config = json!({
            "dependencies": {
                "foo": ["_local", "foo"]
            }
        });
        let mut pairs = Vec::new();
        collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, vec!["dependencies", "foo"]);
        assert_eq!(pairs[0].1, vec!["_local", "foo"]);
    }

    #[test]
    fn collect_pairs_multiple_leaves() {
        let config = json!({
            "dependencies": {
                "foo": ["_local", "foo"],
                "bar": ["_local", "bar"]
            }
        });
        let mut pairs = Vec::new();
        collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap();
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn collect_pairs_deeply_nested() {
        let config = json!({
            "a": { "b": { "c": ["x", "y", "z"] } }
        });
        let mut pairs = Vec::new();
        collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap();
        assert_eq!(pairs[0].0, vec!["a", "b", "c"]);
        assert_eq!(pairs[0].1, vec!["x", "y", "z"]);
    }

    #[test]
    fn collect_pairs_rejects_empty_array() {
        let config = json!({ "a": [] });
        let mut pairs = Vec::new();
        let err = collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap_err();
        assert!(err.to_string().contains("must not be empty"), "{err}");
    }

    #[test]
    fn collect_pairs_rejects_non_string_array_element() {
        let config = json!({ "a": ["ok", 42] });
        let mut pairs = Vec::new();
        let err = collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap_err();
        assert!(err.to_string().contains("not a string"), "{err}");
    }

    #[test]
    fn collect_pairs_rejects_self_swap() {
        let config = json!({ "a": ["a"] });
        let mut pairs = Vec::new();
        let err = collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap_err();
        assert!(err.to_string().contains("cannot name itself"), "{err}");
    }

    #[test]
    fn collect_pairs_rejects_scalar_leaf() {
        let config = json!({ "a": "oops" });
        let mut pairs = Vec::new();
        let err = collect_pairs(&config, &mut Vec::new(), &mut pairs).unwrap_err();
        assert!(err.to_string().contains("expected object or string array"), "{err}");
    }

    // ── process_file ───────────────────────────────────────────────────────

    #[test]
    fn process_file_swaps_when_both_paths_exist() {
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("package.json");
        fs::write(&pkg, r#"{
            "dependencies": { "foo": "^1.0.0" },
            "_local":       { "foo": "file:../foo" }
        }"#).unwrap();

        let pairs = vec![(
            vec!["dependencies".into(), "foo".into()],
            vec!["_local".into(),       "foo".into()],
        )];
        let n = process_file(&pkg, &pairs, false).unwrap();
        assert_eq!(n, 1);

        let doc: Value = serde_json::from_str(&fs::read_to_string(&pkg).unwrap()).unwrap();
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("file:../foo"));
        assert_eq!(doc["_local"]["foo"].as_str(),       Some("^1.0.0"));
    }

    #[test]
    fn process_file_skips_pair_when_one_path_absent() {
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("package.json");
        // No "_local" section — partner path is absent.
        fs::write(&pkg, r#"{"dependencies": {"foo": "^1.0.0"}}"#).unwrap();

        let pairs = vec![(
            vec!["dependencies".into(), "foo".into()],
            vec!["_local".into(),       "foo".into()],
        )];
        let n = process_file(&pkg, &pairs, false).unwrap();
        assert_eq!(n, 0);

        // File is unchanged.
        let doc: Value = serde_json::from_str(&fs::read_to_string(&pkg).unwrap()).unwrap();
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("^1.0.0"));
    }

    #[test]
    fn process_file_dry_run_does_not_write() {
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("package.json");
        let original = r#"{"dependencies":{"foo":"^1.0.0"},"_local":{"foo":"file:../foo"}}"#;
        fs::write(&pkg, original).unwrap();

        let pairs = vec![(
            vec!["dependencies".into(), "foo".into()],
            vec!["_local".into(),       "foo".into()],
        )];
        let n = process_file(&pkg, &pairs, true).unwrap();
        assert_eq!(n, 1);
        assert_eq!(fs::read_to_string(&pkg).unwrap(), original);
    }

    // ── self-inverting end-to-end ──────────────────────────────────────────

    #[test]
    fn two_runs_restore_original() {
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("package.json");
        fs::write(&pkg, r#"{
            "dependencies": { "foo": "^1.0.0", "bar": "^2.0.0" },
            "_local":       { "foo": "file:../foo", "bar": "file:../bar" }
        }"#).unwrap();

        let pairs = vec![
            (vec!["dependencies".into(), "foo".into()], vec!["_local".into(), "foo".into()]),
            (vec!["dependencies".into(), "bar".into()], vec!["_local".into(), "bar".into()]),
        ];

        process_file(&pkg, &pairs, false).unwrap();

        let mid: Value = serde_json::from_str(&fs::read_to_string(&pkg).unwrap()).unwrap();
        assert_eq!(mid["dependencies"]["foo"].as_str(), Some("file:../foo"));
        assert_eq!(mid["dependencies"]["bar"].as_str(), Some("file:../bar"));
        assert_eq!(mid["_local"]["foo"].as_str(),        Some("^1.0.0"));
        assert_eq!(mid["_local"]["bar"].as_str(),        Some("^2.0.0"));

        process_file(&pkg, &pairs, false).unwrap();

        let restored: Value = serde_json::from_str(&fs::read_to_string(&pkg).unwrap()).unwrap();
        assert_eq!(restored["dependencies"]["foo"].as_str(), Some("^1.0.0"));
        assert_eq!(restored["dependencies"]["bar"].as_str(), Some("^2.0.0"));
        assert_eq!(restored["_local"]["foo"].as_str(),        Some("file:../foo"));
        assert_eq!(restored["_local"]["bar"].as_str(),        Some("file:../bar"));
    }

    #[test]
    fn independent_configs_for_dev_and_git() {
        // Shows that the same package.json can hold three states:
        // registry, local file:, and git — and each config file only touches
        // its own pair of sections.
        let dir = TempDir::new().unwrap();
        let pkg = dir.path().join("package.json");
        fs::write(&pkg, r#"{
            "dependencies": { "foo": "^1.0.0" },
            "_local":       { "foo": "file:../foo" },
            "_git":         { "foo": "git+https://github.com/org/foo.git#main" }
        }"#).unwrap();

        let dev_pairs = vec![(
            vec!["dependencies".into(), "foo".into()],
            vec!["_local".into(),       "foo".into()],
        )];
        let git_pairs = vec![(
            vec!["dependencies".into(), "foo".into()],
            vec!["_git".into(),         "foo".into()],
        )];

        // Activate dev mode.
        process_file(&pkg, &dev_pairs, false).unwrap();
        let doc: Value = serde_json::from_str(&fs::read_to_string(&pkg).unwrap()).unwrap();
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("file:../foo"));
        assert_eq!(doc["_local"]["foo"].as_str(),       Some("^1.0.0"));

        // Restore, then activate git mode.
        process_file(&pkg, &dev_pairs, false).unwrap();
        process_file(&pkg, &git_pairs, false).unwrap();
        let doc: Value = serde_json::from_str(&fs::read_to_string(&pkg).unwrap()).unwrap();
        assert_eq!(doc["dependencies"]["foo"].as_str(), Some("git+https://github.com/org/foo.git#main"));
        assert_eq!(doc["_git"]["foo"].as_str(),         Some("^1.0.0"));
        // _local is untouched.
        assert_eq!(doc["_local"]["foo"].as_str(),       Some("file:../foo"));
    }
}
