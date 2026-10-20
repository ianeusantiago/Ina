//! One-shot, tool-less `claude -p` calls that answer with JSON matching a schema: the planner,
//! the Home assistant, the final report and the reviewer all go through here.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

/// `claude -p` args: JSON output checked against `schema`, no tools, no MCP servers.
pub fn args(prompt: &str, schema: &str) -> Vec<String> {
    [
        "-p",
        prompt,
        "--output-format",
        "json",
        "--json-schema",
        schema,
        "--tools",
        "",
        "--strict-mcp-config",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Reads Claude Code's JSON envelope (`is_error`, `result`, `structured_output`).
pub fn parse<T: DeserializeOwned>(stdout: &str) -> Result<T, String> {
    #[derive(Deserialize)]
    struct Envelope<T> {
        #[serde(default)]
        is_error: bool,
        #[serde(default)]
        result: Option<String>,
        structured_output: Option<T>,
    }
    let env: Envelope<T> =
        serde_json::from_str(stdout.trim()).map_err(|e| format!("unparseable output: {e}"))?;
    if env.is_error {
        return Err(env.result.unwrap_or_else(|| "unknown error".into()));
    }
    env.structured_output
        .ok_or_else(|| "no structured output".into())
}

/// Runs an authorized `claude -p` spec and parses its answer. A failed run with no output
/// reports the exit code and the end of stderr; a run past `timeout` is killed.
pub fn run<T: DeserializeOwned>(
    spec: &crate::pty::SpawnSpec,
    timeout: Duration,
) -> Result<T, String> {
    let mut child = crate::pty::piped_command(spec)
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;
    let (status, output) =
        crate::pty::wait_output(&mut child, timeout).map_err(|e| e.to_string())?;
    let status = status.ok_or("timed out")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !status.success() && stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "exit {:?}: {}",
            status.code(),
            crate::pty::tail(&stderr, 500)
        ));
    }
    parse(&stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Answer {
        ok: bool,
    }

    #[test]
    fn args_disable_tools_and_mcp() {
        let a = args("hi", "{}");
        assert_eq!(a[..2], ["-p", "hi"]);
        let tools = a.iter().position(|x| x == "--tools").unwrap();
        assert_eq!(a[tools + 1], "");
        assert!(a.contains(&"--strict-mcp-config".to_string()));
    }

    #[test]
    fn parses_the_envelope() {
        let ok: Answer = parse(r#"{"structured_output":{"ok":true}}"#).unwrap();
        assert_eq!(ok, Answer { ok: true });
        assert_eq!(
            parse::<Answer>(r#"{"is_error":true,"result":"boom"}"#).unwrap_err(),
            "boom"
        );
        assert_eq!(parse::<Answer>("{}").unwrap_err(), "no structured output");
        assert!(
            parse::<Answer>("not json")
                .unwrap_err()
                .starts_with("unparseable")
        );
    }
}
