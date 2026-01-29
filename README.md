# brainpro



A local agentic coding assistant. Vendor-neutral, multi-model routing—send coding to Claude, planning to Qwen, exploration to GPT.

## Two Paths

| Path | Entry Point | Persona | Use Case |
|------|-------------|---------|----------|
| **Direct** | `yo` CLI | MrCode (7 tools) | Local dev, quick tasks |
| **Gateway** | `brainpro-gateway` + `brainpro-agent` | MrBot (12+ tools) | Remote access, daemon mode, Docker |

## Features

- **Local execution** - Runs on your machine, project-scoped file access
- **Multi-backend LLM** - Venice, OpenAI, Anthropic, Ollama, custom endpoints
- **Model routing** - Auto-select models by task type (planning/coding/exploration)
- **Built-in tools** - Read, Write, Edit, Grep, Glob, Bash, Search
- **MCP integration** - External tool servers via Model Context Protocol
- **Subagents** - Delegate to specialized agents with restricted tools
- **Skill packs** - Reusable instruction sets with tool restrictions
- **Permission system** - Granular allow/ask/deny rules
- **Session transcripts** - JSONL audit logs

## Quick Start

### Direct (yo)

```bash
cargo build --release
yo -p "explain main.rs"    # one-shot
yo                          # interactive REPL
```

### Gateway + Daemon (Docker)

```bash
docker-compose up -d
# Connect via WebSocket at ws://localhost:18789
```

## Documentation

- **[DESIGN.md](DESIGN.md)** - Technical architecture, protocols, internals
- **[USERGUIDE.md](USERGUIDE.md)** - Setup, configuration, security hardening

<img width="470" height="420" alt="{CC2FBA1C-F12A-474A-AA0F-952C240874E8}" src="https://github.com/user-attachments/assets/b45efb93-948e-4857-b528-f7d6c6b9240f" />


## Inspired By

- [Claude Code](https://github.com/anthropics/claude-code)
- [opencode](https://github.com/opencode-ai/opencode)
- [clawdbot](https://github.com/crjaensch/clawdbot)
