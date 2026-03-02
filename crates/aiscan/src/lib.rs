// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! AI-content detection.
//!
//! Three implementations of [`env_traits::AiEnv`]:
//!
//! - [`HeuristicAiEnv`] — pure-Rust statistical heuristic, no network.
//! - [`HttpAiEnv`] — POSTs to an external scoring service via [`NetworkEnv`].
//! - [`NoopAiEnv`] — always returns `(false, 0.0)`.
//!
//! Use [`AiEnvConfig`] + [`build_ai_env`] to select an implementation from
//! environment variables at startup.

mod heuristic;
mod http;

pub use heuristic::HeuristicAiEnv;
pub use http::HttpAiEnv;

use anyhow::{anyhow, Result};
use env_traits::{AiEnv, NetworkEnv};
use std::path::Path;

// ── NoopAiEnv ────────────────────────────────────────────────────────────────

/// Always returns `(false, 0.0)`.  Selected when `AI_SCAN_BACKEND=none`.
#[derive(Default, Clone, Copy)]
pub struct NoopAiEnv;

impl AiEnv for NoopAiEnv {
    fn scan(&self, _path: &Path, _content: &[u8]) -> Result<(bool, f64)> {
        Ok((false, 0.0))
    }
}

// ── Factory ──────────────────────────────────────────────────────────────────

/// Configuration read from environment variables at binary startup.
///
/// Build it with [`AiEnvConfig::from_env`] then pass it to
/// [`build_ai_env`].
#[derive(Debug, Clone)]
pub struct AiEnvConfig {
    pub backend:  String,
    pub endpoint: String,
}

impl AiEnvConfig {
    /// Read `AI_SCAN_BACKEND` and `AI_SCAN_ENDPOINT` from `std::env`.
    ///
    /// If `AI_SCAN_ENDPOINT` is set and `AI_SCAN_BACKEND` is empty the
    /// backend defaults to `"http"`.
    pub fn from_env() -> Self {
        let endpoint = std::env::var("AI_SCAN_ENDPOINT").unwrap_or_default().trim().to_string();
        let mut backend = std::env::var("AI_SCAN_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if !endpoint.is_empty() && backend.is_empty() {
            backend = "http".to_string();
        }
        Self { backend, endpoint }
    }
}

/// Construct a boxed [`AiEnv`] from `config` and a `network` implementation.
///
/// | `config.backend`   | Result                             |
/// |--------------------|------------------------------------|
/// | `"none"`           | [`NoopAiEnv`]                      |
/// | `"http"`           | [`HttpAiEnv`] (needs endpoint)     |
/// | `"heuristic"` / `` | [`HeuristicAiEnv`]                 |
pub fn build_ai_env<N: NetworkEnv + 'static>(
    config: AiEnvConfig,
    network: N,
) -> Result<Box<dyn AiEnv>> {
    match config.backend.as_str() {
        "none" => Ok(Box::new(NoopAiEnv)),
        "http" => {
            if config.endpoint.is_empty() {
                return Err(anyhow!(
                    "AI_SCAN_BACKEND=http requires AI_SCAN_ENDPOINT to be set"
                ));
            }
            Ok(Box::new(HttpAiEnv::new(config.endpoint, network)))
        }
        "heuristic" | "" => Ok(Box::new(HeuristicAiEnv)),
        other => Err(anyhow!(
            "unknown AI_SCAN_BACKEND {:?} (valid: none, http, heuristic)",
            other
        )),
    }
}
