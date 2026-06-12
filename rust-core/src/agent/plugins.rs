//! Unified Plugin System — Native, TypeScript, and Python backends.
//!
//! Defines the `Plugin` trait and `PluginRegistry` with support for:
//! - Native Rust plugins
//! - TypeScript plugins (via ts-bridge)
//! - Python plugins (via py-extensions)
//!
//! Features:
//! - Plugin registry: register, list, call by name
//! - Dynamic discovery: scan directories for plugins
//! - Fallback chain: try Native → TS → Python

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

// ─── Plugin Backend Type ────────────────────────────────────────────────────────

/// Which backend a plugin is implemented in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginBackend {
    Native,
    TypeScript,
    Python,
}

impl std::fmt::Display for PluginBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginBackend::Native => write!(f, "Native"),
            PluginBackend::TypeScript => write!(f, "TypeScript"),
            PluginBackend::Python => write!(f, "Python"),
        }
    }
}

// ─── Plugin Info ────────────────────────────────────────────────────────────────

/// Metadata about a loaded plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub description: String,
    pub backend: PluginBackend,
    pub version: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub source_path: Option<String>,
}

// ─── Plugin Trait ───────────────────────────────────────────────────────────────

/// The core Plugin trait — every plugin must implement this.
pub trait Plugin: Send + Sync {
    /// The unique name of the plugin.
    fn name(&self) -> &str;

    /// A human-readable description.
    fn description(&self) -> &str;

    /// Which backend this plugin uses.
    fn backend(&self) -> PluginBackend;

    /// Version string.
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// Execute the plugin with JSON arguments, returning a JSON result.
    fn call(&self, args: Value) -> anyhow::Result<Value>;

    /// Whether this plugin is currently enabled.
    fn enabled(&self) -> bool {
        true
    }

    /// Get the full PluginInfo for this plugin.
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name().to_string(),
            description: self.description().to_string(),
            backend: self.backend(),
            version: self.version().to_string(),
            enabled: self.enabled(),
            source_path: None,
        }
    }
}

// ─── Native Plugin Example ──────────────────────────────────────────────────────

/// A native Rust plugin (demonstration).
pub struct NativePlugin {
    info: PluginInfo,
    handler: Box<dyn Fn(Value) -> anyhow::Result<Value> + Send + Sync>,
}

impl NativePlugin {
    pub fn new(
        name: &str,
        description: &str,
        handler: impl Fn(Value) -> anyhow::Result<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            info: PluginInfo {
                name: name.to_string(),
                description: description.to_string(),
                backend: PluginBackend::Native,
                version: "0.1.0".to_string(),
                enabled: true,
                source_path: None,
            },
            handler: Box::new(handler),
        }
    }
}

impl Plugin for NativePlugin {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn description(&self) -> &str {
        &self.info.description
    }

    fn backend(&self) -> PluginBackend {
        PluginBackend::Native
    }

    fn call(&self, args: Value) -> anyhow::Result<Value> {
        (self.handler)(args)
    }

    fn info(&self) -> PluginInfo {
        self.info.clone()
    }
}

// ─── TypeScript Plugin Wrapper ──────────────────────────────────────────────────

/// A plugin backed by a TypeScript bridge call.
///
/// In production this would hold a handle to the ts-bridge process;
/// for now we use a mock callback or a function pointer.
pub struct TsPlugin {
    info: PluginInfo,
    call_fn: Box<dyn Fn(Value) -> anyhow::Result<Value> + Send + Sync>,
}

impl TsPlugin {
    pub fn new(
        name: &str,
        description: &str,
        call_fn: impl Fn(Value) -> anyhow::Result<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            info: PluginInfo {
                name: name.to_string(),
                description: description.to_string(),
                backend: PluginBackend::TypeScript,
                version: "0.1.0".to_string(),
                enabled: true,
                source_path: None,
            },
            call_fn: Box::new(call_fn),
        }
    }
}

impl Plugin for TsPlugin {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn description(&self) -> &str {
        &self.info.description
    }

    fn backend(&self) -> PluginBackend {
        PluginBackend::TypeScript
    }

    fn call(&self, args: Value) -> anyhow::Result<Value> {
        (self.call_fn)(args)
    }

    fn info(&self) -> PluginInfo {
        self.info.clone()
    }
}

// ─── Python Plugin Wrapper ──────────────────────────────────────────────────────

/// A plugin backed by a Python extension call.
pub struct PyPlugin {
    info: PluginInfo,
    call_fn: Box<dyn Fn(Value) -> anyhow::Result<Value> + Send + Sync>,
}

impl PyPlugin {
    pub fn new(
        name: &str,
        description: &str,
        call_fn: impl Fn(Value) -> anyhow::Result<Value> + Send + Sync + 'static,
    ) -> Self {
        Self {
            info: PluginInfo {
                name: name.to_string(),
                description: description.to_string(),
                backend: PluginBackend::Python,
                version: "0.1.0".to_string(),
                enabled: true,
                source_path: None,
            },
            call_fn: Box::new(call_fn),
        }
    }
}

impl Plugin for PyPlugin {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn description(&self) -> &str {
        &self.info.description
    }

    fn backend(&self) -> PluginBackend {
        PluginBackend::Python
    }

    fn call(&self, args: Value) -> anyhow::Result<Value> {
        (self.call_fn)(args)
    }

    fn info(&self) -> PluginInfo {
        self.info.clone()
    }
}

// ─── Plugin Registry ────────────────────────────────────────────────────────────

/// Registry managing all loaded plugins.
pub struct PluginRegistry {
    /// All registered plugins, keyed by name.
    plugins: HashMap<String, Arc<dyn Plugin>>,
    /// Scan directories for auto-discovery.
    scan_dirs: Vec<PathBuf>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            scan_dirs: Vec::new(),
        }
    }

    /// Register a plugin with the registry.
    pub fn register(&mut self, plugin: Arc<dyn Plugin>) {
        let name = plugin.name().to_string();
        self.plugins.insert(name, plugin);
    }

    /// Register a boxed plugin.
    pub fn register_boxed(&mut self, plugin: Box<dyn Plugin>) {
        let arc: Arc<dyn Plugin> = Arc::from(plugin);
        self.register(arc);
    }

    /// List all registered plugins with their metadata.
    pub fn list(&self) -> Vec<PluginInfo> {
        self.plugins.values().map(|p| p.info()).collect()
    }

    /// List enabled plugins.
    pub fn list_enabled(&self) -> Vec<PluginInfo> {
        self.plugins
            .values()
            .filter(|p| p.enabled())
            .map(|p| p.info())
            .collect()
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Plugin>> {
        self.plugins.get(name)
    }

    /// Call a plugin by name with JSON arguments, using fallback chain.
    ///
    /// The fallback chain tries: Native → TypeScript → Python plugins.
    pub fn call(&self, name: &str, args: Value) -> anyhow::Result<Value> {
        // Try exact match first
        if let Some(plugin) = self.plugins.get(name) {
            return plugin.call(args);
        }

        // Try fallback chain: look for any plugin matching the name
        // across backends in priority order
        for backend in &[
            PluginBackend::Native,
            PluginBackend::TypeScript,
            PluginBackend::Python,
        ] {
            for plugin in self.plugins.values() {
                if plugin.backend() == *backend && plugin.name() == name {
                    return plugin.call(args);
                }
            }
        }

        anyhow::bail!("Plugin '{name}' not found in registry")
    }

    /// Call a plugin, trying fallback if the primary backend fails.
    pub fn call_with_fallback(
        &self,
        name: &str,
        args: Value,
        preferred_backend: PluginBackend,
    ) -> anyhow::Result<Value> {
        // Try preferred backend first
        for plugin in self.plugins.values() {
            if plugin.name() == name && plugin.backend() == preferred_backend {
                return plugin.call(args);
            }
        }

        // Fallback to other backends
        let fallback_order: Vec<PluginBackend> = match preferred_backend {
            PluginBackend::Native => vec![PluginBackend::TypeScript, PluginBackend::Python],
            PluginBackend::TypeScript => vec![PluginBackend::Native, PluginBackend::Python],
            PluginBackend::Python => vec![PluginBackend::Native, PluginBackend::TypeScript],
        };

        for backend in &fallback_order {
            for plugin in self.plugins.values() {
                if plugin.name() == name && plugin.backend() == *backend {
                    return plugin.call(args);
                }
            }
        }

        anyhow::bail!("Plugin '{name}' not found with fallback from {preferred_backend}")
    }

    /// Unregister a plugin.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn Plugin>> {
        self.plugins.remove(name)
    }

    /// Add a directory to scan for plugins.
    pub fn add_scan_dir(&mut self, dir: PathBuf) {
        self.scan_dirs.push(dir);
    }

    /// Get the scan directories.
    pub fn scan_dirs(&self) -> &[PathBuf] {
        &self.scan_dirs
    }

    /// Scan directories for plugins (stub — real implementation would look for
    /// plugin manifests or shared libraries).
    pub fn discover(&mut self) -> anyhow::Result<usize> {
        let mut count = 0;

        for dir in &self.scan_dirs.clone() {
            if !dir.exists() {
                continue;
            }

            // Look for plugin directories containing a plugin.toml manifest
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() {
                        let manifest = entry_path.join("plugin.toml");
                        if manifest.exists()
                            && let Ok(plugin) = self.load_plugin_from_manifest(&manifest)
                        {
                            self.register_boxed(plugin);
                            count += 1;
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    /// Load a plugin from a plugin.toml manifest file.
    fn load_plugin_from_manifest(&self, path: &Path) -> anyhow::Result<Box<dyn Plugin>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest: {}", path.display()))?;
        let manifest: PluginManifest = toml::from_str(&content)
            .with_context(|| format!("Failed to parse manifest: {}", path.display()))?;

        let info = PluginInfo {
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            backend: manifest.backend,
            version: manifest.version.unwrap_or_else(|| "0.1.0".to_string()),
            enabled: true,
            source_path: Some(path.to_string_lossy().to_string()),
        };

        // Create plugin based on backend type
        match manifest.backend {
            PluginBackend::Native => {
                let info_clone = info.clone();
                Ok(Box::new(NativePlugin {
                    info,
                    handler: Box::new(move |args| {
                        Ok(serde_json::json!({
                            "plugin": info_clone.name,
                            "result": "native stub",
                            "args": args
                        }))
                    }),
                }))
            }
            PluginBackend::TypeScript => {
                let info_clone = info.clone();
                Ok(Box::new(TsPlugin {
                    info,
                    call_fn: Box::new(move |args| {
                        Ok(serde_json::json!({
                            "plugin": info_clone.name,
                            "result": "typescript stub — bridge call would go here",
                            "args": args
                        }))
                    }),
                }))
            }
            PluginBackend::Python => {
                let info_clone = info.clone();
                Ok(Box::new(PyPlugin {
                    info,
                    call_fn: Box::new(move |args| {
                        Ok(serde_json::json!({
                            "plugin": info_clone.name,
                            "result": "python stub — pyo3 call would go here",
                            "args": args
                        }))
                    }),
                }))
            }
        }
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRegistry")
            .field("plugins_count", &self.plugins.len())
            .field("scan_dirs", &self.scan_dirs)
            .finish()
    }
}

// ─── Plugin Manifest ────────────────────────────────────────────────────────────

/// Plugin manifest format (plugin.toml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub description: String,
    pub backend: PluginBackend,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub entry_point: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_native(name: &str) -> Box<dyn Plugin> {
        let name_owned = name.to_string();
        let name_for_closure = name_owned.clone();
        Box::new(NativePlugin::new(
            &name_owned,
            &format!("Native plugin: {name_owned}"),
            move |args| {
                Ok(serde_json::json!({
                    "plugin": name_for_closure,
                    "called": true,
                    "args": args
                }))
            },
        ))
    }

    fn make_ts(name: &str) -> Box<dyn Plugin> {
        let name_owned = name.to_string();
        let name_for_closure = name_owned.clone();
        Box::new(TsPlugin::new(
            &name_owned,
            &format!("TS plugin: {name_owned}"),
            move |args| {
                Ok(serde_json::json!({
                    "plugin": name_for_closure,
                    "bridge": "ts",
                    "args": args
                }))
            },
        ))
    }

    fn make_py(name: &str) -> Box<dyn Plugin> {
        let name_owned = name.to_string();
        let name_for_closure = name_owned.clone();
        Box::new(PyPlugin::new(
            &name_owned,
            &format!("Python plugin: {name_owned}"),
            move |args| {
                Ok(serde_json::json!({
                    "plugin": name_for_closure,
                    "bridge": "py",
                    "args": args
                }))
            },
        ))
    }

    #[test]
    fn test_plugin_registry_register_and_list() {
        let mut registry = PluginRegistry::new();
        assert!(registry.is_empty());

        registry.register_boxed(make_native("hello"));
        registry.register_boxed(make_ts("ts_greet"));
        registry.register_boxed(make_py("py_analyze"));

        assert_eq!(registry.len(), 3);

        let plugins = registry.list();
        assert_eq!(plugins.len(), 3);

        let names: Vec<String> = plugins.into_iter().map(|p| p.name).collect();
        assert!(names.contains(&"hello".to_string()));
        assert!(names.contains(&"ts_greet".to_string()));
        assert!(names.contains(&"py_analyze".to_string()));
    }

    #[test]
    fn test_plugin_call_exact_match() {
        let mut registry = PluginRegistry::new();
        registry.register_boxed(make_native("hello"));

        let result = registry
            .call("hello", serde_json::json!({"key": "val"}))
            .unwrap();
        assert_eq!(result["called"], true);
        assert_eq!(result["args"]["key"], "val");
    }

    #[test]
    fn test_plugin_call_not_found() {
        let registry = PluginRegistry::new();
        let result = registry.call("nonexistent", Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn test_fallback_chain() {
        let mut registry = PluginRegistry::new();
        // Register only a TS version of "analyze"
        registry.register_boxed(make_ts("analyze"));

        // Try calling with Native preferred — should fallback to TS
        let result = registry
            .call_with_fallback("analyze", Value::Null, PluginBackend::Native)
            .unwrap();
        assert_eq!(result["bridge"], "ts");
    }

    #[test]
    fn test_plugin_info_metadata() {
        let plugin = make_native("test");
        let info = plugin.info();

        assert_eq!(info.name, "test");
        assert_eq!(info.backend, PluginBackend::Native);
        assert!(info.enabled);
    }

    #[test]
    fn test_plugin_backend_display() {
        assert_eq!(PluginBackend::Native.to_string(), "Native");
        assert_eq!(PluginBackend::TypeScript.to_string(), "TypeScript");
        assert_eq!(PluginBackend::Python.to_string(), "Python");
    }

    #[test]
    fn test_unregister_plugin() {
        let mut registry = PluginRegistry::new();
        registry.register_boxed(make_native("temp"));

        assert_eq!(registry.len(), 1);
        let removed = registry.unregister("temp");
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_plugin_manifest_serialization() {
        let manifest = PluginManifest {
            name: "test_plugin".to_string(),
            description: "A test plugin".to_string(),
            backend: PluginBackend::Python,
            version: Some("1.0.0".to_string()),
            entry_point: Some("main.py".to_string()),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("test_plugin"));
        assert!(json.contains("Python"));
    }

    #[test]
    fn test_list_enabled() {
        let mut registry = PluginRegistry::new();
        registry.register_boxed(make_native("a"));
        registry.register_boxed(make_ts("b"));

        let enabled = registry.list_enabled();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().all(|p| p.enabled));
    }
}
