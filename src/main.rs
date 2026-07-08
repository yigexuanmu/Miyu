mod agent;
mod alarm;
mod cli;
mod clipboard;
mod config;
mod config_tui;
mod default_kb;
mod default_models;
mod diff_config;
mod diff_display;
mod i18n;
mod llm;
mod memory;
mod models_cache;
mod paths;
mod plugin;
mod plugin_manager;
mod prompts;
mod render;
mod shell;
mod state;
mod tools;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::parse();
    cli::run(cli).await
}
