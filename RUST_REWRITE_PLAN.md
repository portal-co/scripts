# Rust Rewrite Plan

<!-- AIKEY-l4qkxonqry2b4gj7bsrkqpryiy -->

This document is the authoritative plan for two concurrent goals:

1. **Rewrite every Go tool and library in Rust**, placing them as workspace
   crates inside the existing Cargo workspace.
2. **Introduce pluggable environment traits** for the five operation categories
   that appear across the codebase — file I/O, git, GitHub API, network, and AI
   — so that every binary is testable in isolation and the real implementations
   can be swapped for fakes in CI or future alternative backends.

---

## Table of Contents

1. [Inventory of Existing Go Code](#1-inventory-of-existing-go-code)
2. [Target Rust Workspace Layout](#2-target-rust-workspace-layout)
3. [Environment Trait Design](#3-environment-trait-design)
4. [Crate-by-Crate Rewrite Plan](#4-crate-by-crate-rewrite-plan)
5. [Shell Script Treatment](#5-shell-script-treatment)
6. [CI / Workflow Changes](#6-ci--workflow-changes)
7. [Migration Sequence & Branch Strategy](#7-migration-sequence--branch-strategy)
8. [Testing Strategy](#8-testing-strategy)
9. [Open Questions](#9-open-questions)

---

## 1. Inventory of Existing Go Code

### Libraries (`pkg/`)

| Package | File(s) | Responsibility |
|---|---|---|
| `pkg/aiscan` | `aiscan.go`, `heuristic.go`, `http.go` | `Scanner` interface + `HeuristicScanner` (pure Rust logic), `HTTPScanner` (POST to remote endpoint), `NoopScanner`, `FromEnv` factory |
| `pkg/keyguard` | `keyguard.go` | Read key from `key.agents_.md` / `AGENTS.md` at HEAD or a specific commit; resolve base/anchor commit for CI; list changed files; scan files for key presence |
| `pkg/pkgjson` | `pkgjson.go` | Parse `package.json` for name/version/private/workspaces; in-place semver bump without reserialising the whole document; detect indentation style |
| `pkg/repoutils` | `repoutils.go` | Git root/name helpers; `gh`-CLI wrappers for org/repo listing; GitHub REST API content listing; URL parsing; `RunCmd` |

### Binaries (`tools/`)

| Tool | Entry Point | Dependencies | What it does |
|---|---|---|---|
| `check_ai_key` | `tools/check_ai_key/main.go` | `aiscan`, `keyguard`, `repoutils` | CI entrypoint: resolves anchor commit, reads key, diffs changed files, key-scans then AI-scans missing files; exits 0/1/2 |
| `inject_key` | `tools/inject_key/main.go` | stdlib only | Upserts / rotates `key.agents_.md` in a repo; generates 128-bit base32 AIKEY token |
| `bump_npm_version` | `tools/bump_npm_version/main.go` | `pkgjson` | Walks repo for `package.json`s, bumps semver, prints `name@ver\tpath` lines consumed by CI; supports dry-run |
| `agents_zip` | `tools/agents_zip/main.go` | `repoutils` | Assembles a zip archive of all `*agents_.md` files from a list of GitHub or local repos |
| `scaffold_repo` | `tools/scaffold_repo/main.go` | `repoutils` | Scaffolds a new Cargo + npm workspace repo on disk; optionally git-init and commit |
| `copy-feed-files` | `copy-feed-files.go` (root) | `repoutils` | Scans all org repos for `*.{reponame}.feed-out.md` files via GitHub API and copies them locally as `*.feed-in.md` |
| `forfiles` | `tools/forfiles/forfiles.go` | stdlib only | Reads lines from stdin, runs a command per line in parallel (like `xargs -P`) |

### Shell wrappers (`tools/git/`)

`addignores.sh`, `codeall.sh`, `commitall2.sh`, `commitandpushall.sh`,
`fetch-repos.sh`, `fetch-repos-gh.sh`, `fmtallcargo.sh`, `ocrall.sh`,
`pull.sh`, `pullall.sh`, `pushall.sh`, `sortallignores.sh`, `splatall.sh`,
`updateallcargo.sh`, `updateallcargoplus.sh` — all thin wrappers that either
call `forfiles` or `gh`/`git` directly. These are addressed in
[§5](#5-shell-script-treatment).

---

## 2. Target Rust Workspace Layout

The existing `Cargo.toml` workspace is currently empty. All new crates go
under `crates/`. The workspace `members` list is updated as each crate is added.

```
scripts/
├── Cargo.toml                  ← workspace root (update members list)
├── crates/
│   ├── env-traits/             ← NEW: the five environment trait definitions
│   ├── env-real/               ← NEW: real (production) implementations
│   ├── env-fake/               ← NEW: in-memory / fake implementations for tests
│   ├── aiscan/                 ← replaces pkg/aiscan
│   ├── keyguard/               ← replaces pkg/keyguard
│   ├── pkgjson/                ← replaces pkg/pkgjson
│   ├── repoutils/              ← replaces pkg/repoutils
│   ├── check-ai-key/           ← replaces tools/check_ai_key  (binary)
│   ├── inject-key/             ← replaces tools/inject_key    (binary)
│   ├── bump-npm-version/       ← replaces tools/bump_npm_version (binary)
│   ├── agents-zip/             ← replaces tools/agents_zip    (binary)
│   ├── scaffold-repo/          ← replaces tools/scaffold_repo (binary)
│   ├── copy-feed-files/        ← replaces copy-feed-files.go  (binary)
│   └── forfiles/               ← replaces tools/forfiles      (binary)
└── (Go source files kept until each crate is done, then deleted)
```

### `Cargo.toml` workspace-level dependencies (shared via `[workspace.dependencies]`)

```toml
[workspace.dependencies]
# async runtime
tokio       = { version = "1", features = ["full"] }
# HTTP client
reqwest     = { version = "0.12", features = ["json", "blocking"] }
# JSON
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
# zip
zip         = "2"
# regex
regex       = "1"
# CLI flags
clap        = { version = "4", features = ["derive"] }
# error handling
anyhow      = "1"
thiserror   = "1"
# tracing / logging
tracing     = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
# testing fakes
tempfile    = "3"
```

---

## 3. Environment Trait Design

### 3.1 Motivation

Every Go tool shells out to `git` or `gh` via `exec.Command`, reads real files
with `os.ReadFile`, and makes real HTTP calls. That makes unit testing hard and
prevents injecting alternative backends (e.g. a pure-Rust `git2` implementation,
a mock GitHub server, or a recorded HTTP replay). The rewrite addresses this by
defining all external operations as traits that every library and binary receives
as a generic parameter.

### 3.2 The five traits (crate `env-traits`)

All traits live in `crates/env-traits/src/lib.rs`. They are intentionally
minimal — only the operations the existing tools actually use.

---

#### `FileEnv`

Covers all local filesystem operations.

```rust
pub trait FileEnv: Send + Sync {
    /// Read the full contents of a file.
    fn read_file(&self, path: &Path) -> Result<Vec<u8>>;

    /// Write (create or overwrite) a file.
    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<()>;

    /// Test whether a path exists and is a regular file.
    fn file_exists(&self, path: &Path) -> bool;

    /// Test whether a path exists and is a directory.
    fn dir_exists(&self, path: &Path) -> bool;

    /// Create a directory and all parents.
    fn create_dir_all(&self, path: &Path) -> Result<()>;

    /// Walk a directory tree; yields (absolute_path, is_dir) pairs.
    fn walk(&self, root: &Path)
        -> Result<Box<dyn Iterator<Item = Result<(PathBuf, bool)>> + '_>>;

    /// Read a single env var (mirrors std::env::var behaviour).
    fn env_var(&self, key: &str) -> Option<String>;
}
```

**Used by:** `keyguard`, `pkgjson`, `inject-key`, `bump-npm-version`,
`scaffold-repo`, `check-ai-key`, `copy-feed-files`, `agents-zip`.

---

#### `GitEnv`

Covers pure-git operations (no network implied).

```rust
pub trait GitEnv: Send + Sync {
    /// Equivalent of `git rev-parse --show-toplevel`.
    fn repo_root(&self) -> Result<PathBuf>;

    /// Equivalent of `git rev-parse <rev>`, returns the full SHA.
    fn rev_parse(&self, repo_root: &Path, rev: &str) -> Result<String>;

    /// Equivalent of `git show <commit>:<path>`, returns file bytes.
    fn show_file(&self, repo_root: &Path, commit: &str, path: &str)
        -> Result<Vec<u8>>;

    /// Equivalent of `git diff --name-only --diff-filter=ACMR <a> HEAD`.
    fn changed_files(
        &self,
        repo_root: &Path,
        base: &str,
    ) -> Result<Vec<String>>;

    /// Equivalent of `git merge-base HEAD origin/<branch>`.
    fn merge_base(
        &self,
        repo_root: &Path,
        branch: &str,
    ) -> Result<String>;

    /// Fetch a remote ref (used before merge-base on shallow clones).
    fn fetch(&self, repo_root: &Path, remote: &str, refspec: &str)
        -> Result<()>;

    /// Run `git init` in a directory.
    fn init(&self, dir: &Path) -> Result<()>;

    /// Stage all and commit with a message.
    fn add_and_commit(&self, repo_root: &Path, message: &str) -> Result<()>;
}
```

**Used by:** `keyguard`, `repoutils`, `scaffold-repo`, `check-ai-key`.

---

#### `GitHubEnv`

Covers GitHub API / `gh` CLI operations.

```rust
pub trait GitHubEnv: Send + Sync {
    /// Return the login of the owner of the current repo
    /// (mirrors `gh repo view --json owner --jq .owner.login`).
    fn current_owner(&self) -> Result<String>;

    /// List all repository names in an org
    /// (mirrors `gh repo list <org> --limit N --json name --jq .[].name`).
    fn list_repos(&self, org: &str, limit: usize) -> Result<Vec<String>>;

    /// Recursively list files in a repo path via the Contents API.
    fn list_contents(
        &self,
        org: &str,
        repo: &str,
        path: &str,
    ) -> Result<Vec<GitHubFile>>;

    /// Download the content of a file identified by its GitHub raw download URL.
    /// This is intentionally part of `GitHubEnv` rather than `NetworkEnv`
    /// because the download is a first-class GitHub operation: it may require
    /// authentication headers, may be proxied through the `gh` CLI in
    /// restricted environments, and needs a distinct fake in tests (returning
    /// pre-seeded content keyed by path rather than by arbitrary URL).
    fn download_file(&self, download_url: &str) -> Result<Vec<u8>>;
}

pub struct GitHubFile {
    pub name: String,
    pub path: String,
    pub r#type: String,       // "file" | "dir"
    pub download_url: Option<String>,
}
```

**Used by:** `repoutils`, `copy-feed-files`, `agents-zip`.

---

#### `NetworkEnv`

A thin, purpose-specific HTTP trait (separate from `GitHubEnv` so that the
AI scanner can use it independently without pulling in GitHub concepts).

```rust
pub trait NetworkEnv: Send + Sync {
    /// POST JSON bytes to a URL; returns the response body bytes.
    /// Non-2xx responses must be returned as `Err`.
    fn post_json(&self, url: &str, body: &[u8]) -> Result<Vec<u8>>;

    /// GET a URL; returns the response body bytes.
    /// Non-2xx responses must be returned as `Err`.
    fn get(&self, url: &str) -> Result<Vec<u8>>;
}
```

**Used by:** `aiscan` (HTTP backend), `copy-feed-files`, `agents-zip`.

---

#### `AiEnv`

Wraps the scanning concern so the check tool can treat the whole AI-detection
subsystem as a single swappable dependency rather than wiring `NetworkEnv`
itself.

```rust
pub trait AiEnv: Send + Sync {
    /// Inspect file content; return (likely_ai, confidence in [0,1]).
    /// Error means the scan itself failed.
    fn scan(
        &self,
        path: &Path,
        content: &[u8],
    ) -> Result<(bool, f64)>;
}
```

**Used by:** `check-ai-key`.  
The `aiscan` crate provides `HeuristicAiEnv`, `HttpAiEnv`, and `NoopAiEnv`
implementations that depend on `NetworkEnv`. An `AiEnvFromEnv::new(network)`
factory replaces the Go `FromEnv()`.

---

### 3.3 Combining traits

Binaries that need several environments can either take them as separate generic
parameters or combine them into a single struct. For small binaries the
multi-generic approach is cleaner:

```rust
pub fn run<F, G, H>(file: &F, git: &G, gh: &H) -> anyhow::Result<()>
where
    F: FileEnv,
    G: GitEnv,
    H: GitHubEnv,
{ ... }
```

For larger surfaces, a composed `Env` struct works well:

```rust
pub struct Env<F, G, H, N, A> {
    pub file: F,
    pub git: G,
    pub github: H,
    pub network: N,
    pub ai: A,
}
```

### 3.4 Real implementations (crate `env-real`)

| Trait | Implementation struct | Backend |
|---|---|---|
| `FileEnv` | `OsFileEnv` | `std::fs` + `std::env` |
| `GitEnv` | `ProcessGitEnv` | `std::process::Command` → `git` binary (mirrors Go exactly) |
| `GitHubEnv` | `GhCliGitHubEnv` | `std::process::Command` → `gh` CLI (mirrors Go exactly) |
| `NetworkEnv` | `ReqwestNetworkEnv` | `reqwest` (async behind a `tokio::runtime::Handle`) |
| `AiEnv` | Built from `aiscan::AiEnvFromEnv` | reads `AI_SCAN_BACKEND` / `AI_SCAN_ENDPOINT` |

For `GitEnv` an optional `Git2GitEnv` (using the `git2` crate) can be added
later without changing any call sites.

### 3.5 Fake implementations (crate `env-fake`)

| Trait | Fake struct | State |
|---|---|---|
| `FileEnv` | `FakeFileEnv` | `HashMap<PathBuf, Vec<u8>>` in-memory filesystem + env var map |
| `GitEnv` | `FakeGitEnv` | Pre-seeded commit map; `repo_root` returns a configurable path; `changed_files` returns a configurable list |
| `GitHubEnv` | `FakeGitHubEnv` | Configurable repo list + file tree |
| `NetworkEnv` | `FakeNetworkEnv` | Request-keyed response map; records calls for assertions |
| `AiEnv` | `FakeAiEnv` | Returns a configured `(bool, f64)` per path or a global default |

All fakes derive `Default` for ergonomic test setup:
```rust
let file = FakeFileEnv::default()
    .with_file("/repo/key.agents_.md", b"Key: AIKEY-abc123");
let git  = FakeGitEnv::default()
    .with_repo_root("/repo")
    .with_changed_files(vec!["src/lib.rs".into()]);
let ai   = FakeAiEnv::default().always(false, 0.0);
```

---

## 4. Crate-by-Crate Rewrite Plan

Each sub-section lists: the target crate, its Go source(s), external crate
dependencies, which traits it consumes, which traits (if any) it provides,
and a checklist of items to implement.

---

### 4.1 `crates/env-traits`

**No Go equivalent.** Pure trait definitions + shared data types.

- [ ] Define `FileEnv`, `GitEnv`, `GitHubEnv`, `NetworkEnv`, `AiEnv` as
      described in §3.2.
- [ ] Define `GitHubFile` struct (shared between `GitHubEnv` and `repoutils`).
- [ ] `pub type Result<T> = anyhow::Result<T>;` re-export for ergonomics.
- [ ] Zero external dependencies other than `anyhow`.

---

### 4.2 `crates/env-real`

**No Go equivalent.** All five real implementations.

- [ ] `OsFileEnv` — thin wrapper around `std::fs`; `walk` uses
      `walkdir` crate.
- [ ] `ProcessGitEnv` — each method runs `git <args>` via
      `std::process::Command`, returns stderr as `Err` on non-zero exit.
- [ ] `GhCliGitHubEnv` — methods run `gh repo list ...`, `gh api ...`,
      `gh repo view ...` via `Command`.  `list_contents` recursively calls
      the GitHub REST API (via `gh api`) and handles pagination.
- [ ] `ReqwestNetworkEnv` — uses `reqwest::blocking` for simplicity in the
      initial version (all current Go HTTP calls are synchronous).
- [ ] Unit tests that smoke-test each real implementation when the relevant
      binary (`git`, `gh`) is on `PATH`.

---

### 4.3 `crates/env-fake`

**No Go equivalent.** All five fake/in-memory implementations.

- [ ] `FakeFileEnv` — `HashMap<PathBuf, Vec<u8>>` + `HashMap<String, String>`
      for env vars; builder methods `with_file`, `with_env`.
- [ ] `FakeGitEnv` — builder methods `with_repo_root`, `with_rev`,
      `with_show_file`, `with_changed_files`, `with_merge_base`.
- [ ] `FakeGitHubEnv` — builder methods `with_repos`, `with_contents`,
      `with_download`.
- [ ] `FakeNetworkEnv` — `with_response(url, body)` + `assert_called(url)`.
- [ ] `FakeAiEnv` — `always(likely, conf)` + per-path overrides.

---

### 4.4 `crates/aiscan`

**Replaces:** `pkg/aiscan/`  
**Trait consumed:** `NetworkEnv` (by `HttpAiEnv` only)  
**Trait provided:** implements `AiEnv`

- [ ] Port `HeuristicScanner` → `HeuristicAiEnv`.  All phrase lists,
      `splitSentences`, `sigmoidNorm`, scoring weights must match the Go
      implementation exactly so CI behaviour is identical.
- [ ] Port `HTTPScanner` → `HttpAiEnv<N: NetworkEnv>`.  Uses `N::post_json`.
      Request / response shape unchanged.
- [ ] `NoopAiEnv` — always returns `(false, 0.0)`.
- [ ] `AiEnvFromEnv::new<N: NetworkEnv>(network: N) -> Result<impl AiEnv>` —
      reads `AI_SCAN_BACKEND` and `AI_SCAN_ENDPOINT` from `FileEnv::env_var`
      (pass in `&dyn FileEnv` or just read from `std::env` here; see note).
- [ ] Unit tests for `HeuristicAiEnv` using the same test cases as the Go
      heuristic, plus regression fixtures.
- [ ] Unit tests for `HttpAiEnv` using `FakeNetworkEnv`.

> **Note on `env_var` for `FromEnv`:** Because env var reads in `FromEnv` are
> config-time (not per-scan), passing `FileEnv` just for this is awkward.
> Prefer a dedicated `AiEnvConfig { backend, endpoint }` struct built from
> `std::env::var` at the binary's `main()`, then passed into
> `AiEnvFromEnv::from_config`.

---

### 4.5 `crates/keyguard`

**Replaces:** `pkg/keyguard/`  
**Traits consumed:** `FileEnv`, `GitEnv`

- [ ] `fn read_key<F: FileEnv>(file: &F, repo_root: &Path) -> Result<Option<String>>`  
      Tries `key.agents_.md` then `AGENTS.md`.
- [ ] `fn read_key_at_commit<G: GitEnv>(git: &G, repo_root: &Path, commit: &str) -> Result<Option<String>>`  
      Uses `G::show_file`.
- [ ] `fn base_commit<F: FileEnv, G: GitEnv>(file: &F, git: &G, repo_root: &Path) -> Result<Option<String>>`  
      Reads `GITHUB_EVENT_NAME` / `GITHUB_BASE_REF` from `file.env_var`,
      calls `G::fetch` + `G::merge_base` for PR events, falls back to
      `G::rev_parse("HEAD^")`.
- [ ] `fn changed_files<G: GitEnv>(git: &G, repo_root: &Path, base: &str) -> Result<Vec<String>>`  
      Delegates to `G::changed_files`.
- [ ] `fn scan_for_key<F: FileEnv>(file: &F, repo_root: &Path, paths: &[String], key: &str) -> Result<Vec<String>>`  
      Returns paths that do NOT contain the key literal.
- [ ] Unit tests using `FakeFileEnv` + `FakeGitEnv` covering: no key file,
      key in `AGENTS.md`, PR event merge-base, push event HEAD^, orphan commit.

---

### 4.6 `crates/pkgjson`

**Replaces:** `pkg/pkgjson/`  
**Traits consumed:** `FileEnv`

- [ ] `fn read<F: FileEnv>(file: &F, path: &Path) -> Result<Package>`
- [ ] `struct Package { name, version, private, has_workspaces }`
- [ ] `impl Package { fn is_publishable(&self, add_missing: bool) -> bool }`
- [ ] `fn set_version<F: FileEnv>(file: &F, path: &Path, new_version: &str) -> Result<()>`  
      In-place regex splice, preserving key order and indentation — port the
      Go regex approach using the `regex` crate.
- [ ] `fn bump_version(version: &str, part: BumpPart) -> Result<String>`  
      Pure function; no env access needed.
- [ ] `enum BumpPart { Major, Minor, Patch }`
- [ ] `fn detect_indent(data: &[u8]) -> &str`
- [ ] Unit tests for: round-trip read/write, version absent → insert after
      name field, compact vs. pretty JSON, all bump parts, bad semver error.

---

### 4.7 `crates/repoutils`

**Replaces:** `pkg/repoutils/`  
**Traits consumed:** `GitEnv`, `GitHubEnv`

This crate is thin; most logic lives in the trait implementations.

- [ ] `fn current_repo_name<G: GitEnv>(git: &G) -> Result<String>`  
      `git.repo_root().map(|p| p.file_name()...)`
- [ ] `fn parse_org_repo(input: &str) -> OrgRepo`  
      Pure; handles `org/repo`, full GitHub HTTPS URL, SSH URL; no env access.
- [ ] `struct OrgRepo { org: String, repo: String, is_remote: bool }`
- [ ] `fn is_git_url(s: &str) -> bool` — pure predicate.
- [ ] `fn repo_name_from_url(url: &str) -> String` — pure extraction.
- [ ] Unit tests for `parse_org_repo` covering all URL formats.

---

### 4.8 `crates/check-ai-key` (binary)

**Replaces:** `tools/check_ai_key/main.go`  
**Traits consumed:** `FileEnv`, `GitEnv`, `AiEnv`

- [ ] `pub fn run<F, G, A>(file: &F, git: &G, ai: &A) -> ExitCode`  
      Exact logic parity with Go `run()`:
  1. Resolve repo root via `G::repo_root`.
  2. Resolve anchor commit via `keyguard::base_commit`.
  3. Read expected key via `keyguard::read_key_at_commit`.
  4. Determine changed files via `keyguard::changed_files`.
  5. Scan for key via `keyguard::scan_for_key`.
  6. AI-scan missing files with `A::scan`, skip binary / short files.
  7. Print summary; return `ExitCode` 0 / 1 / 2.
- [ ] `fn should_skip(path: &str, content: &[u8]) -> bool` — port exact
      extension list from Go.
- [ ] `main()` constructs real impls and calls `run`.
- [ ] Integration test using all fake impls: no-key scenario, all-pass, one
      flagged as AI, anchor = None (orphan skip).

---

### 4.9 `crates/inject-key` (binary)

**Replaces:** `tools/inject_key/main.go`  
**Traits consumed:** `FileEnv`, `GitEnv`

- [ ] `fn generate_key() -> Result<String>` — 128 bits via `rand::random`,
      base32 encoded with `base32` crate (no padding), prefixed `AIKEY-`.
- [ ] `fn extract_existing_key(content: &str) -> Option<String>` — pure parser.
- [ ] `fn render_file(key: &str) -> String` — render the markdown template;
      content must be byte-for-byte identical to Go version.
- [ ] `pub fn run<F: FileEnv, G: GitEnv>(file: &F, git: &G, opts: Opts) -> Result<()>`
- [ ] CLI via `clap`: `--repo`, `--rotate`, `--dry-run`.
- [ ] Unit tests: key generation format, extract/no-extract, render idempotency.

---

### 4.10 `crates/bump-npm-version` (binary)

**Replaces:** `tools/bump_npm_version/main.go`  
**Traits consumed:** `FileEnv`, `GitEnv`

- [ ] Walk `repo_root` via `FileEnv::walk`, skipping `node_modules`, `.git`,
      and hidden dirs; collect `package.json` paths.
- [ ] For each: `pkgjson::read`, `is_publishable`, `bump_version`,
      `set_version` (unless dry-run).
- [ ] Print `name@ver\tpath` on success (same tab-separated format for CI).
- [ ] Exit 2 if nothing bumped.
- [ ] CLI: `--repo`, `--bump`, `--add-missing`, `--dry-run`.
- [ ] Integration tests using `FakeFileEnv` seeded with a few `package.json`s.

---

### 4.11 `crates/agents-zip` (binary)

**Replaces:** `tools/agents_zip/main.go`  
**Traits consumed:** `FileEnv`, `GitHubEnv`

- [ ] For each repo arg: `repoutils::parse_org_repo` → local walk or GitHub
      API listing.
- [ ] Filter files matching `*agents_.md`.
- [ ] Download remote files via `GitHubEnv::download_file`; read local files
      via `FileEnv::read_file`.
- [ ] Build zip in-memory using the `zip` crate; write via `FileEnv::write_file`.
- [ ] CLI: `-o <output>`, positional `<repo...>`.
- [ ] Unit tests using fakes: one local repo, one remote repo, missing download.

---

### 4.12 `crates/scaffold-repo` (binary)

**Replaces:** `tools/scaffold_repo/main.go`  
**Traits consumed:** `FileEnv`, `GitEnv`

- [ ] Port `build_root_cargo`, `build_package_json`, `build_gitignore` as
      pure functions returning `String`.
- [ ] `pub fn scaffold<F: FileEnv, G: GitEnv>(file: &F, git: &G, opts: ScaffoldOpts) -> Result<()>`
- [ ] `pub fn update<F: FileEnv, G: GitEnv>(file: &F, git: &G, repo_root: &Path) -> Result<()>`
- [ ] CLI: `--name`, `--out`, `--description`, `--license`, `--workspace-private`,
      `--git-init`, `--update`.
- [ ] Unit tests using `FakeFileEnv` + `FakeGitEnv` for both scaffold and update paths.

---

### 4.13 `crates/copy-feed-files` (binary)

**Replaces:** `copy-feed-files.go`  
**Traits consumed:** `FileEnv`, `GitEnv`, `GitHubEnv`

- [ ] Resolve repo name + org via `GitEnv::repo_root` + `GitHubEnv::current_owner`.
- [ ] List all org repos via `GitHubEnv::list_repos`.
- [ ] For each repo, `list_contents` recursively and filter by
      `*.{reponame}.feed-out.md` suffix.
- [ ] Download each match via `GitHubEnv::download_file`.
- [ ] Transform path: strip feed-out suffix, append `.feed-in.md`, write via
      `FileEnv::write_file`.
- [ ] Unit tests using all three fakes.

---

### 4.14 `crates/forfiles` (binary)

**Replaces:** `tools/forfiles/forfiles.go`  
**Traits consumed:** none (pure process orchestration; `FileEnv` not needed)

- [ ] Read lines from stdin with `BufReader::lines`.
- [ ] For each line, spawn `tokio::process::Command` with placeholder replaced.
- [ ] Collect all `JoinHandle`s; wait for all; forward combined stdout/stderr.
- [ ] CLI: positional `<placeholder> <cmd> [args...]`.
- [ ] Mirror exact semantics: lines are spawned concurrently, all output is
      printed, non-zero exit of a child is logged to stderr but does not abort
      others.
- [ ] Unit test (with a real echo/true command) or skip in favour of the
      shell wrapper being unchanged.

---

## 5. Shell Script Treatment

The `tools/git/*.sh` scripts use `forfiles` solely as an inline `xargs -P`
replacement. With the Rust `forfiles` binary installed the scripts work
unchanged; the only required update is to replace:

```sh
go run $(dirname $0)/../forfiles/forfiles.go '^' ...
```

with:

```sh
forfiles '^' ...
```

after the binary is on `$PATH`. This can be done as a single commit after the
`forfiles` crate is published/installed. Until then the `go run` form continues
to work in parallel.

`splat.sh`, `update_commit.sh`, and the CI `splat.sh` wrapper have no Go
dependency and need no changes.

---

## 6. CI / Workflow Changes

### 6.1 `actions/ai_key_check.yaml`

Replace the Go step:

```yaml
# Before
- uses: actions/setup-go@v5
  with:
    go-version: stable
- name: Check AI submission key in changed files
  run: go run ./tools/check_ai_key

# After
- uses: dtolnay/rust-toolchain@stable
- uses: Swatinem/rust-cache@v2
- name: Build check-ai-key
  run: cargo build --release -p check-ai-key
- name: Check AI submission key in changed files
  run: ./target/release/check-ai-key
```

### 6.2 `actions/rotate_key.yaml`

```yaml
# Before
- uses: actions/setup-go@v5
  with:
    go-version: stable
- name: Rotate AI submission key
  run: go run ./tools/inject_key -rotate

# After
- uses: dtolnay/rust-toolchain@stable
- uses: Swatinem/rust-cache@v2
- name: Build inject-key
  run: cargo build --release -p inject-key
- name: Rotate AI submission key
  run: ./target/release/inject-key --rotate
```

### 6.3 `actions/npm-publish.yaml`

```yaml
# Before
- uses: actions/setup-go@v5
  with:
    go-version: stable
# ... go run ./tools/bump_npm_version ...

# After
- uses: dtolnay/rust-toolchain@stable
- uses: Swatinem/rust-cache@v2
- name: Build bump-npm-version
  run: cargo build --release -p bump-npm-version
# ... ./target/release/bump-npm-version ...
```

### 6.4 `go.mod`, `go.sum` deletion

Both files and all `*.go` sources are deleted once every crate listed in §4
has been merged and all CI workflows pass on `main`.

### 6.5 New `actions/test.yaml` Rust step

Add `cargo test --all --workspace --locked` (it is already listed in the
yaml but the workspace currently has no members).

---

## 7. Migration Sequence & Branch Strategy

Work proceeds on `feat/rust` (already exists) with one PR per crate group.

| Phase | Crates / work | Prerequisite |
|---|---|---|
| 0 | Update `Cargo.toml` members list, add `[workspace.dependencies]` | — |
| 1 | `env-traits`, `env-fake` | Phase 0 |
| 2 | `env-real` | Phase 1 |
| 3 | `aiscan`, `keyguard`, `pkgjson`, `repoutils` (libraries) | Phase 1 |
| 4 | `check-ai-key`, `inject-key` (highest CI impact) | Phases 2, 3 |
| 5 | Update `ai_key_check.yaml` and `rotate_key.yaml` to use Rust binaries | Phase 4 |
| 6 | `bump-npm-version` | Phase 3 |
| 7 | Update `npm-publish.yaml` | Phase 6 |
| 8 | `forfiles`, `agents-zip`, `scaffold-repo`, `copy-feed-files` | Phases 2, 3 |
| 9 | Update shell wrappers to drop `go run` | Phase 8 |
| 10 | Delete all Go source files + `go.mod` | Phase 9 + all CI green |

Each phase produces a PR that keeps the Go tools running in parallel until the
corresponding workflow step has been switched, verified, and green on `main`.

---

## 8. Testing Strategy

### Unit tests

Every library crate (`aiscan`, `keyguard`, `pkgjson`, `repoutils`) must have
`#[cfg(test)]` modules covering the cases listed in §4. Tests use only `env-fake`
and must pass in a sandboxed environment (no real `git`, `gh`, or network).

### Integration tests (binary crates)

Binary crates have an `tests/` directory with `#[test]` functions that call
the public `run(...)` function with fake environments. These also run on every
`cargo test --workspace`.

### End-to-end (optional)

A `scripts/tests/e2e/` harness (shell or Rust) that checks out a temp repo,
runs the real binaries, and asserts exit codes / file state. Guarded by
`#[cfg(feature = "e2e")]` or an env var so CI only runs it when `git` and `gh`
are available.

### Parity tests

For `aiscan::HeuristicAiEnv`, a golden-file test suite reproduces the Go
heuristic on a corpus of fixture texts, asserting identical `(likely_ai,
confidence)` output. This is the most important parity guarantee.

---

## 9. Open Questions

| # | Question | Proposed resolution |
|---|---|---|
| Q1 | Should `GitEnv` use `git2` (libgit2) or shell out to the `git` binary? | Start with `ProcessGitEnv` (shell-out) in `env-real` for exact parity. Add `Git2GitEnv` as an opt-in feature later. |
| Q2 | `forfiles` is currently called with `go run ...` inline in shell scripts on the developer's machine. How do we install the Rust binary so those scripts work locally? | Add a `cargo install --path crates/forfiles` step to the developer setup docs. The `go run` fallback form is kept until the binary is confirmed present. |
| Q3 | Should `env-traits` use async traits (`async-trait`) or sync-only? | Sync for now (all current operations are fast or spawn child processes). Async can be added to `NetworkEnv` later if batch operations need it. |
| Q4 | The `copy-feed-files` binary currently uses `gh api` for GitHub calls but `http.Get` for raw downloads. Should both routes go through `NetworkEnv`? | No. `download_file` belongs on `GitHubEnv`, not `NetworkEnv`. Raw GitHub download URLs require the same authentication and may be proxied through the `gh` CLI in restricted environments; splitting them across two traits would force callers to fake two different mechanisms for what is logically one GitHub operation. `NetworkEnv` is reserved for generic HTTP (i.e. the AI scanner's external endpoint). |
| Q5 | `bump-npm-version` prints tab-separated output consumed by shell `read`. Should the output format change? | No. The format must remain identical to avoid breaking `npm-publish.yaml`. |
| Q6 | `scaffold_repo` generates `package.json` via `json.MarshalIndent` with `""` as indent (no indent), producing compact JSON. Should the Rust version match exactly? | Yes — produce identical bytes using `serde_json::to_string` (compact). |
