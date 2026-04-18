# Skills TODO

## WASI binaries

Some tools are good candidates for compilation to WASI so they can be bundled
directly into the skill and run without a Rust toolchain installed. Agents
would invoke them via a WASI runtime (e.g. `wasmtime`) rather than requiring
`cargo build`.

Tools to compile to WASI and bundle into the skill package:

- [ ] `freeze` — pure computation (SHA-256 + TOML), no OS-specific exec needed for
      `record` and `check` subcommands. WASI target for those two; native for `exec`.
- [ ] `forfiles` — parallel process spawning; WASI preview2 with wasi-threads or
      keep native. Evaluate after wasi-threads stabilizes.
- [ ] `retry-forfiles` — same as forfiles.
- [ ] `gen-patch` — reads filesystem + git remote URL via subprocess; WASI with
      wasi-filesystem + command-line access. Feasible with preview2.
- [ ] `bump-npm-version` — pure file I/O + JSON/semver; straightforward WASI target.
- [ ] `agents-zip` — file I/O + zip; feasible, but GitHub API access needs HTTP
      support (wasi-http in preview2).

Tools that stay native (require exec/fork semantics not available in WASI):

- `cleanse` — closes all fds then `exec`s; fundamentally POSIX-only.
- `run` — auto-rebuild via `cargo build` + `exec`; requires native process control.
- `inject-key` — writes files and calls git; keep native.
- `scaffold-repo` — writes files and calls git init; keep native.
- `copy-feed-files` — GitHub API + file writes; could be WASI with wasi-http, but
  low priority.

## Skill packaging

Once WASI binaries are ready:
- Add `wasm/` directory inside the relevant skill folder
- Bundle `<tool>.wasm` and a `run.sh` wrapper that detects wasmtime/wasmer
- Update the skill's SKILL.md with WASI invocation instructions alongside native
