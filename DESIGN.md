# brainpro Design

Technical architecture and internals.

## System Overview

```
User Input
    │
    ▼
┌─────────────────┐
│  CLI / Gateway  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Agent Loop    │  Turn-based: prompt → LLM → tools → repeat
│   (agent.rs)    │  Max iterations (default 12)
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌───────┐ ┌─────────┐
│ LLM   │ │ Policy  │  allow/ask/deny
│Backend│ │ Engine  │
└───────┘ └─────────┘
```

## Two Execution Paths

### Direct: `yo` → MrCode

```
User → yo CLI → MrCode persona → Agent Loop → LLM
```

- Single binary, no daemon
- 7 tools: Read, Write, Edit, Glob, Grep, Bash, Search
- Interactive REPL or one-shot mode

### Gateway: Client → WebSocket → Unix Socket → MrBot

```
Client → WebSocket → brainpro-gateway → Unix socket → brainpro-agent → MrBot
```

- Persistent daemon (`brainpro-agent`)
- Gateway handles auth, WebSocket clients
- 12+ tools including Task, AskUserQuestion, plan mode
- Yield/resume for approval flows

## Agent Loop

The core loop in `agent.rs`:

1. Receive user prompt
2. Build messages array with system prompt + history
3. Call LLM with tool schemas
4. Parse response for tool calls
5. For each tool call:
   - Policy check (allow/ask/deny)
   - If `ask`: yield and wait for approval (gateway mode) or prompt user (CLI)
   - Execute tool
   - Log result to transcript
6. Add assistant message + tool results to history
7. Repeat until LLM returns no tool calls (or max iterations)

**Max iterations**: Default 12, configurable via `--max-turns` or config.

## Persona System

Personas define agent identity, tools, and behavior.

| Persona | Tools | Description |
|---------|-------|-------------|
| **MrCode** | 7 | Focused CLI assistant |
| **MrBot** | 12+ | Full-featured with personality |

### Modularized Prompts

```
config/persona/{name}/
├── manifest.md      # Tool list, assembly order
├── identity.md      # Who the agent is
├── tooling.md       # Tool usage instructions
├── soul.md          # Personality & values (MrBot only)
├── plan-mode.md     # Conditional: planning instructions
└── optimize.md      # Conditional: optimization mode
```

**manifest.md** frontmatter:
```yaml
name: mrcode
display_name: MrCode
description: Focused coding assistant
default_tools:
  - Read
  - Write
  - Edit
  - Glob
  - Grep
  - Bash
  - Search
permission_mode: default
```

**Assembly order** (from manifest):
1. identity.md (always)
2. soul.md (MrBot only)
3. tooling.md (always)
4. plan-mode.md (if plan mode active)
5. optimize.md (if optimize mode active)

## Sessions & Transcripts

**Session storage**: `~/.brainpro/sessions/{uuid}.json`

**Transcript format**: JSONL with events:
- User/assistant messages
- Tool calls and results
- Permission decisions
- Subagent lifecycle
- Skill pack activations
- Errors

## Protocol Layers

### Client ↔ Gateway

- **Transport**: WebSocket
- **Format**: JSON-RPC style messages
- **Port**: 18789 (default)

### Gateway ↔ Agent Daemon

- **Transport**: Unix socket (`/run/brainpro.sock`)
- **Format**: NDJSON (newline-delimited JSON)
- **Streaming**: Events flow continuously

### Event Types

| Event | Description |
|-------|-------------|
| `Thinking` | Agent reasoning (content before tools) |
| `ToolCall` | Tool invocation with name/args |
| `ToolResult` | Execution result, duration, ok/error |
| `Content` | Final text response |
| `Done` | Turn complete with usage stats |
| `Yield` | Paused for approval/input |
| `Error` | Error with code/message |

### Yield/Resume Flow

When policy requires approval in gateway mode:
1. Agent emits `Yield { turn_id, reason, tool_call_id, ... }`
2. Turn state saved to store
3. Client presents approval UI
4. Client sends `ResumeTurn` with `approved: true/false`
5. Agent continues or aborts tool execution

## LLM Vendor Neutrality

### OpenAI-Compatible API

All backends use the OpenAI chat completion format:
- `/v1/chat/completions`
- Messages array with roles
- Tool schemas in OpenAI format
- Streaming optional

### Backend Registry

Backends are lazy-loaded on first use.

```toml
[backends.venice]
base_url = "https://api.venice.ai/api/v1"
api_key_env = "VENICE_API_KEY"

[backends.claude]
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
```

**Built-in backends**: Venice (default), OpenAI, Anthropic, Ollama

**Target format**: `model@backend` (e.g., `claude-3-5-sonnet-latest@claude`)

## Policy Engine

### Permission Modes

| Mode | Behavior |
|------|----------|
| `default` | Read-only allowed; Write/Edit/Bash require approval |
| `acceptEdits` | File mutations allowed; Bash requires approval |
| `bypassPermissions` | All tools allowed (trusted environments only) |

### Rule Matching

Rules are evaluated in order: `allow` → `ask` → `deny` → mode default.

**Pattern syntax**:
- `"Write"` - Match all Write calls
- `"Bash(git:*)"` - Bash commands starting with "git"
- `"Bash(npm install)"` - Exact command match
- `"mcp.server.*"` - All tools from MCP server

### Built-in Protections

- `curl` and `wget` blocked by default
- Paths validated to project root
- Symlinks resolved to prevent escape

## Module Reference

| File | Responsibility |
|------|----------------|
| `main.rs` | Entry, CLI parsing, config bootstrap |
| `cli.rs` | REPL, slash commands |
| `agent.rs` | Agent loop, tool orchestration |
| `config.rs` | Hierarchical config loading |
| `policy.rs` | Permission decision engine |
| `backend.rs` | Backend registry, lazy loading |
| `llm.rs` | HTTP client for LLM APIs |
| `transcript.rs` | JSONL session logging |
| `compact.rs` | Context compaction via summarization |
| `subagent.rs` | Subagent runtime, tool filtering |
| `model_routing.rs` | Task-based model selection |
| `tools/*.rs` | Individual tool implementations |
| `mcp/*.rs` | MCP client, server lifecycle |
| `skillpacks/*.rs` | Skill pack parsing, activation |

## Extension Points

### Subagents

Delegate to restricted agents defined in `.brainpro/agents/<name>.toml`:
```toml
name = "scout"
description = "Read-only exploration"
allowed_tools = ["Read", "Grep", "Glob"]
max_turns = 8
```

### Skill Packs

Reusable instruction sets in `.brainpro/skills/<name>/SKILL.md`:
```markdown
---
name: safe-reader
description: Read-only mode
allowed-tools: Read, Grep, Glob
---
Instructions here...
```

### MCP Integration

Connect external tool servers via Model Context Protocol:
```toml
[mcp.servers.calc]
command = "/path/to/mcp-calc"
transport = "stdio"
auto_start = false
```

### Model Routing

Automatic model selection based on task keywords:
```toml
[model_routing.routes]
planning = "qwen3-235b@venice"
coding = "claude-3-5-sonnet@claude"
exploration = "gpt-4o-mini@chatgpt"
```

### Custom Slash Commands

User commands in `.brainpro/commands/<name>.md`:
```markdown
---
description: Fix issue by number
allowed_tools: [Read, Edit]
---
Fix issue #$ARGUMENTS
```
