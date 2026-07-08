use crate::plugin::traits::{Plugin, PluginMetadata, PluginConfigField, ConfigFieldType};
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::collections::HashMap;

#[derive(Default)]
pub struct WeatherPlugin {
    enabled: bool,
    config: HashMap<String, String>,
}

impl WeatherPlugin {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            config: HashMap::new(),
        }
    }
}

impl Plugin for WeatherPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "weather",
            name: "天气",
            description: "天气查询",
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

    fn register_tools(&self, registry: &mut ToolRegistry) -> Result<()> {
        // 原有的天气工具注册逻辑
        // crate::tools::weather::register(registry);
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
