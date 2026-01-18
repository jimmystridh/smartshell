# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run -- <subcommand>  # Run with arguments
cargo test               # Run tests
cargo clippy             # Lint
cargo fmt                # Format code
```

## Architecture

smartshell is a Rust CLI tool that uses LLMs (OpenAI GPT-4o or Claude) to generate and explain zsh commands.

### CLI Structure (clap derive)

Single-file application (`src/main.rs`) with subcommands:
- `complete` - Generate zsh commands from natural language queries, optionally modifying an existing command buffer
- `explain` - Explain what a zsh command does
- `toggle-provider` - Switch between OpenAI and Claude APIs
- `os-name` - Print detected OS/distribution
- `preflight` - Verify required environment and dependencies

### LLM Provider System

- Provider selected via `LZSH_LLM_PROVIDER` env var (defaults to "openai")
- OpenAI: requires `OPENAI_API_KEY`, uses gpt-4o model
- Claude: requires `ANTHROPIC_API_KEY`, uses claude-3-7-sonnet-20250219 model
- API calls run in background thread with spinner animation

### Key Functions

- `llm_api_call()` - Orchestrates async API call with spinner, dispatches to provider
- `openai_api_call()` / `claude_api_call()` - Provider-specific request/response handling
- `get_distribution_name()` - OS detection for context-aware command generation
- `preflight_check()` - Validates API keys and required binaries (jq, curl/wget)
