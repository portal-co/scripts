# scripts

A collection of tools and GitHub Actions workflows for managing the `portal-co` organization's repositories. The project is a mixed Go/Rust workspace; both languages implement roughly the same set of tools (the Rust crates appear to be rewrites of the original Go tools).

## What this repo contains

### AI submission key system

The main distinct feature of this repo is a system for verifying that code submitted to CI was actually written with awareness of the current task context, rather than being blindly copied from training data or a previous session.

The mechanism works as follows:

1. Before starting an agent session, `inject-key` (or `inject_key`) writes a random `AIKEY-<base32>` token into `key.agents_.md` in the target repo. The token is 128 bits of cryptographic randomness.
2. The agent is expected to read that file and embed the token as a comment in every file it creates or modifies.
3. On every push and pull request, `check-ai-key` (or `check_ai_key`) runs in CI. It reads the key that was current at the base commit, checks each changed file for the token, and for any file missing the token, runs an AI-content scan. If the scan flags a file as likely AI-generated, the check fails.
4. The key can be rotated via `inject-key --rotate`, which invalidates any stale key from training data or prior sessions.

The AI content scanner (`aiscan`) has three backends:
- `heuristic` (default): a local, offline heuristic based on phrase density, hedge phrase density, and markdown structural density. Threshold is 0.5 on a sigmoid-normalised score.
- `http`: delegates to an external HTTP endpoint (`AI_SCAN_ENDPOINT`).
- `none`: disables AI scanning while still enforcing key presence.

### Tools (Go implementations in `tools/`, `pkg/`)

| Tool | Location | Purpose |
|---|---|---|
| `check_ai_key` | `tools/check_ai_key/main.go` | CI entrypoint: verifies changed files contain the AI submission key or are not AI-generated |
| `inject_key` | `tools/inject_key/main.go` | Writes or rotates the AI submission key in `key.agents_.md` |
| `bump_npm_version` | `tools/bump_npm_version/main.go` | Walks a repo's `package.json` files and bumps semver versions; outputs machine-readable name@version lines for CI tagging |
| `agents_zip` | `tools/agents_zip/main.go` | Assembles a zip archive of all `*agents_.md` files from listed GitHub repos or local paths |
| `scaffold_repo` | `tools/scaffold_repo/main.go` | Scaffolds a new Cargo + npm workspace repo with `Cargo.toml`, `package.json`, `.gitignore`, and optionally runs `git init` |
| `copy-feed-files` | `copy-feed-files.go` | Scans all repos in the current GitHub org for `*.{current-repo}.feed-out.md` files and copies them locally as `*.feed-in.md` |
| `forfiles` | `tools/forfiles/forfiles.go` | Reads lines from stdin and runs a command per line in parallel (used by shell scripts below) |

### Rust crates (in `crates/`)

The Rust workspace (`Cargo.toml`) declares Rust rewrites of most of the Go tools. The `src/main.rs` files are not yet present (the `crates/` directories contain only `Cargo.toml` files), so these are stubs/placeholders for work in progress.

| Crate | Binary name | Description |
|---|---|---|
| `agents-zip` | `agents-zip` | Zip archive of `agents_.md` files |
| `bump-npm-version` | `bump-npm-version` | Semver bump for npm packages |
| `inject-key` | `inject-key` | Upsert/rotate AI submission key |
| `check-ai-key` | `check-ai-key` | AI key CI check (referenced in `ai_key_check.yaml`) |
| `scaffold-repo` | `scaffold-repo` | Scaffold a new Cargo + npm workspace |
| `copy-feed-files` | `copy-feed-files` | Copy feed files across org repos |
| `forfiles` | `forfiles` | Parallel per-line command runner |
| `gen-agent-sandbox` | `gen-agent-sandbox` | Generate Pi + Claude Code sandbox extensions from YAML ([crate README](crates/gen-agent-sandbox/README.md)) |
| `repoutils` | (library) | Shared repo utility helpers |
| `pkgjson` | (library) | `package.json` reading and version bumping |

Note: the GitHub Actions workflows reference the Rust binaries (`cargo install --git ...`), not the Go tools. The Go code is the reference implementation; the Rust crates are what actually get installed in CI.

### GitHub Actions workflows (in `actions/`)

These are template workflows intended to be copied into other repos via `splat.sh`.

| File | Name | Trigger | Purpose |
|---|---|---|---|
| `ai_key_check.yaml` | AI Key Check | push, pull_request | Installs and runs `check-ai-key` via `cargo install` |
| `rotate_key.yaml` | Rotate AI Submission Key | workflow_dispatch, workflow_call | Installs `inject-key`, rotates the key, commits and pushes |
| `npm-publish.yaml` | npm Publish | push to main, workflow_call | Bumps versions with `bump-npm-version`, commits, tags, and publishes to npm |
| `rust-publish.yaml` | Release-plz | push to main | Uses `release-plz` to cut Rust crate releases and open release PRs |
| `lint.yaml` | Lint | push, pull_request | Runs `super-linter` and optionally auto-fixes issues on PRs |
| `test.yaml` | Test | push, pull_request | Runs `cargo test`, `npm test`, and Gradle tests if the relevant config files exist |

All publish and release jobs are gated to `github.repository_owner == 'portal-co'`.

### Shell scripts (in `tools/git/`)

Small scripts for operating on multiple repos at once. Most pipe [`listrepos`](tools/listrepos) into `forfiles` for discovery. Suffix `N` on script names means **max depth N** (repos at depths 1..N), not exact depth only.

| Script | Purpose |
|---|---|
| `pullall.sh` | `git pull --no-rebase` on **every `git remote`** in each repo (`listrepos --max-depth 1 --git`) |
| `pushall.sh` | `git push` to **every `git remote`** in each repo |
| `commitall.sh` | `git add -A && git commit -m update` in each git repo |
| `commitandpushall.sh` | Commit, then push **all remotes** in each repo |
| `updateallcargo.sh` | `cargo update` then commit; only clean git repos with `Cargo.toml` |
| `updateallcargoplus.sh` | Variant of the above |
| `fmtallcargo.sh` | `cargo fmt` then commit; same discovery filters as `updateallcargo.sh` |
| `upgradeallcargo.sh` | `cargo upgrade` (cargo-edit) then commit; same discovery filters |
| `sortallignores.sh` | Sorts `.gitignore` files across all subdirectories |
| `addignores.sh` | Adds `.gitignore` entries across repos |
| `codeall.sh` | Opens all subdirectories in VS Code |
| `splatall.sh` | Runs `splat.sh` across all subdirectories |
| `ocrall.sh` | Runs `ocrmypdf --skip-text` on all PDFs in the current directory |
| `pull.sh` | Clones or pulls a single `org/repo` |

### Top-level shell scripts

- **`splat.sh`**: Copies the `actions/` and `lint/` directories into a target repo's `.github/` directory, optionally installs `rolldown zshy typescript` dev dependencies, then commits and pushes via `update_commit.sh`. Used to deploy the standard workflow set to other repos.
- **`update_commit.sh`**: `git add -A && git commit -m "Update" && git push` for a given directory.

### Other

- **`prompts/`**: Git submodule pointing to `https://github.com/portal-co/prompts`. Not used directly by any script in this repo.
- **`lint/super-linter.env`**: Configuration for `super-linter`, referenced by `lint.yaml`.
- **`TESTING.md`**: Notes on how to write and run tests; currently mostly placeholder content.

## Current state

- The Go tools are the working implementations. They are tested and have meaningful logic.
- The Rust crates are declared in `Cargo.toml` but their `src/main.rs` files are absent — the crates are stubs. The CI actions install the Rust binaries via `cargo install --git`, which means this repo's main branch must have working Rust source for those actions to function in consuming repos.
- The `lint.yaml` and `test.yaml` workflows have a structural bug: `runs-on` is missing from their `jobs.<name>` blocks, so they would fail to parse as valid GitHub Actions.
- There is no automated test suite for the scripts themselves in this repo. The `scripts/harness/README.md` is a placeholder.

## Usage

### Deploying standard workflows to another repo

```sh
./splat.sh /path/to/target/repo
```

This overwrites `.github/workflows/` in the target with the contents of `actions/`, copies the linter config, and pushes.

### Injecting an AI submission key

```sh
# Go version
go run ./tools/inject_key [-repo <path>] [-rotate] [-dry-run]

# Rust version (once crate is implemented)
inject-key [--repo <path>] [--rotate] [--dry-run]
```

### Bumping npm versions

```sh
go run ./tools/bump_npm_version [-repo <path>] [-bump patch|minor|major] [-add-missing] [-dry-run]
```

### Building an agents zip

```sh
go run ./tools/agents_zip [-o output.zip] <org/repo1> <org/repo2> ...
```

## Language and dependencies

- **Go**: standard library only for most tools; no module dependencies beyond the internal `pkg/` packages.
- **Rust**: `tokio`, `clap`, `serde`/`serde_json`, `zip`, `anyhow`, `reqwest`, `rand`, `base32`, `walkdir`. External crates: `env-traits`/`env-fake`/`env-real` and `aiscan`/`keyguard` from `portal-co` GitHub repos.
