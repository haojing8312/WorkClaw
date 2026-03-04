use crate::agent::types::{Tool, ToolContext};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct SidecarBridgeTool {
    sidecar_url: String,
    endpoint: String,
    tool_name: String,
    tool_description: String,
    schema: Value,
    // MCP 特有字段
    mcp_server_name: Option<String>,
    mcp_tool_name: Option<String>,
}

impl SidecarBridgeTool {
    pub fn new(
        sidecar_url: String,
        endpoint: String,
        tool_name: String,
        tool_description: String,
        schema: Value,
    ) -> Self {
        Self {
            sidecar_url,
            endpoint,
            tool_name,
            tool_description,
            schema,
            mcp_server_name: None,
            mcp_tool_name: None,
        }
    }

    pub fn new_mcp(
        sidecar_url: String,
        tool_name: String,
        tool_description: String,
        schema: Value,
        mcp_server_name: String,
        mcp_tool_name: String,
    ) -> Self {
        Self {
            sidecar_url,
            endpoint: "/api/mcp/call-tool".to_string(),
            tool_name,
            tool_description,
            schema,
            mcp_server_name: Some(mcp_server_name),
            mcp_tool_name: Some(mcp_tool_name),
        }
    }
}

impl Tool for SidecarBridgeTool {
    fn name(&self) -> &str {
        &self.tool_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let url = format!("{}{}", self.sidecar_url, self.endpoint);

        // MCP 工具需要包装请求体
        let body = if let (Some(server), Some(tool)) = (&self.mcp_server_name, &self.mcp_tool_name)
        {
            json!({
                "serverName": server,
                "toolName": tool,
                "arguments": input,
            })
        } else {
            input
        };

        let resp = client.post(&url).json(&body).send()?;

        if !resp.status().is_success() {
            let error_body: Value = resp.json().unwrap_or(json!({}));
            return Err(anyhow!(
                "Sidecar 调用失败: {}",
                error_body["error"].as_str().unwrap_or("Unknown error")
            ));
        }

        let result: Value = resp.json()?;
        // MCP 工具返回的结果在 content 字段
        if let Some(content) = result["content"].as_str() {
            Ok(content.to_string())
        } else if let Some(output) = result["output"].as_str() {
            Ok(output.to_string())
        } else {
            Ok(serde_json::to_string(&result).unwrap_or_default())
        }
    }
}
