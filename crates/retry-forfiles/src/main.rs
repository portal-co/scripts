// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Like `forfiles`, but retries commands that fail until they all succeed
//! or a per-invocation attempt limit is reached.
//!
//! Usage:
//!   retry-forfiles [OPTIONS] <placeholder> <command> [args...]
//!
//! Options:
//!   --attempts <N>     Maximum number of attempts per line (default: 5).
//!                      Must be >= 1.  Use 1 for identical behaviour to forfiles.
//!   --delay <SECS>     Seconds to wait between retry rounds (default: 1.0).
//!                      Fractional values are accepted (e.g. 0.5).
//!   -C, --cwd <PATH>   Path template applied as each child's working
//!                      directory.  Occurrences of `<placeholder>` in PATH
//!                      are substituted per-line before chdir.  When
//!                      omitted, children inherit the parent's cwd.
//!
//! Every occurrence of `placeholder` in `command` and `args` is replaced with
//! the input line.  All lines are run concurrently within each round.  After
//! each round, only the lines whose commands failed are retried; lines that
//! succeeded are done.
//!
//! Exits 0 when every line's command eventually succeeded, 1 otherwise.
//!
//! # Motivation
//!
//! On macOS, parallel operations (git fetches, cargo installs, npm scripts,
//! file-system operations) sporadically fail with transient errors:
//!   - `Resource temporarily unavailable` (EAGAIN)
//!   - `The file exists` / `Operation not permitted` (race in APFS)
//!   - `Connection reset by peer` (flaky Wi-Fi / VPN interruption)
//!   - Non-zero exit from `git` due to pack-file lock contention
//!
//! A single automatic retry pass resolves the vast majority of these.

use std::io::{self, BufRead};
use std::process;
use std::time::Duration;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "retry-forfiles",
    about = "Run a command per stdin line in parallel, retrying failures until all succeed or a limit is reached"
)]
struct Cli {
    /// Maximum number of attempts for each line (>= 1).
    #[arg(long, default_value_t = 5)]
    attempts: usize,

    /// Seconds to wait between retry rounds (fractional values accepted).
    #[arg(long, default_value_t = 1.0)]
    delay: f64,

    /// Path template used as each child's working directory.  Occurrences of
    /// the placeholder are substituted per-line before chdir.
    #[arg(short = 'C', long = "cwd")]
    cwd: Option<String>,

    /// The placeholder string that is substituted with each input line.
    placeholder: String,

    /// Command and arguments (occurrences of placeholder are substituted).
    #[arg(trailing_var_arg = true, required = true)]
    command: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.attempts < 1 {
        eprintln!("retry-forfiles: --attempts must be >= 1");
        process::exit(1);
    }

    let delay = Duration::from_secs_f64(cli.delay.max(0.0));

    let lines: Vec<String> = io::stdin()
        .lock()
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .collect();

    if lines.is_empty() {
        process::exit(0);
    }

    let failures = forfiles_core::run_all_until_success(
        lines,
        &cli.placeholder,
        &cli.command,
        cli.cwd.as_deref(),
        cli.attempts,
        delay,
    )
    .await;

    if failures > 0 {
        process::exit(1);
    }
}
