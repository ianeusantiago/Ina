//! What an agent did, read from the CLI's own session log (not the terminal screen).
//!
//! Claude Code: `<config>/projects/<cwd with non-alphanumerics as '-'>/<session>.jsonl`.
//! Codex: `<CODEX_HOME>/sessions/YYYY/MM/DD/rollout-*.jsonl` whose `session_meta` has the cwd.
//! The newest log for the agent's cwd is used.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use serde_json::Value;

use crate::plan::AgentCli;

/// Only the tail of very long logs is read.
const MAX_READ: u64 = 8 << 20;
pub const MAX_EVENTS: usize = 400;
const MAX_TEXT: usize = 2000;
const MAX_DETAIL: usize = 160;
/// Codex logs older than this are not searched.
const CODEX_DAYS: u64 = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    Shell,
    Edit,
    Write,
    Read,
    Search,
    Web,
    Agent,
    Todo,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Event {
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Tool {
        tool: Tool,
        /// Command, file path, pattern or tool name (shortened).
        detail: String,
        added: u32,
        removed: u32,
    },
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

fn first_line(s: &str) -> String {
    clip(s.trim().lines().next().unwrap_or(""), MAX_DETAIL)
}

fn lines(v: &Value) -> u32 {
    v.as_str()
        .map(|s| {
            if s.is_empty() {
                0
            } else {
                s.lines().count() as u32
            }
        })
        .unwrap_or(0)
}

fn tool(tool: Tool, detail: &str) -> Event {
    Event::Tool {
        tool,
        detail: first_line(detail),
        added: 0,
        removed: 0,
    }
}

/// Prompts INA or the CLI inject (`<command-name>`, `<environment_context>`, ...).
fn injected(text: &str) -> bool {
    text.trim_start().starts_with('<')
}

fn text_event(user: bool, text: &str) -> Option<Event> {
    let text = text.trim();
    if text.is_empty() || (user && injected(text)) {
        return None;
    }
    let text = clip(text, MAX_TEXT);
    Some(if user {
        Event::User { text }
    } else {
        Event::Assistant { text }
    })
}

fn claude_tool(name: &str, input: &Value) -> Event {
    let s = |k: &str| input[k].as_str().unwrap_or("");
    match name {
        "Bash" | "PowerShell" => tool(Tool::Shell, s("command")),
        "Edit" | "MultiEdit" => {
            let (added, removed) = match input["edits"].as_array() {
                Some(edits) => edits.iter().fold((0, 0), |(a, r), e| {
                    (a + lines(&e["new_string"]), r + lines(&e["old_string"]))
                }),
                None => (lines(&input["new_string"]), lines(&input["old_string"])),
            };
            Event::Tool {
                tool: Tool::Edit,
                detail: first_line(s("file_path")),
                added,
                removed,
            }
        }
        "Write" => Event::Tool {
            tool: Tool::Write,
            detail: first_line(s("file_path")),
            added: lines(&input["content"]),
            removed: 0,
        },
        "Read" => tool(Tool::Read, s("file_path")),
        "Grep" | "Glob" => tool(Tool::Search, s("pattern")),
        "WebFetch" => tool(Tool::Web, s("url")),
        "WebSearch" => tool(Tool::Web, s("query")),
        "Task" | "Agent" => tool(Tool::Agent, s("description")),
        "TodoWrite" => tool(Tool::Todo, ""),
        other => tool(Tool::Other, other),
    }
}

/// Events in one Claude Code transcript line.
pub fn parse_claude(v: &Value) -> Vec<Event> {
    let role = v["type"].as_str().unwrap_or("");
    if !matches!(role, "user" | "assistant") || v["isMeta"].as_bool() == Some(true) {
        return vec![];
    }
    let user = role == "user";
    let content = &v["message"]["content"];
    if let Some(text) = content.as_str() {
        return text_event(user, text).into_iter().collect();
    }
    content
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| match b["type"].as_str()? {
                    "text" => text_event(user, b["text"].as_str()?),
                    "tool_use" if !user => Some(claude_tool(b["name"].as_str()?, &b["input"])),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One event per file in an `apply_patch` body.
fn codex_patch(patch: &str) -> Vec<Event> {
    let mut out: Vec<Event> = vec![];
    for line in patch.lines() {
        let file = ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
            .iter()
            .find_map(|p| line.strip_prefix(p).map(|f| (p.contains("Add"), f)));
        if let Some((new, f)) = file {
            out.push(Event::Tool {
                tool: if new { Tool::Write } else { Tool::Edit },
                detail: first_line(f),
                added: 0,
                removed: 0,
            });
        } else if let Some(Event::Tool { added, removed, .. }) = out.last_mut() {
            if line.starts_with('+') {
                *added += 1;
            } else if line.starts_with('-') {
                *removed += 1;
            }
        }
    }
    out
}

fn codex_command(args: &Value) -> String {
    let cmd = if args["command"].is_null() {
        &args["cmd"]
    } else {
        &args["command"]
    };
    match cmd {
        Value::Array(parts) => {
            let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
            // `["powershell", "-Command", "<script>"]`: show the script.
            parts.last().map(|s| s.to_string()).unwrap_or_default()
        }
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Events in one Codex rollout line.
pub fn parse_codex(v: &Value) -> Vec<Event> {
    if v["type"] != "response_item" {
        return vec![];
    }
    let p = &v["payload"];
    match p["type"].as_str().unwrap_or("") {
        "message" => {
            let user = p["role"] == "user";
            if !user && p["role"] != "assistant" {
                return vec![];
            }
            p["content"]
                .as_array()
                .map(|c| {
                    c.iter()
                        .filter_map(|b| text_event(user, b["text"].as_str()?))
                        .collect()
                })
                .unwrap_or_default()
        }
        "function_call" | "local_shell_call" => {
            let name = p["name"].as_str().unwrap_or("shell");
            let args = p["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str::<Value>(a).ok())
                .unwrap_or_else(|| p["action"].clone());
            if name == "apply_patch" {
                return codex_patch(args["input"].as_str().unwrap_or(""));
            }
            let command = codex_command(&args);
            if command.is_empty() {
                vec![tool(Tool::Other, name)]
            } else {
                vec![tool(Tool::Shell, &command)]
            }
        }
        "custom_tool_call" => match p["name"].as_str() {
            Some("apply_patch") => codex_patch(p["input"].as_str().unwrap_or("")),
            Some(name) => vec![tool(Tool::Other, name)],
            None => vec![],
        },
        "web_search_call" => vec![tool(Tool::Web, p["action"]["query"].as_str().unwrap_or(""))],
        _ => vec![],
    }
}

/// Claude Code's project folder name for `cwd`.
pub fn claude_project_dir(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn home() -> Option<PathBuf> {
    crate::policy::home_dir()
}

fn config_dir(var: &str, default: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(default)))
}

fn modified(p: &Path) -> SystemTime {
    p.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn jsonl_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
                .collect()
        })
        .unwrap_or_default()
}

fn newest(files: Vec<PathBuf>) -> Option<PathBuf> {
    files.into_iter().max_by_key(|p| modified(p))
}

fn same_dir(a: &str, b: &Path) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm(a) == norm(&b.to_string_lossy())
}

fn codex_cwd(file: &Path) -> Option<String> {
    let mut line = String::new();
    BufReader::new(File::open(file).ok()?)
        .take(256 << 10)
        .read_line(&mut line)
        .ok()?;
    let v: Value = serde_json::from_str(&line).ok()?;
    (v["type"] == "session_meta").then(|| v["payload"]["cwd"].as_str().map(String::from))?
}

/// Random UUID v4 for `claude --session-id` (no extra dependency: std's random hasher keys).
pub fn new_session_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    let random = || {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        );
        h.finish()
    };
    let (a, b) = (random(), random());
    let n = ((a as u128) << 64 | b as u128) & !(0xf000 << 64) & !(0xc000 << 48);
    let n = n | (0x4000 << 64) | (0x8000 << 48);
    let h = format!("{n:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// Session log of `cli` in `cwd`: the Claude session `session_id` if INA knows it (none until
/// that session writes), else the newest log for the folder.
pub fn find_log(cli: AgentCli, cwd: &Path, session_id: Option<&str>) -> Option<PathBuf> {
    match cli {
        AgentCli::Claude => {
            let projects = config_dir("CLAUDE_CONFIG_DIR", ".claude")?.join("projects");
            let dir = projects.join(claude_project_dir(cwd));
            match session_id {
                Some(id) => {
                    let name = format!("{id}.jsonl");
                    let direct = dir.join(&name);
                    if direct.is_file() {
                        return Some(direct);
                    }
                    // Very long folder names are shortened by Claude Code: search by file name.
                    std::fs::read_dir(&projects)
                        .ok()?
                        .filter_map(Result::ok)
                        .map(|e| e.path().join(&name))
                        .find(|p| p.is_file())
                }
                None => newest(jsonl_in(&dir)),
            }
        }
        AgentCli::Codex => {
            let root = config_dir("CODEX_HOME", ".codex")?.join("sessions");
            let since = SystemTime::now() - Duration::from_secs(CODEX_DAYS * 86_400);
            let mut files = vec![];
            // sessions/YYYY/MM/DD/*.jsonl
            let sub = |d: &Path| -> Vec<PathBuf> {
                std::fs::read_dir(d)
                    .map(|rd| {
                        rd.filter_map(Result::ok)
                            .map(|e| e.path())
                            .filter(|p| p.is_dir())
                            .collect()
                    })
                    .unwrap_or_default()
            };
            for y in sub(&root) {
                for m in sub(&y) {
                    for d in sub(&m) {
                        files.extend(jsonl_in(&d).into_iter().filter(|f| modified(f) >= since));
                    }
                }
            }
            files.sort_by_key(|f| std::cmp::Reverse(modified(f)));
            files
                .into_iter()
                .find(|f| codex_cwd(f).is_some_and(|c| same_dir(&c, cwd)))
        }
    }
}

/// Events of a log file (its tail if very long), at most `MAX_EVENTS`, oldest first.
pub fn read_log(cli: AgentCli, file: &Path) -> std::io::Result<Vec<Event>> {
    let mut f = File::open(file)?;
    let len = f.metadata()?.len();
    let skip_partial = len > MAX_READ;
    if skip_partial {
        f.seek(SeekFrom::Start(len - MAX_READ))?;
    }
    let mut lines = BufReader::new(f).lines().map_while(Result::ok);
    if skip_partial {
        lines.next();
    }
    let parse = match cli {
        AgentCli::Claude => parse_claude,
        AgentCli::Codex => parse_codex,
    };
    let mut events: Vec<Event> = lines
        .filter_map(|l| serde_json::from_str::<Value>(&l).ok())
        .flat_map(|v| parse(&v))
        .collect();
    let extra = events.len().saturating_sub(MAX_EVENTS);
    events.drain(..extra);
    Ok(events)
}

/// Text typed in the interface, as sent to the agent's terminal: one line, no control chars.
pub fn terminal_input(text: &str) -> Option<String> {
    let line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let line = line.trim();
    (!line.is_empty() && line.chars().count() <= 4000).then(|| line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn t(tool: Tool, detail: &str, added: u32, removed: u32) -> Event {
        Event::Tool {
            tool,
            detail: detail.into(),
            added,
            removed,
        }
    }

    #[test]
    fn claude_tools_and_text() {
        let v = json!({"type": "assistant", "message": {"content": [
            {"type": "text", "text": "Done."},
            {"type": "tool_use", "name": "Bash", "input": {"command": "npm test\necho ok"}},
            {"type": "tool_use", "name": "Edit", "input": {"file_path": "a.ts", "old_string": "x", "new_string": "y\nz"}},
            {"type": "tool_use", "name": "Write", "input": {"file_path": "b.ts", "content": "1\n2\n3"}},
            {"type": "tool_use", "name": "mcp__x__y", "input": {}}
        ]}});
        assert_eq!(
            parse_claude(&v),
            vec![
                Event::Assistant {
                    text: "Done.".into()
                },
                t(Tool::Shell, "npm test", 0, 0),
                t(Tool::Edit, "a.ts", 2, 1),
                t(Tool::Write, "b.ts", 3, 0),
                t(Tool::Other, "mcp__x__y", 0, 0),
            ]
        );
    }

    #[test]
    fn claude_skips_injected_meta_and_tool_results() {
        let user = |c: Value| json!({"type": "user", "message": {"content": c}});
        assert_eq!(
            parse_claude(&user(json!("fix the bug"))),
            vec![Event::User {
                text: "fix the bug".into()
            }]
        );
        assert!(parse_claude(&user(json!("<command-name>/x</command-name>"))).is_empty());
        assert!(parse_claude(&user(json!([{"type": "tool_result", "content": "out"}]))).is_empty());
        assert!(
            parse_claude(&json!({"type": "user", "isMeta": true, "message": {"content": "x"}}))
                .is_empty()
        );
        assert!(parse_claude(&json!({"type": "summary"})).is_empty());
    }

    #[test]
    fn codex_shell_patch_and_messages() {
        let item = |p: Value| json!({"type": "response_item", "payload": p});
        let shell = item(json!({"type": "function_call", "name": "shell",
            "arguments": "{\"command\":[\"powershell\",\"-Command\",\"cargo test\"]}"}));
        assert_eq!(
            parse_codex(&shell),
            vec![t(Tool::Shell, "cargo test", 0, 0)]
        );
        let patch = item(json!({"type": "custom_tool_call", "name": "apply_patch",
            "input": "*** Begin Patch\n*** Update File: src/a.rs\n-old\n+new\n+more\n*** Add File: b.rs\n+x\n*** End Patch"}));
        assert_eq!(
            parse_codex(&patch),
            vec![
                t(Tool::Edit, "src/a.rs", 2, 1),
                t(Tool::Write, "b.rs", 1, 0)
            ]
        );
        let msg = item(json!({"type": "message", "role": "assistant",
            "content": [{"type": "output_text", "text": "All set"}]}));
        assert_eq!(
            parse_codex(&msg),
            vec![Event::Assistant {
                text: "All set".into()
            }]
        );
        let env = item(json!({"type": "message", "role": "user",
            "content": [{"type": "input_text", "text": "<environment_context>x</environment_context>"}]}));
        assert!(parse_codex(&env).is_empty());
    }

    #[test]
    fn project_dir_matches_claude_naming() {
        assert_eq!(
            claude_project_dir(Path::new("C:\\Users\\me\\ws.todo")),
            "C--Users-me-ws-todo"
        );
    }

    #[test]
    fn reads_log_tail_capped() {
        let dir = std::env::temp_dir().join(format!("ina-activity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.jsonl");
        let line = json!({"type": "user", "message": {"content": "hi"}}).to_string();
        let body: String = (0..MAX_EVENTS + 5)
            .map(|_| format!("{line}\nnot json\n"))
            .collect();
        std::fs::write(&file, body).unwrap();
        let events = read_log(AgentCli::Claude, &file).unwrap();
        assert_eq!(events.len(), MAX_EVENTS);
        assert_eq!(find_log(AgentCli::Claude, &dir.join("missing"), None), None);
        assert_eq!(
            find_log(
                AgentCli::Claude,
                &dir,
                Some("00000000-0000-4000-8000-000000000000")
            ),
            None
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn session_ids_are_distinct_uuid_v4() {
        let (a, b) = (new_session_id(), new_session_id());
        assert_ne!(a, b);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'));
        assert!(matches!(
            parts[3].chars().next(),
            Some('8' | '9' | 'a' | 'b')
        ));
    }

    #[test]
    fn terminal_input_is_one_safe_line() {
        assert_eq!(terminal_input(" a\nb\x1b[2J "), Some("a b [2J".into()));
        assert_eq!(terminal_input("\n\t"), None);
        assert!(terminal_input(&"x".repeat(4001)).is_none());
    }
}
