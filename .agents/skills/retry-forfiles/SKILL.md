---
name: retry-forfiles
description: >
  Like forfiles, but retries failed commands up to a per-invocation attempt limit.
  Use when running parallel commands against flaky external services (git push,
  npm publish, API calls) where transient failures are expected.
  Trigger on: "retry forfiles", "retry parallel", "forfiles retry", "retry on failure".
---

# retry-forfiles

Like `forfiles`, but retries individual commands that fail until they succeed or
a per-invocation attempt limit is reached. Useful for operations against flaky
external services where transient failures are expected.

## Installation

```bash
cargo build -p retry-forfiles
# or: cargo install --path crates/retry-forfiles
./retry-forfiles [OPTIONS] <placeholder> <command> [args...]
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--attempts <n>` | 3 | Maximum attempts per input line before giving up |
| `--delay <ms>` | 1000 | Milliseconds to wait between retries |
| `--backoff` | false | Exponential backoff (doubles delay on each retry) |

## Arguments

Same as `forfiles`: a placeholder token, a command, and optional args. Every
occurrence of the placeholder in the command or its args is replaced with the
input line.

## Usage

### Push to every repo with retry

```bash
cat repos.txt | ./retry-forfiles --attempts 5 {} git -C {} push
```

### npm publish with backoff

```bash
cat packages.txt | ./retry-forfiles --attempts 3 --backoff {} npm publish {}
```

### API calls that rate-limit

```bash
cat ids.txt | ./retry-forfiles --attempts 4 --delay 2000 {} curl -s "https://api.example.com/items/{}"
```

## Behavior

- All input lines start concurrently (same as `forfiles`)
- A failed command is retried up to `--attempts` times before marking it failed
- Retry waits `--delay` ms between attempts (or exponential if `--backoff`)
- Exit code is 0 only if every command eventually succeeds
- Logs each retry attempt to stderr with the input line and attempt number

## When to use vs forfiles

Use `retry-forfiles` when:
- Commands hit external network services (git, npm, HTTP APIs)
- You expect occasional transient failures (rate limits, timeouts, flaky CI)

Use plain `forfiles` when:
- Commands are local and deterministic
- A failure should fail fast without wasting time retrying
