use super::traits::{Plugin, PluginMetadata, PluginState};
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct PluginRegistry {
    plugins: Vec<Arc<RwLock<dyn Plugin>>>,
    states: HashMap<String, PluginState>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            states: HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: impl Plugin + 'static) {
        let metadata = plugin.metadata();
        let state = self
            .states
            .entry(metadata.id.to_string())
            .or_insert_with(PluginState::default);
        
        let plugin = Arc::new(RwLock::new(plugin));
        
        if state.enabled {
            plugin.write().unwrap().set_enabled(true);
        } else {
            plugin.write().unwrap().set_enabled(false);
        }
        
        self.plugins.push(plugin);
    }

    pub fn get_plugin(&self, id: &str) -> Option<Arc<RwLock<dyn Plugin>>> {
        self.plugins.iter().find(|p| {
            let meta = p.read().unwrap().metadata();
            meta.id == id
        }).cloned()
    }

    pub fn list_plugins(&self) -> Vec<PluginMetadata> {
        self.plugins
            .iter()
            .map(|p| p.read().unwrap().metadata())
            .collect()
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        if let Some(plugin) = self.get_plugin(id) {
            plugin.write().unwrap().set_enabled(enabled);
            self.states
                .entry(id.to_string())
                .or_insert_with(PluginState::default)
                .enabled = enabled;
            Ok(())
        } else {
            bail!("Plugin not found: {}", id)
        }
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.get_plugin(id)
            .map(|p| p.read().unwrap().enabled())
            .unwrap_or(false)
    }

    pub fn register_all_tools(&self, registry: &mut ToolRegistry) -> Result<()> {
        for plugin in &self.plugins {
            let plugin = plugin.read().unwrap();
            if plugin.enabled() {
                plugin.register_tools(registry)?;
            }
        }
        Ok(())
    }

    pub fn save_states(&self) -> HashMap<String, PluginState> {
        let mut states = self.states.clone();
        for plugin in &self.plugins {
            let plugin = plugin.read().unwrap();
            let metadata = plugin.metadata();
            let state = states
                .entry(metadata.id.to_string())
                .or_insert_with(PluginState::default);
            state.enabled = plugin.enabled();
        }
        states
    }

    pub fn load_states(&mut self, states: HashMap<String, PluginState>) {
        self.states = states;
        for plugin in &self.plugins {
            let metadata = plugin.read().unwrap().metadata();
            if let Some(state) = self.states.get(metadata.id) {
                plugin.write().unwrap().set_enabled(state.enabled);
            }
        }
    }

    pub fn update_config(&mut self, id: &str, field: &str, value: &str) -> Result<()> {
        if let Some(plugin) = self.get_plugin(id) {
            plugin.write().unwrap().set_config_field(field, value)?;
            let state = self
                .states
                .entry(id.to_string())
                .or_insert_with(PluginState::default);
            state.config.insert(field.to_string(), value.to_string());
            Ok(())
        } else {
            bail!("Plugin not found: {}", id)
        }
    }

    pub fn get_config(&self, id: &str) -> Option<HashMap<String, String>> {
        self.get_plugin(id)
            .map(|p| {
                let plugin = p.read().unwrap();
                plugin
                    .config_fields()
                    .iter()
                    .filter_map(|f| {
                        plugin
                            .get_config_field(&f.name)
                            .map(|v| (f.name.clone(), v))
                    })
                    .collect()
            })
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
