// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Load YAML policy and emit Pi + Claude Code sandbox artifacts.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const CLAUDE_HOOK_TEMPLATE: &str = include_str!("../templates/claude_hook.ts");
const PI_HOOK_TEMPLATE: &str = include_str!("../templates/pi_extension.ts");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub meta: Option<Meta>,
    pub gate: Gate,
    pub bash: Option<BashSection>,
    pub prompts: Option<PromptsSection>,
    pub optional_shell_parser: Option<ShellParserSection>,
    /// When empty, defaults to `npx --yes tsx`.
    #[serde(default)]
    pub claude_ts_runner: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub env: String,
    pub value: String,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BashSection {
    #[serde(default = "default_bash_tool_names")]
    pub tool_names: Vec<String>,
    #[serde(default)]
    pub deny_substrings: Vec<String>,
    #[serde(default)]
    pub deny_regexes: Vec<String>,
    #[serde(default)]
    pub command_prefix: Option<String>,
    pub connection_script: Option<ConnectionScript>,
}

fn default_bash_tool_names() -> Vec<String> {
    vec!["bash".into(), "Bash".into()]
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ConnectionScript {
    pub path: String,
    #[serde(default)]
    pub trigger_substrings: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PromptsSection {
    #[serde(default)]
    pub session_fragment: Option<String>,
    #[serde(default)]
    pub user_submit_fragment: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ShellParserSection {
    #[serde(default)]
    pub command: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEmbed {
    pub gate_env: String,
    pub gate_value: String,
    pub bash_tool_names: Vec<String>,
    pub deny_substrings: Vec<String>,
    pub deny_regexes: Vec<String>,
    pub command_prefix: Option<String>,
    pub connection_script_path: Option<String>,
    pub connection_script_triggers: Vec<String>,
    pub session_fragment: Option<String>,
    pub user_submit_fragment: Option<String>,
    pub shell_parser_argv: Option<Vec<String>>,
}

pub fn load_config(path: &Path) -> Result<Config> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let c: Config = serde_yaml::from_str(&raw).context("parse YAML")?;
    Ok(c)
}

fn normalize_prefix(s: &Option<String>) -> Option<String> {
    let t = s.as_ref()?.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn build_policy_embed(config: &Config) -> PolicyEmbed {
    let bash = config.bash.clone().unwrap_or_default();
    let prompts = config.prompts.clone().unwrap_or_default();
    let (csp, cst) = match &bash.connection_script {
        Some(cs) if !cs.path.trim().is_empty() => (
            Some(cs.path.trim().to_string()),
            cs.trigger_substrings.clone(),
        ),
        _ => (None, Vec::new()),
    };
    let shell_parser_argv = config.optional_shell_parser.as_ref().and_then(|s| {
        if s.command.is_empty() {
            None
        } else {
            Some(s.command.clone())
        }
    });
    PolicyEmbed {
        gate_env: config.gate.env.clone(),
        gate_value: config.gate.value.clone(),
        bash_tool_names: if bash.tool_names.is_empty() {
            default_bash_tool_names()
        } else {
            bash.tool_names.clone()
        },
        deny_substrings: bash.deny_substrings.clone(),
        deny_regexes: bash.deny_regexes.clone(),
        command_prefix: normalize_prefix(&bash.command_prefix),
        connection_script_path: csp,
        connection_script_triggers: cst,
        session_fragment: prompts
            .session_fragment
            .clone()
            .filter(|s| !s.trim().is_empty()),
        user_submit_fragment: prompts
            .user_submit_fragment
            .clone()
            .filter(|s| !s.trim().is_empty()),
        shell_parser_argv,
    }
}

fn claude_tool_matcher(tool_names: &[String]) -> String {
    tool_names.join("|")
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn render_shim(gate: &Gate, ts_runner: &[String]) -> String {
    let env_ref = format!(concat!("\"", "$", "{{", "{}", ":-", "}}", "\""), gate.env);
    let gate_check = format!(
        "if [[ {} != {} ]]; then\n  exit 0\nfi",
        env_ref,
        shell_single_quote(&gate.value)
    );
    let run_elems = ts_runner
        .iter()
        .map(|a| shell_single_quote(a))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         {}\n\
         RUN_TS=( {} )\n\
         SCRIPT_DIR=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\n\
         exec \"${{RUN_TS[@]}}\" \"$SCRIPT_DIR/portal-claude-sandbox-hook.ts\"\n",
        gate_check, run_elems
    )
}

fn inject_policy(template: &str, policy: &PolicyEmbed) -> Result<String> {
    let json = serde_json::to_string(policy).context("serialize policy")?;
    if template.contains("<<<POLICY_JSON>>>") {
        Ok(template.replace("<<<POLICY_JSON>>>", &json))
    } else {
        anyhow::bail!("template missing <<<POLICY_JSON>>> placeholder");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EmitOpts {
    pub pi_only: bool,
    pub claude_only: bool,
    pub emit_plugin_json: bool,
}

pub fn emit(config: &Config, out_dir: &Path, source_config: &Path, opts: EmitOpts) -> Result<()> {
    if opts.emit_plugin_json && opts.pi_only {
        anyhow::bail!("--emit-plugin-json requires Claude outputs (omit --pi-only)");
    }
    fs::create_dir_all(out_dir).with_context(|| format!("mkdir {}", out_dir.display()))?;
    let hooks_dir = out_dir.join("hooks");
    if !opts.pi_only {
        fs::create_dir_all(&hooks_dir).context("mkdir hooks")?;
    }

    let policy = build_policy_embed(config);
    let ts_runner = if config.claude_ts_runner.is_empty() {
        vec!["npx".into(), "--yes".into(), "tsx".into()]
    } else {
        config.claude_ts_runner.clone()
    };

    let banner = format!("// Source policy: {}\n", source_config.display());

    if !opts.claude_only {
        let mut pi = inject_policy(PI_HOOK_TEMPLATE, &policy)?;
        pi = banner.clone() + &pi;
        fs::write(out_dir.join("portal-pi-sandbox.ts"), pi)
            .context("write portal-pi-sandbox.ts")?;
    }

    if !opts.pi_only {
        let mut hook = inject_policy(CLAUDE_HOOK_TEMPLATE, &policy)?;
        hook = banner + &hook;
        fs::write(out_dir.join("portal-claude-sandbox-hook.ts"), hook)
            .context("write portal-claude-sandbox-hook.ts")?;

        let shim = render_shim(&config.gate, &ts_runner);
        let shim_path = out_dir.join("portal-sandbox-shim.sh");
        fs::write(&shim_path, shim).context("write portal-sandbox-shim.sh")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shim_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shim_path, perms).context("chmod shim")?;
        }

        let abs_shim = fs::canonicalize(&shim_path)
            .with_context(|| format!("canonicalize {}", shim_path.display()))?;

        let mut hooks_obj = serde_json::Map::new();
        let mut inner = serde_json::Map::new();

        inner.insert(
            "PreToolUse".into(),
            serde_json::json!([{
                "matcher": claude_tool_matcher(&policy.bash_tool_names),
                "hooks": [{
                    "type": "command",
                    "command": abs_shim.display().to_string()
                }]
            }]),
        );

        if policy.user_submit_fragment.is_some() {
            inner.insert(
                "UserPromptSubmit".into(),
                serde_json::json!([{
                    "hooks": [{
                        "type": "command",
                        "command": abs_shim.display().to_string()
                    }]
                }]),
            );
        }

        if policy.session_fragment.is_some() {
            inner.insert(
                "SessionStart".into(),
                serde_json::json!([{
                    "matcher": "startup|resume|clear|compact",
                    "hooks": [{
                        "type": "command",
                        "command": abs_shim.display().to_string()
                    }]
                }]),
            );
        }

        hooks_obj.insert("hooks".into(), serde_json::Value::Object(inner));
        let hooks_json = serde_json::to_string_pretty(&serde_json::Value::Object(hooks_obj))
            .context("hooks json")?;
        fs::write(hooks_dir.join("hooks.json"), hooks_json).context("write hooks/hooks.json")?;

        if opts.emit_plugin_json {
            let meta = config.meta.as_ref().cloned().unwrap_or(Meta {
                name: "portal-agent-sandbox".into(),
                version: "0.1.0".into(),
                description: "Generated sandbox hooks".into(),
            });
            let plugin = serde_json::json!({
                "name": meta.name,
                "version": meta.version,
                "description": meta.description,
                "hooks": "./hooks/hooks.json"
            });
            let s = serde_json::to_string_pretty(&plugin).context("plugin json")?;
            fs::write(out_dir.join("plugin.json"), s).context("write plugin.json")?;
        }
    }

    Ok(())
}
