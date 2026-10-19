//! Home assistant: turns a request like "create a workspace called X and do Y" into an intent
//! (new workspace + team request) or a plain answer. New workspaces live under one base folder,
//! so trusting that folder once in Claude Code covers all of them.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const BASE_DIR: &str = "INA Projects";
pub const MAX_REQUEST_LEN: usize = 4000;
const MAX_NAME: usize = 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Intent {
    /// "create" (new workspace + team) or "answer" (just reply).
    pub action: String,
    #[serde(default)]
    pub workspace_name: String,
    /// What the team should do, as a request for the planner.
    #[serde(default)]
    pub task: String,
    /// Short, plain reply to show the user.
    pub reply: String,
}

pub const INTENT_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["action","workspaceName","task","reply"],"properties":{"action":{"type":"string","enum":["create","answer"]},"workspaceName":{"type":"string","maxLength":60},"task":{"type":"string","maxLength":4000},"reply":{"type":"string","maxLength":1000}}}"#;

pub fn base(home: &Path) -> PathBuf {
    home.join(BASE_DIR)
}

pub fn intent_prompt(request: &str) -> String {
    format!(
        "You are the assistant on INA's home screen. INA runs teams of AI coding agents (one \
         orchestrator plus agents) in workspaces (project folders). Read the user's message. If \
         they want something built or done in a new project, answer with action \"create\", a \
         short workspaceName for the new folder (use the name they gave, else invent a short \
         one; letters, numbers, spaces, - or _), and task: a clear request describing the work \
         and any team the user asked for (number of agents, roles), written for a planner. \
         Otherwise use action \"answer\" with empty workspaceName and task. In every case, reply \
         in the user's language with 1-2 short plain sentences (for \"create\": what you are \
         about to set up). No code or jargon.\n\nUser message:\n{request}"
    )
}

pub fn intent_args(request: &str) -> Vec<String> {
    [
        "-p",
        &intent_prompt(request),
        "--output-format",
        "json",
        "--json-schema",
        INTENT_SCHEMA,
        "--tools",
        "",
        "--strict-mcp-config",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn parse_output(stdout: &str) -> Result<Intent, String> {
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(default)]
        is_error: bool,
        #[serde(default)]
        result: Option<String>,
        structured_output: Option<Intent>,
    }
    let env: Envelope =
        serde_json::from_str(stdout.trim()).map_err(|e| format!("unparseable output: {e}"))?;
    if env.is_error {
        return Err(env.result.unwrap_or_else(|| "unknown error".into()));
    }
    let mut intent = env.structured_output.ok_or("no structured output")?;
    if intent.action == "create" {
        intent.workspace_name = valid_name(&intent.workspace_name)?;
        if intent.task.trim().is_empty() {
            return Err("no task for the new workspace".into());
        }
    } else if intent.action != "answer" {
        return Err(format!("unknown action {:?}", intent.action));
    }
    Ok(intent)
}

/// A folder name that is safe on Windows and stays inside the base folder.
pub fn valid_name(name: &str) -> Result<String, String> {
    let name = name.trim().trim_end_matches('.').trim();
    let ok_chars = name
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'));
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if name.is_empty()
        || name.chars().count() > MAX_NAME
        || !ok_chars
        || name.starts_with('.')
        || reserved
    {
        return Err(format!("invalid workspace name {name:?}"));
    }
    Ok(name.to_string())
}

/// Creates `<base>/<name>`; refuses a folder that already has files in it.
pub fn create_dir(base: &Path, name: &str) -> Result<PathBuf, String> {
    let dir = base.join(valid_name(name)?);
    if dir.exists() {
        let empty = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .next()
            .is_none();
        if !empty {
            return Err(format!("folder already exists: {}", dir.display()));
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_checked() {
        assert_eq!(valid_name("  Loja Online ").unwrap(), "Loja Online");
        assert_eq!(valid_name("app-v2_final").unwrap(), "app-v2_final");
        assert_eq!(valid_name("café").unwrap(), "café");
        for bad in [
            "", "..", "../x", "a/b", "a\\b", "C:", "con", "COM1", "lpt9.txt", ".git",
        ] {
            assert!(valid_name(bad).is_err(), "{bad:?} accepted");
        }
        assert!(valid_name(&"x".repeat(61)).is_err());
    }

    #[test]
    fn parses_intents() {
        let out = |s: &str| format!(r#"{{"is_error":false,"structured_output":{s}}}"#);
        let create = parse_output(&out(
            r#"{"action":"create","workspaceName":"Loja","task":"build a shop","reply":"ok"}"#,
        ))
        .unwrap();
        assert_eq!(create.workspace_name, "Loja");
        let answer = parse_output(&out(
            r#"{"action":"answer","workspaceName":"","task":"","reply":"hi"}"#,
        ))
        .unwrap();
        assert_eq!(answer.reply, "hi");
        assert!(
            parse_output(&out(
                r#"{"action":"create","workspaceName":"../x","task":"t","reply":"r"}"#
            ))
            .is_err()
        );
        assert!(
            parse_output(&out(
                r#"{"action":"create","workspaceName":"ok","task":" ","reply":"r"}"#
            ))
            .is_err()
        );
        assert!(parse_output(r#"{"is_error":true,"result":"boom"}"#).is_err());
    }

    #[test]
    fn creates_only_new_or_empty_folders() {
        let base = std::env::temp_dir().join(format!("ina-home-base-{}", std::process::id()));
        let dir = create_dir(&base, "Projeto").unwrap();
        assert!(dir.is_dir());
        assert_eq!(create_dir(&base, "Projeto").unwrap(), dir);
        std::fs::write(dir.join("f"), "x").unwrap();
        assert!(create_dir(&base, "Projeto").is_err());
        assert!(create_dir(&base, "../fora").is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
