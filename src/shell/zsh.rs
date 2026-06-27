use crate::i18n::text as t;
use crate::paths::MiyuPaths;
use anyhow::Result;
use std::io::Write;
use std::path::Path;

const BEGIN_MARKER: &str = "# >>> miyu zsh hook >>>";
const END_MARKER: &str = "# <<< miyu zsh hook <<<";

pub fn hook() -> &'static str {
    r#"command_not_found_handler() {
    [[ -o interactive ]] || return 127

    local text="$*"
    local miyu_leading_pattern='^[[:space:]]*([-#./~0-9]|[[:digit:]]+[.)])'
    local miyu_shell_syntax_pattern='[/\\=|;&<>$`(){}\[\]*]'
    [[ -n "$text" ]] || return 127
    (( ${#text} <= 120 )) || return 127
    [[ "$text" != *$'\n'* && "$text" != *$'\r'* ]] || return 127
    [[ ! "$text" =~ $miyu_leading_pattern ]] || return 127
    [[ ! "$text" =~ $miyu_shell_syntax_pattern ]] || return 127

    miyu --shell-intercept --shell zsh -- "$@" 2>/dev/null
    return 127
}
"#
}

pub fn install(paths: &MiyuPaths) -> Result<()> {
    if let Some(parent) = paths.zsh_hook_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&paths.zsh_hook_file, hook())?;
    let rc_path = home_file(".zshrc");
    append_source_block(&rc_path, BEGIN_MARKER, END_MARKER, &paths.zsh_hook_file)?;
    println!(
        "{}: {}",
        t("installed zsh hook", "已安装 zsh hook"),
        paths.zsh_hook_file.display()
    );
    println!("{}: {}", t("updated", "已更新"), rc_path.display());
    Ok(())
}

fn home_file(name: &str) -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(name))
        .unwrap_or_else(|| std::path::PathBuf::from(name))
}

fn append_source_block(rc_path: &Path, begin: &str, end: &str, hook_file: &Path) -> Result<()> {
    let existing = std::fs::read_to_string(rc_path).unwrap_or_default();
    if existing.contains(begin) && existing.contains(end) {
        return Ok(());
    }
    if let Some(parent) = rc_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc_path)?;
    if !existing.ends_with('\n') && !existing.is_empty() {
        writeln!(file)?;
    }
    writeln!(file, "{begin}")?;
    writeln!(file, "[ -r {:?} ] && source {:?}", hook_file, hook_file)?;
    writeln!(file, "{end}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_hook_defines_command_not_found_handler() {
        let hook = hook();
        assert!(hook.contains("command_not_found_handler"));
        assert!(hook.contains("--shell zsh"));
        assert!(hook.contains("return 127"));
    }
}
