// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Shared utilities for the `swap-pkg-paths` and `swap-pkg-tree` binaries.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

// ── Directory walking ─────────────────────────────────────────────────────────

pub const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", ".cache"];

/// Recursively collect every `package.json` under `dir`, skipping
/// [`SKIP_DIRS`] and hidden directories.
pub fn collect_package_jsons(dir: &Path, out: &mut Vec<PathBuf>) {
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

// ── JSON path utilities ───────────────────────────────────────────────────────

/// Navigate a parsed JSON document by a sequence of object keys.
/// Returns `None` if any key is absent or any intermediate node is not an
/// object.
pub fn get_path<'a>(doc: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut current = doc;
    for key in path {
        current = current.as_object()?.get(key.as_str())?;
    }
    Some(current)
}

/// Navigate a parsed JSON document mutably by a sequence of object keys.
/// Returns `None` if any key is absent or any intermediate node is not an
/// object.
pub fn get_path_mut<'a>(doc: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let mut current = doc;
    for key in path {
        current = current.as_object_mut()?.get_mut(key.as_str())?;
    }
    Some(current)
}

/// Atomically swap the values at two object paths inside a JSON document.
///
/// Both values are cloned before either mutation so there is no risk of
/// cascading: each path is touched exactly once regardless of whether the
/// two paths overlap structurally.
///
/// Returns `true` when both paths existed and the swap was performed.
/// Returns `false` (without modifying the document) when either path is
/// absent.
pub fn swap_json_paths(doc: &mut Value, path_a: &[String], path_b: &[String]) -> bool {
    let val_a = match get_path(doc, path_a) {
        Some(v) => v.clone(),
        None => return false,
    };
    let val_b = match get_path(doc, path_b) {
        Some(v) => v.clone(),
        None => return false,
    };
    // Both exist — write back.
    if let Some(slot) = get_path_mut(doc, path_a) {
        *slot = val_b;
    }
    if let Some(slot) = get_path_mut(doc, path_b) {
        *slot = val_a;
    }
    true
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn get_path_present() {
        let doc = json!({"a": {"b": "hello"}});
        let path = vec!["a".to_string(), "b".to_string()];
        assert_eq!(get_path(&doc, &path), Some(&json!("hello")));
    }

    #[test]
    fn get_path_missing_key() {
        let doc = json!({"a": {}});
        let path = vec!["a".to_string(), "nope".to_string()];
        assert_eq!(get_path(&doc, &path), None);
    }

    #[test]
    fn get_path_through_non_object() {
        let doc = json!({"a": "scalar"});
        let path = vec!["a".to_string(), "b".to_string()];
        assert_eq!(get_path(&doc, &path), None);
    }

    #[test]
    fn swap_json_paths_basic() {
        let mut doc = json!({"x": "one", "y": "two"});
        let a = vec!["x".to_string()];
        let b = vec!["y".to_string()];
        assert!(swap_json_paths(&mut doc, &a, &b));
        assert_eq!(doc["x"], json!("two"));
        assert_eq!(doc["y"], json!("one"));
    }

    #[test]
    fn swap_json_paths_self_inverting() {
        let original = json!({"x": "one", "y": "two"});
        let mut doc = original.clone();
        let a = vec!["x".to_string()];
        let b = vec!["y".to_string()];
        swap_json_paths(&mut doc, &a, &b);
        swap_json_paths(&mut doc, &a, &b);
        assert_eq!(doc, original);
    }

    #[test]
    fn swap_json_paths_missing_returns_false() {
        let mut doc = json!({"x": "one"});
        let a = vec!["x".to_string()];
        let b = vec!["nope".to_string()];
        assert!(!swap_json_paths(&mut doc, &a, &b));
        // Document is unchanged.
        assert_eq!(doc["x"], json!("one"));
    }

    #[test]
    fn swap_json_paths_nested() {
        let mut doc = json!({
            "deps":   {"foo": "^1.0.0"},
            "_local": {"foo": "file:../foo"}
        });
        let a = vec!["deps".to_string(),   "foo".to_string()];
        let b = vec!["_local".to_string(), "foo".to_string()];
        assert!(swap_json_paths(&mut doc, &a, &b));
        assert_eq!(doc["deps"]["foo"],   json!("file:../foo"));
        assert_eq!(doc["_local"]["foo"], json!("^1.0.0"));
    }
}
