use crate::llm::{ChatMessage, ChatStreamChunk, OpenAiCompatibleClient};
use crate::prompts::COMPACT_SYSTEM_PROMPT;
use crate::state::{StateStore, Turn};
use anyhow::Result;

use super::overflow::estimate_tokens;

const COMPACT_PROMPT_OVERHEAD: usize = 2000;
const MAX_MERGE_ROUNDS: usize = 5;

pub struct Compactor {
    client: OpenAiCompatibleClient,
    state: StateStore,
    context_window: usize,
    reserved_tokens: usize,
}

impl Compactor {
    pub fn new(
        client: OpenAiCompatibleClient,
        state: StateStore,
        context_window: usize,
        reserved_tokens: usize,
    ) -> Self {
        Self {
            client,
            state,
            context_window,
            reserved_tokens,
        }
    }

    pub async fn perform_compact<F>(&self, on_chunk: &mut F) -> Result<Option<String>>
    where
        F: FnMut(ChatStreamChunk) -> Result<()>,
    {
        let turns = self.state.load_visible_turns()?;
        if turns.is_empty() {
            return Ok(None);
        }

        let head: Vec<&Turn> = turns.iter().collect();
        let previous_summary = self.state.load_last_summary()?;
        let prev_text = previous_summary.as_ref().map(|t| t.assistant_content.clone());

        let usable = self
            .context_window
            .saturating_sub(self.reserved_tokens)
            .saturating_sub(COMPACT_PROMPT_OVERHEAD);

        let head_text = turns_to_text(&head);
        let head_tokens = estimate_tokens(&head_text);

        let summary = if head_tokens <= usable {
            compact_single_pass(&self.client, &head_text, prev_text.as_deref(), on_chunk).await?
        } else {
            let segments = split_into_segments(&head, usable);
            let mut summaries = Vec::new();
            for segment in &segments {
                let segment_text = turns_to_text(segment);
                let s = compact_single_pass(&self.client, &segment_text, None, &mut |_| Ok(())).await?;
                summaries.push(s);
            }
            merge_summaries_tree(&self.client, &summaries, prev_text.as_deref(), usable, on_chunk).await?
        };

        let last_seq = turns.last().unwrap().seq;
        self.state.hide_turns_before_seq(last_seq)?;
        self.state.insert_summary_turn(&summary)?;
        Ok(Some(summary))
    }
}

fn turns_to_text(turns: &[&Turn]) -> String {
    let mut output = String::new();
    for (i, turn) in turns.iter().enumerate() {
        if turn.is_summary {
            continue;
        }
        output.push_str(&format!("--- Turn {} ---\n", i + 1));
        output.push_str("User: ");
        output.push_str(&turn.user_content);
        output.push_str("\nAssistant: ");
        output.push_str(&turn.assistant_content);
        if let Some(reasoning) = &turn.assistant_reasoning {
            if !reasoning.trim().is_empty() {
                output.push_str("\n[Reasoning: ");
                output.push_str(reasoning);
                output.push(']');
            }
        }
        for report in &turn.tool_reports {
            output.push_str("\n[Tool Report: ");
            output.push_str(report);
            output.push(']');
        }
        output.push('\n');
    }
    output
}

fn build_compact_prompt(history: &str, previous_summary: Option<&str>) -> String {
    match previous_summary {
        Some(prev) => format!(
            "Update the anchored summary below using the conversation history above.\n\
             Preserve still-true details, remove stale details, and merge in the new facts.\n\
             <previous-summary>\n{prev}\n</previous-summary>\n\n\
             <conversation-history>\n{history}\n</conversation-history>"
        ),
        None => format!(
            "Create a new anchored summary from the conversation history.\n\n\
             <conversation-history>\n{history}\n</conversation-history>"
        ),
    }
}

async fn compact_single_pass<F>(
    client: &OpenAiCompatibleClient,
    history: &str,
    previous_summary: Option<&str>,
    on_chunk: &mut F,
) -> Result<String>
where
    F: FnMut(ChatStreamChunk) -> Result<()>,
{
    let prompt = build_compact_prompt(history, previous_summary);
    let messages = vec![
        ChatMessage::system(COMPACT_SYSTEM_PROMPT.to_string()),
        ChatMessage::plain("user", &prompt),
    ];
    let result = client
        .chat_stream(messages, vec![], on_chunk)
        .await?;
    Ok(result.content)
}

async fn compact_single_pass_text<F>(
    client: &OpenAiCompatibleClient,
    text: &str,
    previous_summary: Option<&str>,
    on_chunk: &mut F,
) -> Result<String>
where
    F: FnMut(ChatStreamChunk) -> Result<()>,
{
    let prompt = match previous_summary {
        Some(prev) => format!(
            "Update the anchored summary below using the segment summaries above.\n\
             Preserve still-true details, remove stale details, and merge in the new facts.\n\
             <previous-summary>\n{prev}\n</previous-summary>\n\n\
             <segment-summaries>\n{text}\n</segment-summaries>"
        ),
        None => format!(
            "Merge the following segment summaries into a single coherent summary.\n\n\
             <segment-summaries>\n{text}\n</segment-summaries>"
        ),
    };
    let messages = vec![
        ChatMessage::system(COMPACT_SYSTEM_PROMPT.to_string()),
        ChatMessage::plain("user", &prompt),
    ];
    let result = client
        .chat_stream(messages, vec![], on_chunk)
        .await?;
    Ok(result.content)
}

fn split_into_segments<'a>(turns: &[&'a Turn], budget_tokens: usize) -> Vec<Vec<&'a Turn>> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    let mut current_tokens = 0usize;

    for turn in turns {
        let turn_tokens = estimate_tokens(&turn_to_text(turn));
        if current_tokens + turn_tokens > budget_tokens && !current.is_empty() {
            segments.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        current.push(*turn);
        current_tokens += turn_tokens;
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn turn_to_text(turn: &Turn) -> String {
    let mut output = String::new();
    output.push_str(&turn.user_content);
    output.push_str(&turn.assistant_content);
    if let Some(reasoning) = &turn.assistant_reasoning {
        output.push_str(reasoning);
    }
    for report in &turn.tool_reports {
        output.push_str(report);
    }
    output
}

async fn merge_summaries_tree<F>(
    client: &OpenAiCompatibleClient,
    summaries: &[String],
    previous_summary: Option<&str>,
    usable_tokens: usize,
    on_chunk: &mut F,
) -> Result<String>
where
    F: FnMut(ChatStreamChunk) -> Result<()>,
{
    if summaries.len() == 1 {
        return Ok(summaries[0].clone());
    }

    let mut current: Vec<String> = summaries.to_vec();

    for _round in 0..MAX_MERGE_ROUNDS {
        let combined = current.join("\n\n---\n\n");
        let combined_tokens = estimate_tokens(&combined);

        if combined_tokens <= usable_tokens {
            return compact_single_pass_text(client, &combined, previous_summary, on_chunk).await;
        }

        let mut next = Vec::new();
        let mut batch = Vec::new();
        let mut batch_tokens = 0usize;

        for s in &current {
            let s_tokens = estimate_tokens(s);
            if batch_tokens + s_tokens > usable_tokens && !batch.is_empty() {
                let batch_text = batch.join("\n\n---\n\n");
                let merged = compact_single_pass_text(client, &batch_text, None, &mut |_| Ok(())).await?;
                next.push(merged);
                batch.clear();
                batch_tokens = 0;
            }
            batch.push(s.clone());
            batch_tokens += s_tokens;
        }
        if !batch.is_empty() {
            let batch_text = batch.join("\n\n---\n\n");
            let merged = compact_single_pass_text(client, &batch_text, None, &mut |_| Ok(())).await?;
            next.push(merged);
        }

        if next.len() >= current.len() {
            let combined = current.join("\n\n---\n\n");
            return compact_single_pass_text(client, &combined, previous_summary, on_chunk).await;
        }
        current = next;
    }

    let combined = current.join("\n\n---\n\n");
    compact_single_pass_text(client, &combined, previous_summary, on_chunk).await
}
