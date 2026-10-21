//! Plan usage limits for Claude Code and Codex.
//!
//! Claude: sessions INA starts get a status line that runs INA itself (`--usage-sink <file>`);
//! Claude Code hands it `rate_limits` on stdin and INA stores them. Codex: `codex app-server`,
//! JSON-RPC `account/rateLimits/read`. No credentials are read by INA.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

pub const SINK_FLAG: &str = "--usage-sink";
pub const CLAUDE_FILE: &str = "usage-claude.json";
const CODEX_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_STDIN: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    /// Window length in minutes (300 = 5-hour session, 10080 = week).
    pub minutes: u32,
    pub used_percent: f64,
    /// Unix seconds.
    pub resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub windows: Vec<Window>,
    /// Unix seconds when the numbers were read.
    pub updated_at: i64,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `resets_at` may come as Unix seconds or an ISO string; only numbers are kept.
fn secs(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

/// Windows from Claude Code's status line `rate_limits` object.
pub fn parse_claude(rate_limits: &Value) -> Vec<Window> {
    [("five_hour", 300), ("seven_day", 10080)]
        .iter()
        .filter_map(|(key, minutes)| {
            let w = rate_limits.get(key)?;
            Some(Window {
                minutes: *minutes,
                used_percent: w.get("used_percentage")?.as_f64()?,
                resets_at: w.get("resets_at").and_then(secs),
            })
        })
        .collect()
}

/// Windows from Codex's `account/rateLimits/read` result.
pub fn parse_codex(result: &Value) -> Vec<Window> {
    let limits = &result["rateLimits"];
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let w = limits.get(key)?;
            Some(Window {
                minutes: u32::try_from(w.get("windowDurationMins")?.as_u64()?).ok()?,
                used_percent: w.get("usedPercent")?.as_f64()?,
                resets_at: w.get("resetsAt").and_then(secs),
            })
        })
        .collect()
}

/// Status line entry point: store the limits Claude Code sends, print a short line.
pub fn run_sink(file: &Path) {
    let mut input = String::new();
    let _ = std::io::stdin().take(MAX_STDIN).read_to_string(&mut input);
    if let Ok(v) = serde_json::from_str::<Value>(input.trim_start_matches('\u{feff}')) {
        let windows = parse_claude(&v["rate_limits"]);
        if !windows.is_empty()
            && let Err(e) = write_claude(file, &windows)
        {
            eprintln!("INA usage: {e}");
        }
    }
    println!("INA");
}

fn write_claude(file: &Path, windows: &[Window]) -> std::io::Result<()> {
    let usage = Usage {
        windows: windows.to_vec(),
        updated_at: now(),
    };
    let tmp = file.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(&usage)?)?;
    std::fs::rename(&tmp, file)
}

pub fn read_claude(file: &Path) -> Option<Usage> {
    let v: Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    let windows = v["windows"]
        .as_array()?
        .iter()
        .filter_map(|w| {
            Some(Window {
                minutes: u32::try_from(w["minutes"].as_u64()?).ok()?,
                used_percent: w["usedPercent"].as_f64()?,
                resets_at: w["resetsAt"].as_i64(),
            })
        })
        .collect();
    Some(Usage {
        windows,
        updated_at: v["updatedAt"].as_i64()?,
    })
}

/// `--settings` JSON giving a Claude session INA's status line. `None` if either path could
/// break out of the quoted command (the status line runs through a shell).
pub fn claude_settings(exe: &Path, file: &Path) -> Option<String> {
    let safe = |p: &Path| {
        let s = p.to_str()?;
        (p.is_absolute() && !s.chars().any(|c| "\"$`%!^&|<>\r\n".contains(c)))
            .then(|| s.replace('\\', "/"))
    };
    let command = format!("\"{}\" {SINK_FLAG} \"{}\"", safe(exe)?, safe(file)?);
    Some(serde_json::json!({ "statusLine": { "type": "command", "command": command } }).to_string())
}

/// Ask `codex app-server` for the account's limits; the child is killed afterwards.
pub fn read_codex(exe: &Path) -> Result<Usage, String> {
    let mut cmd = Command::new(exe);
    for k in crate::policy::child_env_remove() {
        cmd.env_remove(k);
    }
    crate::policy::hide_window(&mut cmd);
    let mut child = cmd
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(m) = serde_json::from_str::<Value>(&line)
                && m["id"] == 2
            {
                let _ = tx.send(m);
                return;
            }
        }
    });
    let requests = [
        serde_json::json!({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "ina", "version": env!("CARGO_PKG_VERSION")}}}),
        serde_json::json!({"method": "initialized"}),
        serde_json::json!({"id": 2, "method": "account/rateLimits/read"}),
    ];
    let sent = requests
        .iter()
        .try_for_each(|r| writeln!(stdin, "{r}"))
        .and_then(|_| stdin.flush());
    let reply = sent.map_err(|e| e.to_string()).and_then(|_| {
        rx.recv_timeout(CODEX_TIMEOUT)
            .map_err(|_| "codex timed out".into())
    });
    let _ = child.kill();
    let _ = child.wait();
    let reply = reply?;
    if let Some(err) = reply.get("error") {
        return Err(err["message"].as_str().unwrap_or("codex error").to_string());
    }
    Ok(Usage {
        windows: parse_codex(&reply["result"]),
        updated_at: now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_claude_status_line_limits() {
        let v = json!({
            "five_hour": {"used_percentage": 42.5, "resets_at": 1_800_000_000},
            "seven_day": {"used_percentage": 10}
        });
        assert_eq!(
            parse_claude(&v),
            vec![
                Window {
                    minutes: 300,
                    used_percent: 42.5,
                    resets_at: Some(1_800_000_000)
                },
                Window {
                    minutes: 10080,
                    used_percent: 10.0,
                    resets_at: None
                },
            ]
        );
        assert!(parse_claude(&Value::Null).is_empty());
    }

    #[test]
    fn parses_codex_rate_limits() {
        let v = json!({"rateLimits": {
            "primary": {"usedPercent": 3, "windowDurationMins": 300, "resetsAt": 5},
            "secondary": null
        }});
        assert_eq!(
            parse_codex(&v),
            vec![Window {
                minutes: 300,
                used_percent: 3.0,
                resets_at: Some(5)
            }]
        );
    }

    #[test]
    fn claude_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("ina-usage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(CLAUDE_FILE);
        let w = vec![Window {
            minutes: 300,
            used_percent: 7.0,
            resets_at: Some(9),
        }];
        write_claude(&file, &w).unwrap();
        assert_eq!(read_claude(&file).unwrap().windows, w);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_reject_shell_metacharacters() {
        let s = claude_settings(Path::new("C:/a/ina.exe"), Path::new("C:/d/u.json")).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(
            v["statusLine"]["command"],
            "\"C:/a/ina.exe\" --usage-sink \"C:/d/u.json\""
        );
        assert!(
            claude_settings(Path::new("C:/a\"&calc/ina.exe"), Path::new("C:/d/u.json")).is_none()
        );
        assert!(
            claude_settings(Path::new("C:/%PATH%/ina.exe"), Path::new("C:/d/u.json")).is_none()
        );
        assert!(claude_settings(Path::new("ina.exe"), Path::new("C:/d/u.json")).is_none());
    }
}
