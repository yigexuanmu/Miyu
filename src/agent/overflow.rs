use crate::llm::{ChatContent, ChatMessage, Usage};

const CHARS_PER_TOKEN: usize = 4;
const RESERVED_RATIO: f32 = 0.1;
const MIN_RESERVED_TOKENS: usize = 4096;

pub struct OverflowCheck {
    pub context_window: Option<usize>,
    pub reserved_tokens: usize,
    pub trim_at_ratio: f32,
}

impl OverflowCheck {
    pub fn new(
        context_window: Option<usize>,
        trim_at_ratio: f32,
        reserved_tokens: Option<usize>,
    ) -> Self {
        let reserved_tokens = reserved_tokens.unwrap_or_else(|| {
            context_window
                .map(|w| ((w as f32 * RESERVED_RATIO) as usize).max(MIN_RESERVED_TOKENS))
                .unwrap_or(MIN_RESERVED_TOKENS)
        });
        Self {
            context_window,
            reserved_tokens,
            trim_at_ratio,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.context_window.is_some()
    }

    #[allow(dead_code)]
    pub fn usable_tokens(&self) -> Option<usize> {
        self.context_window
            .map(|w| w.saturating_sub(self.reserved_tokens))
    }

    pub fn threshold(&self) -> Option<usize> {
        self.context_window
            .map(|w| (w as f32 * self.trim_at_ratio).max(1.0) as usize)
    }

    pub fn check_usage(&self, usage: &Usage) -> bool {
        let Some(threshold) = self.threshold() else {
            return false;
        };
        usage.total_tokens as usize >= threshold
    }

    #[allow(dead_code)]
    pub fn check_estimate(&self, messages: &[ChatMessage]) -> bool {
        let Some(threshold) = self.threshold() else {
            return false;
        };
        estimate_messages_tokens(messages) >= threshold
    }
}

#[allow(dead_code)]
pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> usize {
    let chars: usize = messages.iter().map(message_chars).sum();
    (chars / CHARS_PER_TOKEN).max(1)
}

pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() / CHARS_PER_TOKEN).max(1)
}

#[allow(dead_code)]
fn message_chars(msg: &ChatMessage) -> usize {
    let role_chars = msg.role.chars().count();
    let content_chars = match &msg.content {
        Some(ChatContent::Text(s)) => s.chars().count(),
        Some(ChatContent::Parts(parts)) => parts
            .iter()
            .map(|p| match p {
                crate::llm::ChatContentPart::Text { text } => text.chars().count(),
                crate::llm::ChatContentPart::ImageUrl { image_url } => {
                    image_url.url.chars().count()
                }
            })
            .sum(),
        None => 0,
    };
    let tool_chars = msg
        .tool_calls
        .as_ref()
        .map(|calls| {
            calls
                .iter()
                .map(|c| c.function.name.chars().count() + c.function.arguments.chars().count())
                .sum::<usize>()
        })
        .unwrap_or(0);
    role_chars + content_chars + tool_chars
}
