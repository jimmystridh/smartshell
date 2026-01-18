#!/usr/bin/env zsh
# smartshell.zsh - Source in .zshrc: source /path/to/smartshell/smartshell.zsh

if ! command -v smartshell &> /dev/null; then
  local script_dir="${0:A:h}"
  if [[ -x "$script_dir/target/release/smartshell" ]]; then
    path+=("$script_dir/target/release")
  elif [[ -x "$script_dir/target/debug/smartshell" ]]; then
    path+=("$script_dir/target/debug")
  else
    echo "smartshell: binary not found. Run 'cargo build --release' first."
  fi
fi

SMSH_LLM_PROVIDER=${SMSH_LLM_PROVIDER:-openai}

# Keychain/env API key lookup
__smsh_get_api_key() {
  emulate -L zsh
  local provider="$1"

  if [[ "$provider" == "openai" ]]; then
    [[ -n "$SMSH_OPENAI_API_KEY" ]] && { echo "$SMSH_OPENAI_API_KEY"; return 0; }
    [[ -n "$OPENAI_API_KEY" ]] && { echo "$OPENAI_API_KEY"; return 0; }
  elif [[ "$provider" == "claude" ]]; then
    [[ -n "$SMSH_ANTHROPIC_API_KEY" ]] && { echo "$SMSH_ANTHROPIC_API_KEY"; return 0; }
    [[ -n "$ANTHROPIC_API_KEY" ]] && { echo "$ANTHROPIC_API_KEY"; return 0; }
  fi

  [[ "$OSTYPE" != darwin* ]] && return 1
  command -v security &> /dev/null || return 1

  local service account
  case "$provider" in
    openai) service="${SMSH_OPENAI_KEYCHAIN_SERVICE:-smartshell.openai}"; account="${SMSH_OPENAI_KEYCHAIN_ACCOUNT:-$USER}" ;;
    claude) service="${SMSH_ANTHROPIC_KEYCHAIN_SERVICE:-smartshell.anthropic}"; account="${SMSH_ANTHROPIC_KEYCHAIN_ACCOUNT:-$USER}" ;;
    *) return 1 ;;
  esac
  security find-generic-password -s "$service" -a "$account" -w 2>/dev/null
}

__smartshell_complete() {
  emulate -L zsh
  local buffer_context="$BUFFER" cursor_position=$CURSOR REPLY read_op_status

  autoload -Uz read-from-minibuffer
  read-from-minibuffer '> Query: '
  read_op_status=$?
  BUFFER="$buffer_context"; CURSOR=$cursor_position

  [[ $read_op_status -ne 0 ]] && { zle -M "Completion aborted."; return 1; }
  [[ -z "$REPLY" ]] && { zle -M "Completion aborted (empty input)."; return 0; }

  local api_key=$(__smsh_get_api_key "$SMSH_LLM_PROVIDER")
  [[ -z "$api_key" ]] && { zle -M "Error: No API key for $SMSH_LLM_PROVIDER"; return 1; }

  local cmd_args=("complete" "--query" "$REPLY")
  [[ -n "$buffer_context" ]] && cmd_args+=("--buffer" "$buffer_context")

  local output exit_code
  output=$(SMSH_API_KEY="$api_key" smartshell "${cmd_args[@]}")
  exit_code=$?

  [[ $exit_code -eq 2 ]] && { zle -M "$output"; return 1; }  # LLM refused
  [[ $exit_code -ne 0 ]] && { zle -M "Error: $output"; return 1; }
  [[ "$output" == \#* ]] && { zle -M "$output"; return 1; }

  BUFFER="$output"; CURSOR=$#BUFFER
  zle redisplay
}

__smartshell_explain() {
  emulate -L zsh
  [[ -z "$BUFFER" ]] && { zle -M "Nothing to explain."; return 0; }

  local api_key=$(__smsh_get_api_key "$SMSH_LLM_PROVIDER")
  [[ -z "$api_key" ]] && { zle -M "Error: No API key for $SMSH_LLM_PROVIDER"; return 1; }

  local output exit_code
  output=$(SMSH_API_KEY="$api_key" smartshell explain --buffer "$BUFFER")
  exit_code=$?

  [[ $exit_code -ne 0 ]] && { zle -M "Error: $output"; return 1; }
  zle -R "$output"
  read -k 1
}

__smartshell_toggle_provider() {
  emulate -L zsh
  if [[ "$SMSH_LLM_PROVIDER" == "openai" ]]; then
    export SMSH_LLM_PROVIDER="claude"
    zle -M "Switched to Claude"
  else
    export SMSH_LLM_PROVIDER="openai"
    zle -M "Switched to OpenAI"
  fi
}

zle -N __smartshell_complete
zle -N __smartshell_explain
zle -N __smartshell_toggle_provider

: ${SMSH_COMPLETE_KEY:=^G}
: ${SMSH_EXPLAIN_KEY:=^E}
: ${SMSH_TOGGLE_KEY:=^T}

[[ -n "$SMSH_COMPLETE_KEY" ]] && bindkey "$SMSH_COMPLETE_KEY" __smartshell_complete
[[ -n "$SMSH_EXPLAIN_KEY" ]] && bindkey "$SMSH_EXPLAIN_KEY" __smartshell_explain
[[ -n "$SMSH_TOGGLE_KEY" ]] && bindkey "$SMSH_TOGGLE_KEY" __smartshell_toggle_provider

if [[ -n "$SMSH_EXPLAIN_KEY" && -n "${ZSH_AUTOSUGGEST_CLEAR_WIDGETS+x}" ]]; then
  (( ${ZSH_AUTOSUGGEST_CLEAR_WIDGETS[(Ie)__smartshell_explain]} )) || \
    ZSH_AUTOSUGGEST_CLEAR_WIDGETS+=(__smartshell_explain)
fi
