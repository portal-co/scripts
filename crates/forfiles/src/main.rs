// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Read lines from stdin and execute a command per line in parallel.
//!
//! Usage:
//!   forfiles <placeholder> <command> [args...]
//!
//! Every occurrence of `placeholder` in `command` and `args` is replaced
//! with the input line before spawning.  All lines are spawned concurrently;
//! stdout/stderr from each child is forwarded.  A non-zero exit from any
//! child is reported to stderr but does not abort other children.
//!
//! Exits 0 when all commands succeeded, 1 otherwise.

use std::io::{self, BufRead};
use std::process;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: forfiles <placeholder> <command> [args...]");
        process::exit(1);
    }

    let placeholder = &args[0];
    let cmd_template = &args[1..];

    let lines: Vec<String> = io::stdin()
        .lock()
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .collect();

    let failures = forfiles_core::run_all(lines, placeholder, cmd_template).await;

    if failures > 0 {
        process::exit(1);
    }
}
