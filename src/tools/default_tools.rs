use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

const MAX_READ_BYTES: u64 = 512 * 1024;
const MAX_COMMAND_OUTPUT_CHARS: usize = 20_000;

pub fn register(registry: &mut ToolRegistry, allow_command_execution: bool) {
    register_readonly(registry);
    registry.register(ToolSpec::new(
        "run_command",
        t("Run a shell command in the workspace. Disabled unless skills.allow_command_execution is true.", "在工作区运行 shell 命令。除非 skills.allow_command_execution 为 true，否则禁用。"),
        json!({"type":"object","properties":{"command":{"type":"string","description": t("Command to run.", "要运行的命令。")},"timeout_seconds":{"type":"integer","description": t("Optional timeout in seconds.", "可选超时时间，单位秒。")}},"required":["command"],"additionalProperties":false}),
        move |args| async move { run_command(args, allow_command_execution).await },
    ));
    registry.register(ToolSpec::new(
        "task_agent",
        t("Create a focused subtask plan for a complex task. Current implementation returns a structured handoff prompt for the main agent.", "为复杂任务创建聚焦的子任务计划。当前实现会返回给主 agent 使用的结构化交接提示。"),
        json!({"type":"object","properties":{"description":{"type":"string","description": t("Short task description.", "简短任务描述。")},"prompt":{"type":"string","description": t("Detailed subtask prompt.", "详细子任务提示。")}},"required":["prompt"],"additionalProperties":false}),
        |args| async move { task_agent(args) },
    ));
}

pub fn register_readonly(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "inspect_system",
        t("Inspect read-only local system context. Use this first for OS, package manager, update, driver, shell, desktop environment, session, kernel, or host questions.", "检查只读本机系统上下文。遇到系统、包管理器、更新、驱动、shell、桌面环境、会话、内核或主机相关问题时优先使用。"),
        json!({"type":"object","properties":{},"additionalProperties":false}),
        |_| async move { inspect_system() },
    ));
    registry.register(ToolSpec::new(
        "read_file",
        t("Read a UTF-8 text file or list a directory in the local workspace. Use absolute paths or workspace-relative paths.", "读取 UTF-8 文本文件，或列出本地工作区目录。使用绝对路径或工作区相对路径。"),
        json!({"type":"object","properties":{"path":{"type":"string","description": t("File or directory path.", "文件或目录路径。")},"offset":{"type":"integer","description": t("Starting line, 1-based.", "起始行，1 起始。")},"limit":{"type":"integer","description": t("Maximum lines to read.", "最多读取行数。")}},"required":["path"],"additionalProperties":false}),
        |args| async move { read_file(args) },
    ));
    registry.register(ToolSpec::new(
        "find_files",
        t("Find files by filename pattern under a workspace directory. Similar to glob. Avoid broad system paths such as /.", "在工作区目录下按文件名模式查找文件，类似 glob。避免使用 / 等过宽系统路径。"),
        json!({"type":"object","properties":{"path":{"type":"string","description": t("Directory to search.", "搜索目录。")},"pattern":{"type":"string","description": t("Glob pattern.", "Glob 模式。")},"max_results":{"type":"integer","description": t("Maximum results.", "最多结果数。")}},"required":["pattern"],"additionalProperties":false}),
        |args| async move { find_files(args).await },
    ));
    registry.register(ToolSpec::new(
        "search_text",
        t("Search text in files using ripgrep under a workspace directory. Avoid broad system paths such as /.", "在工作区目录下用 ripgrep 搜索文件内容。避免使用 / 等过宽系统路径。"),
        json!({"type":"object","properties":{"path":{"type":"string","description": t("Directory to search.", "搜索目录。")},"pattern":{"type":"string","description": t("Regex or text pattern.", "正则或文本模式。")},"include":{"type":"string","description": t("Optional file glob filter.", "可选文件 glob 过滤。")},"max_results":{"type":"integer","description": t("Maximum results.", "最多结果数。")}},"required":["pattern"],"additionalProperties":false}),
        |args| async move { search_text(args).await },
    ));
}

fn inspect_system() -> Result<String> {
    let mut env = BTreeMap::new();
    for key in [
        "SHELL",
        "TERM",
        "LANG",
        "PATH",
        "XDG_CURRENT_DESKTOP",
        "XDG_SESSION_TYPE",
        "DESKTOP_SESSION",
        "WAYLAND_DISPLAY",
        "DISPLAY",
    ] {
        if let Ok(value) = std::env::var(key) {
            if !value.trim().is_empty() {
                env.insert(key, value);
            }
        }
    }
    let os_release = read_small_file("/etc/os-release");
    let arch_release = read_small_file("/etc/arch-release").is_some();
    let debian_version = read_small_file("/etc/debian_version");
    let fedora_release = read_small_file("/etc/fedora-release");
    let proc_version = read_small_file("/proc/version");
    let proc_cmdline = read_small_file("/proc/cmdline");
    let macos_system_version = read_small_file("/System/Library/CoreServices/SystemVersion.plist");
    let macos = parse_macos_system_version(macos_system_version.as_deref());
    let package_manager_guess = package_manager_guess(
        &os_release,
        arch_release,
        debian_version.is_some(),
        fedora_release.is_some(),
        macos_system_version.is_some(),
    );
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "platform": std::env::consts::OS,
        "os_release": os_release,
        "arch_release": arch_release,
        "debian_version": debian_version,
        "fedora_release": fedora_release,
        "macos": macos,
        "kernel_version": proc_version,
        "kernel_cmdline": proc_cmdline,
        "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS,
        "family": std::env::consts::FAMILY,
        "username": std::env::var("USER").ok().or_else(|| std::env::var("USERNAME").ok()),
        "hostname": read_small_file("/etc/hostname").map(|value| value.trim().to_string()),
        "env": env,
        "package_manager_guess": package_manager_guess,
        "notes": [
            "This tool is read-only and does not execute shell commands.",
            "Use this before answering OS/package-manager/update/driver/desktop questions."
        ],
    }))?)
}

fn read_small_file(path: &str) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 {
        return None;
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn package_manager_guess(
    os_release: &Option<String>,
    arch_release: bool,
    debian_version: bool,
    fedora_release: bool,
    macos: bool,
) -> Vec<&'static str> {
    let lower = os_release
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut managers = Vec::new();
    if arch_release || lower.contains("id=arch") || lower.contains("id_like=arch") {
        managers.push("pacman");
    }
    if debian_version
        || lower.contains("id=debian")
        || lower.contains("id=ubuntu")
        || lower.contains("id_like=debian")
    {
        managers.push("apt");
    }
    if fedora_release || lower.contains("id=fedora") || lower.contains("id_like=fedora") {
        managers.push("dnf");
    }
    if macos || std::env::consts::OS == "macos" {
        if Path::new("/opt/homebrew").exists() || Path::new("/usr/local/Homebrew").exists() {
            managers.push("brew");
        }
        if Path::new("/opt/local").exists() {
            managers.push("port");
        }
        if !managers
            .iter()
            .any(|manager| matches!(*manager, "brew" | "port"))
        {
            managers.push("brew");
        }
    }
    if managers.is_empty() {
        managers.push("unknown");
    }
    managers
}

fn parse_macos_system_version(raw: Option<&str>) -> Value {
    let Some(raw) = raw else {
        return Value::Null;
    };
    json!({
        "product_name": plist_value(raw, "ProductName"),
        "product_version": plist_value(raw, "ProductVersion"),
        "product_build_version": plist_value(raw, "ProductBuildVersion"),
    })
}

fn plist_value(raw: &str, key: &str) -> Option<String> {
    let marker = format!("<key>{key}</key>");
    let after_key = raw.split(&marker).nth(1)?;
    let after_string = after_key.split("<string>").nth(1)?;
    after_string
        .split("</string>")
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn read_file(args: Value) -> Result<String> {
    let path = path_arg(&args, "path")?;
    if path.is_dir() {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path)?.take(500) {
            let entry = entry?;
            let suffix = if entry.file_type()?.is_dir() { "/" } else { "" };
            entries.push(format!("{}{}", entry.file_name().to_string_lossy(), suffix));
        }
        entries.sort();
        return Ok(entries.join("\n"));
    }
    let metadata = std::fs::metadata(&path)?;
    if metadata.len() > MAX_READ_BYTES {
        bail!(
            "{}: {} {}",
            t("file too large to read directly", "文件过大，无法直接读取"),
            metadata.len(),
            t("bytes", "字节")
        );
    }
    let text = std::fs::read_to_string(&path)?;
    let offset = args
        .get("offset")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1) as usize;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(2000)
        .max(1) as usize;
    let lines = text
        .lines()
        .enumerate()
        .skip(offset.saturating_sub(1))
        .take(limit)
        .map(|(index, line)| format!("{}: {}", index + 1, line))
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

async fn find_files(args: Value) -> Result<String> {
    let path = optional_path(&args).unwrap_or(std::env::current_dir()?);
    ensure_safe_search_path(&path)?;
    let pattern = required(&args, "pattern")?;
    let max_results = max_results(&args);
    let output = Command::new("rg")
        .arg("--files")
        .arg("-g")
        .arg(pattern)
        .current_dir(path)
        .stdin(Stdio::null())
        .output()
        .await?;
    command_output_limited(output, max_results)
}

async fn search_text(args: Value) -> Result<String> {
    let path = optional_path(&args).unwrap_or(std::env::current_dir()?);
    ensure_safe_search_path(&path)?;
    let pattern = required(&args, "pattern")?;
    let max_results = max_results(&args);
    let mut command = Command::new("rg");
    command
        .arg("--line-number")
        .arg("--max-count")
        .arg(max_results.to_string())
        .arg(pattern);
    if let Some(include) = args
        .get("include")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        command.arg("-g").arg(include.trim());
    }
    let output = command
        .current_dir(path)
        .stdin(Stdio::null())
        .output()
        .await?;
    command_output_limited(output, max_results)
}

async fn run_command(args: Value, allowed: bool) -> Result<String> {
    if !allowed {
        bail!("{}", t("command execution is disabled; set skills.allow_command_execution=true in config.jsonc to enable run_command", "命令执行已禁用；请在 config.jsonc 中设置 skills.allow_command_execution=true 以启用 run_command"));
    }
    let command = required(&args, "command")?;
    let timeout = args
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .min(120);
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout),
        Command::new("sh")
            .arg("-lc")
            .arg(command)
            .stdin(Stdio::null())
            .output(),
    )
    .await??;
    command_output(output)
}

fn task_agent(args: Value) -> Result<String> {
    let prompt = required(&args, "prompt")?;
    let description = args
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("subtask");
    Ok(serde_json::to_string_pretty(
        &json!({"description": description, "prompt": prompt, "note": t("Subagent execution is not implemented yet; use this as a structured handoff.", "子 agent 执行尚未实现；请把它作为结构化交接内容使用。")}),
    )?)
}

fn command_output(output: std::process::Output) -> Result<String> {
    let stdout = clip_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = clip_output(&String::from_utf8_lossy(&output.stderr));
    Ok(serde_json::to_string_pretty(
        &json!({"success": output.status.success(), "exit_code": output.status.code(), "stdout": stdout, "stderr": stderr}),
    )?)
}

fn command_output_limited(output: std::process::Output, max_lines: usize) -> Result<String> {
    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout_raw
        .lines()
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n");
    let stderr = clip_output(&String::from_utf8_lossy(&output.stderr));
    let truncated = stdout_raw.lines().nth(max_lines).is_some();
    Ok(serde_json::to_string_pretty(&json!({
        "success": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": clip_output(&stdout),
        "stderr": stderr,
        "truncated": truncated,
        "max_results": max_lines
    }))?)
}

fn ensure_safe_search_path(path: &Path) -> Result<()> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if path == Path::new("/") || path == Path::new("/home") || path == Path::new("/usr") {
        bail!(
            "refusing broad system search path: {}; use a specific workspace or subdirectory",
            path.display()
        );
    }
    Ok(())
}

fn max_results(args: &Value) -> usize {
    args.get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(100)
        .clamp(1, 500) as usize
}

fn clip_output(value: &str) -> String {
    let value = value.trim();
    if value.chars().count() <= MAX_COMMAND_OUTPUT_CHARS {
        value.to_string()
    } else {
        format!(
            "{}\n...[{} {MAX_COMMAND_OUTPUT_CHARS} {}]",
            value
                .chars()
                .take(MAX_COMMAND_OUTPUT_CHARS)
                .collect::<String>(),
            t("truncated to", "已截断到"),
            t("chars", "字符")
        )
    }
}

fn path_arg(args: &Value, key: &str) -> Result<PathBuf> {
    let value = required(args, key)?;
    Ok(expand_path(&value))
}

fn optional_path(args: &Value) -> Option<PathBuf> {
    args.get("path")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(expand_path)
}

fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    } else {
        Ok(value.to_string())
    }
}
