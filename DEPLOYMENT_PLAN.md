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

1. **Template variables** — today `splat.sh` does a raw `cp`. Introduce minimal
   templating (repo name, publish flags, language detection) so a single template
   set serves Rust-only, JS-only, and mixed repos. Use `scaffold_repo`'s existing
   language detection logic.

2. **Conflict detection** — before overwriting a repo's `.github/workflows/`, diff
   against the template. If the repo has diverged (custom jobs, secrets), emit a
   warning and open a PR instead of force-pushing. AI-assisted conflict resolution
   can generate a merge suggestion (see §6).

3. **New workflow: `deploy-scripts.yaml`** — add to this repo's `actions/`:
   ```yaml
   on:
     push:
       branches: [main]
       paths: ["actions/**", "lint/**"]
     workflow_dispatch:
   ```
   Calls `go run ./tools/deploy` (new tool, see below) against each managed repo.

---

## 3. AI Prompts via Rice

### Problem

The `prompts/` submodule (portal-co/prompts) holds the canonical agent docs.
Target repos need their `AGENTS.md` (or equivalent) to stay in sync without
manual updates.

### Solution: rice splice markers in AGENTS.md

Rice's `@path` syntax lets a file "pull" content from another file or URL.
After adoption, a target repo's `AGENTS.md` would look like:

```markdown
<!-- @https://raw.githubusercontent.com/portal-co/prompts/main/main.agents_.md -->
<!-- [[begin ...]] -->
... spliced content ...
<!-- [[end]] -->
```

Or, using rice's zip resolver, against an `agents.zip` bundle built by `agents_zip`.

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

The Go tools (`inject_key`, `check_ai_key`, `scaffold_repo`, `agents_zip`,
`forfiles`, `copy-feed-files`) are stable and correct. The Rust rewrite should:

1. Target `crates/` as the workspace, one crate per tool.
2. Reuse `pkg/keyguard` and `pkg/aiscan` semantics — translate the Go packages
   into a `keyguard` Rust crate with the same API surface.
3. The rice library (already Rust) can be a direct dependency for the new
   agent-doc management crates.
4. Publish as binary crates so managed repos can `cargo install` them without
   this repo being a Go dependency.
5. Gate the switchover with a `scripts-version` field in `.portal-config.yaml`:
   `v1` = Go, `v2` = Rust. Deploy workflow picks binaries accordingly.

---

## Open Questions (for human review)

- **Submodule vs. zip for prompts** — submodule is simpler for active dev; zip
  is safer for stable downstream consumers. Pick a default or let `.portal-config.yaml`
  choose.
- **Push-based vs. pull-based deploy** — feed files (pull) are lower-blast-radius;
  a deploy workflow that writes to target repos (push) is faster but needs write
  tokens. Recommend pull for the general case, push opt-in for repos with
  `auto-update: true`.
- **AI scan backend for `aiscan`** — today the default is the heuristic scanner.
  An HTTP backend (`AI_SCAN_ENDPOINT`) is available but requires a service.
  Is there a canonical endpoint to configure in CI secrets?
- **Key rotation schedule** — `rotate_key.yaml` has a commented-out cron.
  Suggest enabling `0 3 * * 1` (weekly) for most repos, per-task for high-frequency
  agent repos.
- **License for rice as a deploy dependency** — rice is MPL-2.0; confirm
  this is acceptable for all managed repos.
