use crate::config::{AppConfig, DynamicPluginConfig};
use crate::plugin::registry::PluginRegistry;
use crate::plugin::builtin;
use anyhow::Result;
use std::collections::HashMap;

pub struct PluginManager {
    registry: PluginRegistry,
    config: HashMap<String, DynamicPluginConfig>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            registry: PluginRegistry::new(),
            config: HashMap::new(),
        }
    }

    pub fn init(&mut self, config: &AppConfig) -> Result<()> {
        builtin::register_builtin_plugins(&mut self.registry);
        
        self.config = config.plugins.dynamic.clone();
        
        for (id, plugin_config) in &self.config {
            if let Some(plugin) = self.registry.get_plugin(id) {
                let mut plugin = plugin.write().unwrap();
                plugin.set_enabled(plugin_config.enabled);
                for (key, value) in &plugin_config.config {
                    plugin.set_config_field(key, value)?;
                }
            }
        }
        
        Ok(())
    }

    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }

    pub fn sync_to_config(&self, config: &mut AppConfig) {
        config.plugins.dynamic = self.registry
            .save_states()
            .into_iter()
            .map(|(id, state)| {
                let plugin_config = DynamicPluginConfig {
                    enabled: state.enabled,
                    config: state.config,
                };
                (id, plugin_config)
            })
            .collect();
    }

    pub fn register_plugin(&mut self, plugin: impl crate::plugin::Plugin + 'static) {
        self.registry.register(plugin);
    }

    pub fn enable_plugin(&mut self, id: &str, enabled: bool) -> Result<()> {
        self.registry.set_enabled(id, enabled)
    }

    pub fn update_plugin_config(&mut self, id: &str, field: &str, value: &str) -> Result<()> {
        self.registry.update_config(id, field, value)
    }

    pub fn list_plugins(&self) -> Vec<crate::plugin::PluginMetadata> {
        self.registry.list_plugins()
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.registry.is_enabled(id)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
