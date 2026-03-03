// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! CI entrypoint: verify the AI submission key in every changed file.
//!
//! Exit codes:
//!   0 — all checks passed (or no key was set at the anchor commit)
//!   1 — one or more files failed the check
//!   2 — unrecoverable error (bad environment, git failure, etc.)

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use env_traits::{AiEnv, FileEnv, GitEnv};

/// Well-known binary / non-text file extensions to skip.
const SKIP_EXTENSIONS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".bmp",
    ".pdf", ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z",
    ".wasm", ".bin", ".exe", ".dll", ".so", ".dylib",
    ".mp3", ".mp4", ".wav", ".ogg", ".flac",
    ".ttf", ".otf", ".woff", ".woff2",
    ".lock",
];

/// Returns `true` for binary or very short files that should not be AI-scanned.
pub fn should_skip(path: &str, content: &[u8]) -> bool {
    if content.len() < 32 {
        return true;
    }
    // Null-byte check on the first 512 bytes.
    let head = &content[..content.len().min(512)];
    if head.contains(&0u8) {
        return true;
    }
    let lower = path.to_lowercase();
    SKIP_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

struct Flagged {
    path:       String,
    confidence: f64,
}

/// Core logic.  Returns the process exit code.
pub fn run<F, G>(file: &F, git: &G, ai: &dyn AiEnv) -> ExitCode
where
    F: FileEnv,
    G: GitEnv,
{
    // 1. Resolve repo root.
    let repo_root = match git.repo_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("check-ai-key: cannot determine repo root: {e}");
            return ExitCode::from(2);
        }
    };

    // 2. Resolve anchor commit.
    let anchor = match keyguard::base_commit(file, git, &repo_root) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("check-ai-key: cannot resolve base commit: {e}");
            return ExitCode::from(2);
        }
    };
    let anchor = match anchor {
        Some(a) => a,
        None => {
            println!("check-ai-key: no anchor commit (orphan/initial); skipping.");
            return ExitCode::SUCCESS;
        }
    };
    println!("check-ai-key: anchor commit: {anchor}");

    // 3. Read expected key at the anchor commit.
    let key = match keyguard::read_key_at_commit(git, &repo_root, &anchor) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("check-ai-key: cannot read key at anchor commit: {e}");
            return ExitCode::from(2);
        }
    };
    let key = match key {
        Some(k) => k,
        None => {
            println!("check-ai-key: no key at anchor commit {anchor}; skipping.");
            return ExitCode::SUCCESS;
        }
    };
    println!("check-ai-key: expected key: {key}");

    // 4. Determine changed files.
    let files = match keyguard::changed_files(git, &repo_root, &anchor) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("check-ai-key: cannot determine changed files: {e}");
            return ExitCode::from(2);
        }
    };
    if files.is_empty() {
        println!("check-ai-key: no changed files.");
        return ExitCode::SUCCESS;
    }
    println!("check-ai-key: checking {} file(s)…", files.len());

    // 5. Find files missing the key.
    let missing = match keyguard::scan_for_key(file, &repo_root, &files, &key) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("check-ai-key: error scanning for key: {e}");
            return ExitCode::from(2);
        }
    };
    if missing.is_empty() {
        println!("check-ai-key: all changed files contain the submission key. ✓");
        return ExitCode::SUCCESS;
    }
    println!(
        "check-ai-key: {} file(s) missing key; running AI scan…",
        missing.len()
    );

    // 6. AI-scan files that lack the key.
    let flagged = ai_scan(&repo_root, &missing, ai, file);

    if flagged.is_empty() {
        println!("check-ai-key: AI scan found no AI-generated content. ✓");
        return ExitCode::SUCCESS;
    }

    // 7. Report failures.
    eprintln!(
        "\n❌  AI key check failed: {} file(s) appear AI-generated and are missing the submission key.\n",
        flagged.len()
    );
    eprintln!("Expected key: {key}\n");
    eprintln!("To fix: embed the key (from key.agents_.md) in each flagged file.\n");
    eprintln!("Flagged files:");
    for f in &flagged {
        eprintln!("  {} (confidence {:.0}%)", f.path, f.confidence * 100.0);
    }
    eprintln!();
    ExitCode::from(1)
}

fn ai_scan<F: FileEnv>(
    repo_root: &Path,
    paths: &[String],
    ai: &dyn AiEnv,
    file: &F,
) -> Vec<Flagged> {
    let mut flagged = Vec::new();
    for rel in paths {
        let abs: PathBuf = repo_root.join(rel);
        let content = match file.read_file(&abs) {
            Ok(c) => c,
            Err(_) => continue, // deleted or unreadable
        };
        if should_skip(rel, &content) {
            println!("check-ai-key:   skip  {rel} (binary or too short)");
            continue;
        }
        match ai.scan(&abs, &content) {
            Err(e) => {
                println!("check-ai-key:   warn  {rel}: scanner error: {e}");
            }
            Ok((likely, confidence)) => {
                if likely {
                    println!(
                        "check-ai-key:   FAIL  {rel} (AI confidence {:.0}%)",
                        confidence * 100.0
                    );
                    flagged.push(Flagged { path: rel.clone(), confidence });
                } else {
                    println!(
                        "check-ai-key:   pass  {rel} (AI confidence {:.0}%)",
                        confidence * 100.0
                    );
                }
            }
        }
    }
    flagged
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use env_fake::{FakeAiEnv, FakeFileEnv, FakeGitEnv};

    fn repo() -> PathBuf {
        PathBuf::from("/repo")
    }

    const KEY: &str = "AIKEY-testkey234abc";
    const KEY_CONTENT: &[u8] = b"# AI Submission Key\n\nKey: AIKEY-testkey234abc\n";

    fn base_git() -> FakeGitEnv {
        FakeGitEnv::default()
            .with_repo_root("/repo")
            .with_rev("HEAD^", "anchor")
            .with_show_file("anchor", "key.agents_.md", KEY_CONTENT)
            .with_changed_files(vec!["src/lib.rs".into()])
    }

    #[test]
    fn all_files_have_key_passes() {
        let content = format!("// {KEY}\nfn main() {{}}");
        let file = FakeFileEnv::default().with_file("/repo/src/lib.rs", content.as_bytes());
        let git = base_git();
        let ai = FakeAiEnv::default().always(false, 0.0);
        assert_eq!(run(&file, &git, &ai), ExitCode::SUCCESS);
    }

    #[test]
    fn missing_key_not_ai_passes() {
        let file = FakeFileEnv::default()
            .with_file("/repo/src/lib.rs", b"fn main() {}");
        let git = base_git();
        let ai = FakeAiEnv::default().always(false, 0.1);
        assert_eq!(run(&file, &git, &ai), ExitCode::SUCCESS);
    }

    #[test]
    fn missing_key_and_ai_flagged_fails() {
        let content = b"fn main() { /* padding to exceed the 32 byte minimum threshold */ }";
        let file = FakeFileEnv::default()
            .with_file("/repo/src/lib.rs", content.as_ref());
        let git = base_git();
        let ai = FakeAiEnv::default().always(true, 0.9);
        assert_eq!(run(&file, &git, &ai), ExitCode::from(1));
    }

    #[test]
    fn orphan_commit_skips_check() {
        let file = FakeFileEnv::default();
        // FakeGitEnv with no rev("HEAD^") → rev_parse fails → orphan
        let git = FakeGitEnv::default().with_repo_root("/repo");
        let ai = FakeAiEnv::default().always(true, 0.99);
        assert_eq!(run(&file, &git, &ai), ExitCode::SUCCESS);
    }

    #[test]
    fn should_skip_binary_file() {
        let mut content = vec![0u8; 100];
        content[10] = 0; // null byte
        assert!(should_skip("foo.rs", &content));
    }

    #[test]
    fn should_skip_short_file() {
        assert!(should_skip("foo.rs", b"short"));
    }

    #[test]
    fn should_skip_known_extension() {
        assert!(should_skip("image.png", &vec![0x89u8; 100]));
    }
}

fn main() -> ExitCode {
    use aiscan::{build_ai_env, AiEnvConfig};
    use env_real::{OsFileEnv, ProcessGitEnv, ReqwestNetworkEnv};

    let file    = OsFileEnv;
    let git     = ProcessGitEnv;
    let config  = AiEnvConfig::from_env();
    let network = ReqwestNetworkEnv;
    let ai = match build_ai_env(config, network) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("check-ai-key: cannot build AI scanner: {e}");
            return ExitCode::from(2);
        }
    };
    run(&file, &git, &*ai)
}
