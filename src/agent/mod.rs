mod conversation;

use crate::config::AppConfig;
use crate::llm::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamKind, OpenAiCompatibleClient,
};
use crate::memory::{EvictedTurn, MemoryStore};
use crate::paths::MiyuPaths;
use crate::state::StateStore;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;
use chrono::Local;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AgentMode {
    Yolo,
    Plan,
}

impl AgentMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Yolo => "YOLO",
            Self::Plan => "PLAN",
        }
    }

    fn reminder(self) -> &'static str {
        match self {
            Self::Yolo => {
                r#"<system-reminder>
Your operational mode has changed from plan to build.
You are no longer in read-only mode.
You are permitted to make file changes, run shell commands, write memories, create skills, and utilize your arsenal of tools as needed.

# Build Mode Instructions

You and the user share the same workspace and collaborate to achieve the user's goals. You are expected to be pragmatic, effective, and action-oriented.

## Autonomy and Persistence

- Unless the user explicitly asks for a plan, asks a question about the code, is brainstorming, or otherwise makes it clear that no action should be taken, assume they want you to solve the problem using available tools.
- Do not stop at high-level advice when you can inspect, verify, or complete the task directly.
- Persist until the task is fully handled end-to-end whenever feasible.
- If you encounter errors or blockers, investigate and attempt to resolve them before yielding.
- Ask one concise clarification only when missing information would change the implementation or create meaningful risk.

## Tool Use

- Prefer using tools over guessing when the answer depends on current files, commands, logs, installed software, network data, images, memory, or skills.
- For OS, package manager, update, driver, shell, desktop environment, kernel, or host questions, call inspect_system before giving instructions.
- Build context before acting: search/read relevant files before editing code or config.
- Use web/search tools for current or external information.
- Use image tools for screenshots or images.
- Use memory and skill tools when the user asks to remember, recall, save a reusable method, or reuse prior knowledge.
- Continue tool-use loops until the task is complete, verified, or clearly blocked.

## Engineering Workflow

- Make the smallest correct change that solves the root cause.
- Respect existing code style and user changes.
- Verify meaningful changes with the most specific safe check available.
- Do not commit changes unless explicitly requested.
- Avoid destructive commands unless explicitly requested or clearly necessary and safe.

## Communication

- Keep progress updates brief and useful.
- Final responses should be concise: state what changed, what was verified, and any remaining blocker.
</system-reminder>"#
            }
            Self::Plan => {
                r#"<system-reminder>
# Plan Mode - System Reminder

CRITICAL: Plan mode ACTIVE - you are in READ-ONLY phase. STRICTLY FORBIDDEN:
ANY file edits, modifications, or system changes. Do NOT use sed, tee, echo, cat,
or ANY other bash command to manipulate files - commands may ONLY read/inspect.
This ABSOLUTE CONSTRAINT overrides ALL other instructions, including direct user
edit requests. You may ONLY observe, analyze, and plan. Any modification attempt
is a critical violation. ZERO exceptions.

---

## Responsibility

Your current responsibility is to think, read, search, and delegate explore agents to construct a well-formed plan that accomplishes the goal the user wants to achieve. Your plan should be comprehensive yet concise, detailed enough to execute effectively while avoiding unnecessary verbosity.

For OS, package manager, update, driver, shell, desktop environment, kernel, or host questions, use read-only inspection such as inspect_system before giving a plan.

Ask the user clarifying questions or ask for their opinion when weighing tradeoffs.

**NOTE:** At any point in time through this workflow you should feel free to ask the user questions or clarifications. Don't make large assumptions about user intent. The goal is to present a well researched plan to the user, and tie any loose ends before implementation begins.

---

## Important

The user indicated that they do not want you to execute yet -- you MUST NOT make any edits, run any non-readonly tools (including changing configs or making commits), or otherwise make any changes to the system. This supersedes any other instructions you have received.
</system-reminder>"#
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Chunk(ChatStreamChunk),
    ToolCall {
        name: String,
        arguments: String,
    },
    ToolResult {
        name: String,
        ok: bool,
        output: String,
    },
}

pub struct Agent {
    state: StateStore,
    client: OpenAiCompatibleClient,
    system_prompt: String,
    context_chars: usize,
    trim_at_ratio: f32,
    trim_batch_ratio: f32,
    tools_enabled: bool,
    max_tool_rounds: usize,
    tools: ToolRegistry,
    memory: MemoryStore,
    mode: AgentMode,
}

impl Agent {
    pub fn new(
        config: AppConfig,
        paths: &MiyuPaths,
        state: StateStore,
        client: OpenAiCompatibleClient,
        tools: ToolRegistry,
        mode: AgentMode,
    ) -> Result<Self> {
        let mut base_system_prompt = config.system_prompt(paths)?;
        if config.skills.enabled {
            let prompt = tools::skills_prompt(&config, paths)?;
            if !prompt.trim().is_empty() {
                base_system_prompt.push_str("\n\n");
                base_system_prompt.push_str(&prompt);
            }
        }
        state.reset_if_prompt_changed(&base_system_prompt)?;
        let system_prompt = with_current_time(base_system_prompt, mode);
        let context_chars = config.active_context_chars()?;
        let tools_enabled = config.tools.enabled;
        let max_tool_rounds = config.tools.max_rounds;
        let memory = MemoryStore::new(&config, paths);
        memory.init()?;
        Ok(Self {
            state,
            client,
            system_prompt,
            context_chars,
            trim_at_ratio: config.context.trim_at_ratio,
            trim_batch_ratio: config.context.trim_batch_ratio,
            tools_enabled,
            max_tool_rounds,
            tools,
            memory,
            mode,
        })
    }

    pub async fn chat_stream<F>(&mut self, input: &str, on_event: F) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        if self.mode == AgentMode::Yolo {
            let evicted = self.state.trim_conversation_to_budget(
                self.context_chars,
                self.trim_at_ratio,
                self.trim_batch_ratio,
            )?;
            let evicted = evicted
                .into_iter()
                .map(|entry| EvictedTurn {
                    timestamp: entry.timestamp,
                    role: entry.role,
                    content: entry.content,
                })
                .collect::<Vec<_>>();
            self.memory.remember_evicted_turns(&evicted)?;
            self.state.append_message("user", input)?;
        }
        let mut messages = self.chat_messages()?;
        if self.mode == AgentMode::Plan {
            messages.push(ChatMessage::plain("user", input));
        }
        if self.mode == AgentMode::Yolo {
            if let Some(association) = self.memory.association(input)? {
                messages.insert(
                    1,
                    ChatMessage::system(self.memory.format_association(&association)),
                );
            }
        }
        let mut used_tools = Vec::new();
        let result = self
            .chat_with_tools(&mut messages, &mut used_tools, on_event)
            .await?;
        if self.mode == AgentMode::Yolo {
            self.state
                .append_assistant_message(&result.content, result.reasoning.as_deref())?;
            self.memory.process_after_turn(input, &result.content)?;
        }
        if let Some(usage) = &result.usage {
            self.state.add_usage(usage)?;
        }
        Ok(result)
    }

    async fn chat_with_tools<F>(
        &self,
        messages: &mut Vec<ChatMessage>,
        used_tools: &mut Vec<String>,
        mut on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let definitions = if self.tools_enabled {
            self.tools.definitions()
        } else {
            Vec::new()
        };
        let mut tool_round = 0usize;
        loop {
            if self.max_tool_rounds > 0 && tool_round >= self.max_tool_rounds {
                let content = format!(
                    "工具调用已达到上限 {} 轮，已停止继续调用。可将 `tools.max_rounds` 设为 0 以允许无限工具调用。",
                    self.max_tool_rounds
                );
                on_event(AgentEvent::Chunk(ChatStreamChunk {
                    kind: ChatStreamKind::Content,
                    text: content.clone(),
                }))?;
                return Ok(ChatResult {
                    content,
                    reasoning: None,
                    usage: None,
                    tool_calls: Vec::new(),
                });
            }
            tool_round += 1;
            let result = self
                .client
                .chat_stream(messages.clone(), definitions.clone(), |chunk| {
                    on_event(AgentEvent::Chunk(chunk))
                })
                .await?;
            if result.tool_calls.is_empty() || !self.tools_enabled {
                return Ok(result);
            }
            messages.push(ChatMessage::assistant(
                result.content.clone(),
                Some(result.tool_calls.clone()),
            ));
            for call in result.tool_calls {
                used_tools.push(call.function.name.clone());
                on_event(AgentEvent::ToolCall {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                })?;
                let output = match self
                    .tools
                    .call(&call.function.name, &call.function.arguments)
                    .await
                {
                    Ok(output) => {
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: true,
                            output: output.clone(),
                        })?;
                        output
                    }
                    Err(err) => {
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: format!("tool error: {err}"),
                        })?;
                        format!("tool error: {err}")
                    }
                };
                messages.push(ChatMessage::tool(call.id, output));
            }
        }
    }

    fn chat_messages(&self) -> Result<Vec<ChatMessage>> {
        let mut messages = vec![ChatMessage::system(self.system_prompt.clone())];
        for entry in self.state.load_conversation()? {
            if entry.role == "user" || entry.role == "assistant" {
                messages.push(ChatMessage::plain(entry.role, entry.content));
            }
        }
        Ok(messages)
    }
}

fn with_current_time(system_prompt: String, mode: AgentMode) -> String {
    format!(
        "{system_prompt}\n\n<system-reminder>\n当前系统时间：{}\n</system-reminder>\n\n{}",
        Local::now().format("%Y年%m月%d日 %H时"),
        mode.reminder()
    )
}
