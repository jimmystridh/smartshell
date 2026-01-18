# smartshell

LLM-powered zsh command generation. Press `Ctrl+G`, describe what you want, get a command.

```
> Query: find rust files modified this week
find . -name "*.rs" -mtime -7

> Query: compress directory excluding node_modules
tar --exclude='node_modules' -czvf archive.tar.gz .
```

## Install

```bash
cargo build --release
cp target/release/smartshell ~/.local/bin/
cp smartshell.zsh ~/.local/bin/
echo 'source ~/.local/bin/smartshell.zsh' >> ~/.zshrc
```

## API Keys

**macOS Keychain (recommended):**

```bash
security add-generic-password -U -a "$USER" -s "smartshell.openai" -w "sk-..."
security add-generic-password -U -a "$USER" -s "smartshell.anthropic" -w "sk-ant-..."
```

**Environment variables:** `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`

## Keybindings

| Key | Action |
|-----|--------|
| `Ctrl+G` | Generate command from natural language |
| `Ctrl+E` | Explain current command line |
| `Ctrl+T` | Toggle OpenAI / Claude |

Customize:

```bash
export SMSH_COMPLETE_KEY='^G'
export SMSH_EXPLAIN_KEY='^E'
export SMSH_TOGGLE_KEY='^T'
```

## Config

```bash
export SMSH_LLM_PROVIDER=claude   # default: openai
export SMSH_LOG=~/.smartshell.log # debug logging
```

## CLI

```bash
smartshell complete --query "list large files"
smartshell complete --query "add verbose" --buffer "rsync src/ dest/"
smartshell explain --buffer "tar -xzvf archive.tar.gz"
```

## License

MIT
