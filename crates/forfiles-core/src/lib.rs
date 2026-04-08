// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Shared parallel execution engine for `forfiles` and `retry-forfiles`.
//!
//! # Model
//!
//! Both tools accept a *command template* — an argv vector where every
//! occurrence of a *placeholder* string is substituted with an input *line*
//! before the command is spawned.  The set of (line, cmd) pairs is run
//! concurrently, one task per line.
//!
//! ## `run_all`
//!
//! Fire-and-collect: spawn every line's command once, collect stdout/stderr,
//! and report failures to stderr.  A failed command does **not** abort the
//! others.  Returns the count of lines that exited non-zero.
//!
//! ## `run_all_until_success`
//!
//! Retry variant: after the initial round, any line whose command failed is
//! queued for a second attempt (after an optional back-off delay), and so on
//! up to `max_attempts` total rounds.  This combats transient macOS failures
//! such as resource-temporarily-unavailable errors, sporadic `git` network
//! timeouts, and system-call interrupts on Apple Silicon that do not
//! self-resolve without a retry.
//!
//! Returns the count of lines that **still** failed after all rounds.

use std::time::Duration;

use tokio::process::Command;
use tokio::time::sleep;

// ── Public types ──────────────────────────────────────────────────────────────

/// The result of running one line's command.
#[derive(Debug)]
pub struct LineResult {
    /// The substituted input line.
    pub line: String,
    /// Whether the command exited successfully on this attempt.
    pub success: bool,
    /// Combined stdout bytes (captured, not streamed).
    pub stdout: Vec<u8>,
    /// Combined stderr bytes.
    pub stderr: Vec<u8>,
    /// The exit status, if the process was successfully spawned.
    pub status: Option<std::process::ExitStatus>,
}

// ── run_all ───────────────────────────────────────────────────────────────────

/// Spawn one task per line, wait for all to finish, print their output,
/// and return the number of lines that failed.
///
/// Output from all tasks is printed in the order results arrive (i.e.
/// interleaved by completion time, not input order) so that long-running
/// commands do not suppress shorter ones.
pub async fn run_all(lines: Vec<String>, placeholder: &str, cmd_template: &[String]) -> usize {
    let results = spawn_all(lines, placeholder, cmd_template).await;
    let mut failures = 0;
    for r in results {
        print_result(&r, cmd_template, placeholder);
        if !r.success {
            failures += 1;
        }
    }
    failures
}

// ── run_all_until_success ─────────────────────────────────────────────────────

/// Run up to `max_attempts` rounds.  After each round, any line whose command
/// failed is retried (with an optional `delay` between rounds) until either
/// every line succeeds or `max_attempts` is exhausted.
///
/// Progress is reported to stderr: a banner is printed before each retry round
/// listing the lines being retried and the attempt number.
///
/// Returns the number of lines that still failed after all rounds.
pub async fn run_all_until_success(
    lines: Vec<String>,
    placeholder: &str,
    cmd_template: &[String],
    max_attempts: usize,
    delay: Duration,
) -> usize {
    assert!(max_attempts >= 1, "max_attempts must be at least 1");

    // First round — run everything.
    let mut pending: Vec<String> = lines;
    let mut total_failures = 0;

    for attempt in 1..=max_attempts {
        if attempt > 1 {
            let remaining = pending.len();
            eprintln!(
                "retry-forfiles: attempt {attempt}/{max_attempts} — retrying {remaining} failing line(s){}",
                if delay.is_zero() { String::new() } else { format!(" (after {:.1}s delay)", delay.as_secs_f32()) }
            );
            if !delay.is_zero() {
                sleep(delay).await;
            }
        }

        let results = spawn_all(pending.clone(), placeholder, cmd_template).await;

        let mut still_failing: Vec<String> = Vec::new();
        let mut round_failures = 0;

        for r in results {
            print_result(&r, cmd_template, placeholder);
            if !r.success {
                round_failures += 1;
                still_failing.push(r.line);
            }
        }

        total_failures = round_failures;
        pending = still_failing;

        if pending.is_empty() {
            break;
        }
    }

    // Report anything that never recovered.
    if !pending.is_empty() {
        eprintln!(
            "retry-forfiles: {} line(s) still failing after {max_attempts} attempt(s):",
            pending.len()
        );
        for line in &pending {
            eprintln!("  {line}");
        }
    }

    total_failures
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Spawn one task per line, run all concurrently, and collect the results.
async fn spawn_all(
    lines: Vec<String>,
    placeholder: &str,
    cmd_template: &[String],
) -> Vec<LineResult> {
    let mut handles = Vec::with_capacity(lines.len());

    for line in lines {
        let cmd: Vec<String> = cmd_template
            .iter()
            .map(|s| s.replace(placeholder, &line))
            .collect();
        let line_clone = line.clone();

        let handle = tokio::spawn(async move {
            match Command::new(&cmd[0]).args(&cmd[1..]).output().await {
                Err(e) => {
                    // Spawn failure — treat as a failed run so the caller can retry.
                    LineResult {
                        line: line_clone,
                        success: false,
                        stdout: Vec::new(),
                        stderr: format!("spawn error: {e}").into_bytes(),
                        status: None,
                    }
                }
                Ok(o) => LineResult {
                    line: line_clone,
                    success: o.status.success(),
                    stdout: o.stdout,
                    stderr: o.stderr,
                    status: Some(o.status),
                },
            }
        });
        handles.push(handle);
    }

    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        match h.await {
            Ok(r) => results.push(r),
            Err(e) => eprintln!("forfiles-core: task panicked: {e}"),
        }
    }
    results
}

/// Print captured stdout/stderr and (on failure) an error banner.
fn print_result(r: &LineResult, cmd_template: &[String], placeholder: &str) {
    if !r.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&r.stdout));
    }
    if !r.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&r.stderr));
    }
    if !r.success {
        let display_cmd = cmd_template.join(" ").replace(placeholder, &r.line);
        match r.status {
            Some(s) => eprintln!("forfiles: command exited with {s}: {display_cmd}"),
            None => eprintln!("forfiles: command failed to spawn: {display_cmd}"),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a cmd_template that echoes the line on stdout.
    fn echo_cmd(placeholder: &str) -> Vec<String> {
        vec!["echo".into(), placeholder.into()]
    }

    // Helper: build a cmd_template that always exits non-zero.
    fn fail_cmd() -> Vec<String> {
        vec!["false".into()]
    }

    // Helper: build a cmd_template that succeeds on the Nth attempt.
    // We use a tiny shell script written to a temp file instead, because
    // we can't share mutable state across tokio tasks easily in a test.
    // Instead we test retry behaviour via the counter file approach below.

    #[tokio::test]
    async fn run_all_succeeds_returns_zero_failures() {
        let lines = vec!["a".into(), "b".into(), "c".into()];
        let failures = run_all(lines, "^", &echo_cmd("^")).await;
        assert_eq!(failures, 0);
    }

    #[tokio::test]
    async fn run_all_counts_failures() {
        let lines = vec!["x".into()];
        let failures = run_all(lines, "^", &fail_cmd()).await;
        assert_eq!(failures, 1);
    }

    #[tokio::test]
    async fn run_all_until_success_succeeds_immediately() {
        let lines = vec!["a".into(), "b".into()];
        let failures = run_all_until_success(
            lines,
            "^",
            &echo_cmd("^"),
            3,
            Duration::ZERO,
        )
        .await;
        assert_eq!(failures, 0);
    }

    #[tokio::test]
    async fn run_all_until_success_exhausts_attempts() {
        // `false` always fails; we should use all 3 attempts and return 2 failures.
        let lines = vec!["x".into(), "y".into()];
        let failures = run_all_until_success(
            lines,
            "^",
            &fail_cmd(),
            3,
            Duration::ZERO,
        )
        .await;
        assert_eq!(failures, 2);
    }

    #[tokio::test]
    async fn run_all_until_success_retries_until_pass() {
        // Use a counter file to make the command fail twice then succeed.
        // We write a tiny shell one-liner that:
        //   1. reads a counter from a temp file
        //   2. increments it
        //   3. exits non-zero until counter >= 3
        let dir = tempfile::tempdir().unwrap();
        let counter_path = dir.path().join("counter");
        std::fs::write(&counter_path, "0").unwrap();
        let counter_str = counter_path.to_str().unwrap().to_string();

        // Shell fragment: increment counter file; fail if counter < 3.
        let script = format!(
            "c=$(cat {p}); c=$((c+1)); echo $c > {p}; [ $c -ge 3 ]",
            p = counter_str
        );
        let cmd_template = vec!["sh".into(), "-c".into(), script];

        let lines = vec!["ignored".into()];
        let failures = run_all_until_success(
            lines,
            "^",
            &cmd_template,
            5,
            Duration::ZERO,
        )
        .await;
        assert_eq!(failures, 0);

        // Verify it actually ran exactly 3 times (failed twice, passed on 3rd).
        let final_count: u32 = std::fs::read_to_string(&counter_path)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(final_count, 3);
    }

    #[tokio::test]
    async fn placeholder_substituted_in_all_positions() {
        // cmd = ["echo", "^-^"] with line "x" should produce "x-x"
        let cmd = vec!["echo".into(), "^-^".into()];
        let results = spawn_all(vec!["x".into()], "^", &cmd).await;
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(
            String::from_utf8_lossy(&results[0].stdout).trim(),
            "x-x"
        );
    }
}
