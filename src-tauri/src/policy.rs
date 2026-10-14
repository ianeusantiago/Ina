//! Execution policy: which programs may run, with which args, where. Everything spawned goes through here.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::pty::SpawnSpec;

pub const MAX_SESSIONS: usize = 16;
const MAX_ARGS: usize = 32;
const MAX_ARG_LEN: usize = 32 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("program not found on PATH: {0}")]
    ProgramNotFound(&'static str),
    #[error("working directory is invalid or outside the workspace: {0}")]
    CwdOutsideWorkspace(String),
    #[error("argument rejected: {0}")]
    BadArg(String),
    #[error("session limit reached ({MAX_SESSIONS})")]
    TooManySessions,
}

/// Programs INA is allowed to launch. The frontend can only pick one of these, never a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program {
    Claude,
    Shell,
}

impl Program {
    fn exe_name(self) -> &'static str {
        match self {
            Program::Claude => "claude.exe",
            Program::Shell => "powershell.exe",
        }
    }
}

/// Resolve to an absolute `.exe` on PATH. `.cmd`/`.bat` shims are never used: they run through
/// cmd.exe, whose argument parsing allows injection.
pub fn resolve_program(program: Program, path_var: &OsStr) -> Result<PathBuf, PolicyError> {
    let name = program.exe_name();
    std::env::split_paths(path_var)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
        .ok_or(PolicyError::ProgramNotFound(name))
}

/// `cwd` must exist and resolve (after symlinks/`..`) to `root` or a directory inside it.
pub fn validate_cwd(root: &Path, cwd: &Path) -> Result<PathBuf, PolicyError> {
    let bad = || PolicyError::CwdOutsideWorkspace(cwd.display().to_string());
    let root = root.canonicalize().map_err(|_| bad())?;
    let cwd = cwd.canonicalize().map_err(|_| bad())?;
    if cwd.is_dir() && cwd.starts_with(&root) {
        Ok(cwd)
    } else {
        Err(bad())
    }
}

pub fn validate_args(args: &[String]) -> Result<(), PolicyError> {
    if args.len() > MAX_ARGS {
        return Err(PolicyError::BadArg(format!(
            "too many arguments ({})",
            args.len()
        )));
    }
    for a in args {
        if a.contains('\0') {
            return Err(PolicyError::BadArg("NUL byte".into()));
        }
        if a.len() > MAX_ARG_LEN {
            return Err(PolicyError::BadArg("argument too long".into()));
        }
    }
    Ok(())
}

/// Build a spawn spec that satisfies every policy check.
pub fn authorize(
    program: Program,
    args: Vec<String>,
    workspace_root: &Path,
    cwd: &Path,
    running_sessions: usize,
    path_var: &OsStr,
) -> Result<SpawnSpec, PolicyError> {
    if running_sessions >= MAX_SESSIONS {
        return Err(PolicyError::TooManySessions);
    }
    validate_args(&args)?;
    Ok(SpawnSpec {
        program: resolve_program(program, path_var)?,
        args,
        cwd: validate_cwd(workspace_root, cwd)?,
        cols: 80,
        rows: 24,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ina-policy-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn resolves_only_exe_from_absolute_path_entries() {
        let dir = tempdir("resolve");
        fs::write(dir.join("claude.cmd"), "").unwrap();
        let path = std::env::join_paths([dir.clone()]).unwrap();
        assert_eq!(
            resolve_program(Program::Claude, &path),
            Err(PolicyError::ProgramNotFound("claude.exe"))
        );
        fs::write(dir.join("claude.exe"), "").unwrap();
        assert_eq!(
            resolve_program(Program::Claude, &path).unwrap(),
            dir.join("claude.exe")
        );
        // Relative PATH entries are ignored (would resolve against the current directory).
        let rel = std::env::join_paths([PathBuf::from(".")]).unwrap();
        assert!(resolve_program(Program::Shell, &rel).is_err());
    }

    #[test]
    fn cwd_must_stay_inside_workspace() {
        let root = tempdir("cwd");
        fs::create_dir_all(root.join("sub")).unwrap();
        assert!(validate_cwd(&root, &root).is_ok());
        assert!(validate_cwd(&root, &root.join("sub")).is_ok());
        assert!(validate_cwd(&root, &root.join("sub").join("..").join("..")).is_err());
        assert!(validate_cwd(&root, &root.join("missing")).is_err());
        assert!(validate_cwd(&root.join("sub"), &root).is_err());
        // Sibling with a shared name prefix is not "inside".
        let sibling = PathBuf::from(format!("{}-evil", root.display()));
        fs::create_dir_all(&sibling).unwrap();
        assert!(validate_cwd(&root, &sibling).is_err());
    }

    #[test]
    fn args_are_bounded() {
        assert!(validate_args(&["-p".into(), "hello; rm -rf /".into()]).is_ok());
        assert!(validate_args(&["a\0b".into()]).is_err());
        assert!(validate_args(&vec!["x".into(); MAX_ARGS + 1]).is_err());
        assert!(validate_args(&["x".repeat(MAX_ARG_LEN + 1)]).is_err());
    }

    #[test]
    fn authorize_enforces_session_limit() {
        let root = tempdir("auth");
        let path = std::env::var_os("PATH").unwrap();
        assert_eq!(
            authorize(Program::Shell, vec![], &root, &root, MAX_SESSIONS, &path).unwrap_err(),
            PolicyError::TooManySessions
        );
        let spec = authorize(Program::Shell, vec![], &root, &root, 0, &path).unwrap();
        assert!(spec.program.is_absolute());
    }
}
