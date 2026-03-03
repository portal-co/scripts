# Multitool Sketch: `portal-scripts`

This document sketches the design of the unified `portal-scripts` CLI — the
single entry point for deploying, scaffolding, and maintaining repos managed
by portal-co/scripts. It will live at `crates/portal-scripts/` in the Rust
workspace.

---

## Subcommands

```
portal-scripts <SUBCOMMAND>

Subcommands:
  deploy        Deploy (or update) scripts components to a target repo
  scaffold      Create a new repo with full portal-co scaffolding
  rotate        Rotate the AI submission key in the current repo
  upgrade       Detect and fix conflicts in an existing repo
  fetch-feeds   Fetch feed-in files from the org (copy-feed-files)
  agents-build  Build agents.zip from the local prompts/ submodule
```

---

## `portal-scripts deploy`

```
portal-scripts deploy [OPTIONS] <TARGET>

Arguments:
  <TARGET>  Path to the target repo (or '.' for current repo)

Options:
  --components <LIST>   Comma-separated components to deploy.
                        Default: read from TARGET/.portal-config.yaml,
                        or 'ai-key,prompts,ci-workflows' if absent.
                        Valid: ai-key, prompts, ci-workflows,
                               publish-rust, publish-npm, scaffold
  --push                Push a commit after deploying (default for initial
                        deploy; omit for dry-run)
  --agents-zip <PATH>   Path to pre-built agents.zip. If absent and
                        'prompts' component is requested, builds from
                        the local prompts/ submodule automatically.
  --dry-run             Print what would change; do not write files
```

### What it does (per component)

**`ai-key`**
1. Copy `actions/ai_key_check.yaml` → `TARGET/.github/workflows/`
2. Copy `actions/rotate_key.yaml` → `TARGET/.github/workflows/`
3. If `TARGET/key.agents_.md` is absent, run `inject-key --repo TARGET`
   to generate a fresh key.
4. Enable weekly key rotation cron in `rotate_key.yaml`
   (`# schedule:` lines uncommented).

**`prompts`**
1. Ensure `TARGET/AGENTS.md` exists (create from template if absent).
2. Add/update rice splice markers for each `agents_.md` from the zip:
   ```markdown
   <!-- @main.agents_.md -->
   <!-- [[begin main.agents_.md]] -->
   ... spliced content from zip ...
   <!-- [[end]] -->
   ```
3. Run `rice TARGET/AGENTS.md` (using ZipResolver backed by agents.zip)
   to materialise content.

**`ci-workflows`**
1. Copy `actions/lint.yaml` and `actions/test.yaml` into target.
2. Detect existing conflicting workflows:
   - If identical to template: overwrite silently.
   - If diverged: write to a `*.portal-pending.yaml` alongside and
     emit a warning asking the human to merge.

**`publish-rust`** / **`publish-npm`**
1. Copy the relevant publish workflow.
2. Same conflict policy as `ci-workflows`.

**`scaffold`**
1. Call `scaffold-repo --update` logic: create `crates/`, `packages/`,
   `Cargo.toml`, `package.json`, `.gitignore` if absent.

---

## `portal-scripts scaffold`

Thin wrapper around `crates/scaffold-repo` with deploy bundled:

```
portal-scripts scaffold [OPTIONS]

Options:
  --name        <NAME>   Repository name (required)
  --out         <DIR>    Parent directory (required)
  --description <DESC>   Workspace description (required)
  --license     <SPDX>   License string for Cargo.toml
  --private              Set publish = false
  --no-deploy            Skip running deploy after scaffold (default: deploy)
  --components  <LIST>   Components to deploy after scaffold
                         (default: ai-key,prompts,ci-workflows)
```

After scaffold:
1. Creates the directory structure (existing `scaffold-repo` logic).
2. Calls `portal-scripts deploy . --components <LIST> --push`.

---

## `portal-scripts rotate`

```
portal-scripts rotate [OPTIONS]

Options:
  --repo <PATH>   Repo to rotate key in (default: current repo)
  --dry-run       Print new key without writing
```

Thin wrapper around `inject-key --rotate`. Intended for use when a human
wants to trigger an out-of-schedule rotation (e.g., before a new agent task).

---

## `portal-scripts upgrade`

```
portal-scripts upgrade [OPTIONS] [TARGET]

Arguments:
  [TARGET]  Repo to upgrade (default: current repo)

Options:
  --dry-run         Report conflicts without fixing
  --ai-pr           Open a PR via a TASK agent for conflicts requiring
                    judgment (legacy agent doc reformat, CI job merges)
  --components <L>  Limit check to specific components
```

### Conflict detection table

| Check | How detected | Auto-fix? |
|---|---|---|
| Missing `key.agents_.md` | file absent | Yes — inject fresh key |
| Stale key (predates last rotation) | key matches no recent rotation | Yes — rotate |
| Missing `ai_key_check.yaml` | workflow absent | Yes — copy template |
| CI workflow diverged | diff against template | No — `.portal-pending.yaml` + warning |
| Missing `AGENTS.md` / rice markers | file absent or no `[[begin` | Yes — add markers, splice |
| Stale spliced content | hash mismatch vs zip | Yes — re-splice |
| Missing scaffold structure | `crates/` or `packages/` absent | Yes — `scaffold-repo --update` |
| `.portal-config.yaml` absent | file absent | Yes — write default |
| Legacy agent doc format | no `agents_.md` suffix convention | `--ai-pr` only |

### AI-PR path (`--ai-pr`)

For conflicts that require judgment, `upgrade --ai-pr`:
1. Creates a feed-out file: `upgrade/{repo}.{target-repo}.feed-out.md`
   containing the conflict description and template diff.
2. The target repo's CI picks it up via `fetch-feeds` and dispatches a
   TASK agent to file a PR resolving the conflict.
3. The TASK agent's PR must embed the current AI key (enforced by
   `ai_key_check.yaml`) — proving the agent read live context before
   submitting.

### Batch upgrade (org-wide)

```bash
gh repo list portal-co --json name --jq '.[].name' \
  | cargo run -p forfiles -- '^' \
    portal-scripts upgrade /path/to/portal-hot/^
```

---

## `portal-scripts fetch-feeds`

```
portal-scripts fetch-feeds [OPTIONS]

Options:
  --repo <PATH>   Repo to fetch feeds into (default: current repo)
```

Direct wrapper around `copy-feed-files`. Included here so users have one
tool to learn rather than knowing about `copy-feed-files` separately.

---

## `portal-scripts agents-build`

```
portal-scripts agents-build [OPTIONS]

Options:
  --prompts <PATH>   Path to prompts submodule (default: ./prompts)
  --out     <PATH>   Output zip path (default: ./agents.zip)
  --repos   <LIST>   Additional org/repo specs to include alongside prompts
```

Calls `agents-zip` with the local `prompts/` path. Produces `agents.zip`
used by `deploy --components prompts` and the CI `deploy-scripts.yaml`
workflow.

---

## Crate design

```
crates/portal-scripts/
├── Cargo.toml
└── src/
    ├── main.rs          ← clap App, dispatch to subcommands
    ├── deploy.rs        ← deploy subcommand + per-component logic
    ├── scaffold.rs      ← thin wrapper around scaffold-repo crate
    ├── rotate.rs        ← thin wrapper around inject-key crate
    ├── upgrade.rs       ← conflict detection + remediation
    ├── fetch_feeds.rs   ← thin wrapper around copy-feed-files crate
    └── agents_build.rs  ← thin wrapper around agents-zip crate
```

### Dependencies

```toml
[dependencies]
# workspace env infrastructure
env-traits  = { workspace = true }
env-real    = { workspace = true }
# tool libraries (call as library functions, not subprocess)
keyguard    = { workspace = true }
repoutils   = { workspace = true }
# rice for AGENTS.md splicing (installed via cargo install --git at deploy time)
# note: rice is NOT a compile-time dep of portal-scripts; it is invoked as a
# subprocess after `cargo install --git https://github.com/portal-co/rice.git rice`
# CLI
clap        = { workspace = true }
anyhow      = { workspace = true }
# zip for agents.zip
zip         = { workspace = true }
```

The key design point: **subcommands call library functions from sibling
crates directly** rather than spawning subprocesses. This keeps error
propagation clean, avoids path discovery, and makes integration tests
straightforward with fake environments.

---

## Relationship to existing tools

| Old invocation | New invocation |
|---|---|
| `go run ./tools/inject_key -rotate` | `portal-scripts rotate` |
| `go run ./tools/scaffold_repo -name foo -out ..` | `portal-scripts scaffold --name foo --out ..` |
| `go run copy-feed-files.go` | `portal-scripts fetch-feeds` |
| `sh splat.sh <dir>` | `portal-scripts deploy <dir>` |
| `go run ./tools/agents_zip -o agents.zip portal-co/prompts` | `portal-scripts agents-build` |

The individual Rust binaries (`inject-key`, `scaffold-repo`, etc.) remain
available as standalone tools for scripting and CI use. `portal-scripts` is
the human-facing convenience layer on top.

---

## CI integration

### New `actions/deploy-scripts.yaml` (in this repo)

```yaml
name: Deploy scripts to managed repos

on:
  push:
    branches: [main]
    paths:
      - actions/**
      - lint/**
      - prompts/**      # submodule pointer change
  workflow_dispatch:

permissions:
  contents: write

jobs:
  build-agents-zip:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
        with:
          submodules: true
      - uses: dtolnay/rust-toolchain@stable
      - name: Install portal-scripts
        run: cargo install --git https://github.com/portal-co/scripts.git portal-scripts
      - run: portal-scripts agents-build --out agents.zip
      - uses: actions/upload-artifact@v4
        with:
          name: agents-zip
          path: agents.zip

  deploy:
    needs: build-agents-zip
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: actions/download-artifact@v4
        with:
          name: agents-zip
      - uses: dtolnay/rust-toolchain@stable
      - name: Install portal-scripts and forfiles
        run: |
          cargo install --git https://github.com/portal-co/scripts.git portal-scripts
          cargo install --git https://github.com/portal-co/scripts.git forfiles
      - name: Deploy to managed repos
        env:
          GH_TOKEN: ${{ secrets.DEPLOY_TOKEN }}
        run: |
          gh repo list portal-co --json name --jq '.[].name' \
            | forfiles '^' \
              portal-scripts deploy \
                /tmp/repos/^ \
                --agents-zip agents.zip \
                --push
```

---

## `.portal-config.yaml` format

```yaml
scripts-managed: true
scripts-version: v2        # v1=Go (legacy), v2=Rust (default)
auto-update: pull          # push (initial) | pull (feed-file) | false

components:
  - ai-key
  - prompts
  - ci-workflows
  # - publish-rust
  # - publish-npm

key-rotation: weekly       # weekly | per-task | manual
```

---

## TODO: AI scan service

The `aiscan` HTTP backend (`AI_SCAN_BACKEND=http`) requires a remote endpoint.
Provisioning this service is deferred. When it exists:

1. The service repo should accept a feed-in file describing the hosting request.
2. `portal-scripts deploy` can set `AI_SCAN_ENDPOINT` as a repo/org secret
   when the `ai-key` component is deployed.
3. Until then, `AI_SCAN_BACKEND` is left unset (heuristic) or `none`.
