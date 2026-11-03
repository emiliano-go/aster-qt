// SPDX-License-Identifier: AGPL-3.0-or-later
use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

fn main() {
    let config = match load_config() {
        Some(config) => config,
        None => {
            eprintln!(
                "aster-mcp: could not read mcp.json. Start Aster Qt first, then retry."
            );
            std::process::exit(1);
        }
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let id = serde_json::from_str::<Value>(&line)
            .ok()
            .and_then(|value| value.get("id").cloned())
            .unwrap_or(Value::Null);
        let response = match ureq::post(&config.url)
            .set("Authorization", &format!("Bearer {}", config.token))
            .set("Content-Type", "application/json")
            .send_string(&line)
        {
            Ok(response) => response.into_string().unwrap_or_default(),
            Err(err) => serde_json::to_string(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": format!("Aster Qt MCP unavailable: {err}") }
            }))
            .unwrap_or_default(),
        };
        let _ = writeln!(stdout, "{response}");
        let _ = stdout.flush();
    }
}

struct Config {
    url: String,
    token: String,
}

fn load_config() -> Option<Config> {
    let path = data_dir().join("mcp.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    Some(Config {
        url: value.get("url")?.as_str()?.to_string(),
        token: value.get("token")?.as_str()?.to_string(),
    })
}

fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ASTER_QT_DATA_DIR") {
        return PathBuf::from(dir);
    }
    match std::env::var_os("XDG_DATA_HOME") {
        Some(dir) => PathBuf::from(dir).join("com.astermail.qt"),
        None => PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".local/share/com.astermail.qt"),
    }
}
