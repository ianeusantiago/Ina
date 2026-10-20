//! Read-only check of Claude Code's folder trust (`projects[<dir>].hasTrustDialogAccepted` in
//! `.claude.json`). INA never writes that file: the user trusts a folder by running `claude`
//! there once. Agent sessions are refused until then, instead of hanging on the trust prompt.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    #[error("cannot locate the Claude Code config (no home directory)")]
    NoConfig,
    #[error("Claude Code config is not valid JSON: {0}")]
    Parse(String),
    #[error("Claude Code config: {0}")]
    Io(#[from] std::io::Error),
}

/// Error text the frontend matches to show the trust instructions.
pub const NOT_TRUSTED: &str = "workspace folder is not trusted by Claude Code";

/// `$CLAUDE_CONFIG_DIR/.claude.json`, else `~/.claude.json`.
pub fn config_path() -> Result<PathBuf, TrustError> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir).join(".claude.json"));
    }
    crate::policy::home_dir()
        .map(|h| h.join(".claude.json"))
        .ok_or(TrustError::NoConfig)
}

/// Claude Code keys projects by the path with forward slashes.
fn project_key(dir: &Path) -> String {
    dir.to_string_lossy().replace('\\', "/")
}

fn same_key(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// True if `dir` or one of its ancestors has been trusted. A missing config means untrusted.
pub fn is_trusted(config: &Path, dir: &Path) -> Result<bool, TrustError> {
    let cfg: Value = match fs::read_to_string(config) {
        Ok(s) => serde_json::from_str(&s).map_err(|e| TrustError::Parse(e.to_string()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let Some(projects) = cfg.get("projects").and_then(Value::as_object) else {
        return Ok(false);
    };
    Ok(dir.ancestors().any(|d| {
        let key = project_key(d);
        projects.iter().any(|(k, v)| {
            same_key(k, &key) && v.get("hasTrustDialogAccepted") == Some(&Value::Bool(true))
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str, body: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ina-trust-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let f = dir.join(".claude.json");
        if let Some(b) = body {
            fs::write(&f, b).unwrap();
        }
        f
    }

    #[test]
    fn trust_is_inherited_from_ancestors() {
        let f = config(
            "anc",
            Some(
                r#"{"projects":{"C:/work":{"hasTrustDialogAccepted":true},"C:/other":{"hasTrustDialogAccepted":false}}}"#,
            ),
        );
        assert!(is_trusted(&f, Path::new(r"C:\work\app")).unwrap());
        assert_eq!(
            is_trusted(&f, Path::new(r"c:\WORK")).unwrap(),
            cfg!(windows)
        );
        assert!(!is_trusted(&f, Path::new(r"C:\other")).unwrap());
        assert!(!is_trusted(&f, Path::new(r"C:\elsewhere")).unwrap());
    }

    #[test]
    fn missing_or_bad_config() {
        assert!(!is_trusted(&config("missing", None), Path::new(r"C:\w")).unwrap());
        assert!(!is_trusted(&config("noproj", Some("{}")), Path::new(r"C:\w")).unwrap());
        let bad = config("bad", Some("{not json"));
        assert!(matches!(
            is_trusted(&bad, Path::new(r"C:\w")),
            Err(TrustError::Parse(_))
        ));
    }
}
