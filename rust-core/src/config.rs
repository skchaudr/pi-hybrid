//! Configuration loading, validation, and environment-variable merging.
//!
//! ## Environment variable overrides (higher priority than config file)
//! - `PI_PROVIDER` → `config.provider`
//! - `PI_SESSION_DB` → `config.session.db_path`
//! - `PI_MAX_TURNS` → `config.agent.max_turns`
//! - `PI_LOG_LEVEL` → `config.logging.level`
//! - `PI_DEEPSEEK_API_KEY`, `PI_GLM_API_KEY` → per-provider API key overrides
//! - `OLLAMA_HOST` → `providers.ollama.api_base` (e.g. `127.0.0.1:9000`)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, info, warn};

// ── Top-level configuration ──────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PiConfig {
    /// Default provider name (e.g. "deepseek").
    pub provider: String,
    /// Map of provider name → ProviderConfig.
    pub providers: HashMap<String, ProviderConfig>,
    /// Session persistence settings.
    pub session: SessionConfig,
    /// TypeScript bridge settings.
    pub bridge: BridgeConfig,
    /// Logging settings.
    pub logging: LoggingConfig,
    /// Agent loop settings.
    pub agent: AgentBlock,
}

impl Default for PiConfig {
    fn default() -> Self {
        Self {
            provider: "deepseek".to_string(),
            providers: builtin_providers(),
            session: SessionConfig::default(),
            bridge: BridgeConfig::default(),
            logging: LoggingConfig::default(),
            agent: AgentBlock::default(),
        }
    }
}

impl PiConfig {
    /// Load configuration from the canonical path, merge with environment
    /// variables, and validate.
    pub fn load(config_path: Option<&Path>) -> anyhow::Result<Self> {
        let path = config_path
            .map(PathBuf::from)
            .unwrap_or_else(default_config_path);

        // Start with built-in defaults.
        let mut config = Self::default();

        // Merge file on top of defaults.
        if path.exists() {
            let raw = std::fs::read_to_string(&path).map_err(|e| {
                anyhow::anyhow!("Failed to read config file {}: {e}", path.display())
            })?;
            let file_cfg: PiConfig = toml::from_str(&raw).map_err(|e| {
                anyhow::anyhow!("Failed to parse config file {}: {e}", path.display())
            })?;
            // Merge provider entries from file into defaults (file wins).
            for (k, v) in file_cfg.providers {
                config.providers.insert(k, v);
            }
            // Override scalar fields if non-empty in file.
            if !file_cfg.provider.is_empty() {
                config.provider = file_cfg.provider;
            }
            if !file_cfg.session.db_path.is_empty() {
                config.session.db_path = file_cfg.session.db_path;
            }
            if !file_cfg.bridge.ts_bridge_path.is_empty() {
                config.bridge.ts_bridge_path = file_cfg.bridge.ts_bridge_path;
            }
            if file_cfg.bridge.ts_bridge_timeout != 0 {
                config.bridge.ts_bridge_timeout = file_cfg.bridge.ts_bridge_timeout;
            }
            if !file_cfg.logging.level.is_empty() {
                config.logging.level = file_cfg.logging.level;
            }
            if file_cfg.agent.max_turns != 0 {
                config.agent.max_turns = file_cfg.agent.max_turns;
            }
            if !file_cfg.agent.default_model.is_empty() {
                config.agent.default_model = file_cfg.agent.default_model;
            }
            info!(path = %path.display(), "Config file loaded");
        } else {
            debug!(path = %path.display(), "No config file found, using defaults");
        }

        // Apply environment variable overrides (highest priority).
        Self::apply_env_overrides(&mut config);

        // Validate.
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variable overrides to the configuration.
    fn apply_env_overrides(config: &mut Self) {
        if let Ok(val) = std::env::var("PI_PROVIDER")
            && !val.is_empty()
        {
            info!(provider = %val, "PI_PROVIDER override");
            config.provider = val;
        }
        if let Ok(val) = std::env::var("PI_SESSION_DB")
            && !val.is_empty()
        {
            info!(db_path = %val, "PI_SESSION_DB override");
            config.session.db_path = val;
        }
        if let Ok(val) = std::env::var("PI_MAX_TURNS") {
            if let Ok(n) = val.parse::<usize>() {
                info!(max_turns = n, "PI_MAX_TURNS override");
                config.agent.max_turns = n;
            } else {
                warn!(value = %val, "PI_MAX_TURNS is not a valid usize, ignoring");
            }
        }
        if let Ok(val) = std::env::var("PI_LOG_LEVEL")
            && !val.is_empty()
        {
            info!(level = %val, "PI_LOG_LEVEL override");
            config.logging.level = val;
        }
        // Per-provider API key overrides.
        if let Ok(key) = std::env::var("PI_DEEPSEEK_API_KEY")
            && !key.is_empty()
        {
            if let Some(provider) = config.providers.get_mut("deepseek") {
                provider.set_api_key(key);
            } else {
                debug!("PI_DEEPSEEK_API_KEY set but no 'deepseek' provider configured");
            }
        }
        if let Ok(key) = std::env::var("PI_GLM_API_KEY")
            && !key.is_empty()
        {
            if let Some(provider) = config.providers.get_mut("glm") {
                provider.set_api_key(key);
            } else {
                debug!("PI_GLM_API_KEY set but no 'glm' provider configured");
            }
        }
        if let Ok(host) = std::env::var("OLLAMA_HOST")
            && !host.is_empty()
            && let Some(provider) = config.providers.get_mut("ollama")
        {
            let api_base = if host.starts_with("http://") || host.starts_with("https://") {
                format!("{host}/v1")
            } else {
                format!("http://{host}/v1")
            };
            info!(api_base = %api_base, "OLLAMA_HOST override");
            provider.api_base = api_base;
        }
    }

    /// Validate the entire configuration, collecting ALL errors.
    ///
    /// Returns `Ok(())` if valid, or an error string containing all
    /// validation failures separated by newlines.
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut errors: Vec<String> = Vec::new();

        // 1. Provider must exist in the providers map if set.
        if !self.provider.is_empty() && !self.providers.contains_key(&self.provider) {
            errors.push(format!(
                "provider '{}' is not defined in [providers]",
                self.provider
            ));
        }

        // 2. Each provider: api_key_env must either exist or be "none".
        for (name, provider) in &self.providers {
            match provider.api_key_env.as_deref() {
                None | Some("") => {
                    errors.push(format!("provider '{name}': api_key_env is not set"));
                }
                Some("none") => {
                    // "none" means local/no key needed — OK.
                }
                Some(env_var) => {
                    if std::env::var(env_var).is_err() {
                        errors.push(format!(
                            "provider '{name}': api_key_env '{env_var}' is not set in environment"
                        ));
                    }
                }
            }
        }

        // 3. session.db_path parent directory must exist and be writable (or creatable).
        if !self.session.db_path.is_empty() {
            let db = Path::new(&self.session.db_path);
            if let Some(parent) = db.parent()
                && !parent.as_os_str().is_empty()
            {
                if parent.exists() {
                    if parent.is_dir() {
                        // Check writeability.
                        match std::fs::metadata(parent) {
                            Ok(meta) => {
                                if meta.permissions().readonly() {
                                    errors.push(format!(
                                        "session.db_path parent directory '{}' is read-only",
                                        parent.display()
                                    ));
                                }
                            }
                            Err(e) => {
                                errors.push(format!(
                                        "session.db_path parent directory '{}': cannot read metadata: {e}",
                                        parent.display()
                                    ));
                            }
                        }
                    } else {
                        errors.push(format!(
                            "session.db_path parent '{}' is not a directory",
                            parent.display()
                        ));
                    }
                } else {
                    // Parent doesn't exist — check if its parent exists and is writable.
                    if let Some(grandparent) = parent.parent() {
                        if grandparent.exists() && grandparent.is_dir() {
                            match std::fs::metadata(grandparent) {
                                Ok(meta) if meta.permissions().readonly() => {
                                    errors.push(format!(
                                            "session.db_path parent directory '{}' does not exist and grandparent '{}' is read-only",
                                            parent.display(),
                                            grandparent.display()
                                        ));
                                }
                                Err(e) => {
                                    errors.push(format!(
                                            "session.db_path parent directory '{}': cannot check grandparent: {e}",
                                            parent.display()
                                        ));
                                }
                                _ => {} // creatable — OK.
                            }
                        } else {
                            errors.push(format!(
                                    "session.db_path parent directory '{}' does not exist and cannot be created",
                                    parent.display()
                                ));
                        }
                    } else {
                        errors.push(format!(
                                "session.db_path parent directory '{}' does not exist and cannot be created",
                                parent.display()
                            ));
                    }
                }
            }
        }

        // 4. bridge.ts_bridge_path: if not "none" and not empty, file must exist or warn.
        if !self.bridge.ts_bridge_path.is_empty() && self.bridge.ts_bridge_path != "none" {
            let bridge_path = Path::new(&self.bridge.ts_bridge_path);
            if !bridge_path.exists() {
                // Non-fatal warning only — emit via tracing (caller may promote).
                warn!(
                    path = %self.bridge.ts_bridge_path,
                    "bridge.ts_bridge_path does not exist"
                );
            }
        }

        // 5. agent.max_turns: must be > 0 and <= 500.
        if self.agent.max_turns == 0 {
            errors.push("agent.max_turns must be > 0".to_string());
        } else if self.agent.max_turns > 500 {
            errors.push(format!(
                "agent.max_turns is {} but must be <= 500",
                self.agent.max_turns
            ));
        }

        // 6. agent.default_model: must be non-empty.
        if self.agent.default_model.is_empty() {
            errors.push("agent.default_model must not be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Configuration validation failed:\n  - {}",
                errors.join("\n  - ")
            ))
        }
    }

    /// Validate for warnings only — returns non-fatal issues.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Bridge path warning (non-fatal).
        if !self.bridge.ts_bridge_path.is_empty() && self.bridge.ts_bridge_path != "none" {
            let bridge_path = Path::new(&self.bridge.ts_bridge_path);
            if !bridge_path.exists() {
                warnings.push(format!(
                    "bridge.ts_bridge_path '{}' does not exist",
                    self.bridge.ts_bridge_path
                ));
            }
        }

        warnings
    }

    /// Derive an `AgentConfig` from this `PiConfig`.
    pub fn to_agent_config(&self) -> super::agent::AgentConfig {
        super::agent::AgentConfig {
            model: self.agent.default_model.clone(),
            max_turns: self.agent.max_turns,
            context_window_tokens: 200_000,
            bridge_command: if self.bridge.ts_bridge_path.is_empty()
                || self.bridge.ts_bridge_path == "none"
            {
                std::env::var("PI_BRIDGE_COMMAND").unwrap_or_default()
            } else {
                self.bridge.ts_bridge_path.clone()
            },
            max_subagents: 8,
            db_path: self.session.db_path.clone(),
        }
    }
}

// ── Sub-config structs ───────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    /// Path to the SQLite session database.
    pub db_path: String,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            db_path: default_session_db_path(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BridgeConfig {
    /// Path to the TypeScript bridge executable. "none" disables.
    pub ts_bridge_path: String,
    /// Timeout in milliseconds for bridge calls.
    pub ts_bridge_timeout: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            ts_bridge_path: String::new(),
            ts_bridge_timeout: 30_000,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: trace, debug, info, warn, error.
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "info".to_string()
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AgentBlock {
    /// Maximum number of turns per agent run.
    pub max_turns: usize,
    /// Default model name to use.
    pub default_model: String,
}

impl Default for AgentBlock {
    fn default() -> Self {
        Self {
            max_turns: 50,
            default_model: "deepseek-chat".to_string(),
        }
    }
}

// ── Provider configuration ───────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Human-readable name of the provider.
    pub name: String,
    /// Base URL for the provider's API.
    pub api_base: String,
    /// Environment variable name that stores the API key,
    /// or "none" if no key is required (local models).
    pub api_key_env: Option<String>,
    /// Default model to use for this provider.
    pub default_model: String,
    /// Resolved API key value (populated from environment after load).
    #[serde(skip)]
    api_key_value: Option<String>,
}

impl ProviderConfig {
    /// Set the API key value (used during env-var override).
    pub fn set_api_key(&mut self, key: String) {
        self.api_key_value = Some(key);
    }

    /// Get the resolved API key, if available.
    pub fn api_key(&self) -> Option<&str> {
        self.api_key_value.as_deref().or_else(|| {
            self.api_key_env
                .as_deref()
                .and_then(|env_var| std::env::var(env_var).ok())
                .as_deref()
                .map(|_| {
                    // We can't return a reference to the owned String from env::var.
                    // This is a limitation; callers should use api_key_resolved() instead.
                    None
                })
                .unwrap_or(None)
        })
    }

    /// Get the resolved API key as an owned String.
    pub fn api_key_resolved(&self) -> Option<String> {
        self.api_key_value.clone().or_else(|| {
            self.api_key_env
                .as_deref()
                .and_then(|env_var| std::env::var(env_var).ok())
        })
    }
}

// ── Built-in provider defaults ───────────────────────────────────────

/// Returns the built-in provider defaults (DeepSeek and GLM).
pub fn builtin_providers() -> HashMap<String, ProviderConfig> {
    let mut map = HashMap::new();

    map.insert(
        "deepseek".to_string(),
        ProviderConfig {
            name: "DeepSeek".to_string(),
            api_base: "https://api.deepseek.com/v1".to_string(),
            api_key_env: Some("PI_DEEPSEEK_API_KEY".to_string()),
            default_model: "deepseek-chat".to_string(),
            api_key_value: None,
        },
    );

    map.insert(
        "glm".to_string(),
        ProviderConfig {
            name: "GLM (ZhipuAI)".to_string(),
            api_base: "https://open.bigmodel.cn/api/paas/v4".to_string(),
            api_key_env: Some("PI_GLM_API_KEY".to_string()),
            default_model: "glm-4-flash".to_string(),
            api_key_value: None,
        },
    );

    map.insert(
        "ollama".to_string(),
        ProviderConfig {
            name: "Ollama (local)".to_string(),
            api_base: "http://127.0.0.1:9000/v1".to_string(),
            api_key_env: Some("none".to_string()),
            default_model: "qwen2.5-coder:7b".to_string(),
            api_key_value: None,
        },
    );

    map
}

// ── Path helpers ─────────────────────────────────────────────────────

/// Canonical path to the config file: `~/.pi-hybrid/config.toml`.
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".pi-hybrid").join("config.toml")
}

/// Default session database path: `~/.pi-hybrid/sessions.db`.
fn default_session_db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    format!("{home}/.pi-hybrid/sessions.db")
}

/// Generate a default, commented TOML configuration file.
pub fn generate_default_toml() -> String {
    r#"# Pi Hybrid Configuration
# Generated by: rust-core --init-config
#
# Environment variable overrides (higher priority than this file):
#   PI_PROVIDER    → provider
#   PI_SESSION_DB  → session.db_path
#   PI_MAX_TURNS   → agent.max_turns
#   PI_LOG_LEVEL   → logging.level
#   PI_DEEPSEEK_API_KEY → deepseek API key
#   PI_GLM_API_KEY     → GLM API key
#   OLLAMA_HOST        → ollama API host (default 127.0.0.1:9000)

# Default LLM provider (must match a key in [providers])
provider = "deepseek"

# ── Session ───────────────────────────────────────────────────────────
[session]
# Path to the SQLite session database
db_path = "~/.pi-hybrid/sessions.db"

# ── Bridge ────────────────────────────────────────────────────────────
[bridge]
# Path to the TypeScript bridge executable ("none" to disable)
ts_bridge_path = ""
# Timeout in milliseconds for bridge calls
ts_bridge_timeout = 30000

# ── Logging ───────────────────────────────────────────────────────────
[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# ── Agent ─────────────────────────────────────────────────────────────
[agent]
# Maximum number of turns per agent run (1-500)
max_turns = 50
# Default model name
default_model = "deepseek-chat"

# ── Provider: DeepSeek ────────────────────────────────────────────────
[providers.deepseek]
name = "DeepSeek"
api_base = "https://api.deepseek.com/v1"
api_key_env = "PI_DEEPSEEK_API_KEY"
default_model = "deepseek-chat"

# ── Provider: GLM (ZhipuAI) ──────────────────────────────────────────
[providers.glm]
name = "GLM (ZhipuAI)"
api_base = "https://open.bigmodel.cn/api/paas/v4"
api_key_env = "PI_GLM_API_KEY"
default_model = "glm-4-flash"

# ── Provider: Ollama (local, Mac mini / MacBook Air) ─────────────────
# Set OLLAMA_HOST=127.0.0.1:9000 in the shell (see ~/.zshlocal).
# Switch default_model to any pulled model:
#   qwen2.5-coder:7b   — fast, both machines
#   qwen2.5-coder:14b  — heavier, Mac mini primary
# Gemma 12B via Google Eloquence is separate (not Ollama); add when wired.
[providers.ollama]
name = "Ollama (local)"
api_base = "http://127.0.0.1:9000/v1"
api_key_env = "none"
default_model = "qwen2.5-coder:7b"
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── Helpers ──────────────────────────────────────────────────────

    fn tmp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn write_config(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    fn config_with_defaults() -> PiConfig {
        let mut cfg = PiConfig::default();
        // Replace built-in providers with test ones that don't need env vars.
        cfg.providers.clear();
        cfg.providers.insert(
            "deepseek".to_string(),
            ProviderConfig {
                name: "DeepSeek".to_string(),
                api_base: "https://api.deepseek.com/v1".to_string(),
                api_key_env: Some("none".to_string()),
                default_model: "deepseek-chat".to_string(),
                api_key_value: None,
            },
        );
        cfg.providers.insert(
            "glm".to_string(),
            ProviderConfig {
                name: "GLM".to_string(),
                api_base: "https://open.bigmodel.cn/api/paas/v4".to_string(),
                api_key_env: Some("none".to_string()),
                default_model: "glm-4-flash".to_string(),
                api_key_value: None,
            },
        );
        cfg
    }

    // ── Test: defaults are sensible ──────────────────────────────────

    #[test]
    fn default_config_has_builtin_providers() {
        let cfg = PiConfig::default();
        assert_eq!(cfg.provider, "deepseek");
        assert!(cfg.providers.contains_key("deepseek"));
        assert!(cfg.providers.contains_key("glm"));
        assert!(cfg.providers.contains_key("ollama"));
        assert_eq!(cfg.agent.max_turns, 50);
        assert_eq!(cfg.agent.default_model, "deepseek-chat");
    }

    #[test]
    fn builtin_providers_have_correct_entries() {
        let providers = builtin_providers();
        let ds = providers.get("deepseek").unwrap();
        assert_eq!(ds.name, "DeepSeek");
        assert_eq!(ds.api_base, "https://api.deepseek.com/v1");
        assert_eq!(ds.api_key_env.as_deref(), Some("PI_DEEPSEEK_API_KEY"));

        let glm = providers.get("glm").unwrap();
        assert_eq!(glm.name, "GLM (ZhipuAI)");
        assert_eq!(glm.default_model, "glm-4-flash");

        let ollama = providers.get("ollama").unwrap();
        assert_eq!(ollama.api_key_env.as_deref(), Some("none"));
        assert_eq!(ollama.default_model, "qwen2.5-coder:7b");
    }

    // ── Test: validation rejects missing provider ────────────────────

    #[test]
    fn validate_rejects_unknown_provider() {
        let mut cfg = config_with_defaults();
        cfg.provider = "nonexistent".to_string();
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn validate_accepts_known_provider() {
        let cfg = config_with_defaults();
        // deepseek exists in providers map
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_empty_provider_is_ok() {
        let mut cfg = config_with_defaults();
        cfg.provider = String::new();
        // Empty provider skips the check (provider not set).
        assert!(cfg.validate().is_ok());
    }

    // ── Test: provider api_key_env validation ────────────────────────

    #[test]
    fn validate_rejects_provider_missing_api_key_env() {
        let mut cfg = config_with_defaults();
        cfg.providers.clear();
        cfg.providers.insert(
            "test".to_string(),
            ProviderConfig {
                name: "Test".to_string(),
                api_base: "http://localhost".to_string(),
                api_key_env: None,
                default_model: "test-model".to_string(),
                api_key_value: None,
            },
        );
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("test") && err.contains("api_key_env"));
    }

    #[test]
    fn validate_accepts_provider_with_none_key() {
        let mut cfg = config_with_defaults();
        cfg.provider = "local".to_string();
        cfg.providers.clear();
        cfg.providers.insert(
            "local".to_string(),
            ProviderConfig {
                name: "Local".to_string(),
                api_base: "http://localhost:8080".to_string(),
                api_key_env: Some("none".to_string()),
                default_model: "llama3".to_string(),
                api_key_value: None,
            },
        );
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_provider_with_unset_env_var() {
        let mut cfg = config_with_defaults();
        cfg.providers.clear();
        cfg.providers.insert(
            "test".to_string(),
            ProviderConfig {
                name: "Test".to_string(),
                api_base: "http://localhost".to_string(),
                api_key_env: Some("PI_NONEXISTENT_KEY_XYZ123".to_string()),
                default_model: "test-model".to_string(),
                api_key_value: None,
            },
        );
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("PI_NONEXISTENT_KEY_XYZ123"));
    }

    #[test]
    fn validate_accepts_provider_with_set_env_var() {
        // Temporarily set an env var.
        unsafe { std::env::set_var("PI_TEST_KEY_TMP", "test-value") };
        let mut cfg = config_with_defaults();
        cfg.provider = "test".to_string();
        cfg.providers.clear();
        cfg.providers.insert(
            "test".to_string(),
            ProviderConfig {
                name: "Test".to_string(),
                api_base: "http://localhost".to_string(),
                api_key_env: Some("PI_TEST_KEY_TMP".to_string()),
                default_model: "test-model".to_string(),
                api_key_value: None,
            },
        );
        let result = cfg.validate();
        unsafe { std::env::remove_var("PI_TEST_KEY_TMP") };
        assert!(result.is_ok());
    }

    // ── Test: session.db_path validation ─────────────────────────────

    #[test]
    fn validate_session_db_path_writable_parent() {
        let dir = tmp_dir();
        let db_path = dir.path().join("sessions.db");
        let mut cfg = config_with_defaults();
        cfg.session.db_path = db_path.to_string_lossy().to_string();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_session_db_path_parent_not_directory() {
        let dir = tmp_dir();
        let file_path = dir.path().join("somefile.txt");
        std::fs::write(&file_path, "data").unwrap();
        let db_path = file_path.join("sessions.db");
        let mut cfg = config_with_defaults();
        cfg.session.db_path = db_path.to_string_lossy().to_string();
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not a directory"));
    }

    #[test]
    fn validate_session_db_path_empty_is_ok() {
        let mut cfg = config_with_defaults();
        cfg.session.db_path = String::new();
        assert!(cfg.validate().is_ok());
    }

    // ── Test: agent.max_turns validation ─────────────────────────────

    #[test]
    fn validate_rejects_max_turns_zero() {
        let mut cfg = config_with_defaults();
        cfg.agent.max_turns = 0;
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("max_turns"));
    }

    #[test]
    fn validate_rejects_max_turns_over_500() {
        let mut cfg = config_with_defaults();
        cfg.agent.max_turns = 501;
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("500"));
    }

    #[test]
    fn validate_accepts_max_turns_500() {
        let mut cfg = config_with_defaults();
        cfg.agent.max_turns = 500;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_accepts_max_turns_1() {
        let mut cfg = config_with_defaults();
        cfg.agent.max_turns = 1;
        assert!(cfg.validate().is_ok());
    }

    // ── Test: agent.default_model validation ─────────────────────────

    #[test]
    fn validate_rejects_empty_default_model() {
        let mut cfg = config_with_defaults();
        cfg.agent.default_model = String::new();
        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("default_model"));
    }

    #[test]
    fn validate_accepts_non_empty_default_model() {
        let mut cfg = config_with_defaults();
        cfg.agent.default_model = "gpt-4".to_string();
        assert!(cfg.validate().is_ok());
    }

    // ── Test: all errors collected at once ───────────────────────────

    #[test]
    fn validate_collects_all_errors() {
        let mut cfg = config_with_defaults();
        cfg.provider = "nonexistent".to_string();
        cfg.agent.max_turns = 0;
        cfg.agent.default_model = String::new();
        // Also add a bad provider.
        cfg.providers.insert(
            "bad".to_string(),
            ProviderConfig {
                name: "Bad".to_string(),
                api_base: "http://localhost".to_string(),
                api_key_env: None,
                default_model: "bad-model".to_string(),
                api_key_value: None,
            },
        );

        let result = cfg.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Should contain error about nonexistent provider.
        assert!(err.contains("nonexistent"));
        // Should contain error about max_turns.
        assert!(err.contains("max_turns"));
        // Should contain error about default_model.
        assert!(err.contains("default_model"));
        // Should contain error about bad provider's api_key_env.
        assert!(err.contains("bad"));
    }

    // ── Test: env var overrides ──────────────────────────────────────

    #[test]
    fn env_override_provider() {
        unsafe { std::env::set_var("PI_PROVIDER", "glm") };
        let mut cfg = config_with_defaults();
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_PROVIDER") };
        assert_eq!(cfg.provider, "glm");
    }

    #[test]
    fn env_override_max_turns() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("PI_MAX_TURNS", "42") };
        let mut cfg = config_with_defaults();
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_MAX_TURNS") };
        assert_eq!(cfg.agent.max_turns, 42);
    }

    #[test]
    fn env_override_max_turns_invalid_ignored() {
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe { std::env::set_var("PI_MAX_TURNS", "not-a-number") };
        let mut cfg = config_with_defaults();
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_MAX_TURNS") };
        assert_eq!(cfg.agent.max_turns, 50); // default preserved
    }

    #[test]
    fn env_override_log_level() {
        unsafe { std::env::set_var("PI_LOG_LEVEL", "trace") };
        let mut cfg = config_with_defaults();
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_LOG_LEVEL") };
        assert_eq!(cfg.logging.level, "trace");
    }

    #[test]
    fn env_override_session_db() {
        unsafe { std::env::set_var("PI_SESSION_DB", "/tmp/test-sessions.db") };
        let mut cfg = config_with_defaults();
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_SESSION_DB") };
        assert_eq!(cfg.session.db_path, "/tmp/test-sessions.db");
    }

    #[test]
    fn env_override_deepseek_key() {
        let mut cfg = config_with_defaults();
        unsafe { std::env::set_var("PI_DEEPSEEK_API_KEY", "sk-test-123") };
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_DEEPSEEK_API_KEY") };
        let ds = cfg.providers.get("deepseek").unwrap();
        assert_eq!(ds.api_key_resolved(), Some("sk-test-123".to_string()));
    }

    #[test]
    fn env_override_glm_key() {
        let mut cfg = config_with_defaults();
        unsafe { std::env::set_var("PI_GLM_API_KEY", "glm-test-456") };
        PiConfig::apply_env_overrides(&mut cfg);
        unsafe { std::env::remove_var("PI_GLM_API_KEY") };
        let glm = cfg.providers.get("glm").unwrap();
        assert_eq!(glm.api_key_resolved(), Some("glm-test-456".to_string()));
    }

    // ── Test: load from file ─────────────────────────────────────────

    #[test]
    fn load_merges_file_over_defaults() {
        let dir = tmp_dir();
        let config_path = write_config(
            dir.path(),
            "config.toml",
            r#"
provider = "glm"

[agent]
max_turns = 25

[providers.deepseek]
name = "DeepSeek"
api_base = "https://api.deepseek.com/v1"
api_key_env = "none"
default_model = "deepseek-chat"

[providers.glm]
name = "GLM"
api_base = "https://open.bigmodel.cn/api/paas/v4"
api_key_env = "none"
default_model = "glm-4-flash"
"#,
        );

        let cfg = PiConfig::load(Some(&config_path)).unwrap();
        assert_eq!(cfg.provider, "glm");
        assert_eq!(cfg.agent.max_turns, 25);
        // Defaults should still be present for unset fields.
        assert!(cfg.providers.contains_key("deepseek"));
    }

    #[test]
    fn load_with_missing_file_uses_defaults() {
        unsafe {
            std::env::set_var("PI_DEEPSEEK_API_KEY", "sk-test");
            std::env::set_var("PI_GLM_API_KEY", "glm-test");
        }
        let dir = tmp_dir();
        let nonexistent = dir.path().join("does_not_exist.toml");
        let cfg = PiConfig::load(Some(&nonexistent)).unwrap();
        unsafe {
            std::env::remove_var("PI_DEEPSEEK_API_KEY");
            std::env::remove_var("PI_GLM_API_KEY");
        }
        assert_eq!(cfg.provider, "deepseek"); // default
        assert_eq!(cfg.agent.max_turns, 50); // default
    }

    // ── Test: generate_default_toml produces valid TOML ──────────────

    #[test]
    fn generated_toml_is_parseable() {
        let toml_str = generate_default_toml();
        let parsed: Result<PiConfig, _> = toml::from_str(&toml_str);
        assert!(parsed.is_ok(), "Generated TOML should be parseable");
    }

    // ── Test: warnings ───────────────────────────────────────────────

    #[test]
    fn warnings_for_missing_bridge_path() {
        let mut cfg = config_with_defaults();
        cfg.bridge.ts_bridge_path = "/nonexistent/bridge".to_string();
        let warnings = cfg.warnings();
        assert!(!warnings.is_empty());
        assert!(warnings[0].contains("/nonexistent/bridge"));
    }

    #[test]
    fn no_warnings_when_bridge_is_none() {
        let mut cfg = config_with_defaults();
        cfg.bridge.ts_bridge_path = "none".to_string();
        let warnings = cfg.warnings();
        assert!(warnings.is_empty());
    }

    #[test]
    fn no_warnings_when_bridge_is_empty() {
        let cfg = config_with_defaults();
        let warnings = cfg.warnings();
        assert!(warnings.is_empty());
    }

    // ── Test: to_agent_config ────────────────────────────────────────

    #[test]
    fn to_agent_config_derives_correctly() {
        let mut cfg = config_with_defaults();
        cfg.agent.default_model = "test-model".to_string();
        cfg.agent.max_turns = 30;
        cfg.session.db_path = "/tmp/test.db".to_string();

        let agent_cfg = cfg.to_agent_config();
        assert_eq!(agent_cfg.model, "test-model");
        assert_eq!(agent_cfg.max_turns, 30);
        assert_eq!(agent_cfg.db_path, "/tmp/test.db");
        assert_eq!(agent_cfg.context_window_tokens, 200_000);
        assert_eq!(agent_cfg.max_subagents, 8);
    }

    // ── Test: ProviderConfig api_key resolution ──────────────────────

    #[test]
    fn provider_config_api_key_resolved_from_env() {
        unsafe { std::env::set_var("PI_TEST_FROM_ENV", "env-key") };
        let provider = ProviderConfig {
            name: "Test".to_string(),
            api_base: "http://localhost".to_string(),
            api_key_env: Some("PI_TEST_FROM_ENV".to_string()),
            default_model: "test".to_string(),
            api_key_value: None,
        };
        assert_eq!(provider.api_key_resolved(), Some("env-key".to_string()));
        unsafe { std::env::remove_var("PI_TEST_FROM_ENV") };
    }

    #[test]
    fn provider_config_api_key_resolved_from_override() {
        let mut provider = ProviderConfig {
            name: "Test".to_string(),
            api_base: "http://localhost".to_string(),
            api_key_env: Some("PI_NONEXISTENT_OVERRIDE".to_string()),
            default_model: "test".to_string(),
            api_key_value: None,
        };
        provider.set_api_key("override-key".to_string());
        // Override takes precedence.
        assert_eq!(
            provider.api_key_resolved(),
            Some("override-key".to_string())
        );
    }

    #[test]
    fn provider_config_api_key_none_when_not_set() {
        let provider = ProviderConfig {
            name: "Test".to_string(),
            api_base: "http://localhost".to_string(),
            api_key_env: Some("PI_NONEXISTENT_VAR_AAAA".to_string()),
            default_model: "test".to_string(),
            api_key_value: None,
        };
        assert_eq!(provider.api_key_resolved(), None);
    }
}
