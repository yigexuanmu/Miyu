use crate::config::DiffDisplayPluginConfig;
use std::sync::OnceLock;

static DIFF_CONFIG: OnceLock<DiffDisplayPluginConfig> = OnceLock::new();

pub fn init_diff_config(config: DiffDisplayPluginConfig) {
    let _ = DIFF_CONFIG.set(config);
}

pub fn get_diff_config() -> Option<&'static DiffDisplayPluginConfig> {
    DIFF_CONFIG.get()
}

pub fn get_diff_config_or_default() -> DiffDisplayPluginConfig {
    #[cfg(test)]
    {
        return DiffDisplayPluginConfig {
            enabled: false,
            ..Default::default()
        };
    }
    #[cfg(not(test))]
    {
        DIFF_CONFIG.get().cloned().unwrap_or_default()
    }
}
