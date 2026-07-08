use crate::plugin::traits::{Plugin, PluginMetadata, PluginConfigField};
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Default)]
pub struct WebPlugin {
    enabled: bool,
    config: HashMap<String, String>,
}

impl Plugin for WebPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "web",
            name: "web",
            description: "web plugin",
            version: "1.0.0",
            author: Some("Miyu"),
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn register_tools(&self, _registry: &mut ToolRegistry) -> Result<()> {
        Ok(())
    }

    fn config_fields(&self) -> Vec<PluginConfigField> {
        vec![]
    }

    fn set_config_field(&mut self, _name: &str, _value: &str) -> Result<()> {
        Ok(())
    }

    fn get_config_field(&self, _name: &str) -> Option<String> {
        None
    }
}
