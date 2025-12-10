# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**droid-mcp-rs** is a high-performance Rust-based MCP (Model Context Protocol) server that wraps the Factory.ai Droid CLI for AI-assisted coding tasks. It enables seamless integration with Claude Code and other MCP clients with configurable autonomy levels and session management.

- **Language**: Rust 2021 Edition
- **Version**: 0.1.0
- **License**: MIT
- **Author**: jakvbs

## Common Development Commands

### Building
```bash
cargo build              # Development build
cargo build --release    # Optimized release build with LTO
cargo clean              # Clean build artifacts
```

### Testing
```bash
cargo test               # Run all tests
cargo clippy             # Lint code
cargo fmt                # Format code
```

### Running
```bash
cargo run                # Start MCP server via stdio
```

### Configuration
```bash
# Set environment variables for configuration
export DROID_BIN=/path/to/droid           # Override droid binary path
export DROID_MCP_CONFIG_PATH=./config.json # Override config file path
```

## High-Level Architecture

### MCP Server Flow
```
Claude Code (MCP Client)
    ↓ stdio transport
MCP Server (main.rs) - Entry point with clap CLI
    ↓
Tool Handler (server.rs::droid) - Parameter validation and MCP protocol
    ↓
CLI Wrapper (droid.rs::run) - Process spawning and stream parsing
    ↓
droid exec CLI (subprocess) - Factory.ai Droid execution
```

### Core Components

**1. MCP Server Entry (`src/main.rs` ~93 lines)**
- Parses CLI arguments with clap
- Initializes MCP server with stdio transport
- Provides comprehensive help text with environment variables and usage

**2. Tool Handler (`src/server.rs` ~369 lines)**
- Implements MCP protocol with `rmcp` SDK
- Defines `droid` tool with JSON schema validation
- Handles parameter validation (prompt/file mutual exclusivity, autonomy levels)
- Resolves working directories and file paths
- Encodes output using TOON format

**3. Droid CLI Wrapper (`src/droid.rs` ~820 lines)**
- Core execution logic for Droid CLI
- Manages configuration loading from `droid-mcp.config.json`
- Reads Factory config from `~/.factory/config.json` for custom models
- Handles DROID.md system prompt injection (max 1MB)
- Parses JSON stream output from droid CLI
- Enforces timeout and size limits
- Session management with SESSION_ID tracking

**4. Module Declarations (`src/lib.rs` ~3 lines)**
- Exports public modules for library usage

### Key Design Patterns

1. **Lazy Configuration Loading**: Uses `OnceLock` for static configuration caching (server config and Factory config)
2. **Stream Processing**: Async line-by-line JSON parsing from droid CLI stdout
3. **Size Limits**: Multiple truncation boundaries (10MB agent messages, 50MB all messages, 1MB DROID.md)
4. **Timeout Enforcement**: Configurable timeouts with async tokio::time::timeout wrapper
5. **Custom Model System**: Supports custom model references via `custom:Display-Name-index` format
6. **DROID.md Injection**: Automatically prepends project context as system prompt

### Configuration System

**Server Configuration (`droid-mcp.config.json`)**
- Located in working directory or via `DROID_MCP_CONFIG_PATH`
- Fields: `additional_args`, `timeout_secs`, `max_timeout_secs`, `default_auto`, `default_model`, `allow_high_autonomy`
- Defaults: 600s timeout, high autonomy enabled

**Factory Configuration (`~/.factory/config.json`)**
- Contains custom model definitions with display names, model IDs, base URLs, API keys
- Automatically loaded and cached
- Models listed in MCP server instructions for tool discovery

### Critical File Paths & Constants

- **Config**: `./droid-mcp.config.json` or `DROID_MCP_CONFIG_PATH`
- **Factory Config**: `~/.factory/config.json` (Windows: `%USERPROFILE%\.factory\config.json`)
- **DROID.md**: `<working_dir>/DROID.md` (project-specific context)
- **Timeouts**: Default 600s, max 3600s
- **Size Limits**: 10MB agent messages, 50MB all messages, 1MB DROID.md, 100KB stderr

### Autonomy Levels

- **DEFAULT** (no `--auto` flag): Read-only operations
- **low**: File creation/editing in project directories
- **medium**: Package installation, git commits, local builds
- **high**: Git push, production deployments, script execution (requires `allow_high_autonomy: true`)

### Session Management

- Each execution returns a `SESSION_ID` in the result
- Pass `SESSION_ID` parameter to resume multi-turn conversations
- Sessions maintain full context across calls

## Development Workflow

### Adding New Parameters
1. Add field to `DroidArgs` struct in `src/server.rs`
2. Add to `Options` struct in `src/droid.rs`
3. Update parameter validation in `server.rs::droid`
4. Add CLI argument construction in `droid.rs::run_internal`
5. Update help text in `src/main.rs` if user-facing

### Modifying Stream Parsing
- JSON stream parsing logic in `droid.rs::run_internal` (lines ~677-777)
- Handles `type: "error"`, `type: "completion"`, `type: "message"`
- Updates `DroidResult` fields based on stream events
- Enforces size limits with truncation flags

### Configuration Changes
- Server config: `load_server_config()` in `src/droid.rs`
- Factory config: `load_factory_config()` in `src/droid.rs`
- Custom model listing: `list_custom_models()` and `get_model_info()`

## Dependencies

**Core**: `rmcp` (MCP SDK), `serde`, `serde_json`, `tokio`, `anyhow`, `clap`
**Encoding**: `toon-format` (for MCP output encoding)
**Testing**: `tempfile`

See `Cargo.toml` for complete dependency list.

## Related Projects

- [codex-mcp-rs](../codex-mcp-rs/) - Codex CLI MCP wrapper (sibling project)
- [gemini-mcp-rs](../gemini-mcp-rs/) - Gemini CLI MCP wrapper (sibling project)
- [Factory.ai Droid](https://factory.ai) - Official Droid CLI tool

## Integration Points

### MCP Client Configuration
**Claude Code:**
```bash
claude mcp add droid-rs -s user --transport stdio -- /path/to/droid-mcp-rs
```

**Claude Desktop** (`settings.json`):
```json
{
  "mcpServers": {
    "droid": {
      "command": "/path/to/droid-mcp-rs"
    }
  }
}
```

### Custom Model Usage
- Models are referenced as `custom:Display-Name-index` (e.g., `custom:Sonnet-4.5-[88code]-0`)
- Index corresponds to position in `~/.factory/config.json` custom_models array
- Server automatically lists available models in tool instructions

### DROID.md Context
- Place `DROID.md` in working directory for project-specific instructions
- Content is prepended to all prompts as `<system_prompt>...</system_prompt>`
- Automatically truncated at 1MB with UTF-8 boundary awareness
