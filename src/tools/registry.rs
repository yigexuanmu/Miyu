use crate::llm::{FunctionDefinition, ToolDefinition};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type ToolFuture = Pin<Box<dyn Future<Output = Result<String>> + Send>>;
pub type ToolHandler = Arc<dyn Fn(Value, ToolProgress) -> ToolFuture + Send + Sync>;

#[derive(Clone, Default)]
pub struct ToolProgress {
    sender: Option<mpsc::UnboundedSender<String>>,
}

impl ToolProgress {
    pub fn new(sender: mpsc::UnboundedSender<String>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    pub fn report(&self, message: impl Into<String>) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(message.into());
        }
    }
}

#[derive(Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub permission: ToolPermission,
    pub display_name: Option<String>,
    handler: ToolHandler,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolPermission {
    ReadOnly,
    Writes,
}

impl ToolSpec {
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            permission: ToolPermission::ReadOnly,
            display_name: None,
            handler: Arc::new(move |args, _progress| Box::pin(handler(args))),
        }
    }

    pub fn new_with_progress<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value, ToolProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            permission: ToolPermission::ReadOnly,
            display_name: None,
            handler: Arc::new(move |args, progress| Box::pin(handler(args, progress))),
        }
    }

    pub fn writes(mut self) -> Self {
        self.permission = ToolPermission::Writes;
        self
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function",
            function: FunctionDefinition {
                name: self.name.clone(),
                description: self.description.clone(),
                parameters: self.parameters.clone(),
            },
        }
    }

    async fn call(&self, args: Value, progress: ToolProgress) -> Result<String> {
        (self.handler)(args, progress).await
    }

    fn call_future(&self, args: Value, progress: ToolProgress) -> ToolFuture {
        (self.handler)(args, progress)
    }
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolSpec>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: ToolSpec) {
        self.tools.insert(tool.name.clone(), tool);
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(ToolSpec::definition).collect()
    }

    pub fn definitions_except(&self, excluded: &[&str]) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| !excluded.iter().any(|name| *name == tool.name))
            .map(ToolSpec::definition)
            .collect()
    }

    pub fn permission(&self, name: &str) -> Result<ToolPermission> {
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        Ok(tool.permission)
    }

    pub async fn call(&self, name: &str, arguments: &str) -> Result<String> {
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let args = if arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(arguments)?
        };
        tool.call(args, ToolProgress::default()).await
    }

    pub fn call_with_progress_future(
        &self,
        name: &str,
        arguments: &str,
        sender: mpsc::UnboundedSender<String>,
    ) -> Result<ToolFuture> {
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let args = if arguments.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(arguments)?
        };
        Ok(tool.call_future(args, ToolProgress::new(sender)))
    }

    pub fn get(&self, name: &str) -> Option<&ToolSpec> {
        self.tools.get(name)
    }

    pub fn display_name(&self, name: &str) -> Option<String> {
        self.tools
            .get(name)
            .and_then(|t| t.display_name.clone())
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    pub fn clone_filtered(&self, allowed: &[&str]) -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for name in allowed {
            if let Some(spec) = self.tools.get(*name) {
                registry.register(spec.clone());
            }
        }
        registry
    }
}

pub fn empty_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}
