// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Remove stale Cargo coordination lock files (flock-based) so a new `cargo` can run
//! after a process was killed and left the cache or target tree in a bad state.
//!
//! This only deletes a lock file when a non-blocking **exclusive** lock can be
//! taken, so a live `cargo` that still holds a shared or exclusive lock is not
//! disturbed.
//!
//! Recognized file names (current Cargo):
//! - `.cargo-lock` / `.cargo-build-lock` under the target / build tree
//! - `.package-cache` / `.package-cache-mutate` under the home package cache

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Parser;
use walkdir::WalkDir;

/// File base names that Cargo uses for `flock` coordination (not `Cargo.lock` the dep graph).
const CARGO_FLOCK_NAMES: &[&str] = &[
    ".cargo-lock",
    ".cargo-build-lock",
    ".package-cache",
    ".package-cache-mutate",
];

#[derive(Parser, Debug)]
#[command(
    name = "cargo-unlock",
    about = "Remove stale Cargo flock lock files (target dir, CARGO_HOME cache)"
)]
struct Args {
    /// Only print what would be removed
    #[arg(long)]
    dry_run: bool,

    /// Do not scan the Cargo home directory (package cache locks)
    #[arg(long)]
    no_cargo_home: bool,

    /// Directory trees to scan for `.cargo-lock` and related files.
    /// Defaults to the current directory if none are given (useful from a workspace root).
    #[arg(value_name = "DIR")]
    roots: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut roots = args.roots;
    // Default to cwd when we also scan CARGO_HOME; otherwise require explicit dirs.
    if roots.is_empty() && !args.no_cargo_home {
        roots.push(PathBuf::from("."));
    }

    if !args.no_cargo_home {
        if let Some(h) = cargo_home() {
            roots.push(h);
        }
    }

    if roots.is_empty() {
        bail!("no directories to scan (pass DIR … or omit --no-cargo-home to include `.` and CARGO_HOME)");
    }

    let mut removed = 0u32;
    let mut skipped_in_use = 0u32;
    let mut skipped_io = 0u32;

    for root in &roots {
        if !root.exists() {
            eprintln!("cargo-unlock: skip missing root {}", root.display());
            continue;
        }
        for path in collect_lock_files(root) {
            match try_remove_stale_lock(&path, args.dry_run) {
                Ok(TryRemove::Removed) => {
                    println!("removed {}", path.display());
                    removed += 1;
                }
                Ok(TryRemove::InUse) => {
                    eprintln!("in use (skip) {}", path.display());
                    skipped_in_use += 1;
                }
                Ok(TryRemove::DryRunWouldRemove) => {
                    println!("would remove {}", path.display());
                    removed += 1;
                }
                Err(e) => {
                    eprintln!("cargo-unlock: {}: {e:#}", path.display());
                    skipped_io += 1;
                }
            }
        }
    }

    if args.dry_run {
        eprintln!(
            "cargo-unlock: dry-run: {} could be removed, {} in-use skipped, {} errors",
            removed, skipped_in_use, skipped_io
        );
    } else {
        eprintln!(
            "cargo-unlock: {} removed, {} in-use skipped, {} errors",
            removed, skipped_in_use, skipped_io
        );
    }

    Ok(())
}

fn cargo_home() -> Option<PathBuf> {
    std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            let home = std::env::var_os("HOME")?;
            Some(PathBuf::from(home).join(".cargo"))
        })
}

fn collect_lock_files(root: &Path) -> Vec<PathBuf> {
    let set: &[&str] = CARGO_FLOCK_NAMES;
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|n| set.contains(&n))
        {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out.dedup();
    out
}

enum TryRemove {
    Removed,
    InUse,
    DryRunWouldRemove,
}

fn try_remove_stale_lock(path: &Path, dry_run: bool) -> Result<TryRemove> {
    if dry_run {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("open {}", path.display()))?;
        return match try_lock_exclusive(&file) {
            Ok(true) => Ok(TryRemove::DryRunWouldRemove),
            Ok(false) => Ok(TryRemove::InUse),
            Err(e) => Err(e).with_context(|| format!("lock {}", path.display())),
        };
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("open {}", path.display()))?;

    match try_lock_exclusive(&file) {
        Ok(true) => {
            // Exclusive lock: no other holder (shared or exclusive). Safe to unlink.
            fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
            Ok(TryRemove::Removed)
        }
        Ok(false) => Ok(TryRemove::InUse),
        Err(e) => Err(e).with_context(|| format!("lock {}", path.display())),
    }
}

/// Returns `Ok(true)` if this process acquired an exclusive lock; `Ok(false)` if another
/// process holds a conflicting lock; `Err` on an unexpected I/O error.
fn try_lock_exclusive(file: &std::fs::File) -> anyhow::Result<bool> {
    #[cfg(unix)]
    {
        use std::fs::TryLockError;
        match file.try_lock() {
            Ok(()) => Ok(true),
            Err(TryLockError::WouldBlock) => Ok(false),
            Err(TryLockError::Error(e)) => {
                if error_is_unsupported_lock(&e) {
                    // Match Cargo's flock fallback: treat as unconstrained.
                    Ok(true)
                } else {
                    Err(anyhow::Error::new(e))
                }
            }
        }
    }
    #[cfg(windows)]
    {
        use std::fs::TryLockError;
        match file.try_lock() {
            Ok(()) => Ok(true),
            Err(TryLockError::WouldBlock) => Ok(false),
            Err(TryLockError::Error(e)) => {
                if error_is_unsupported_lock(&e) {
                    Ok(true)
                } else {
                    Err(anyhow::Error::new(e))
                }
            }
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Ok(true)
    }
}

#[cfg(unix)]
fn error_is_unsupported_lock(err: &io::Error) -> bool {
    use std::io::ErrorKind;
    err.kind() == ErrorKind::Unsupported
        || err.raw_os_error().is_some_and(|c| {
            c == libc::ENOTSUP || c == libc::EOPNOTSUPP || c == libc::ENOSYS
        })
}

#[cfg(windows)]
fn error_is_unsupported_lock(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Unsupported
}

#[cfg(all(not(unix), not(windows)))]
fn error_is_unsupported_lock(_err: &io::Error) -> bool {
    false
}
