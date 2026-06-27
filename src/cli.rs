use crate::agent::{Agent, AgentEvent, AgentMode};
use crate::config::AppConfig;
use crate::i18n::{is_zh, text as t};
use crate::llm::OpenAiCompatibleClient;
use crate::memory::MemoryStore;
use crate::paths::MiyuPaths;
use crate::render;
use crate::shell;
use crate::state::StateStore;
use crate::tools;
use anyhow::{bail, Result};
use clap::{Arg, ArgAction, Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use crossterm::cursor::{self, Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyModifiers,
};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "miyu", version, about = "Miyu CLI AI Agent")]
pub struct Cli {
    #[arg(long)]
    pub plan: bool,

    #[arg(long, hide = true)]
    pub shell_intercept: bool,

    #[arg(long, hide = true)]
    pub shell: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub message: Vec<String>,
}

pub fn parse() -> Cli {
    let matches = localized_command().get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
}

fn localized_command() -> clap::Command {
    let mut command = Cli::command();
    command = command
        .about(t("Miyu CLI AI Agent", "Miyu 命令行 AI 助手"))
        .override_usage(t(
            "miyu [OPTIONS] [MESSAGE]... [COMMAND]",
            "miyu [选项] [消息]... [命令]",
        ));
    if is_zh() {
        command = command
            .subcommand_help_heading("命令")
            .arg_required_else_help(false)
            .next_help_heading("选项")
            .help_template("{about}\n\n用法: {usage}\n\n命令:\n{subcommands}\n参数:\n{positionals}\n选项:\n{options}\n{after-help}")
            .after_help("提示：不带参数进入 REPL；直接输入消息会发送一次对话。使用 MIYU_LANG=en_US 可切换英文。")
            .disable_help_subcommand(true);
    } else {
        command = command
            .after_help(
                "Tip: run without arguments to enter the REPL; pass MESSAGE to send one chat turn. Set MIYU_LANG=zh_CN for Chinese.",
            )
            .disable_help_subcommand(true);
    }
    command = localize_top_args(command);
    command = localize_subcommands(command);
    command = apply_localized_help_flags(command, true);
    if is_zh() {
        command = apply_chinese_help_template(command);
    }
    command
}

fn apply_localized_help_flags(mut command: clap::Command, root: bool) -> clap::Command {
    command = command.disable_help_flag(true).arg(
        Arg::new("help")
            .short('h')
            .long("help")
            .help(t("Print help", "显示帮助"))
            .action(ArgAction::Help),
    );
    if root {
        command = command.disable_version_flag(true).arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .help(t("Print version", "显示版本"))
                .action(ArgAction::Version),
        );
    }
    let subcommands = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_string())
        .collect::<Vec<_>>();
    for name in subcommands {
        command = command.mut_subcommand(&name, |subcommand| {
            apply_localized_help_flags(subcommand, false)
        });
    }
    command
}

fn apply_chinese_help_template(mut command: clap::Command) -> clap::Command {
    command = command.help_template(
        "{about}\n\n用法: {usage}\n\n命令:\n{subcommands}\n参数:\n{positionals}\n选项:\n{options}\n{after-help}",
    );
    let subcommands = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_string())
        .collect::<Vec<_>>();
    for name in subcommands {
        command = command.mut_subcommand(&name, apply_chinese_help_template);
    }
    command
}

fn localize_top_args(command: clap::Command) -> clap::Command {
    command
        .mut_arg("plan", |arg| {
            arg.help(t("Run in read-only planning mode", "使用只读计划模式运行"))
        })
        .mut_arg("message", |arg| {
            arg.help(t(
                "Message to send; omitted to enter REPL",
                "要发送的消息；省略则进入 REPL",
            ))
        })
}

fn localize_subcommands(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        (
            "ask",
            "Send one message to the assistant",
            "向助手发送一条消息",
        ),
        (
            "init",
            "Create default config and state files",
            "创建默认配置和状态文件",
        ),
        (
            "paths",
            "Show app config, data, and cache paths",
            "显示应用配置、数据和缓存路径",
        ),
        ("config", "Open or manage configuration", "打开或管理配置"),
        (
            "providers",
            "List or switch provider/model",
            "列出或切换 provider/模型",
        ),
        (
            "fish-init",
            "Install fish natural-language hook",
            "安装 fish 自然语言 hook",
        ),
        (
            "bash-init",
            "Install bash natural-language hook",
            "安装 bash 自然语言 hook",
        ),
        (
            "zsh-init",
            "Install zsh natural-language hook",
            "安装 zsh 自然语言 hook",
        ),
        ("history", "Show conversation history", "显示会话历史"),
        ("kb", "Manage local knowledge base", "管理本地知识库"),
        (
            "memory",
            "Inspect or edit assistant memory",
            "查看或编辑助手记忆",
        ),
        ("skills", "Manage assistant skills", "管理助手 skills"),
        (
            "reset",
            "Clear current conversation history",
            "清空当前会话历史",
        ),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command = command
        .mut_subcommand("ask", localize_ask_command)
        .mut_subcommand("providers", localize_providers_command)
        .mut_subcommand("history", localize_history_command)
        .mut_subcommand("kb", localize_kb_command)
        .mut_subcommand("memory", localize_memory_command)
        .mut_subcommand("skills", localize_skills_command)
        .mut_subcommand("config", localize_config_command);
    command
}

fn localize_ask_command(command: clap::Command) -> clap::Command {
    command.mut_arg("message", |arg| {
        arg.help(t("Message to send", "要发送的消息"))
    })
}

fn localize_providers_command(command: clap::Command) -> clap::Command {
    command.mut_arg("index", |arg| {
        arg.help(t(
            "Provider/model list index to activate",
            "要激活的 provider/模型列表序号",
        ))
    })
}

fn localize_history_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("limit", |arg| {
            arg.help(t("Number of history entries to show", "显示的历史条数"))
        })
        .mut_arg("raw", |arg| {
            arg.help(t("Print raw JSONL entries", "输出原始 JSONL 条目"))
        })
        .mut_arg("no_thinking", |arg| {
            arg.help(t("Hide stored reasoning", "隐藏已保存的思考内容"))
        })
}

fn localize_config_command(command: clap::Command) -> clap::Command {
    command
        .mut_subcommand("validate", |subcommand| {
            subcommand.about(t("Validate configuration", "校验配置"))
        })
        .mut_subcommand("paths", |subcommand| {
            subcommand.about(t("Show configuration paths", "显示配置路径"))
        })
}

fn localize_kb_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("add", "Add a file or directory", "添加文件或目录"),
        ("list", "List indexed files", "列出已索引文件"),
        ("search", "Search knowledge base content", "搜索知识库内容"),
        ("find", "Find files by name", "按文件名查找文件"),
        ("read", "Read a knowledge base file", "读取知识库文件"),
        ("remove", "Remove a knowledge base file", "移除知识库文件"),
        (
            "reindex",
            "Rebuild keyword index on demand",
            "按需重建关键词索引",
        ),
        ("stats", "Show knowledge base statistics", "显示知识库统计"),
        ("embed", "Manage semantic embeddings", "管理语义嵌入"),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command
        .mut_subcommand("add", |subcommand| {
            subcommand
                .mut_arg("path", |arg| arg.help(t("Path to add", "要添加的路径")))
                .mut_arg("recursive", |arg| {
                    arg.help(t(
                        "Compatibility flag; directories are recursive by default",
                        "兼容参数；目录默认递归导入",
                    ))
                })
        })
        .mut_subcommand("search", |subcommand| {
            subcommand
                .mut_arg("query", |arg| arg.help(t("Search query", "搜索查询")))
                .mut_arg("limit", |arg| arg.help(t("Maximum results", "最大结果数")))
        })
        .mut_subcommand("find", |subcommand| {
            subcommand
                .mut_arg("query", |arg| arg.help(t("Filename query", "文件名查询")))
                .mut_arg("limit", |arg| arg.help(t("Maximum results", "最大结果数")))
        })
        .mut_subcommand("read", |subcommand| {
            subcommand
                .mut_arg("file", |arg| {
                    arg.help(t("Knowledge base file name", "知识库文件名"))
                })
                .mut_arg("start", |arg| arg.help(t("Starting line", "起始行")))
                .mut_arg("lines", |arg| arg.help(t("Number of lines", "读取行数")))
        })
        .mut_subcommand("remove", |subcommand| {
            subcommand.mut_arg("file", |arg| arg.help(t("File to remove", "要移除的文件")))
        })
}

fn localize_memory_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("stats", "Show memory statistics", "显示记忆统计"),
        ("reset", "Clear assistant memory", "清空助手记忆"),
        ("search", "Search memories", "搜索记忆"),
        ("remember", "Save a manual fact", "手动保存事实"),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command
        .mut_subcommand("reset", |subcommand| {
            subcommand.mut_arg("include_skills", |arg| {
                arg.help(t(
                    "Also remove generated skills",
                    "同时移除自动生成的 skills",
                ))
            })
        })
        .mut_subcommand("search", |subcommand| {
            subcommand
                .mut_arg("query", |arg| arg.help(t("Search query", "搜索查询")))
                .mut_arg("limit", |arg| arg.help(t("Maximum results", "最大结果数")))
                .mut_arg("forgotten", |arg| {
                    arg.help(t("Include forgotten memories", "包含已遗忘记忆"))
                })
        })
        .mut_subcommand("remember", |subcommand| {
            subcommand
                .mut_arg("content", |arg| arg.help(t("Fact content", "事实内容")))
                .mut_arg("source", |arg| arg.help(t("Source label", "来源标签")))
        })
}

fn localize_skills_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("list", "List skills", "列出 skills"),
        ("show", "Show a skill", "显示 skill"),
        ("enable", "Enable a skill", "启用 skill"),
        ("disable", "Disable a skill", "禁用 skill"),
        ("remove", "Remove a skill", "移除 skill"),
        ("stats", "Show skill statistics", "显示 skill 统计"),
        (
            "prune",
            "Remove disabled generated skills",
            "清理已禁用的自动 skills",
        ),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    for name in ["show", "enable", "disable", "remove"] {
        command = command.mut_subcommand(name, |subcommand| {
            subcommand.mut_arg("name", |arg| arg.help(t("Skill name", "skill 名称")))
        });
    }
    command
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Ask(MessageArgs),
    Init,
    Paths,
    Config(ConfigArgs),
    Providers(ProvidersArgs),
    FishInit,
    BashInit,
    ZshInit,
    History(HistoryArgs),
    Kb(KbArgs),
    Memory(MemoryArgs),
    Skills(SkillsArgs),
    Reset,
}

#[derive(Debug, Args)]
pub struct MessageArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub message: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: Option<ConfigCommand>,
}

#[derive(Debug, Args)]
pub struct HistoryArgs {
    #[arg(short, long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub raw: bool,

    #[arg(long)]
    pub no_thinking: bool,
}

#[derive(Debug, Args)]
pub struct ProvidersArgs {
    pub index: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbArgs {
    #[command(subcommand)]
    pub command: KbCommand,
}

#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    Stats,
    Reset(MemoryResetArgs),
    Search(MemorySearchArgs),
    Remember(MemoryRememberArgs),
}

#[derive(Debug, Args)]
pub struct MemoryResetArgs {
    #[arg(long)]
    pub include_skills: bool,
}

#[derive(Debug, Args)]
pub struct MemorySearchArgs {
    pub query: Vec<String>,
    #[arg(short, long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub forgotten: bool,
}

#[derive(Debug, Args)]
pub struct MemoryRememberArgs {
    pub content: Vec<String>,
    #[arg(short, long, default_value = "manual")]
    pub source: String,
}

#[derive(Debug, Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub command: SkillsCommand,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    List,
    Show(SkillNameArgs),
    Enable(SkillNameArgs),
    Disable(SkillNameArgs),
    Remove(SkillNameArgs),
    Stats,
    Prune,
}

#[derive(Debug, Args)]
pub struct SkillNameArgs {
    pub name: String,
}

#[derive(Debug, Subcommand)]
pub enum KbCommand {
    Add(KbAddArgs),
    List,
    Search(KbSearchArgs),
    Find(KbFindArgs),
    Read(KbReadArgs),
    Remove(KbRemoveArgs),
    Reindex,
    Stats,
    Embed(KbEmbedArgs),
}

#[derive(Debug, Args)]
pub struct KbAddArgs {
    pub path: PathBuf,
    #[arg(
        short,
        long,
        help = "Compatibility flag; directories are recursive by default"
    )]
    pub recursive: bool,
}

#[derive(Debug, Args)]
pub struct KbSearchArgs {
    pub query: Vec<String>,
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbFindArgs {
    pub query: Vec<String>,
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbReadArgs {
    pub file: String,
    #[arg(long, default_value_t = 1)]
    pub start: usize,
    #[arg(long)]
    pub lines: Option<usize>,
}

#[derive(Debug, Args)]
pub struct KbRemoveArgs {
    pub file: String,
}

#[derive(Debug, Args)]
pub struct KbEmbedArgs {
    #[command(subcommand)]
    pub command: KbEmbedCommand,
}

#[derive(Debug, Subcommand)]
pub enum KbEmbedCommand {
    Reindex(KbEmbedReindexArgs),
}

#[derive(Debug, Args)]
pub struct KbEmbedReindexArgs {
    #[arg(long)]
    pub quiet: bool,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Validate,
    Paths,
    #[command(hide = true)]
    PromptSource,
}

pub async fn run(cli: Cli) -> Result<()> {
    let paths = MiyuPaths::new()?;
    let mode = if cli.plan {
        AgentMode::Plan
    } else {
        AgentMode::Yolo
    };

    if cli.shell_intercept {
        let shell_name = cli.shell.as_deref().unwrap_or("fish");
        let message = join_message(cli.message);
        return run_shell_intercept(&paths, shell_name, message).await;
    }

    match cli.command {
        Some(Command::Ask(args)) => {
            run_chat_with_options(&paths, join_message(args.message), None, false, mode).await
        }
        Some(Command::Init) => {
            AppConfig::init_files(&paths)?;
            StateStore::new(&paths)?.init_files()?;
            println!(
                "{} {}",
                t("initialized Miyu at", "Miyu 已初始化于"),
                paths.config_dir.display()
            );
            Ok(())
        }
        Some(Command::Paths) => {
            paths.print();
            Ok(())
        }
        Some(Command::Config(args)) => run_config(&paths, args).await,
        Some(Command::Providers(args)) => run_providers(&paths, args),
        Some(Command::FishInit) => shell::fish::install(&paths),
        Some(Command::BashInit) => shell::bash::install(&paths),
        Some(Command::ZshInit) => shell::zsh::install(&paths),
        Some(Command::History(args)) => run_history(&paths, args),
        Some(Command::Kb(args)) => run_kb(&paths, args).await,
        Some(Command::Memory(args)) => run_memory(&paths, args),
        Some(Command::Skills(args)) => run_skills(&paths, args),
        Some(Command::Reset) => run_reset(&paths),
        None => {
            let message = join_message(cli.message);
            if message.is_empty() {
                run_repl(&paths, mode).await
            } else {
                run_chat_with_options(&paths, message, None, false, mode).await
            }
        }
    }
}

fn run_providers(paths: &MiyuPaths, args: ProvidersArgs) -> Result<()> {
    let mut config = AppConfig::load(paths)?;
    let choices = config.provider_model_choices();
    if choices.is_empty() {
        bail!(
            "{}",
            t(
                "no active provider models; configure or activate a model first",
                "没有已激活的 provider 模型；请先配置或激活模型",
            )
        );
    }
    if let Some(index) = args.index {
        if index == 0 || index > choices.len() {
            bail!(
                "{}: {index}",
                t("provider index out of range", "provider 序号超出范围")
            );
        }
        let choice = &choices[index - 1];
        let provider_id = choice.provider_id.clone();
        let model = choice.model.clone();
        let label = choice.label();
        config.set_active_provider_model(&provider_id, &model)?;
        config.save(paths)?;
        println!(
            "{}: {index}. {label}",
            t("active provider", "当前 provider")
        );
        return Ok(());
    }
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        if let Some(index) = inline_fuzzy_select(
            &choices
                .iter()
                .map(|choice| choice.label())
                .collect::<Vec<_>>(),
        )? {
            let choice = &choices[index];
            let provider_id = choice.provider_id.clone();
            let model = choice.model.clone();
            let label = choice.label();
            config.set_active_provider_model(&provider_id, &model)?;
            config.save(paths)?;
            println!(
                "{}: {}. {label}",
                t("active provider", "当前 provider"),
                index + 1
            );
        }
        return Ok(());
    }
    for (index, choice) in choices.iter().enumerate() {
        let active = config
            .provider(None)
            .map(|provider| {
                provider.id == choice.provider_id && provider.default_model == choice.model
            })
            .unwrap_or(false);
        let marker = if active { "*" } else { " " };
        println!("{marker} {}. {}", index + 1, choice.label());
    }
    Ok(())
}

fn inline_fuzzy_select(items: &[String]) -> Result<Option<usize>> {
    let menu_lines = inline_fuzzy_lines(items.len());
    reserve_inline_fuzzy_space(menu_lines)?;
    let mut session = InlineRawMode::start()?;
    let matcher = SkimMatcherV2::default();
    let mut query = String::new();
    let mut selected = 0usize;
    let (_, cursor_y) = cursor::position().unwrap_or((0, menu_lines.saturating_sub(1)));
    let anchor_y = cursor_y.saturating_sub(menu_lines.saturating_sub(1));
    loop {
        let matches = fuzzy_matches(&matcher, items, &query);
        if selected >= matches.len() {
            selected = matches.len().saturating_sub(1);
        }
        draw_inline_fuzzy(
            &mut session.stdout,
            anchor_y,
            menu_lines,
            &query,
            items,
            &matches,
            selected,
        )?;
        if let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = event::read()?
        {
            match code {
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Esc => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Char('q') if query.is_empty() => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(None);
                }
                KeyCode::Enter => {
                    clear_inline_fuzzy(&mut session.stdout, anchor_y, menu_lines)?;
                    return Ok(matches.get(selected).map(|(_, index)| *index));
                }
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(matches.len().saturating_sub(1));
                }
                KeyCode::Backspace => {
                    query.pop();
                    selected = 0;
                }
                KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    query.push(ch);
                    selected = 0;
                }
                _ => {}
            }
        }
    }
}

fn fuzzy_matches(matcher: &SkimMatcherV2, items: &[String], query: &str) -> Vec<(i64, usize)> {
    let mut matches = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if query.trim().is_empty() {
                Some((0, index))
            } else {
                matcher.fuzzy_match(item, query).map(|score| (score, index))
            }
        })
        .collect::<Vec<_>>();
    if !query.trim().is_empty() {
        matches.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    }
    matches
}

fn draw_inline_fuzzy(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    menu_lines: u16,
    query: &str,
    items: &[String],
    matches: &[(i64, usize)],
    selected: usize,
) -> Result<()> {
    let (cols, _) = terminal::size().unwrap_or((80, 24));
    let width = cols.saturating_sub(2).max(24) as usize;
    let visible = matches.len().min(menu_lines.saturating_sub(2) as usize);
    queue!(stdout, Hide)?;
    for row in 0..menu_lines {
        queue!(
            stdout,
            MoveTo(0, anchor_y + row),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(
        stdout,
        MoveTo(0, anchor_y),
        Print(truncate_display(&format!("> {query}"), width)),
    )?;
    if matches.is_empty() {
        queue!(
            stdout,
            MoveTo(0, anchor_y + 1),
            Print(t("  no matches", "  没有匹配项"))
        )?;
    } else {
        for (row, (_, item_index)) in matches.iter().take(visible).enumerate() {
            let marker = if row == selected { ">" } else { " " };
            let line = truncate_display(&format!("{marker} {}", items[*item_index]), width);
            queue!(stdout, MoveTo(0, anchor_y + row as u16 + 1))?;
            if row == selected {
                queue!(
                    stdout,
                    SetAttribute(Attribute::Reverse),
                    Print(line),
                    SetAttribute(Attribute::Reset)
                )?;
            } else {
                queue!(stdout, Print(line))?;
            }
        }
    }
    queue!(
        stdout,
        MoveTo(0, anchor_y + menu_lines.saturating_sub(1)),
        Print(truncate_display(
            t(
                "[type] search  [j/k] move  [enter] select  [esc/q] cancel",
                "[输入] 搜索  [j/k] 移动  [enter] 选择  [esc/q] 取消",
            ),
            width
        ))
    )?;
    stdout.flush()?;
    Ok(())
}

fn clear_inline_fuzzy(stdout: &mut io::Stdout, anchor_y: u16, lines: u16) -> Result<()> {
    for row in 0..lines {
        queue!(
            stdout,
            MoveTo(0, anchor_y + row),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(stdout, MoveTo(0, anchor_y), Show)?;
    stdout.flush()?;
    Ok(())
}

fn reserve_inline_fuzzy_space(lines: u16) -> Result<()> {
    for _ in 1..lines {
        println!();
    }
    io::stdout().flush()?;
    Ok(())
}

fn inline_fuzzy_lines(item_count: usize) -> u16 {
    ((item_count.min(10) + 2) as u16).max(3)
}

fn truncate_display(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
        )
    }
}

struct InlineRawMode {
    stdout: io::Stdout,
}

impl InlineRawMode {
    fn start() -> Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self {
            stdout: io::stdout(),
        })
    }
}

impl Drop for InlineRawMode {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show);
        let _ = terminal::disable_raw_mode();
    }
}

async fn run_config(paths: &MiyuPaths, args: ConfigArgs) -> Result<()> {
    match args.command {
        Some(ConfigCommand::Validate) => {
            AppConfig::load(paths)?;
            println!(
                "{}: {}",
                t("config is valid", "配置有效"),
                paths.config_file.display()
            );
            Ok(())
        }
        Some(ConfigCommand::Paths) => {
            paths.print();
            Ok(())
        }
        Some(ConfigCommand::PromptSource) => {
            let config = AppConfig::load(paths)?;
            let persona = config.prompt.active_persona.trim();
            let identity = config.prompt.active_identity.trim();
            println!(
                "base_prompt_source: {}",
                if persona.is_empty() {
                    "built-in"
                } else {
                    "persona"
                }
            );
            println!(
                "active_persona: {}",
                if persona.is_empty() { "Miyu" } else { persona }
            );
            if !persona.is_empty() {
                println!(
                    "active_persona_file: {}",
                    config.persona_path(paths, persona).display()
                );
            }
            println!(
                "active_identity: {}",
                if identity.is_empty() {
                    "(none)"
                } else {
                    identity
                }
            );
            println!("prompts_dir: {}", config.prompts_dir_path(paths).display());
            println!(
                "identities_dir: {}",
                config.identities_dir_path(paths).display()
            );
            let system_prompt = config.system_prompt(paths)?;
            println!(
                "system_prompt_first_line: {}",
                system_prompt.lines().next().unwrap_or("")
            );
            println!("system_prompt_chars: {}", system_prompt.chars().count());
            Ok(())
        }
        None => crate::config_tui::run(paths),
    }
}

async fn run_shell_intercept(paths: &MiyuPaths, shell_name: &str, message: String) -> Result<()> {
    if !matches!(shell_name, "fish" | "bash" | "zsh") {
        bail!("{}: {shell_name}", t("unsupported shell", "不支持的 shell"));
    }
    if message.is_empty() || !shell::looks_like_natural_language(&message) {
        bail!(
            "{}",
            t("not a natural language command", "不是自然语言命令")
        );
    }
    run_chat_with_options(paths, message, None, false, AgentMode::Yolo).await
}

async fn run_chat_with_options(
    paths: &MiyuPaths,
    message: String,
    show_reasoning: Option<bool>,
    plain: bool,
    mode: AgentMode,
) -> Result<()> {
    if message.is_empty() {
        return run_repl(paths, mode).await;
    }
    let config = AppConfig::load(paths)?;
    let state = StateStore::new(paths)?;
    state.init_files()?;
    let client = OpenAiCompatibleClient::from_config(&config, paths)?;
    let registry = build_tool_registry(&config, paths, mode)?;
    let reasoning_mode = if show_reasoning == Some(false) {
        render::ReasoningDisplayMode::Hidden
    } else {
        render::ReasoningDisplayMode::from_config(&config.display.reasoning)
    };
    let tool_call_mode = if plain {
        render::ToolCallDisplayMode::Hidden
    } else {
        render::ToolCallDisplayMode::from_config(&config.display.tool_calls)
    };
    let mut agent = Agent::new(config, paths, state, client, registry, mode)?;
    let mut renderer = render::StreamRenderer::new(reasoning_mode, tool_call_mode, plain);
    let result = agent
        .chat_stream(&message, |event| handle_agent_event(&mut renderer, event))
        .await;
    renderer.finish()?;
    result?;
    Ok(())
}

async fn run_repl(paths: &MiyuPaths, initial_mode: AgentMode) -> Result<()> {
    let mut config = AppConfig::load(paths)?;
    let state = StateStore::new(paths)?;
    state.init_files()?;
    let mut client = OpenAiCompatibleClient::from_config(&config, paths)?;
    let mut mode = initial_mode;
    let mut input_history = Vec::<String>::new();
    let mut prefill = None::<String>;

    println!(
        "{}",
        t(
            "Miyu REPL. Press Tab to toggle YOLO/PLAN. Type /help for commands; exit or quit to leave.",
            "Miyu REPL。按 Tab 切换 YOLO/PLAN。输入 /help 查看命令；exit 或 quit 退出。",
        )
    );
    loop {
        let input = match read_repl_input(mode, prefill.take(), &input_history)? {
            Some((new_mode, input)) => {
                mode = new_mode;
                input
            }
            None => break,
        };
        let input = input.trim();
        if input.eq_ignore_ascii_case("exit")
            || input.eq_ignore_ascii_case("quit")
            || input.eq_ignore_ascii_case("/exit")
        {
            break;
        }
        if input.eq_ignore_ascii_case("/help") {
            print_repl_help();
            continue;
        }
        if input.eq_ignore_ascii_case("/plan") {
            mode = AgentMode::Plan;
            println!("{}: {}", t("mode", "模式"), mode.label());
            continue;
        }
        if input.eq_ignore_ascii_case("/yolo") {
            mode = AgentMode::Yolo;
            println!("{}: {}", t("mode", "模式"), mode.label());
            continue;
        }
        if input.eq_ignore_ascii_case("/providers") {
            run_providers(paths, ProvidersArgs { index: None })?;
            reload_repl_config(paths, &mut config, &mut client)?;
            println!("{}", t("configuration reloaded", "配置已重新加载"));
            continue;
        }
        if input.eq_ignore_ascii_case("/config") {
            crate::config_tui::run(paths)?;
            reload_repl_config(paths, &mut config, &mut client)?;
            println!("{}", t("configuration reloaded", "配置已重新加载"));
            continue;
        }
        if input.eq_ignore_ascii_case("/undo") {
            let (removed, prompt) = state.undo_last_turn()?;
            println!("{}: {removed}", t("undone messages", "已撤销消息数"));
            prefill = prompt;
            continue;
        }
        if input.is_empty() {
            continue;
        }
        input_history.push(input.to_string());
        let registry = build_tool_registry(&config, paths, mode)?;
        let mut agent = Agent::new(
            config.clone(),
            paths,
            state.clone(),
            client.clone(),
            registry,
            mode,
        )?;
        let reasoning_mode = render::ReasoningDisplayMode::from_config(&config.display.reasoning);
        let tool_call_mode = render::ToolCallDisplayMode::from_config(&config.display.tool_calls);
        let mut renderer = render::StreamRenderer::new(reasoning_mode, tool_call_mode, false);
        let chat_result = agent
            .chat_stream(input, |event| handle_agent_event(&mut renderer, event))
            .await
            .map(|_| ());
        renderer.finish()?;
        chat_result?;
    }
    Ok(())
}

fn reload_repl_config(
    paths: &MiyuPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
) -> Result<()> {
    *config = AppConfig::load(paths)?;
    *client = OpenAiCompatibleClient::from_config(config, paths)?;
    Ok(())
}

fn print_repl_help() {
    println!("{}", t("commands:", "命令:"));
    println!(
        "  /providers  {}",
        t("switch provider or model", "切换 provider 或模型")
    );
    println!(
        "  /config     {}",
        t("open configuration UI", "打开配置界面")
    );
    println!(
        "  /plan       {}",
        t("switch to read-only planning mode", "切换到只读计划模式")
    );
    println!(
        "  /yolo       {}",
        t("switch to build mode", "切换到执行模式")
    );
    println!(
        "  /undo       {}",
        t(
            "remove last turn and restore prompt",
            "撤销上一轮并恢复输入"
        )
    );
    println!("  /help       {}", t("show this help", "显示此帮助"));
    println!("  /exit       {}", t("leave REPL", "退出 REPL"));
    println!("{}", t("keys:", "快捷键:"));
    println!(
        "  Tab         {}",
        t(
            "toggle YOLO/PLAN, or complete slash commands",
            "切换 YOLO/PLAN，或补全斜杠菜单"
        )
    );
    println!(
        "  Up/Down     {}",
        t("browse input history", "切换输入历史")
    );
    println!(
        "  Esc Esc     {}",
        t("interrupt running reply", "中断当前回复")
    );
}

fn read_repl_input(
    mut mode: AgentMode,
    prefill: Option<String>,
    history: &[String],
) -> Result<Option<(AgentMode, String)>> {
    let mut stdout = io::stdout();
    let mut input = prefill.unwrap_or_default();
    let mut history_index = history.len();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnableBracketedPaste)?;
    clear_repl_line(&mut stdout, mode, &input)?;
    loop {
        match event::read()? {
            Event::Paste(text) => {
                input.push_str(&text);
                clear_repl_line(&mut stdout, mode, &input)?;
            }
            Event::Key(KeyEvent {
                code, modifiers, ..
            }) => match code {
                KeyCode::Tab => {
                    if input.starts_with('/') {
                        if let Some(completed) = complete_repl_command(&input) {
                            input = completed.to_string();
                        }
                    } else {
                        mode = if mode == AgentMode::Yolo {
                            AgentMode::Plan
                        } else {
                            AgentMode::Yolo
                        };
                    }
                    clear_repl_line(&mut stdout, mode, &input)?;
                }
                KeyCode::Esc => {
                    input.clear();
                    clear_repl_line(&mut stdout, mode, &input)?;
                }
                KeyCode::Up => {
                    if !history.is_empty() {
                        history_index = history_index.saturating_sub(1);
                        input = history.get(history_index).cloned().unwrap_or_default();
                        clear_repl_line(&mut stdout, mode, &input)?;
                    }
                }
                KeyCode::Down => {
                    if history_index + 1 < history.len() {
                        history_index += 1;
                        input = history.get(history_index).cloned().unwrap_or_default();
                    } else {
                        history_index = history.len();
                        input.clear();
                    }
                    clear_repl_line(&mut stdout, mode, &input)?;
                }
                KeyCode::Enter => {
                    execute!(stdout, DisableBracketedPaste)?;
                    terminal::disable_raw_mode()?;
                    println!();
                    return Ok(Some((mode, input)));
                }
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                    execute!(stdout, DisableBracketedPaste)?;
                    terminal::disable_raw_mode()?;
                    println!();
                    return Ok(None);
                }
                KeyCode::Char('d')
                    if modifiers.contains(KeyModifiers::CONTROL) && input.is_empty() =>
                {
                    execute!(stdout, DisableBracketedPaste)?;
                    terminal::disable_raw_mode()?;
                    println!();
                    return Ok(None);
                }
                KeyCode::Backspace => {
                    input.pop();
                    clear_repl_line(&mut stdout, mode, &input)?;
                }
                KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                    input.push(ch);
                    clear_repl_line(&mut stdout, mode, &input)?;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn clear_repl_line(stdout: &mut io::Stdout, mode: AgentMode, input: &str) -> Result<()> {
    let suggestions = repl_command_suggestions(input);
    let (_, row) = cursor::position()?;
    let visible_input = repl_input_preview(input);
    let prompt = format!("{} > {}", colored_mode_label(mode), visible_input);
    let prompt_width = visible_width(&format!("[{}] > {}", mode.label(), visible_input)) as u16;
    queue!(
        stdout,
        cursor::MoveToColumn(0),
        Clear(ClearType::CurrentLine),
        Print(&prompt),
        MoveTo(0, row.saturating_add(1)),
        Clear(ClearType::CurrentLine)
    )?;
    if !suggestions.is_empty() {
        queue!(
            stdout,
            Print(format!("\x1b[2m{}\x1b[0m", suggestions.join("  ")))
        )?;
    }
    queue!(stdout, MoveTo(prompt_width, row))?;
    stdout.flush()?;
    Ok(())
}

fn repl_input_preview(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', " ↵ ")
}

fn visible_width(value: &str) -> usize {
    let mut width = 0usize;
    let mut escape = false;
    for ch in value.chars() {
        if escape {
            if ch == 'm' {
                escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            escape = true;
        } else if (ch as u32) >= 0x2e80 {
            width += 2;
        } else {
            width += 1;
        }
    }
    width
}

fn colored_mode_label(mode: AgentMode) -> String {
    match mode {
        AgentMode::Yolo => "\x1b[38;5;208m[YOLO]\x1b[0m".to_string(),
        AgentMode::Plan => "\x1b[36m[PLAN]\x1b[0m".to_string(),
    }
}

fn repl_commands() -> [&'static str; 7] {
    [
        "/providers",
        "/config",
        "/plan",
        "/yolo",
        "/undo",
        "/help",
        "/exit",
    ]
}

fn repl_command_suggestions(input: &str) -> Vec<&'static str> {
    if !input.starts_with('/') {
        return Vec::new();
    }
    repl_commands()
        .into_iter()
        .filter(|command| command.starts_with(input))
        .collect()
}

fn complete_repl_command(input: &str) -> Option<&'static str> {
    let suggestions = repl_command_suggestions(input);
    if suggestions.len() == 1 {
        suggestions.first().copied()
    } else {
        None
    }
}

fn run_history(paths: &MiyuPaths, args: HistoryArgs) -> Result<()> {
    let state = StateStore::new(paths)?;
    for entry in state.history(args.limit)? {
        if args.raw {
            println!("{}", serde_json::to_string(&entry)?);
            continue;
        }
        println!("{} {}", entry.timestamp, entry.role);
        if entry.role == "assistant" {
            let response = crate::llm::ChatResult {
                content: entry.content,
                reasoning: if args.no_thinking {
                    None
                } else {
                    entry.reasoning
                },
                usage: None,
                tool_calls: Vec::new(),
            };
            render::print_assistant_response(&response, !args.no_thinking)?;
        } else {
            println!("{}", entry.content);
        }
        println!();
    }
    Ok(())
}

async fn run_kb(paths: &MiyuPaths, args: KbArgs) -> Result<()> {
    let config = AppConfig::load(paths)?;
    let kb = tools::knowledge_base::KnowledgeBase::new(config, paths.clone())?;
    match args.command {
        KbCommand::Add(args) => {
            let added = kb.add_path(&args.path).await?;
            for path in added {
                println!("{} {path}", t("added", "已添加"));
            }
        }
        KbCommand::List => {
            for file in kb.list()? {
                println!("{}\t{} {}", file.name, file.size_bytes, t("bytes", "字节"));
            }
        }
        KbCommand::Search(args) => {
            let query = args.query.join(" ");
            println!("{}", kb.search(&query, args.limit).await?);
        }
        KbCommand::Find(args) => {
            let query = args.query.join(" ");
            println!("{}", kb.find_by_name(&query, args.limit)?);
        }
        KbCommand::Read(args) => {
            println!("{}", kb.read_file(&args.file, args.start, args.lines)?);
        }
        KbCommand::Remove(args) => {
            kb.remove(&args.file)?;
            println!("{} {}", t("removed", "已移除"), args.file);
        }
        KbCommand::Reindex => {
            let files = kb.list()?;
            println!(
                "{}: {}",
                t(
                    "keyword index is rebuilt on demand; files tracked",
                    "关键词索引会按需重建；已跟踪文件数",
                ),
                files.len()
            );
        }
        KbCommand::Stats => {
            println!("{}", kb.stats()?);
        }
        KbCommand::Embed(args) => match args.command {
            KbEmbedCommand::Reindex(args) => {
                kb.reindex_embeddings(args.quiet).await?;
            }
        },
    }
    Ok(())
}

fn run_memory(paths: &MiyuPaths, args: MemoryArgs) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    let store = MemoryStore::new(&config, paths);
    match args.command {
        MemoryCommand::Stats => println!("{}", store.stats()?),
        MemoryCommand::Reset(args) => {
            store.reset_all(args.include_skills)?;
            println!("{}", t("cleared assistant memory", "已清空助手记忆"));
        }
        MemoryCommand::Search(args) => {
            let query = join_message(args.query);
            let limit = args.limit.unwrap_or(10);
            println!("{}", store.recall_memories(&query, limit, args.forgotten)?);
        }
        MemoryCommand::Remember(args) => {
            let content = join_message(args.content);
            let id = store.remember_fact(&content, &args.source)?;
            println!("{}: {id}", t("remembered fact", "已记住事实"));
        }
    }
    Ok(())
}

fn run_skills(paths: &MiyuPaths, args: SkillsArgs) -> Result<()> {
    std::fs::create_dir_all(&paths.skills_dir)?;
    match args.command {
        SkillsCommand::List => {
            for name in skill_names(paths)? {
                let disabled = paths.skills_dir.join(&name).join(".disabled").exists();
                println!(
                    "{}{}",
                    name,
                    if disabled {
                        t(" [disabled]", " [已禁用]")
                    } else {
                        ""
                    }
                );
            }
        }
        SkillsCommand::Show(args) => {
            let path = skill_dir(paths, &args.name)?.join("SKILL.md");
            println!("{}", std::fs::read_to_string(path)?);
        }
        SkillsCommand::Enable(args) => {
            let marker = skill_dir(paths, &args.name)?.join(".disabled");
            if marker.exists() {
                std::fs::remove_file(marker)?;
            }
            println!("{}: {}", t("enabled skill", "已启用 skill"), args.name);
        }
        SkillsCommand::Disable(args) => {
            let marker = skill_dir(paths, &args.name)?.join(".disabled");
            std::fs::write(marker, "disabled\n")?;
            println!("{}: {}", t("disabled skill", "已禁用 skill"), args.name);
        }
        SkillsCommand::Remove(args) => {
            let dir = skill_dir(paths, &args.name)?;
            std::fs::remove_dir_all(dir)?;
            println!("{}: {}", t("removed skill", "已移除 skill"), args.name);
        }
        SkillsCommand::Stats => {
            let names = skill_names(paths)?;
            let disabled = names
                .iter()
                .filter(|name| paths.skills_dir.join(name).join(".disabled").exists())
                .count();
            println!(
                "{}",
                serde_json::json!({
                    "ok": true,
                    "skills_dir": paths.skills_dir.display().to_string(),
                    "skills": names.len(),
                    "disabled": disabled,
                    "enabled": names.len().saturating_sub(disabled),
                })
            );
        }
        SkillsCommand::Prune => {
            let mut removed = 0usize;
            for name in skill_names(paths)? {
                let dir = paths.skills_dir.join(&name);
                let raw = std::fs::read_to_string(dir.join("SKILL.md")).unwrap_or_default();
                if raw.contains("generated_by: miyu") && dir.join(".disabled").exists() {
                    std::fs::remove_dir_all(dir)?;
                    removed += 1;
                }
            }
            println!("{}: {removed}", t("pruned skills", "已清理 skills"));
        }
    }
    Ok(())
}

fn skill_names(paths: &MiyuPaths) -> Result<Vec<String>> {
    let mut names = Vec::new();
    if !paths.skills_dir.exists() {
        return Ok(names);
    }
    for entry in std::fs::read_dir(&paths.skills_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("SKILL.md").is_file() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn skill_dir(paths: &MiyuPaths, name: &str) -> Result<PathBuf> {
    let clean = name.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean == "."
        || clean == ".."
    {
        bail!("{}: {name}", t("invalid skill name", "无效 skill 名称"));
    }
    let dir = paths.skills_dir.join(clean);
    if !dir.join("SKILL.md").is_file() {
        bail!("{}: {name}", t("skill not found", "未找到 skill"));
    }
    Ok(dir)
}

fn run_reset(paths: &MiyuPaths) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    StateStore::new(paths)?.reset_conversation()?;
    MemoryStore::new(&config, paths).clear_evicted_context()?;
    println!(
        "{}",
        t("cleared current conversation history", "已清空当前会话历史")
    );
    Ok(())
}

fn join_message(parts: Vec<String>) -> String {
    parts.join(" ").trim().to_string()
}

fn build_tool_registry(
    config: &AppConfig,
    paths: &MiyuPaths,
    mode: AgentMode,
) -> Result<tools::ToolRegistry> {
    let mut registry = if config.tools.enabled {
        match mode {
            AgentMode::Yolo => tools::builtin_registry(config, paths),
            AgentMode::Plan => tools::readonly_registry(config, paths),
        }
    } else {
        tools::ToolRegistry::new()
    };
    if mode == AgentMode::Yolo && config.tools.enabled && config.skills.enabled {
        tools::register_skills(
            &mut registry,
            config,
            paths,
            config.skills.allow_command_execution,
        )?;
    }
    Ok(registry)
}

fn handle_agent_event(renderer: &mut render::StreamRenderer, event: AgentEvent) -> Result<()> {
    match event {
        AgentEvent::Chunk(chunk) => renderer.write_chunk(chunk),
        AgentEvent::ToolCall { name, arguments } => renderer.write_tool_call(&name, &arguments),
        AgentEvent::ToolResult { name, ok, output } => {
            renderer.write_tool_result(&name, ok, &output)
        }
    }
}
