use crate::agent::registry::ToolRegistry;
use crate::agent::types::{Tool, ToolContext};
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

struct ToolAlias {
    alias: String,
    inner: Arc<dyn Tool>,
}

impl Tool for ToolAlias {
    fn name(&self) -> &str {
        &self.alias
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        self.inner.execute(input, ctx)
    }
}

pub fn register_tool_alias(registry: &ToolRegistry, alias: &str, inner: Arc<dyn Tool>) {
    registry.register(Arc::new(ToolAlias {
        alias: alias.to_string(),
        inner,
    }));
}
