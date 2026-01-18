use clap::{Parser, Subcommand};
use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};

#[derive(Parser)]
#[command(author, version, about = "smartshell: LLM-powered zsh CLI helper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a zsh command from a query or modify an existing one
    Complete {
        #[arg(short, long)]
        buffer: Option<String>,
        #[arg(short, long)]
        query: Option<String>,
    },
    /// Explain the current zsh command
    Explain {
        #[arg(short, long)]
        buffer: Option<String>,
    },
}

fn get_os_context() -> String {
    if cfg!(target_os = "macos") {
        "The target system is macOS.".to_string()
    } else if cfg!(target_os = "linux") {
        "The target system is Linux.".to_string()
    } else {
        String::new()
    }
}

fn get_api_key(provider: &str) -> Option<String> {
    // Check env vars first
    if let Some(key) = env::var("SMSH_API_KEY").ok().filter(|k| !k.is_empty()) {
        return Some(key);
    }
    let env_key = match provider {
        "openai" => env::var("SMSH_OPENAI_API_KEY").or_else(|_| env::var("OPENAI_API_KEY")),
        "claude" => env::var("SMSH_ANTHROPIC_API_KEY").or_else(|_| env::var("ANTHROPIC_API_KEY")),
        _ => return None,
    }.ok().filter(|k| !k.is_empty());
    if env_key.is_some() {
        return env_key;
    }

    // Fall back to macOS Keychain
    #[cfg(target_os = "macos")]
    {
        let service = match provider {
            "openai" => "smartshell.openai",
            "claude" => "smartshell.anthropic",
            _ => return None,
        };
        if let Ok(entry) = keyring::Entry::new(service, &whoami::username()) {
            return entry.get_password().ok();
        }
    }
    None
}

fn log_entry(cmd: &str, query: &str, result: &str) {
    if let Some(path) = env::var("SMSH_LOG").ok().filter(|p| !p.is_empty()) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] {} | query: {} | result: {}", ts, cmd, query, result);
        }
    }
}

fn response_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "response",
        "strict": true,
        "schema": {
            "type": "object",
            "properties": {
                "result": { "type": "string", "description": "The command or explanation" },
                "error": { "type": "boolean", "description": "Set to true if the request is unclear, impossible, or not a valid shell task" }
            },
            "required": ["result", "error"],
            "additionalProperties": false
        }
    })
}

fn llm_api_call(intro: &str, prompt: &str) -> Result<String, String> {
    let provider = env::var("SMSH_LLM_PROVIDER").unwrap_or_else(|_| "openai".to_string());
    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let (tx, rx) = std::sync::mpsc::channel();
    let schema = response_schema();

    let intro = intro.to_string();
    let prompt = prompt.to_string();
    std::thread::spawn(move || {
        let result = match provider.as_str() {
            "openai" => openai_call(&intro, &prompt, &schema),
            "claude" => claude_call(&intro, &prompt, &schema["schema"]),
            _ => Err(format!("Unknown provider: {}", provider)),
        };
        let _ = tx.send(result);
    });

    let mut idx = 0;
    let mut tty = std::fs::OpenOptions::new().write(true).open("/dev/tty").ok();
    loop {
        match rx.try_recv() {
            Ok(result) => {
                if let Some(ref mut t) = tty { let _ = write!(t, "\r\x1b[K"); }
                return result;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if let Some(ref mut t) = tty {
                    let _ = write!(t, "\r{}", spinner[idx % spinner.len()]);
                    let _ = t.flush();
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                idx += 1;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if let Some(ref mut t) = tty { let _ = write!(t, "\r\x1b[K"); }
                return Err("Background thread failed".to_string());
            }
        }
    }
}

fn openai_call(intro: &str, prompt: &str, schema: &serde_json::Value) -> Result<String, String> {
    let api_key = get_api_key("openai").ok_or("OpenAI API key not set")?;
    let resp = reqwest::blocking::Client::new()
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": "gpt-4o",
            "max_tokens": 256,
            "temperature": 0,
            "messages": [
                {"role": "system", "content": intro},
                {"role": "user", "content": prompt}
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": schema
            }
        }))
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;

    let json: serde_json::Value = resp.json().map_err(|e| format!("Invalid response: {}", e))?;
    if let Some(err) = json.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
        return Err(format!("API error: {}", err));
    }
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("Missing content in response")?;
    let parsed: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| format!("Failed to parse response JSON: {}", e))?;
    let result = parsed["result"].as_str().unwrap_or("").to_string();
    let is_error = parsed["error"].as_bool().unwrap_or(false);
    if is_error {
        Err(result)
    } else {
        Ok(result)
    }
}

fn claude_call(intro: &str, prompt: &str, schema: &serde_json::Value) -> Result<String, String> {
    let api_key = get_api_key("claude").ok_or("Anthropic API key not set")?;
    let tool = serde_json::json!({
        "name": "structured_response",
        "description": "Return the structured response",
        "input_schema": schema
    });
    let resp = reqwest::blocking::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({
            "model": "claude-sonnet-4-5-20250929",
            "max_tokens": 512,
            "temperature": 0,
            "system": intro,
            "messages": [{"role": "user", "content": prompt}],
            "tools": [tool],
            "tool_choice": {"type": "tool", "name": "structured_response"}
        }))
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;

    let json: serde_json::Value = resp.json().map_err(|e| format!("Invalid response: {}", e))?;
    if let Some(err) = json.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
        return Err(format!("API error: {}", err));
    }
    let input = &json["content"][0]["input"];
    let result = input["result"].as_str().unwrap_or("").to_string();
    let is_error = input["error"].as_bool().unwrap_or(false);
    if is_error {
        Err(result)
    } else {
        Ok(result)
    }
}

fn main() {
    let cli = Cli::parse();
    let os = get_os_context();

    match cli.command {
        Commands::Complete { buffer, query } => {
            let query = query.or_else(|| {
                print!("> Query: ");
                io::stdout().flush().unwrap();
                let mut s = String::new();
                io::stdin().read_line(&mut s).ok().map(|_| s.trim().to_string())
            }).unwrap_or_default();

            if query.is_empty() {
                println!("Completion aborted (empty input).");
                return;
            }

            let intro = format!(
                "Generate a zsh command. Use only ASCII characters (straight quotes, no curly quotes). \
                If the request is unclear or not a valid shell task, set error=true and put an explanation in result. {}", os
            );
            let prompt = match &buffer {
                Some(b) if !b.is_empty() => format!("Alter zsh command `{}` to comply with query `{}`", b, query),
                _ => query.clone(),
            };

            match llm_api_call(&intro, &prompt) {
                Ok(text) if text.starts_with('#') => {
                    log_entry("complete", &query, &text);
                    println!("{}", text);
                    std::process::exit(1);
                }
                Ok(text) => {
                    log_entry("complete", &query, &text);
                    println!("{}", text);
                }
                Err(e) if e.starts_with("Request failed") || e.starts_with("API error") || e.starts_with("Invalid response") || e.starts_with("Missing") || e.starts_with("Failed to parse") || e.contains("API key") => {
                    log_entry("complete", &query, &format!("ERROR: {}", e));
                    println!("{}", e);
                    std::process::exit(1);
                }
                Err(e) => {
                    log_entry("complete", &query, &format!("REFUSED: {}", e));
                    println!("# {}", e);
                    std::process::exit(2);
                }
            }
        }
        Commands::Explain { buffer } => {
            let buffer = buffer.unwrap_or_default();
            if buffer.is_empty() {
                println!("Nothing to explain.");
                return;
            }

            let intro = format!(
                "Explain zsh commands. Return a short, single-line explanation in the result field. {}", os
            );

            match llm_api_call(&intro, &buffer) {
                Ok(text) => {
                    log_entry("explain", &buffer, &text);
                    println!("# {}", text);
                }
                Err(e) => {
                    log_entry("explain", &buffer, &format!("ERROR: {}", e));
                    println!("{}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
