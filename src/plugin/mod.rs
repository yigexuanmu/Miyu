pub mod registry;
pub mod traits;
pub mod builtin;

pub use traits::{Plugin, PluginMetadata, PluginConfigField, ConfigFieldType};
pub use registry::PluginRegistry;
