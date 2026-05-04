# `portal-sandbox` YAML schema

Non-normative reference for [`portal-sandbox.example.yaml`](portal-sandbox.example.yaml). The generator (`gen-agent-sandbox`) is the source of truth for supported keys.

## Top-level

| Key | Type | Required | Description |
| --- | --- | --- | --- |
| `meta` | object | no | `name`, `version`, `description` for optional `plugin.json`. |
| `gate` | object | **yes** | `env` (variable name) and `value` (string match). |
| `bash` | object | no | Bash/Bash-tool policy. Omitted sections default to safe empties. |
| `prompts` | object | no | Text injected when the gate is active. |
| `optional_shell_parser` | object | no | `command`: argv invoked with the command on stdin; stdout replaces input for checks. |
| `claude_ts_runner` | string[] | no | argv to run the Claude hook `.ts` file. When empty or omitted, defaults to `npx`, `--yes`, `tsx`. |

## `gate`

Both fields are strings. Matching is literal (`process.env[env] === value` in Pi; same in the Claude shim).

## `bash`

| Key | Type | Description |
| --- | --- | --- |
| `tool_names` | string[] | Defaults to `["bash", "Bash"]`. |
| `deny_substrings` | string[] | Block if command contains substring. |
| `deny_regexes` | string[] | Block if any regex matches (JS `RegExp`). |
| `command_prefix` | string | Optional; prepended to allowed commands. |
| `connection_script` | object | Optional `path` and `trigger_substrings[]`; prepends `path` + space when any trigger matches. |

## `prompts`

| Key | Type | Description |
| --- | --- | --- |
| `session_fragment` | string | Claude `SessionStart` context; Pi `session_start` notify + `before_agent_start` append. |
| `user_submit_fragment` | string | Claude `UserPromptSubmit` `additionalContext`; Pi `before_agent_start` append. |

## Output files (`--out-dir`)

| File | Description |
| --- | --- |
| `portal-pi-sandbox.ts` | Pi extension (peer: `@mariozechner/pi-coding-agent`). |
| `portal-claude-sandbox-hook.ts` | stdin JSON hook runner for Claude Code. |
| `portal-sandbox-shim.sh` | Env gate + `exec` runner. |
| `hooks/hooks.json` | Fragment to merge into a plugin or `.claude` hooks. |
| `plugin.json` | Only with `--emit-plugin-json`. |

Regenerate after changing policy or upgrading Pi / Claude Code hook JSON schemas.
