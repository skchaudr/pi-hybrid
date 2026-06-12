//! Model Provider Integration — Provider registry for LLM APIs.
//!
//! Defines ProviderConfig with name, api_base, api_key_env, default_model.
//! Built-in configs for DeepSeek and GLM/Zhipu.
//! Provider registry: list, get, add custom.
//! Wired into the agent bridge client so agent.run(prompt, provider="deepseek") works.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Configuration for an LLM API provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Human-readable name (e.g., "deepseek", "glm").
    pub name: String,
    /// Base URL for the API endpoint.
    pub api_base: String,
    /// Environment variable name for the API key.
    pub api_key_env: String,
    /// Default model to use when none is specified.
    pub default_model: String,
    /// Optional description.
    #[serde(default)]
    pub description: String,
}

impl ProviderConfig {
    /// Create a new provider config.
    pub fn new(
        name: impl Into<String>,
        api_base: impl Into<String>,
        api_key_env: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            api_base: api_base.into(),
            api_key_env: api_key_env.into(),
            default_model: default_model.into(),
            description: String::new(),
        }
    }

    /// Set a human-readable description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Get the API key from the environment.
    pub fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env).ok()
    }

    /// Check if the API key is configured.
    pub fn is_configured(&self) -> bool {
        self.api_key().is_some()
    }
}

/// Built-in provider: DeepSeek.
pub fn deepseek_config() -> ProviderConfig {
    ProviderConfig::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        "DEEPSEEK_API_KEY",
        "deepseek-chat",
    )
    .with_description("DeepSeek API — code generation and reasoning model")
}

/// Built-in provider: GLM / Zhipu AI.
pub fn glm_config() -> ProviderConfig {
    ProviderConfig::new(
        "glm",
        "https://open.bigmodel.cn/api/paas/v4",
        "GLM_API_KEY",
        "glm-4-flash",
    )
    .with_description("GLM/Zhipu AI — ChatGLM and CodeGeeX models")
}

/// A registry of all configured LLM providers.
#[derive(Debug, Default)]
pub struct ProviderRegistry {
    /// All registered providers, keyed by name.
    providers: HashMap<String, ProviderConfig>,
    /// The name of the currently active provider.
    active_provider: Option<String>,
}

impl ProviderRegistry {
    /// Create a new empty provider registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            active_provider: None,
        }
    }

    /// Create a provider registry with the built-in providers pre-registered.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtins();
        registry
    }

    /// Register the built-in DeepSeek and GLM providers.
    pub fn register_builtins(&mut self) {
        self.register(deepseek_config());
        self.register(glm_config());
    }

    /// Register a provider in the registry.
    pub fn register(&mut self, config: ProviderConfig) {
        let name = config.name.clone();
        self.providers.insert(name.clone(), config);
        // Set as active if this is the first one
        if self.active_provider.is_none() {
            self.active_provider = Some(name);
        }
    }

    /// Add a custom provider.
    pub fn add_custom(
        &mut self,
        name: impl Into<String>,
        api_base: impl Into<String>,
        api_key_env: impl Into<String>,
        default_model: impl Into<String>,
    ) {
        self.register(ProviderConfig::new(
            name,
            api_base,
            api_key_env,
            default_model,
        ));
    }

    /// Get a provider config by name.
    pub fn get(&self, name: &str) -> Option<&ProviderConfig> {
        self.providers.get(name)
    }

    /// List all registered providers.
    pub fn list(&self) -> Vec<&ProviderConfig> {
        let mut configs: Vec<&ProviderConfig> = self.providers.values().collect();
        configs.sort_by(|a, b| a.name.cmp(&b.name));
        configs
    }

    /// List provider names only.
    pub fn list_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.providers.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Get the currently active provider.
    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.active_provider
            .as_ref()
            .and_then(|name| self.providers.get(name))
    }

    /// Get the name of the active provider.
    pub fn active_provider_name(&self) -> Option<&str> {
        self.active_provider.as_deref()
    }

    /// Set the active provider by name.
    pub fn set_active(&mut self, name: &str) -> anyhow::Result<()> {
        if self.providers.contains_key(name) {
            self.active_provider = Some(name.to_string());
            Ok(())
        } else {
            anyhow::bail!("Provider '{name}' not found in registry")
        }
    }

    /// Resolve a provider name, falling back to the active provider, then to the first available.
    pub fn resolve(&self, provider_name: Option<&str>) -> Option<&ProviderConfig> {
        if let Some(name) = provider_name {
            self.get(name)
        } else {
            self.active_provider()
        }
    }

    /// Unregister a provider by name.
    pub fn unregister(&mut self, name: &str) -> Option<ProviderConfig> {
        if self.active_provider.as_deref() == Some(name) {
            self.active_provider = None;
            // Try to select another provider
            if let Some(next) = self.providers.keys().find(|k| *k != name) {
                self.active_provider = Some(next.clone());
            }
        }
        self.providers.remove(name)
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Check if a provider is configured (has its API key set).
    pub fn is_configured(&self, name: &str) -> bool {
        self.providers
            .get(name)
            .map(|c| c.is_configured())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_providers_configured() {
        let registry = ProviderRegistry::with_builtins();

        let deepseek = registry.get("deepseek").unwrap();
        assert_eq!(deepseek.name, "deepseek");
        assert_eq!(deepseek.api_base, "https://api.deepseek.com/v1");
        assert_eq!(deepseek.api_key_env, "DEEPSEEK_API_KEY");
        assert_eq!(deepseek.default_model, "deepseek-chat");

        let glm = registry.get("glm").unwrap();
        assert_eq!(glm.name, "glm");
        assert_eq!(glm.api_base, "https://open.bigmodel.cn/api/paas/v4");
        assert_eq!(glm.api_key_env, "GLM_API_KEY");
        assert_eq!(glm.default_model, "glm-4-flash");

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn custom_provider_registration() {
        let mut registry = ProviderRegistry::new();
        registry.register(ProviderConfig::new(
            "my-llm",
            "https://my-llm.example.com/v1",
            "MY_LLM_KEY",
            "my-model-v1",
        ));

        assert_eq!(registry.len(), 1);
        assert!(registry.get("my-llm").is_some());
        assert_eq!(registry.active_provider_name(), Some("my-llm"));
    }

    #[test]
    fn provider_resolution_and_switching() {
        let mut registry = ProviderRegistry::with_builtins();

        // Default active is first registered (deepseek)
        assert_eq!(registry.active_provider_name(), Some("deepseek"));

        // Switch to glm
        registry.set_active("glm").unwrap();
        assert_eq!(registry.active_provider_name(), Some("glm"));

        // Resolve with explicit name
        let provider = registry.resolve(Some("deepseek")).unwrap();
        assert_eq!(provider.name, "deepseek");

        // Resolve without name uses active
        let provider = registry.resolve(None).unwrap();
        assert_eq!(provider.name, "glm");

        // Unknown provider
        assert!(registry.get("unknown").is_none());
        assert!(registry.set_active("unknown").is_err());
    }

    #[test]
    fn api_key_detection() {
        let config = ProviderConfig::new("test", "https://test.com", "TEST_KEY", "test-model");
        // Without env var set, should not be configured
        assert!(!config.is_configured());

        // Set the env var
        unsafe {
            std::env::set_var("TEST_KEY", "sk-test-123");
        }
        assert!(config.is_configured());
        assert_eq!(config.api_key(), Some("sk-test-123".to_string()));

        // Clean up
        unsafe {
            std::env::remove_var("TEST_KEY");
        }
    }

    #[test]
    fn unregister_provider_falls_back() {
        let mut registry = ProviderRegistry::with_builtins();
        assert_eq!(registry.active_provider_name(), Some("deepseek"));

        // Unregister deepseek should fall back to glm
        let removed = registry.unregister("deepseek");
        assert!(removed.is_some());
        assert_eq!(registry.active_provider_name(), Some("glm"));
        assert_eq!(registry.len(), 1);
    }
}
