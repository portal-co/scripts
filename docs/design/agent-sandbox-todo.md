# Agent sandbox backlog (beyond Pi + Claude Code)

This note lists **harder or different** agent surfaces than the Pi Coding Agent extension model and Claude Code `PreToolUse` / `UserPromptSubmit` hooks. It is **non-normative** design backlog, not a commitment to implement any row. For Cursor-specific build-environment quirks already handled in this repo, see [cursor-cargo-target-dir.md](cursor-cargo-target-dir.md).

## Comparison table

| Target | Why it is harder | Possible levers (research / future) |
| --- | --- | --- |
| **Cursor agents** | No single `PreToolUse` hook across the whole IDE; terminal sessions can inject environment (for example `CARGO_TARGET_DIR`); MCP and rules are separate trust surfaces from shell commands. | Project `.cursor/hooks`, workspace trust, rules, MCP allowlists, approved CLI patterns; for programmatic agents, Cursor SDK / cloud agent policy where applicable. |
| **Claude Code subagents / plugin agents** | Subagents can differ in tool allowlists (`disallowedTools` in agent frontmatter). Plugin agents cannot ship `hooks`, `mcpServers`, or `permissionMode` in their own definitions per [plugins reference](https://code.claude.com/docs/en/plugins-reference). | Enforce policy on the **parent** session via project or managed hooks; org-wide [managed settings](https://code.claude.com/en/settings) and `allowManagedHooksOnly` where applicable. |
| **CI / SDK agents** | No local interactive hooks; headless runs may skip some hook semantics (for example `defer` only in `-p` mode per [hooks](https://code.claude.com/docs/en/hooks)). | Containers, read-only trees, pinned `PATH`, secret scoping, separate build identities; for Claude Code SDK, same hook JSON where the runtime supports it. |
| **GitHub Copilot / other IDE agents** | Fragmented extension APIs; policy lives per vendor. | Defer until a concrete product is in scope; prefer org-wide CI gates for anything safety-critical. |
| **Aider, OpenDevin, custom CLIs** | Each has its own event or middleware model. | Wrap invocations in a known environment (container + fixed `PATH`); emit repo-local scripts rather than assuming hook compatibility. |

## Operational reminders

- **Regenerate** sandbox artifacts after upgrading Pi (`@mariozechner/pi-coding-agent`) or Claude Code when hook stdin/stdout schemas change.
- **Enterprise Claude Code**: confirm whether local or plugin hooks are allowed versus `allowManagedHooksOnly` before relying on generated `hooks.json`.
- **Hermetic shell parsing**: the YAML `optional_shell_parser` hook point is intended for a stricter parser (for example a small Rust or Node helper); regex/substring rules are intentionally shallow.
- **Script wrapper requirement** (`bash.script_wrapper`): prefix matching only; users who need `bash -c`, `env`, or quoted paths must add explicit `accepted_invocations` entries for those forms or accept denials.
