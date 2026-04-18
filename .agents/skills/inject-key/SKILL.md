---
name: inject-key
description: >
  Upsert or rotate the AI submission key written to key.agents_.md in a repo.
  The key proves that an agent session was aware of the current task context —
  CI rejects changed files that lack it. Use when starting an agent session,
  rotating a stale key, or setting up the AI key check system for a new repo.
  Trigger on: "inject key", "rotate key", "ai key", "AIKEY", "key.agents_.md".
---

# inject-key

Writes or rotates the `AIKEY-<base32>` token in `key.agents_.md`. This token is
the linchpin of the AI submission key system: agents embed it in every file they
touch, and CI (`check-ai-key`) verifies its presence on every push/PR.

## Installation

```bash
cargo build -p inject-key
# or: cargo install --path crates/inject-key
./inject-key [OPTIONS]
```

## Options

| Flag | Description |
|------|-------------|
| `--repo <path>` | Target repo root. Defaults to git root of cwd |
| `--rotate` | Generate a fresh key, replacing the existing one |
| `--dry-run` | Print what would be written without touching disk |

## Usage

### Start a new agent session

Before handing context to an agent, run:

```bash
./inject-key --repo /path/to/target-repo
```

This upserts `key.agents_.md` with the current key (or generates one if absent).
The agent reads this file and embeds the `AIKEY-<token>` string as a comment in
every file it creates or modifies.

### Rotate a stale key

After a session ends (or if training data contamination is suspected):

```bash
./inject-key --rotate --repo /path/to/target-repo
```

Rotation invalidates any prior key, so files from previous sessions are no longer
considered current-context.

### Preview without writing

```bash
./inject-key --dry-run
```

## Key format

128 bits of cryptographic randomness, base32-encoded (RFC 4648, no padding),
lowercased:

```
AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
```

## What gets written

`key.agents_.md` in the repo root contains the active key and instructions for
agents about how and where to embed it. The CI workflow reads this file at the
base commit to know which key to enforce.

## CI integration

Pair with the `ai_key_check.yaml` workflow (in `actions/`). That workflow:
1. Reads the key from `key.agents_.md` at the base commit
2. For each changed file, checks for the key as a comment
3. Falls back to AI-content scanning (`aiscan`) for files missing the key
4. Fails the check if a file appears AI-generated without the key

See also: `check-ai-key` (the CI binary), `rotate_key.yaml` (automated rotation workflow).
