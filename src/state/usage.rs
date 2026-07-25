use crate::llm::Usage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Default, Serialize, Deserialize)]
struct UsageState {
    requests: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_conversation_usage: Option<Usage>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct UsageSnapshot {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub last_usage: Option<Usage>,
    pub last_conversation_usage: Option<Usage>,
}

impl From<UsageState> for UsageSnapshot {
    fn from(state: UsageState) -> Self {
        let last_conversation_usage = state
            .last_conversation_usage
            .clone()
            .or_else(|| state.last_usage.clone());
        Self {
            requests: state.requests,
            prompt_tokens: state.prompt_tokens,
            completion_tokens: state.completion_tokens,
            total_tokens: state.total_tokens,
            last_usage: state.last_usage,
            last_conversation_usage,
        }
    }
}

pub fn add_usage(path: &Path, usage: &Usage) -> Result<()> {
    add_usage_with_scope(path, usage, true)
}

pub fn add_auxiliary_usage(path: &Path, usage: &Usage) -> Result<()> {
    add_usage_with_scope(path, usage, false)
}

fn add_usage_with_scope(path: &Path, usage: &Usage, is_conversation: bool) -> Result<()> {
    let mut state = if path.exists() {
        let raw = std::fs::read_to_string(path)?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        UsageState::default()
    };
    state.requests += 1;
    state.prompt_tokens += usage.prompt_tokens;
    state.completion_tokens += usage.completion_tokens;
    state.total_tokens += usage.effective_total_tokens();
    state.last_usage = Some(usage.clone());
    if is_conversation {
        state.last_conversation_usage = Some(usage.clone());
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&state)?))?;
    Ok(())
}

pub fn snapshot(path: &Path) -> Result<UsageSnapshot> {
    if !path.exists() {
        return Ok(UsageSnapshot::default());
    }
    let raw = std::fs::read_to_string(path)?;
    let state = serde_json::from_str::<UsageState>(&raw).unwrap_or_default();
    Ok(state.into())
}

pub fn clear_last_usage(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut state = serde_json::from_str::<UsageState>(&raw).unwrap_or_default();
    state.last_usage = None;
    state.last_conversation_usage = None;
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&state)?))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_clears_last_usage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        };

        add_usage(&path, &usage).unwrap();
        let usage_snapshot = snapshot(&path).unwrap();
        assert_eq!(usage_snapshot.last_usage.unwrap().total_tokens, 15);
        assert_eq!(
            usage_snapshot
                .last_conversation_usage
                .unwrap()
                .prompt_tokens,
            10
        );

        clear_last_usage(&path).unwrap();
        let usage_snapshot = snapshot(&path).unwrap();
        assert_eq!(usage_snapshot.total_tokens, 15);
        assert!(usage_snapshot.last_usage.is_none());
        assert!(usage_snapshot.last_conversation_usage.is_none());
    }

    #[test]
    fn auxiliary_usage_does_not_replace_conversation_usage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");

        add_usage(
            &path,
            &Usage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
            },
        )
        .unwrap();
        add_auxiliary_usage(
            &path,
            &Usage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            },
        )
        .unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.total_tokens, 127);
        assert_eq!(snapshot.last_usage.unwrap().prompt_tokens, 5);
        assert_eq!(snapshot.last_conversation_usage.unwrap().prompt_tokens, 100);
    }

    #[test]
    fn total_tokens_falls_back_to_prompt_plus_completion() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");

        add_usage(
            &path,
            &Usage {
                prompt_tokens: 7,
                completion_tokens: 3,
                total_tokens: 0,
            },
        )
        .unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.total_tokens, 10);
    }
}
