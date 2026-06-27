mod archlinux;
mod calculator;
mod deep_research;
mod default_tools;
mod exchange_rate;
mod hash_codec;
mod image_generation;
pub mod knowledge_base;
mod man;
mod memory;
mod moegirl;
mod registry;
mod skills;
mod vision;
mod weather;
mod web;
mod web_images;
mod xuanxue;

use crate::config::AppConfig;
use crate::paths::MiyuPaths;

#[allow(unused_imports)]
pub use registry::{empty_parameters, ToolRegistry, ToolSpec};
pub use skills::{register_skills, skills_prompt};

pub fn builtin_registry(config: &AppConfig, paths: &MiyuPaths) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    default_tools::register(&mut registry, config.skills.allow_command_execution);
    web::register_fetch(&mut registry);
    weather::register(&mut registry);
    exchange_rate::register(&mut registry, config.plugins.exchange_rate.clone());
    xuanxue::register(&mut registry);
    archlinux::register(&mut registry);
    man::register(&mut registry);
    moegirl::register(&mut registry);
    hash_codec::register(&mut registry);
    calculator::register(&mut registry);
    vision::register_print(&mut registry, config.clone());
    if config.plugins.web.enabled {
        web::register(&mut registry, config.plugins.web.clone());
    }
    if config.plugins.web_images.enabled {
        web_images::register(&mut registry, config.clone(), paths.clone(), true);
    }
    if config.plugins.deep_research.enabled {
        let research_tools = registry.clone();
        deep_research::register(&mut registry, config.clone(), paths.clone(), research_tools);
    }
    if config.plugins.vision.enabled {
        vision::register(&mut registry, config.clone(), paths.clone(), true);
    }
    if config.plugins.image_generation.enabled {
        image_generation::register(&mut registry, config.clone());
    }
    if config.plugins.knowledge_base.enabled {
        knowledge_base::register(&mut registry, config.clone(), paths.clone());
    }
    if config.memory_config().enabled {
        memory::register(&mut registry, config.clone(), paths.clone());
    }
    registry
}

pub fn readonly_registry(config: &AppConfig, paths: &MiyuPaths) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    default_tools::register_readonly(&mut registry);
    web::register_fetch(&mut registry);
    archlinux::register(&mut registry);
    man::register(&mut registry);
    if config.plugins.web.enabled {
        web::register(&mut registry, config.plugins.web.clone());
    }
    if config.plugins.web_images.enabled {
        web_images::register(&mut registry, config.clone(), paths.clone(), false);
    }
    if config.plugins.knowledge_base.enabled {
        knowledge_base::register_readonly(&mut registry, config.clone(), paths.clone());
    }
    if config.memory_config().enabled {
        memory::register_readonly(&mut registry, config.clone(), paths.clone());
    }
    registry
}
