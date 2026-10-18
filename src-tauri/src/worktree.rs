//! One git worktree per agent, so agents never edit the same checkout. Worktrees live under
//! `<workspace>/.ina/worktrees/` (inside the workspace, so the cwd policy and folder trust
//! still apply) and `.ina/` is added to the repo's local exclude file.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("workspace is not the root of a git repository")]
    NotRepo,
    #[error("repository has no commits yet; make a first commit before starting a team")]
    NoCommits,
    #[error("git {0}: {1}")]
    Git(String, String),
    #[error("worktree io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
}

/// Git runner for one repository. `exe` comes from the execution policy.
pub struct Git {
    exe: PathBuf,
    root: PathBuf,
    env_remove: Vec<std::ffi::OsString>,
}

impl Git {
    pub fn new(exe: PathBuf, root: PathBuf, env_remove: Vec<std::ffi::OsString>) -> Self {
        Self {
            exe,
            root,
            env_remove,
        }
    }

    fn run(&self, args: &[&str]) -> Result<String, WorktreeError> {
        let mut cmd = Command::new(&self.exe);
        for k in &self.env_remove {
            cmd.env_remove(k);
        }
        let out = cmd
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(WorktreeError::Git(
                args.first().copied().unwrap_or_default().to_string(),
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ))
        }
    }

    /// The workspace must be a repo's top level with at least one commit.
    pub fn check_repo(&self) -> Result<(), WorktreeError> {
        let top = self
            .run(&["rev-parse", "--show-toplevel"])
            .map_err(|_| WorktreeError::NotRepo)?;
        let same = |a: &Path, b: &Path| match (a.canonicalize(), b.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if !same(Path::new(&top), &self.root) {
            return Err(WorktreeError::NotRepo);
        }
        self.run(&["rev-parse", "--verify", "--quiet", "HEAD"])
            .map_err(|_| WorktreeError::NoCommits)?;
        Ok(())
    }

    /// Keep `.ina/` out of `git status` without touching the tracked `.gitignore`.
    fn exclude_ina_dir(&self) -> Result<(), WorktreeError> {
        let common = PathBuf::from(self.run(&["rev-parse", "--git-common-dir"])?);
        let common = if common.is_absolute() {
            common
        } else {
            self.root.join(common)
        };
        let info = common.join("info");
        fs::create_dir_all(&info)?;
        let file = info.join("exclude");
        let current = fs::read_to_string(&file).unwrap_or_default();
        if !current.lines().any(|l| l.trim() == "/.ina/") {
            let sep = if current.is_empty() || current.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            fs::write(&file, format!("{current}{sep}/.ina/\n"))?;
        }
        Ok(())
    }

    /// Create one worktree + branch per agent id, from HEAD. All or nothing.
    pub fn create_team(
        &self,
        run: &str,
        agent_ids: &[&str],
    ) -> Result<Vec<Worktree>, WorktreeError> {
        self.check_repo()?;
        self.exclude_ina_dir()?;
        let base = self.root.join(".ina").join("worktrees");
        fs::create_dir_all(&base)?;
        let mut made = Vec::new();
        for id in agent_ids {
            let wt = Worktree {
                path: base.join(format!("{run}-{id}")),
                branch: format!("ina/{run}/{id}"),
            };
            let path = wt.path.to_string_lossy().to_string();
            if let Err(e) = self.run(&["worktree", "add", "-b", &wt.branch, &path, "HEAD"]) {
                self.remove_team(&made);
                return Err(e);
            }
            made.push(wt);
        }
        Ok(made)
    }

    /// Best-effort rollback: remove worktrees and their branches, reporting failures to stderr.
    pub fn remove_team(&self, worktrees: &[Worktree]) {
        for wt in worktrees {
            let path = wt.path.to_string_lossy().to_string();
            for args in [
                vec!["worktree", "remove", "--force", path.as_str()],
                vec!["branch", "-D", wt.branch.as_str()],
            ] {
                if let Err(e) = self.run(&args) {
                    eprintln!("worktree rollback failed: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git() -> PathBuf {
        let path = std::env::var_os("PATH").unwrap_or_default();
        crate::policy::resolve_program(crate::policy::Program::Git, &path).unwrap()
    }

    fn repo(name: &str, commit: bool) -> Git {
        let dir = std::env::temp_dir().join(format!("ina-wt-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let g = Git::new(
            git(),
            crate::policy::simplify(dir.canonicalize().unwrap()),
            vec![],
        );
        g.run(&["init", "-q"]).unwrap();
        if commit {
            g.run(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "init",
            ])
            .unwrap();
        }
        g
    }

    #[test]
    fn creates_isolated_worktrees_per_agent() {
        let g = repo("ok", true);
        let team = g.create_team("r1", &["api", "ui"]).unwrap();
        assert_eq!(team[1].branch, "ina/r1/ui");
        for wt in &team {
            assert!(wt.path.join(".git").exists(), "{wt:?}");
            assert!(wt.path.starts_with(&g.root));
        }
        assert_eq!(
            g.run(&["status", "--porcelain"]).unwrap(),
            "",
            ".ina/ must be excluded"
        );
        g.remove_team(&team);
        assert!(!team[0].path.exists());
        assert_eq!(g.run(&["branch", "--list", "ina/*"]).unwrap(), "");
    }

    #[test]
    fn failure_rolls_back_created_worktrees() {
        let g = repo("rollback", true);
        g.run(&["branch", "ina/r1/ui"]).unwrap(); // second agent's branch already exists
        assert!(g.create_team("r1", &["api", "ui"]).is_err());
        assert!(!g.root.join(".ina/worktrees/r1-api").exists());
        assert_eq!(g.run(&["branch", "--list", "ina/r1/api"]).unwrap(), "");
    }

    #[test]
    fn requires_repo_root_with_commits() {
        assert!(matches!(
            repo("empty", false).check_repo(),
            Err(WorktreeError::NoCommits)
        ));
        let dir = std::env::temp_dir().join(format!("ina-wt-norepo-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let g = Git::new(git(), dir, vec![]);
        assert!(matches!(g.check_repo(), Err(WorktreeError::NotRepo)));
    }
}
