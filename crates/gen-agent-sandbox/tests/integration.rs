use std::fs;
use std::path::PathBuf;
use std::process::Command;

use gen_agent_sandbox::{build_policy_embed, emit, load_config, EmitOpts};
use tempfile::tempdir;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("minimal.yaml")
}

#[test]
fn minimal_emit_contains_policy_strings() {
    let dir = tempdir().unwrap();
    let cfg = load_config(&fixture_path()).unwrap();
    emit(
        &cfg,
        dir.path(),
        &fixture_path(),
        EmitOpts {
            pi_only: false,
            claude_only: false,
            emit_plugin_json: true,
        },
    )
    .unwrap();

    let pi = fs::read_to_string(dir.path().join("portal-pi-sandbox.ts")).unwrap();
    assert!(pi.contains("TEST_GATE"));
    assert!(pi.contains("__DENY_ME__"));
    assert!(pi.contains("SESSION_CTX"));
    assert!(pi.contains("USER_CTX"));

    let hook = fs::read_to_string(dir.path().join("portal-claude-sandbox-hook.ts")).unwrap();
    assert!(hook.contains("gateEnv") && hook.contains("TEST_GATE"));
    assert!(hook.contains("__DENY_ME__"));

    let shim = fs::read_to_string(dir.path().join("portal-sandbox-shim.sh")).unwrap();
    assert!(shim.contains("TEST_GATE"));
    assert!(shim.contains("RUN_TS=("));
    assert!(shim.contains("'npx'"));

    let hooks = fs::read_to_string(dir.path().join("hooks/hooks.json")).unwrap();
    assert!(hooks.contains("PreToolUse"));
    assert!(hooks.contains("UserPromptSubmit"));
    assert!(hooks.contains("SessionStart"));

    let plugin = fs::read_to_string(dir.path().join("plugin.json")).unwrap();
    assert!(plugin.contains("portal-agent-sandbox"));

    // Default meta when omitted in YAML
    assert!(plugin.contains("0.1.0"));
}

#[test]
fn build_policy_embed_defaults() {
    let cfg = load_config(&fixture_path()).unwrap();
    let p = build_policy_embed(&cfg);
    assert_eq!(p.gate_env, "TEST_GATE");
    assert_eq!(p.gate_value, "1");
    assert!(p.bash_tool_names.contains(&"bash".into()));
}

#[test]
fn pi_only_skips_hooks_dir() {
    let dir = tempdir().unwrap();
    let cfg = load_config(&fixture_path()).unwrap();
    emit(
        &cfg,
        dir.path(),
        &fixture_path(),
        EmitOpts {
            pi_only: true,
            claude_only: false,
            emit_plugin_json: false,
        },
    )
    .unwrap();
    assert!(dir.path().join("portal-pi-sandbox.ts").exists());
    assert!(!dir.path().join("portal-sandbox-shim.sh").exists());
}

#[test]
fn binary_smoke() {
    let exe = std::env::var_os("CARGO_BIN_EXE_gen-agent-sandbox")
        .expect("cargo test sets CARGO_BIN_EXE_gen-agent-sandbox");
    let dir = tempdir().unwrap();
    let status = Command::new(exe)
        .args([
            "--config",
            fixture_path().to_str().unwrap(),
            "--out-dir",
            dir.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}
