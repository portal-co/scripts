// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Read lines from stdin and execute a command per line in parallel.
//!
//! Usage:
//!   forfiles [OPTIONS] <placeholder> <command> [args...]
//!
//! Options:
//!   -C, --cwd <PATH>   Path template applied as each child's working
//!                      directory.  Occurrences of `<placeholder>` in PATH
//!                      are substituted per-line before chdir.  When
//!                      omitted, children inherit the parent's cwd.
//!
//! Every occurrence of `placeholder` in `command` and `args` is replaced
//! with the input line before spawning.  All lines are spawned concurrently;
//! stdout/stderr from each child is forwarded.  A non-zero exit from any
//! child is reported to stderr but does not abort other children.
//!
//! Exits 0 when all commands succeeded, 1 otherwise.

use std::io::{self, BufRead};
use std::process;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "forfiles",
    about = "Read lines from stdin, run a command per line in parallel"
)]
struct Cli {
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

    let lines: Vec<String> = io::stdin()
        .lock()
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .collect();

    let failures = forfiles_core::run_all(
        lines,
        &cli.placeholder,
        &cli.command,
        cli.cwd.as_deref(),
    )
    .await;

    if failures > 0 {
        process::exit(1);
    }
}
