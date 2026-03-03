// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Read lines from stdin and execute a command per line in parallel.
//!
//! Usage:
//!   forfiles <placeholder> <command> [args...]
//!
//! Every occurrence of `placeholder` in `command` and `args` is replaced
//! with the input line before spawning.  All lines are spawned concurrently;
//! stdout/stderr from each child is forwarded.  A non-zero exit from a child
//! is reported to stderr but does not abort other children.

use std::io::{self, BufRead};
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("Usage: forfiles <placeholder> <command> [args...]");
        std::process::exit(1);
    }

    let placeholder = args[0].clone();
    let cmd_template: Vec<String> = args[1..].to_vec();

    let stdin = io::stdin();
    let lines: Vec<String> = stdin
        .lock()
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .collect();

    let mut handles = Vec::with_capacity(lines.len());

    for line in lines {
        let cmd: Vec<String> = cmd_template
            .iter()
            .map(|s| s.replace(&placeholder, &line))
            .collect();
        let ph = placeholder.clone();
        let handle = tokio::spawn(async move {
            let output = Command::new(&cmd[0])
                .args(&cmd[1..])
                .output()
                .await;
            match output {
                Err(e) => eprintln!("forfiles: spawn {:?}: {e}", &cmd[0]),
                Ok(o) => {
                    if !o.stdout.is_empty() {
                        print!("{}", String::from_utf8_lossy(&o.stdout));
                    }
                    if !o.stderr.is_empty() {
                        eprint!("{}", String::from_utf8_lossy(&o.stderr));
                    }
                    if !o.status.success() {
                        eprintln!(
                            "forfiles: command exited with {}: {}",
                            o.status,
                            cmd.join(" ").replace(&ph, "<line>")
                        );
                    }
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }
}
