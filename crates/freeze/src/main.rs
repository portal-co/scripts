// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! freeze — checksum lockfile tool for script-dependent workflows.
//!
//! Subcommands:
//!   freeze record [--lockfile <path>] <files...>
//!       Compute SHA-256 of each file and write them to a lockfile.
//!       Default lockfile: freeze.lock
//!
//!   freeze check [--lockfile <path>]
//!       Re-compute SHA-256 for every entry in the lockfile and verify
//!       they match.  Exits 0 on success, 1 on any mismatch.
//!
//!   freeze exec [--lockfile <path>] -- <cmd> [args...]
//!       Run `check` first; if it passes, replace the current process
//!       with `cmd args...`.  Refuses to run if any checksum fails.
//!
//! Lockfile format (TOML):
//!
//!   [[files]]
//!   path   = "scripts/deploy.sh"
//!   sha256 = "e3b0c44298fc1c149afb..."
//!
//! GitHub Actions usage:
//!   # record once, commit freeze.lock:
//!   freeze record deploy.sh setup.sh
//!
//!   # in CI job step:
//!   freeze exec -- ./deploy.sh --env prod

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "freeze", about = "Record and validate script checksums before exec")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compute SHA-256 of files and write them to a lockfile
    Record {
        #[arg(long, default_value = "freeze.lock")]
        lockfile: PathBuf,
        /// Files to record
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Validate all lockfile entries against current file contents
    Check {
        #[arg(long, default_value = "freeze.lock")]
        lockfile: PathBuf,
        /// Expected SHA-256 of the lockfile itself (embed in CI workflow for end-to-end integrity)
        #[arg(long, value_name = "SHA256")]
        lockfile_hash: Option<String>,
    },
    /// Validate checksums then exec a command (fails if any mismatch)
    Exec {
        #[arg(long, default_value = "freeze.lock")]
        lockfile: PathBuf,
        /// Expected SHA-256 of the lockfile itself (embed in CI workflow for end-to-end integrity)
        #[arg(long, value_name = "SHA256")]
        lockfile_hash: Option<String>,
        /// Command and arguments (separated from freeze args by --)
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
}

// ── Lockfile types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct Lockfile {
    #[serde(default)]
    files: Vec<FileEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FileEntry {
    path: String,
    sha256: String,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = run() {
        eprintln!("freeze: {e}");
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Record { lockfile, files } => cmd_record(&lockfile, &files),
        Cmd::Check { lockfile, lockfile_hash } => cmd_check(&lockfile, lockfile_hash.as_deref()),
        Cmd::Exec { lockfile, lockfile_hash, command } => cmd_exec(&lockfile, lockfile_hash.as_deref(), &command),
    }
}

// ── record ────────────────────────────────────────────────────────────────────

fn cmd_record(lockfile_path: &Path, files: &[PathBuf]) -> Result<()> {
    let mut entries = Vec::with_capacity(files.len());

    for file in files {
        let digest = sha256_file(file)
            .with_context(|| format!("failed to hash '{}'", file.display()))?;
        println!("  recorded  {}  {}", &digest[..16], file.display());
        entries.push(FileEntry {
            path: file.to_string_lossy().into_owned(),
            sha256: digest,
        });
    }

    let lock = Lockfile { files: entries };
    let toml = toml::to_string_pretty(&lock).context("failed to serialize lockfile")?;
    fs::write(lockfile_path, toml)
        .with_context(|| format!("failed to write '{}'", lockfile_path.display()))?;

    let lockfile_hash = sha256_file(lockfile_path)
        .with_context(|| format!("failed to hash '{}'", lockfile_path.display()))?;
    println!("freeze: wrote {} entries to '{}'", lock.files.len(), lockfile_path.display());
    println!("freeze: lockfile hash  {lockfile_hash}");
    println!("freeze: embed in CI:   --lockfile-hash {lockfile_hash}");
    Ok(())
}

// ── check ─────────────────────────────────────────────────────────────────────

fn cmd_check(lockfile_path: &Path, expected_lockfile_hash: Option<&str>) -> Result<()> {
    verify_lockfile_hash(lockfile_path, expected_lockfile_hash)?;
    let results = check_lockfile(lockfile_path)?;
    report_results(&results);

    let failures: usize = results.iter().filter(|r| r.status == Status::Mismatch || r.status == Status::Missing).count();
    if failures > 0 {
        bail!("{failures} checksum failure(s)");
    }
    println!("freeze: all {} checksums OK", results.len());
    Ok(())
}

// ── exec ──────────────────────────────────────────────────────────────────────

fn cmd_exec(lockfile_path: &Path, expected_lockfile_hash: Option<&str>, command: &[String]) -> Result<()> {
    verify_lockfile_hash(lockfile_path, expected_lockfile_hash)?;
    let results = check_lockfile(lockfile_path)?;
    report_results(&results);

    let failures: usize = results.iter().filter(|r| r.status == Status::Mismatch || r.status == Status::Missing).count();
    if failures > 0 {
        bail!("{failures} checksum failure(s) — refusing to execute");
    }

    let (bin, args) = command.split_first().context("no command given")?;

    exec_command(bin, args)
}

fn verify_lockfile_hash(lockfile_path: &Path, expected: Option<&str>) -> Result<()> {
    let Some(expected) = expected else { return Ok(()) };
    let actual = sha256_file(lockfile_path)
        .with_context(|| format!("failed to hash '{}'", lockfile_path.display()))?;
    if actual != expected {
        bail!(
            "lockfile '{}' hash mismatch\n  expected  {}\n  actual    {}",
            lockfile_path.display(),
            expected,
            actual
        );
    }
    eprintln!("  OK (lockfile)  {}  {}", &actual[..16], lockfile_path.display());
    Ok(())
}

fn exec_command(bin: &str, args: &[String]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = process::Command::new(bin).args(args).exec();
        bail!("exec '{}' failed: {err}", bin);
    }

    #[cfg(not(unix))]
    {
        let status = process::Command::new(bin)
            .args(args)
            .status()
            .with_context(|| format!("failed to spawn '{bin}'"))?;
        process::exit(status.code().unwrap_or(1));
    }
}

// ── shared check logic ────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum Status {
    Ok,
    Mismatch,
    Missing,
}

struct CheckResult {
    path: String,
    status: Status,
    actual: Option<String>,
    expected: String,
}

fn check_lockfile(lockfile_path: &Path) -> Result<Vec<CheckResult>> {
    let raw = fs::read_to_string(lockfile_path)
        .with_context(|| format!("cannot read lockfile '{}'", lockfile_path.display()))?;
    let lock: Lockfile = toml::from_str(&raw)
        .with_context(|| format!("invalid lockfile '{}'", lockfile_path.display()))?;

    let mut results = Vec::with_capacity(lock.files.len());
    for entry in &lock.files {
        let path = Path::new(&entry.path);
        match sha256_file(path) {
            Err(_) => results.push(CheckResult {
                path: entry.path.clone(),
                status: Status::Missing,
                actual: None,
                expected: entry.sha256.clone(),
            }),
            Ok(actual) => {
                let status = if actual == entry.sha256 { Status::Ok } else { Status::Mismatch };
                results.push(CheckResult {
                    path: entry.path.clone(),
                    status,
                    actual: Some(actual),
                    expected: entry.sha256.clone(),
                });
            }
        }
    }
    Ok(results)
}

fn report_results(results: &[CheckResult]) {
    for r in results {
        match r.status {
            Status::Ok => println!("  OK        {}  {}", &r.expected[..16], r.path),
            Status::Missing => eprintln!("  MISSING             {}  (expected {})", r.path, &r.expected[..16]),
            Status::Mismatch => {
                let actual = r.actual.as_deref().unwrap_or("?");
                eprintln!(
                    "  MISMATCH  {}  {}  (expected {})",
                    &actual[..16.min(actual.len())],
                    r.path,
                    &r.expected[..16]
                );
            }
        }
    }
}

// ── SHA-256 ───────────────────────────────────────────────────────────────────

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("open '{}'", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).with_context(|| format!("read '{}'", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
