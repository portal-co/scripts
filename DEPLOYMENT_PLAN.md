# Scripts Deployment Plan

This document covers how the tooling in this repository gets deployed to the wider
portal-co ecosystem: CI workflows, AI prompts, the key system, and how older repos
can be brought into conformance, potentially with AI assistance.

---

## Current State

| Component | Location | Language | Status |
|---|---|---|---|
| Key rotation / injection | `tools/inject_key` | Go (binary) | Stable |
| Key CI enforcement | `tools/check_ai_key` | Go (binary) | Stable |
| Key support libraries | `pkg/keyguard`, `pkg/aiscan` | Go | Stable |
| CI workflow templates | `actions/` | YAML | Stable |
| Repo scaffolding | `tools/scaffold_repo` | Go | Stable |
| Agent-doc bundler | `tools/agents_zip` | Go | Stable |
| Bulk git utilities | `tools/git/*.sh` | Shell | Stable |
| Feed-file orchestration | `copy-feed-files.go` | Go | Stable |
| Manual splat deployer | `splat.sh` | Shell | Stable (hacky) |
| AI prompt submodule | `prompts/` → portal-co/prompts | Git submodule | Active |
| Rice (file splicer) | portal-co/rice | Rust | Active, parallel |
| Rust rewrite (tools) | `crates/` (empty) | Rust | Not yet started |

---

## Goals

1. **Self-auto-updating** — repos that adopt these scripts can receive updates
   without manual intervention when this repo changes.
2. **CI scripts** — CI workflow templates are deployed and kept in sync across
   all adopting repos.
3. **AI prompts via rice** — the `prompts/` submodule content is distributed into
   adopting repos via rice splice markers, so agent docs update automatically
   when the submodule advances.
4. **Ease of usage** — a single entry point to deploy or upgrade any target repo.
5. **Composability** — repos can opt into subsets (key system only, full prompts,
   specific CI workflows) rather than being all-or-nothing.
6. **Upgrading older repos** — detection and remediation of conflicts in repos that
   predate these standards, with an AI-assisted PR path for the long tail.

---

## 1. Self-Auto-Updating

### Mechanism

The existing `splat.sh` copies CI workflows and lint config manually. Replace/augment
this with a **`deploy` workflow** in this repo that:

1. Triggers on pushes to `main` (or manually via `workflow_dispatch`).
2. Fetches all org repos (via `repoutils.GetOrgRepos`).
3. For each repo that has opted in (marker file `scripts-managed: true` in a
   `.portal-config.yaml`, or presence of `key.agents_.md`), opens a PR with
   updated CI workflows and rotated key.

The feed-file mechanism (`copy-feed-files.go`) is already designed for this:
the scripts repo can write `.{target-repo}.feed-out.md` files signaling an
available update; target repos' CI picks them up and applies changes.

### Opt-in marker

Add a minimal `.portal-config.yaml` to repos at adoption time:

```yaml
scripts-managed: true
components:
  - ai-key        # rotating key + CI enforcement
  - prompts       # agent docs via rice
  - ci-workflows  # lint, test, publish workflows
```

This lets repos opt into a subset (see Composability, §5).

### Rust rewrite consideration

When the Go tools are rewritten in Rust, the deploy mechanism should not change —
only the binaries called inside it change. Version the tooling via a `scripts-version`
field in `.portal-config.yaml` so repos can pin to stable Go or migrate to Rust
incrementally.

---

## 2. CI Scripts

### What gets deployed

`actions/` currently contains:
- `ai_key_check.yaml` — key enforcement on every push/PR
- `rotate_key.yaml` — manual or scheduled key rotation
- `lint.yaml` — super-linter wrapper
- `test.yaml` — run tests
- `npm-publish.yaml` / `rust-publish.yaml` — publish workflows

### Deployment approach

1. **`cargo install --git`** — all actions install binaries directly from the
   scripts repo at runtime:
   ```yaml
   - uses: dtolnay/rust-toolchain@stable
   - run: cargo install --git https://github.com/portal-co/scripts.git check-ai-key
   - run: check-ai-key
   ```
   This eliminates the relative-path problem entirely: managed repos carry no
   scripts source code and no `./target/release/` references. The binary is on
   `PATH` after `cargo install` and runs from any working directory.

2. **Template variables** — today `splat.sh` does a raw `cp`. Introduce minimal
   templating (repo name, publish flags, language detection) so a single template
   set serves Rust-only, JS-only, and mixed repos. Use `scaffold_repo`'s existing
   language detection logic.

3. **Conflict detection** — before overwriting a repo's `.github/workflows/`, diff
   against the template. If the repo has diverged (custom jobs, secrets), emit a
   warning and open a PR instead of force-pushing. AI-assisted conflict resolution
   can generate a merge suggestion (see §6).

4. **New workflow: `deploy-scripts.yaml`** — add to this repo's `actions/`:
   ```yaml
   on:
     push:
       branches: [main]
       paths: ["actions/**", "lint/**"]
     workflow_dispatch:
   ```
   Calls `portal-scripts deploy` (via `cargo install --git`) against each managed repo.

---

## 3. AI Prompts via Rice

### Problem

The `prompts/` submodule (portal-co/prompts) holds the canonical agent docs.
Target repos need their `AGENTS.md` (or equivalent) to stay in sync without
manual updates.

### Solution: zip from submodule + rice splice markers in AGENTS.md

The `prompts/` submodule stays in this repo as the canonical source. At deploy
time (and on `main` pushes), a CI step runs:

```bash
cargo run -p agents-zip -- -o agents.zip portal-co/prompts
# or, once agents-build exists:
portal-scripts agents-build -o agents.zip
```

This produces a versioned `agents.zip` artifact. Target repos receive the zip
and use rice's `ZipResolver` to splice content into their `AGENTS.md`.

When the prompts submodule updates:
1. This repo's CI rebuilds `agents.zip` via `agents_zip portal-co/prompts`.
2. The deploy workflow pushes the new zip or updated URLs to managed repos.
3. Each managed repo's CI runs `rice AGENTS.md` to refresh spliced content,
   commits the result, and pushes.

### Key inclusion

The key system is already part of the prompts submodule (`key.agents_.md` and
the `ai_key.agents_.md` context document added earlier). Rice ensures every
managed repo's AGENTS.md always reflects the current key instructions without
a separate sync step.

### New CI step in managed repos

```yaml
- name: Refresh agent docs
  run: |
    cargo install --git https://github.com/portal-co/rice.git rice --features cli
    rice AGENTS.md
    git diff --quiet || git commit -am "chore: refresh agent docs from upstream"
```

---

## 4. Ease of Usage

### Single entry point: `portal-scripts` CLI

Today there are multiple disconnected tools (`splat.sh`, `scaffold_repo`,
`inject_key`, `copy-feed-files.go`). Unify under one CLI with subcommands:

```
portal-scripts deploy   <repo>   # deploy/update CI + prompts to a target repo
portal-scripts scaffold <name>   # create a new repo with full setup
portal-scripts rotate            # rotate AI key in the current repo
portal-scripts upgrade           # detect and fix conflicts in current repo
portal-scripts fetch-feeds       # run copy-feed-files for current repo
```

Initially this can be a thin shell wrapper; the Rust rewrite is the natural
home for a proper CLI (using `clap`, which rice already depends on).

### Zero-dependency bootstrap

For repos that don't have Go or Rust installed, provide a GitHub Action that
invokes tools directly from this repo's release artifacts:

```yaml
- uses: portal-co/scripts/actions/deploy@main
  with:
    components: ai-key,prompts
```

---

## 5. Composability

### Component model

| Component | What it installs | Dependencies |
|---|---|---|
| `ai-key` | `key.agents_.md`, `ai_key_check.yaml`, `rotate_key.yaml` | none |
| `prompts` | `AGENTS.md` rice markers, submodule or zip | `ai-key` (key instructions) |
| `ci-workflows` | `lint.yaml`, `test.yaml` | none |
| `publish-rust` | `rust-publish.yaml` | `ci-workflows` |
| `publish-npm` | `npm-publish.yaml` | `ci-workflows` |
| `scaffold` | `Cargo.toml`, `package.json`, `.gitignore`, `crates/`, `packages/` | none |

The `.portal-config.yaml` `components:` list selects exactly which components
are applied. The deploy tool reads this and applies only the relevant pieces.

### Submodule vs. zip vs. inline

Repos can choose how they consume prompts:
- **Submodule** (current) — tightest coupling, automatic updates on `git submodule update`
- **Zip** — `agents_zip` output, pinned at a commit, updated by deploy workflow
- **Inline rice** — content spliced directly into `AGENTS.md`, updated by `rice` CLI

---

## 6. Upgrading Older Repos

### Conflict categories

When running `portal-scripts upgrade` on an existing repo, check for:

| Conflict | Detection | Remediation |
|---|---|---|
| Missing `key.agents_.md` | file absent | inject fresh key |
| Stale AI key | key predates last rotation | rotate |
| Missing `ai_key_check.yaml` | workflow absent | add |
| Conflicting CI workflow | workflow exists but differs from template | diff + PR |
| Missing `prompts/` submodule | no `AGENTS.md` or no rice markers | add submodule + markers |
| Wrong scaffold structure (no `crates/`, `packages/`) | dirs absent | `scaffold_repo -update` |
| Legacy agent docs format | old-style comments, no `agents_.md` structure | AI-assisted reformat |

### AI-assisted upgrade path

For conflicts that require judgment (rewriting legacy agent docs, merging
conflicting CI jobs), the upgrade tool can:

1. Use the feed-file system to dispatch a TASK agent to the target repo.
2. The agent reads the conflict, reads the template, and generates a PR with
   a migration.
3. The PR includes the current AI key, so the `ai_key_check` CI confirms the
   agent engaged with the live context before submitting.

This closes the loop: the same key system that enforces agent quality is used
to upgrade repos into having the key system.

### Batch upgrade

For the org-wide case (hundreds of repos), use `forfiles` to parallelize:

```bash
gh repo list portal-co --json name --jq '.[].name' \
  | go run ./tools/forfiles/forfiles.go '^' \
    portal-scripts upgrade /path/to/portal-hot/^
```

Repos with no conflicts complete silently; repos with conflicts emit a summary
and optionally open PRs.

---

## 7. Rust Rewrite Coordination

**PR #1 (`feat/rust`) is open and nearly complete** — all 14 crates are
implemented and 60 tests pass. The rewrite plan lives at
`RUST_REWRITE_PLAN.md` in the `portal-cold/scripts` working copy.

Summary of what is in the PR:
- Infrastructure: `env-traits`, `env-fake`, `env-real`
- Libraries: `aiscan`, `keyguard`, `pkgjson`, `repoutils`
- Binaries: `check-ai-key`, `inject-key`, `bump-npm-version`, `forfiles`,
  `agents-zip`, `scaffold-repo`, `copy-feed-files`
- CI workflows updated to use Rust binaries (Phases 5, 7 complete)
- Shell wrappers updated to drop `go run` (Phase 9 complete)
- Go sources still present pending Phase 10 (delete after CI verified on `main`)

### Components needed for deployment but not yet in PR #1

| Crate | Purpose | Notes |
|---|---|---|
| `crates/portal-scripts` | The unified multitool CLI (`portal-scripts deploy/scaffold/rotate/upgrade/fetch-feeds`) | Design in `MULTITOOL_SKETCH.md` |
| `crates/agents-build` | Build `agents.zip` from local `prompts/` submodule (thin wrapper around `agents-zip`) | Or add `--local` flag to `agents-zip` |
| `crates/deploy` | Library for push-deploying CI workflows + key + agents zip to a target repo | Used by `portal-scripts deploy` and the `deploy-scripts.yaml` CI workflow |
| `crates/upgrade` | Conflict detection + remediation for older repos | Used by `portal-scripts upgrade` |

### Versioning / switchover

Gate Go → Rust switchover with `scripts-version` in `.portal-config.yaml`.
The deploy tool reads this and invokes the appropriate binary path. Default
is `v2` (Rust) for newly scaffolded repos.

See **`MULTITOOL_SKETCH.md`** for the unified CLI design.

---

## Open Questions — Resolved

| # | Question | Decision |
|---|---|---|
| 1 | Submodule vs. zip for prompts | **Zip built from local `prompts/` submodule.** The submodule stays in this repo for iteration and agent context; `agents-zip` produces the distributable artifact from it. Downstream repos consume the zip, not the submodule. |
| 2 | Push-based vs. pull-based deploy | **Push for initial deployment; pull thereafter.** First adoption of a repo uses `portal-scripts deploy` (push, needs write token). Ongoing updates use feed files (pull). Repos set `auto-update: pull` in `.portal-config.yaml` after initial setup. |
| 3 | AI scan backend / `aiscan` HTTP service | **No service exists yet.** The heuristic scanner remains the default. Provisioning a hosted scan endpoint is a TODO; when it exists it will be modelled as a feed document dispatched to the hosting service's repo. CI repos set `AI_SCAN_BACKEND=none` or leave it unset (heuristic) until the service is available. |
| 4 | Key rotation schedule | **Weekly cron (`0 3 * * 1`) for standard repos; per-task rotation for high-frequency agent repos.** Uncomment the cron in `rotate_key.yaml` as part of the initial deploy step. |
| 5 | MPL-2.0 license for rice | **Acceptable.** Only the compiled rice binary is invoked; the MPL source is not incorporated into managed repos and not modified, so file-level copyleft does not propagate. |
