//! First-run check of Claude Code: installed, onboarded, logged in. Read-only: INA never
//! changes the Claude config, and only presence flags (never account data) leave this module.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::policy::{self, Program};

const VERSION_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSetup {
    pub installed: bool,
    pub version: Option<String>,
    pub onboarded: bool,
    pub logged_in: bool,
    /// Why a check could not run (not "not installed"/"not logged in", which are flags).
    pub error: Option<String>,
}

/// Flags from the Claude Code config. An API key in the environment counts as logged in.
pub fn config_flags(config: &Path, api_key_set: bool) -> Result<(bool, bool), String> {
    let text = match std::fs::read_to_string(config) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((false, api_key_set)),
        Err(e) => return Err(format!("read Claude config: {e}")),
    };
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse Claude config: {e}"))?;
    let onboarded = v["hasCompletedOnboarding"].as_bool().unwrap_or(false);
    let oauth = v["oauthAccount"].as_object().is_some_and(|o| !o.is_empty());
    Ok((onboarded, oauth || api_key_set))
}

fn version(program: &Path) -> Result<String, String> {
    let mut cmd = Command::new(program);
    for k in policy::stripped_env(std::env::vars_os().map(|(k, _)| k)) {
        cmd.env_remove(k);
    }
    let mut child = cmd
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("run claude --version: {e}"))?;
    if crate::pty::wait_timeout(&mut child, VERSION_TIMEOUT)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("claude --version timed out".into());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("claude --version failed: {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

pub fn check() -> ClaudeSetup {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let mut s = ClaudeSetup::default();
    let program = match policy::resolve_program(Program::Claude, &path_var) {
        Ok(p) => p,
        Err(_) => return s,
    };
    s.installed = true;
    match version(&program) {
        Ok(v) => s.version = Some(v),
        Err(e) => s.error = Some(e),
    }
    let api_key = std::env::var_os("ANTHROPIC_API_KEY").is_some_and(|k| !k.is_empty());
    match crate::trust::config_path()
        .map_err(|e| e.to_string())
        .and_then(|c| config_flags(&c, api_key))
    {
        Ok((onboarded, logged_in)) => {
            s.onboarded = onboarded;
            s.logged_in = logged_in;
        }
        Err(e) => s.error = Some(e),
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str, json: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("ina-setup-{name}-{}.json", std::process::id()));
        std::fs::write(&p, json).unwrap();
        p
    }

    #[test]
    fn flags_from_config() {
        let c = config(
            "full",
            r#"{"hasCompletedOnboarding":true,"oauthAccount":{"x":1}}"#,
        );
        assert_eq!(config_flags(&c, false).unwrap(), (true, true));
        let c = config("fresh", r#"{"oauthAccount":{}}"#);
        assert_eq!(config_flags(&c, false).unwrap(), (false, false));
        assert_eq!(config_flags(&c, true).unwrap(), (false, true));
        let missing = std::env::temp_dir().join("ina-setup-missing-nope.json");
        assert_eq!(config_flags(&missing, false).unwrap(), (false, false));
        let c = config("bad", "{oops");
        assert!(config_flags(&c, false).is_err());
    }

    /// Needs Claude Code installed. Run with `--ignored`.
    #[test]
    #[ignore]
    fn real_check() {
        let s = check();
        assert!(s.installed && s.version.is_some(), "{s:?}");
    }
}
