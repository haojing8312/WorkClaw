# Agent-Reach External Capabilities

WorkClaw can detect a user-installed Agent-Reach environment without vendoring Agent-Reach code or making it a hard runtime dependency.

## What WorkClaw Does

WorkClaw currently uses Agent-Reach in three ways:

- detect whether `agent-reach` is installed
- show detected external capability channels in Settings
- surface MCP-backed channels and allow controlled import into WorkClaw MCP management

WorkClaw does not currently:

- install Agent-Reach for the user
- vendor Agent-Reach source code
- treat Agent-Reach as a built-in browser engine
- auto-import all detected MCP channels without user confirmation

## Where To See It

In `Settings`:

- `模型连接` shows `External Content Providers`
- `MCP 服务器` shows `Detected from Agent-Reach`

## Status Meanings

For the Agent-Reach source:

- `Available`: command detected and diagnostics look healthy
- `Partial`: command detected but some dependencies or channels appear unavailable
- `Not Found`: `agent-reach` command not detected

For detected MCP entries:

- `Detected only`: WorkClaw sees the MCP-backed channel but is not managing it yet
- `Imported`: the server has been imported into WorkClaw MCP management

## Safe Import Policy

WorkClaw only allows one-click import for known-safe command templates.

Current allowlist:

- `mcporter`
- `linkedin-mcp`

If a detected MCP entry is not on the allowlist, WorkClaw keeps `Use Template` available so the user can review and save it manually.

## Design Boundary

Agent-Reach is treated as an external capability source, not as a framework embedded into WorkClaw.

That means:

- detection and diagnostics are lightweight
- imported MCP servers become normal WorkClaw-managed MCP servers
- unknown templates are not auto-trusted

## Future Direction

The next integration step is to let higher-level content workflows consume imported MCP tools or other validated Agent-Reach-backed providers, without exposing upstream tool details directly to the chat UX.
