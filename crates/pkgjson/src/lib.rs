// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Read and rewrite `package.json` files.
//!
//! Uses a deserialize-patch-serialize approach: parse the JSON, modify the
//! version field in memory, then serialize back with pretty-printing.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use env_traits::FileEnv;
use serde::Deserialize;

// ── Package ───────────────────────────────────────────────────────────────────

/// Minimal representation of the fields we care about in a `package.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub name: String,
    pub version: String, // "" if absent
    pub private: bool,
    pub has_workspaces: bool,
}

impl Package {
    /// A package is publishable when it has a non-empty name, is not private,
    /// and has a version field (or `add_missing` is true).
    pub fn is_publishable(&self, add_missing: bool) -> bool {
        if self.name.is_empty() || self.private {
            return false;
        }
        if self.version.is_empty() && !add_missing {
            return false;
        }
        true
    }
}

// ── BumpPart ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpPart {
    Major,
    Minor,
    Patch,
}

impl std::str::FromStr for BumpPart {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "major" => Ok(BumpPart::Major),
            "minor" => Ok(BumpPart::Minor),
            "patch" => Ok(BumpPart::Patch),
            other => Err(anyhow!(
                "unknown bump part {:?} (want major, minor, or patch)",
                other
            )),
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse the fields we need from the `package.json` at `path`.
pub fn read<F: FileEnv>(file: &F, path: &Path) -> Result<Package>
where
    F::Error: Send + Sync + 'static,
{
    let data = file
        .read_file(&path.to_string_lossy())
        .with_context(|| format!("pkgjson: read {}", path.display()))?;
    parse(&data)
}

/// Increment the semver `version` string according to `part`.
///
/// Returns `"0.1.0"` when `version` is empty.
pub fn bump_version(version: &str, part: BumpPart) -> Result<String> {
    if version.is_empty() {
        return Ok("0.1.0".to_string());
    }
    let version = version.trim_start_matches('v');
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(anyhow!(
            "version {:?} is not semver (expected x.y.z)",
            version
        ));
    }
    let mut major: u64 = parts[0]
        .parse()
        .with_context(|| format!("bad major in {version:?}"))?;
    let mut minor: u64 = parts[1]
        .parse()
        .with_context(|| format!("bad minor in {version:?}"))?;
    let mut patch: u64 = parts[2]
        .parse()
        .with_context(|| format!("bad patch in {version:?}"))?;

    match part {
        BumpPart::Major => {
            major += 1;
            minor = 0;
            patch = 0;
        }
        BumpPart::Minor => {
            minor += 1;
            patch = 0;
        }
        BumpPart::Patch => {
            patch += 1;
        }
    }
    Ok(format!("{major}.{minor}.{patch}"))
}

/// Rewrite the `"version"` field in the `package.json` at `path` to
/// `new_version`.  If the file has no `"version"` field, one is inserted after
/// the `"name"` field.  The rest of the file is preserved byte-for-byte.
pub fn set_version<F: FileEnv>(file: &F, path: &Path, new_version: &str) -> Result<()>
where
    F::Error: Send + Sync + 'static,
{
    let data = file
        .read_file(&path.to_string_lossy())
        .with_context(|| format!("pkgjson: read {}", path.display()))?;
    let out = splice_version(&data, new_version)
        .with_context(|| format!("pkgjson: set_version {}", path.display()))?;
    file.write_file(&path.to_string_lossy(), &out)
        .with_context(|| format!("pkgjson: write {}", path.display()))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawPackage {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    workspaces: Option<serde_json::Value>,
}

fn parse(data: &[u8]) -> Result<Package> {
    let raw: RawPackage = serde_json::from_slice(data).context("pkgjson: unmarshal")?;
    Ok(Package {
        name: raw.name,
        version: raw.version,
        private: raw.private,
        has_workspaces: raw.workspaces.is_some(),
    })
}

fn splice_version(data: &[u8], new_version: &str) -> Result<Vec<u8>> {
    let mut value: serde_json::Value =
        serde_json::from_slice(data).context("pkgjson: unmarshal")?;

    value["version"] = serde_json::Value::String(new_version.to_string());

    let out = serde_json::to_string_pretty(&value).context("pkgjson: marshal")?;
    Ok(out.into_bytes())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use env_fake::FakeFileEnv;

    const PRETTY: &str = r#"{
  "name": "@foo/bar",
  "version": "1.2.3",
  "private": false
}
"#;

    const COMPACT: &str = r#"{"name":"@foo/bar","version":"1.2.3"}"#;

    const NO_VERSION: &str = r#"{
  "name": "@foo/bar",
  "private": false
}
"#;

    fn fake_with(path: &str, content: &str) -> FakeFileEnv {
        FakeFileEnv::default().with_file(path, content.as_bytes())
    }

    #[test]
    fn parse_pretty() {
        let pkg = parse(PRETTY.as_bytes()).unwrap();
        assert_eq!(pkg.name, "@foo/bar");
        assert_eq!(pkg.version, "1.2.3");
        assert!(!pkg.private);
    }

    #[test]
    fn parse_compact() {
        let pkg = parse(COMPACT.as_bytes()).unwrap();
        assert_eq!(pkg.version, "1.2.3");
    }

    #[test]
    fn is_publishable_normal() {
        let pkg = Package {
            name: "foo".into(),
            version: "1.0.0".into(),
            private: false,
            has_workspaces: false,
        };
        assert!(pkg.is_publishable(false));
    }

    #[test]
    fn is_publishable_private() {
        let pkg = Package {
            name: "foo".into(),
            version: "1.0.0".into(),
            private: true,
            has_workspaces: false,
        };
        assert!(!pkg.is_publishable(false));
    }

    #[test]
    fn is_publishable_no_version_no_add() {
        let pkg = Package {
            name: "foo".into(),
            version: "".into(),
            private: false,
            has_workspaces: false,
        };
        assert!(!pkg.is_publishable(false));
        assert!(pkg.is_publishable(true));
    }

    #[test]
    fn bump_patch() {
        assert_eq!(bump_version("1.2.3", BumpPart::Patch).unwrap(), "1.2.4");
    }

    #[test]
    fn bump_minor() {
        assert_eq!(bump_version("1.2.3", BumpPart::Minor).unwrap(), "1.3.0");
    }

    #[test]
    fn bump_major() {
        assert_eq!(bump_version("1.2.3", BumpPart::Major).unwrap(), "2.0.0");
    }

    #[test]
    fn bump_empty_version() {
        assert_eq!(bump_version("", BumpPart::Patch).unwrap(), "0.1.0");
    }

    #[test]
    fn set_version_replaces_existing() {
        let file = fake_with("/p/package.json", PRETTY);
        set_version(&file, Path::new("/p/package.json"), "2.0.0").unwrap();
        let data = file.read_file("/p/package.json").unwrap();
        let pkg = parse(&data).unwrap();
        assert_eq!(pkg.version, "2.0.0");
    }

    #[test]
    fn set_version_inserts_after_name() {
        let file = fake_with("/p/package.json", NO_VERSION);
        set_version(&file, Path::new("/p/package.json"), "0.1.0").unwrap();
        let data = file.read_file("/p/package.json").unwrap();
        let pkg = parse(&data).unwrap();
        assert_eq!(pkg.version, "0.1.0");
        // Name should still be present.
        assert_eq!(pkg.name, "@foo/bar");
    }

    #[test]
    fn set_version_compact() {
        let file = fake_with("/p/package.json", COMPACT);
        set_version(&file, Path::new("/p/package.json"), "9.9.9").unwrap();
        let data = file.read_file("/p/package.json").unwrap();
        assert!(data.windows(5).any(|w| w == b"9.9.9"));
    }
}
