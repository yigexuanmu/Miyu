use crate::tools::ToolRegistry;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn register_tools(&self, registry: &mut ToolRegistry) -> Result<()>;
    fn config_fields(&self) -> Vec<PluginConfigField>;
    fn set_config_field(&mut self, name: &str, value: &str) -> Result<()>;
    fn get_config_field(&self, name: &str) -> Option<String>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub author: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginConfigField {
    pub name: String,
    pub label: String,
    pub field_type: ConfigFieldType,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConfigFieldType {
    Bool,
    Number,
    Text,
    Select(Vec<String>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginState {
    pub enabled: bool,
    pub config: HashMap<String, String>,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            enabled: true,
            config: HashMap::new(),
        }
    }
}
