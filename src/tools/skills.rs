use super::{ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::i18n::{agent_text as t, text as ui_text};
use crate::paths::MiyuPaths;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
struct SkillEntry {
    name: String,
    description: String,
}

pub fn register_skills(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &MiyuPaths,
) -> Result<()> {
    let entries = discover_skills(config, paths)?;
    register_load_skill(registry, config, paths, &entries);
    Ok(())
}

fn discover_skills(config: &AppConfig, paths: &MiyuPaths) -> Result<Vec<SkillEntry>> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for skills_dir in skill_search_dirs(config, paths) {
        if !skills_dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let skill_dir = entry.path();
            if skill_dir.join(".disabled").exists() {
                continue;
            }
            let skill_file = skill_dir.join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }
            let raw = std::fs::read_to_string(&skill_file)?;
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let Some(skill) = skill_entry(&raw, &dir_name, &skill_file) else {
                continue;
            };
            if !seen.insert(skill.name.clone()) {
                continue;
            }
            entries.push(skill);
        }
    }
    Ok(entries)
}

fn register_load_skill(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &MiyuPaths,
    entries: &[SkillEntry],
) {
    let skill_dirs = skill_search_dirs(config, paths);
    let description = format!(
        "{}\n\n{}\n\n{}",
        t(
            "Load a specialized skill's full instructions and resources into the conversation. The skill name must match one of the available skills listed below.",
            "加载指定技能的完整指令和资源到当前对话。技能名称必须匹配下方列出的可用技能之一。",
        ),
        t(
            "Use this tool before applying a skill or using any scripts/resources from that skill. Do not use skill scripts directly before loading the skill.",
            "应用 skill 或使用其中的脚本/资源前，必须先使用此工具加载该 skill。不要在加载前直接使用 skill 脚本。",
        ),
        available_skills_xml(entries),
    );
    registry.register(ToolSpec::new(
        "load_skill",
        description,
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": t("The name of the skill from the available skills list.", "可用技能列表中的技能名称。")
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
        move |args| {
            let skill_dirs = skill_dirs.clone();
            async move { load_skill(args, &skill_dirs) }
        },
    ));
}

fn load_skill(args: Value, skill_dirs: &[PathBuf]) -> Result<String> {
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.is_empty() {
        anyhow::bail!("skill name is required");
    }
    for dir in skill_dirs {
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let skill_dir = entry.path();
            if skill_dir.join(".disabled").exists() {
                continue;
            }
            let skill_file = skill_dir.join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }
            let raw = std::fs::read_to_string(&skill_file)?;
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let Some(skill) = skill_entry(&raw, &dir_name, &skill_file) else {
                continue;
            };
            if skill.name != name {
                continue;
            }
            let body = strip_frontmatter(&raw);
            let base_dir = skill_dir.display().to_string();
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&skill_dir) {
                for file_entry in entries.flatten() {
                    let fname = file_entry.file_name().to_string_lossy().to_string();
                    if fname == "SKILL.md" || fname.starts_with('.') {
                        continue;
                    }
                    files.push(file_entry.path().display().to_string());
                }
            }
            files.sort();
            let files_xml = if files.is_empty() {
                String::new()
            } else {
                let items = files
                    .iter()
                    .map(|f| format!("<file>{}</file>", xml_escape(f)))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("\n<skill_files>\n{items}\n</skill_files>")
            };
            return Ok(format!(
                "<skill_content name=\"{}\">\n<skill_instructions format=\"markdown\">\n{}\n</skill_instructions>\n\n<skill_base_dir>{}</skill_base_dir>\n{}\n</skill_content>",
                xml_escape(&name),
                xml_escape(&body),
                xml_escape(&base_dir),
                files_xml
            ));
        }
    }
    anyhow::bail!("skill not found: {name}");
}

fn skill_search_dirs(config: &AppConfig, paths: &MiyuPaths) -> Vec<PathBuf> {
    let mut dirs = vec![paths.skills_dir.clone()];
    let active = config.active_persona_skills_dir(paths);
    if active != paths.skills_dir {
        dirs.push(active);
    }
    dirs
}

fn skill_entry(raw: &str, dir_name: &str, skill_file: &std::path::Path) -> Option<SkillEntry> {
    let name = frontmatter_value(raw, "name").unwrap_or_default();
    let description = frontmatter_value(raw, "description").unwrap_or_default();
    if !valid_skill_name(&name) {
        eprintln!(
            "{} {}: {} `{}`",
            ui_text("warning: skipping skill", "警告：跳过 skill"),
            skill_file.display(),
            ui_text("invalid name", "无效名称"),
            name
        );
        return None;
    }
    if name != dir_name {
        eprintln!(
            "{} {}: {} `{}` {} `{}`",
            ui_text("warning: skipping skill", "警告：跳过 skill"),
            skill_file.display(),
            ui_text("name", "名称"),
            name,
            ui_text("does not match directory", "与目录不匹配"),
            dir_name
        );
        return None;
    }
    if !valid_skill_description(&description) {
        eprintln!(
            "{} {}: {}",
            ui_text("warning: skipping skill", "警告：跳过 skill"),
            skill_file.display(),
            ui_text(
                "description must be 1-1024 characters",
                "描述长度必须为 1-1024 个字符"
            )
        );
        return None;
    }
    Some(SkillEntry { name, description })
}

fn valid_skill_name(name: &str) -> bool {
    let len = name.chars().count();
    if !(1..=64).contains(&len) || name.starts_with('-') || name.ends_with('-') {
        return false;
    }
    let mut prev_hyphen = false;
    for ch in name.chars() {
        let valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !valid || (ch == '-' && prev_hyphen) {
            return false;
        }
        prev_hyphen = ch == '-';
    }
    true
}

fn valid_skill_description(description: &str) -> bool {
    let len = description.chars().count();
    (1..=1024).contains(&len)
}

fn available_skills_xml(entries: &[SkillEntry]) -> String {
    let items = entries
        .iter()
        .map(|entry| {
            format!(
                "  <skill>\n    <name>{}</name>\n    <description>{}</description>\n  </skill>",
                xml_escape(&entry.name),
                xml_escape(&entry.description)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("<available_skills>\n{items}\n</available_skills>")
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn frontmatter_value(raw: &str, key: &str) -> Option<String> {
    let mut lines = raw.lines();
    if lines.next()? != "---" {
        return None;
    }
    for line in lines {
        if line == "---" {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn strip_frontmatter(raw: &str) -> String {
    let mut lines = raw.lines();
    if lines.next() != Some("---") {
        return raw.to_string();
    }
    for line in lines.by_ref() {
        if line == "---" {
            return lines.collect::<Vec<_>>().join("\n");
        }
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(root: &std::path::Path) -> MiyuPaths {
        MiyuPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/miyu.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            scripts_dir: root.join("config/scripts"),
            system_scripts_dir: PathBuf::new(),
        }
    }

    #[test]
    fn load_skill_description_lists_only_name_and_description() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: gpu-passthrough\ndescription: GPU switching\n---\n\nUse `gpustoggle --status`.",
        )
        .unwrap();
        let config = AppConfig::default();
        let mut registry = ToolRegistry::new();
        register_skills(&mut registry, &config, &paths).unwrap();
        let description = &registry.get("load_skill").unwrap().description;
        assert!(description.contains("<available_skills>"));
        assert!(description.contains("<name>gpu-passthrough</name>"));
        assert!(description.contains("<description>GPU switching</description>"));
        assert!(!description.contains("gpustoggle --status"));
    }

    #[test]
    fn invalid_skill_names_are_skipped() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("BadName");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: BadName\ndescription: Bad skill\n---\n\nBody.",
        )
        .unwrap();
        let config = AppConfig::default();
        let entries = discover_skills(&config, &paths).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn skill_name_must_match_directory_name() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("gpu-passthrough");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: other-skill\ndescription: GPU switching\n---\n\nBody.",
        )
        .unwrap();
        let config = AppConfig::default();
        let entries = discover_skills(&config, &paths).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn load_skill_returns_base_dir_and_files() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let skill_dir = paths.skills_dir.join("web-search");
        std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: web-search\ndescription: Search the web\n---\n\nRun scripts/web-search.py.",
        )
        .unwrap();
        std::fs::write(skill_dir.join("scripts/web-search.py"), "print('ok')").unwrap();
        let config = AppConfig::default();
        let result = load_skill(
            json!({ "name": "web-search" }),
            &skill_search_dirs(&config, &paths),
        )
        .unwrap();
        assert!(result.contains("<skill_instructions format=\"markdown\">"));
        assert!(result.contains("<skill_base_dir>"));
        assert!(result.contains("<skill_files>"));
        assert!(result.contains("scripts"));
        assert!(!result.contains("Relative paths in this skill"));
    }
}
