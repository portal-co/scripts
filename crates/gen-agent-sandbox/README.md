# gen-agent-sandbox

Generates **Pi Coding Agent** TypeScript extensions and **Claude Code** hook bundles (shell shim + TypeScript stdin handler + `hooks/hooks.json`) from a single YAML policy file.

## Gate semantics

Policy is active only when `gate.env` is set in the process environment and its value **equals** `gate.value` (string comparison).

- **Pi**: the generated extension returns immediately without registering listeners when the gate is inactive.
- **Claude Code**: the generated `portal-sandbox-shim.sh` exits `0` with no stdout when the gate is inactive, so the tool call or prompt proceeds unchanged ([hook exit behavior](https://code.claude.com/docs/en/hooks#how-a-hook-resolves)).

## Usage

```bash
cargo build -p gen-agent-sandbox
./target/debug/gen-agent-sandbox \
  --config templates/agent-sandbox/portal-sandbox.example.yaml \
  --out-dir ./out/sandbox-plugin
# Optional: also write plugin.json for a Claude Code plugin directory layout
./target/debug/gen-agent-sandbox \
  --config path/to/portal-sandbox.yaml \
  --out-dir ./out/sandbox-plugin \
  --emit-plugin-json
```

Flags:

- `--pi-only` — emit only `portal-pi-sandbox.ts`.
- `--claude-only` — emit Claude hook bundle (no Pi file).
- `--emit-plugin-json` — write `plugin.json` (requires Claude outputs; incompatible with `--pi-only`).

## Installing outputs

### Pi

Copy `portal-pi-sandbox.ts` to `~/.pi/agent/extensions/` or `.pi/extensions/` in a project. Peer dependency: `@mariozechner/pi-coding-agent` (see [Pi extensions](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md)). Set the gate environment variable before starting Pi when you want enforcement.

### Claude Code

1. Copy `portal-claude-sandbox-hook.ts`, `portal-sandbox-shim.sh`, and merge `hooks/hooks.json` into your [plugin `hooks/hooks.json`](https://code.claude.com/docs/en/plugins-reference) or project [`.claude` hooks](https://code.claude.com/docs/en/hooks#hook-locations).
2. Ensure `npx` / `tsx` are available, or set `claude_ts_runner` in YAML to another argv prefix (for example `pnpm`, `exec`, `tsx`).
3. Set the same gate env var when you want the shim to invoke the TypeScript handler.

## Policy schema

See [templates/agent-sandbox/schema.md](../../templates/agent-sandbox/schema.md) and [templates/agent-sandbox/portal-sandbox.example.yaml](../../templates/agent-sandbox/portal-sandbox.example.yaml).

### Script wrapper requirement (`bash.script_wrapper`)

When `required: true`, generated hooks **deny** bash commands whose text does not start with one of the `accepted_invocations` prefixes (after `trimStart`). This avoids claiming full shell safety: compound commands, `bash -c`, `env`, etc. will **not** match a simple `./wrap.sh` prefix unless you add explicit prefixes for those forms. Pair with prompts so the model knows to call only the wrapper.

## Related design notes

- [docs/design/agent-sandbox-todo.md](../../docs/design/agent-sandbox-todo.md) — harder targets (Cursor, CI agents, etc.).
