// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Auto-recompile runner for workspace binaries.
//!
//! Usage:
//!   run <binary-name> [args…]
//!
//! For each invocation, `run`:
//!   1. Locates the workspace root by walking up the directory tree until it
//!      finds a `Cargo.toml` that contains `[workspace]`.
//!   2. Looks for a crate at `<workspace-root>/crates/<binary-name>/`.
//!   3. Compares the newest mtime among all `.rs` files and `Cargo.toml`
//!      inside that crate directory against `target/debug/<binary-name>`.
//!   4. If any source file is newer than the binary (or the binary is absent),
//!      runs `cargo build -p <binary-name>` before continuing.
//!   5. Execs `target/debug/<binary-name>` with the forwarded arguments,
//!      replacing the current process on Unix.
//!
//! This makes it safe to invoke Rust script binaries directly from a shell
//! during development without remembering to rebuild after edits.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::SystemTime;

use anyhow::{anyhow, bail, Context, Result};

fn main() {
    if let Err(e) = run() {
        eprintln!("run: {e}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1); // skip argv[0] ("run")
    let binary = args
        .next()
        .ok_or_else(|| anyhow!("Usage: run <binary-name> [args…]"))?;
    let forward: Vec<String> = args.collect();

    let workspace_root = find_workspace_root(
        &env::current_dir().context("failed to get current directory")?,
    )?;

    let crate_dir = workspace_root.join("crates").join(&binary);
    let binary_path = workspace_root
        .join("target")
        .join("debug")
        .join(&binary);

    if crate_dir.is_dir() {
        // Decide whether a rebuild is needed.
        if needs_rebuild(&crate_dir, &binary_path)
            .context("failed to check source freshness")?
        {
            eprintln!("run: rebuilding {binary} …");
            build(&workspace_root, &binary)?;
        }
    } else if !binary_path.exists() {
        // No crate directory and no binary — nothing we can do.
        bail!(
            "crate directory '{}' not found and binary '{}' does not exist",
            crate_dir.display(),
            binary_path.display()
        );
    }
    // If there's no crate directory but the binary exists (e.g. installed
    // from CI), just exec it directly without rebuilding.

    exec_binary(&binary_path, &forward)
}

// ── Workspace root ────────────────────────────────────────────────────────────

/// Walk up from `start` until we find a `Cargo.toml` that contains the
/// string `[workspace]`, which identifies the workspace root.
fn find_workspace_root(start: &Path) -> Result<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let contents = fs::read_to_string(&candidate)
                .with_context(|| format!("reading {}", candidate.display()))?;
            if contents.contains("[workspace]") {
                return Ok(dir);
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => bail!(
                "could not find a workspace Cargo.toml starting from {}",
                start.display()
            ),
        }
    }
}

// ── Staleness check ───────────────────────────────────────────────────────────

/// Return `true` when any source file inside `crate_dir` (recursively,
/// matching `*.rs` or `Cargo.toml`) has a newer mtime than `binary_path`,
/// or when `binary_path` does not exist.
fn needs_rebuild(crate_dir: &Path, binary_path: &Path) -> Result<bool> {
    // If the binary is absent, we always need to build.
    let binary_mtime = match fs::metadata(binary_path) {
        Ok(m) => m
            .modified()
            .context("failed to read binary mtime")?,
        Err(_) => return Ok(true),
    };

    let newest_source = newest_source_mtime(crate_dir)?;
    Ok(newest_source > binary_mtime)
}

/// Return the newest mtime among all `.rs` files and `Cargo.toml` files
/// found (recursively) under `dir`.  Returns `SystemTime::UNIX_EPOCH` if
/// no matching files exist.
fn newest_source_mtime(dir: &Path) -> Result<SystemTime> {
    let mut newest = SystemTime::UNIX_EPOCH;
    visit_source_files(dir, &mut |path| {
        if let Ok(meta) = fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                if mtime > newest {
                    newest = mtime;
                }
            }
        }
    });
    Ok(newest)
}

/// Recursively walk `dir`, calling `cb` for every `.rs` file and every
/// file named `Cargo.toml`.
fn visit_source_files(dir: &Path, cb: &mut impl FnMut(&Path)) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_source_files(&path, cb);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".rs") || name == "Cargo.toml" {
                cb(&path);
            }
        }
    }
}

// ── Build ─────────────────────────────────────────────────────────────────────

/// Run `cargo build -p <binary>` inside `workspace_root`, forwarding
/// stdout and stderr to the terminal.  Fails if the process exits non-zero.
fn build(workspace_root: &Path, binary: &str) -> Result<()> {
    let status = Command::new("cargo")
        .args(["build", "-p", binary])
        .current_dir(workspace_root)
        .status()
        .context("failed to spawn `cargo build`")?;

    if !status.success() {
        bail!("`cargo build -p {binary}` failed with {status}");
    }
    Ok(())
}

// ── Exec ──────────────────────────────────────────────────────────────────────

/// Replace the current process with `binary_path [args…]`.
///
/// On Unix this is a true `execv(2)` — the `run` process image is replaced.
/// On other platforms (Windows) we fall back to spawning a child and
/// forwarding its exit code.
fn exec_binary(binary_path: &Path, args: &[String]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = Command::new(binary_path).args(args).exec();
        // `exec()` only returns if it failed.
        Err(anyhow!("exec '{}' failed: {err}", binary_path.display()))
    }

    #[cfg(not(unix))]
    {
        let status = Command::new(binary_path)
            .args(args)
            .status()
            .with_context(|| format!("failed to spawn '{}'", binary_path.display()))?;
        process::exit(status.code().unwrap_or(1));
    }
}
