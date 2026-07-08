pub mod weather;
pub mod web;
pub mod vision;
pub mod memes;
pub mod knowledge_base;
pub mod memory;
pub mod image_generation;
pub mod xuanxue;
pub mod archlinux;
pub mod man;
pub mod calculator;
pub mod hash_codec;
pub mod deep_research;
pub mod deep_diagnose;
pub mod exchange_rate;
pub mod print_image;
pub mod package_advisor;
pub mod diagnostics;
pub mod linux_game;

use super::registry::PluginRegistry;

pub fn register_builtin_plugins(registry: &mut PluginRegistry) {
    registry.register(weather::WeatherPlugin::default());
    registry.register(web::WebPlugin::default());
    registry.register(vision::VisionPlugin::default());
    registry.register(memes::MemesPlugin::default());
    registry.register(knowledge_base::KnowledgeBasePlugin::default());
    registry.register(memory::MemoryPlugin::default());
    registry.register(image_generation::ImageGenerationPlugin::default());
    registry.register(xuanxue::XuanxuePlugin::default());
    registry.register(archlinux::ArchlinuxPlugin::default());
    registry.register(man::ManPlugin::default());
    registry.register(calculator::CalculatorPlugin::default());
    registry.register(hash_codec::HashCodecPlugin::default());
    registry.register(deep_research::DeepResearchPlugin::default());
    registry.register(deep_diagnose::DeepDiagnosePlugin::default());
    registry.register(exchange_rate::ExchangeRatePlugin::default());
    registry.register(print_image::PrintImagePlugin::default());
    registry.register(package_advisor::PackageAdvisorPlugin::default());
    registry.register(diagnostics::DiagnosticsPlugin::default());
    registry.register(linux_game::LinuxGamePlugin::default());
}
