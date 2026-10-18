//! Result verification: run an agent's approved checks in its worktree and compare the files
//! it declared with what its branch actually changed.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::plan::Check;
use crate::policy;

const CHECK_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Output kept from a failed check, from its end.
const OUTPUT_TAIL: usize = 1500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Every check passed (`0` checks means unverified, not passed).
    Passed(usize),
    Failed {
        check: String,
        detail: String,
    },
}

fn tail(s: &str) -> String {
    let s = s.trim();
    let start = s
        .char_indices()
        .rev()
        .nth(OUTPUT_TAIL)
        .map_or(0, |(i, _)| i);
    s[start..].to_owned()
}

/// Run `checks` in order inside `worktree` (which must be inside `root`); stop at the first
/// failure. Programs and args go through the execution policy; no shell is involved.
pub fn run_checks(checks: &[Check], root: &Path, worktree: &Path) -> Outcome {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for check in checks {
        let name = check.display();
        let fail = |detail: String| Outcome::Failed {
            check: name.clone(),
            detail,
        };
        let spec = match policy::authorize(
            check.program.program(),
            check.args.clone(),
            root,
            worktree,
            0,
            &path_var,
        ) {
            Ok(s) => s,
            Err(e) => return fail(e.to_string()),
        };
        let mut cmd = Command::new(&spec.program);
        for k in &spec.env_remove {
            cmd.env_remove(k);
        }
        let mut child = match cmd
            .args(&spec.args)
            .current_dir(&spec.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return fail(format!("could not start: {e}")),
        };
        let drain = |mut r: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut b = Vec::new();
                let _ = r.read_to_end(&mut b);
                b
            })
        };
        let out = drain(Box::new(child.stdout.take().expect("piped")));
        let err = drain(Box::new(child.stderr.take().expect("piped")));
        let status = crate::pty::wait_timeout(&mut child, CHECK_TIMEOUT);
        let mut text = out.join().unwrap_or_default();
        text.extend(err.join().unwrap_or_default());
        let text = String::from_utf8_lossy(&text);
        match status {
            Ok(Some(s)) if s.success() => {}
            Ok(Some(s)) => {
                return fail(format!("exit {}: {}", s.code().unwrap_or(-1), tail(&text)));
            }
            Ok(None) => {
                return fail(format!(
                    "timed out after {} min",
                    CHECK_TIMEOUT.as_secs() / 60
                ));
            }
            Err(e) => return fail(e.to_string()),
        }
    }
    Outcome::Passed(checks.len())
}

/// Declared-vs-actual file differences, if any (paths compared with `/` separators).
pub fn files_mismatch(declared: &[String], changed: &[String]) -> Option<String> {
    let norm = |p: &String| p.replace('\\', "/").trim_start_matches("./").to_owned();
    let declared: std::collections::BTreeSet<String> = declared.iter().map(norm).collect();
    let changed: std::collections::BTreeSet<String> = changed.iter().map(norm).collect();
    let missing: Vec<&String> = declared.difference(&changed).collect();
    let undeclared: Vec<&String> = changed.difference(&declared).collect();
    let mut parts = Vec::new();
    if !missing.is_empty() {
        parts.push(format!("declared but not changed: {missing:?}"));
    }
    if !undeclared.is_empty() {
        parts.push(format!("changed but not declared: {undeclared:?}"));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::CheckProgram;

    #[test]
    fn mismatch_reports_both_sides() {
        let d = vec!["a.py".to_string(), "./b.py".into()];
        let c = vec!["b.py".to_string(), "c.py".into()];
        let m = files_mismatch(&d, &c).unwrap();
        assert!(m.contains("\"a.py\"") && m.contains("\"c.py\""), "{m}");
        assert_eq!(files_mismatch(&c, &c), None);
    }

    #[test]
    fn tail_keeps_the_end() {
        let s = "x".repeat(OUTPUT_TAIL + 10) + "END";
        let t = tail(&s);
        assert!(t.ends_with("END") && t.chars().count() <= OUTPUT_TAIL + 1);
    }

    /// Needs Python on PATH. Run with `--ignored`.
    #[test]
    #[ignore]
    fn real_checks_pass_and_fail() {
        let dir = std::env::temp_dir().join(format!("ina-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let py = |code: &str| Check {
            program: CheckProgram::Python,
            args: vec!["-c".into(), code.into()],
        };
        assert_eq!(
            run_checks(&[py("print(1)")], &dir, &dir),
            Outcome::Passed(1)
        );
        match run_checks(&[py("import sys; print('boom'); sys.exit(3)")], &dir, &dir) {
            Outcome::Failed { detail, .. } => {
                assert!(detail.starts_with("exit 3") && detail.contains("boom"))
            }
            o => panic!("{o:?}"),
        }
    }
}
