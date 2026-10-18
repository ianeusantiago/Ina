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
    #[error("{0}")]
    Unsafe(&'static str),
    /// `merge-tree` exit 1 (conflicts), with its stdout.
    #[error("conflicts")]
    GitOutput(String),
    #[error("worktree io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        self.run_in(&self.root, args)
    }

    fn run_in(&self, dir: &Path, args: &[&str]) -> Result<String, WorktreeError> {
        let mut cmd = Command::new(&self.exe);
        for k in &self.env_remove {
            cmd.env_remove(k);
        }
        let out = cmd
            .arg("-C")
            .arg(dir)
            .args(args)
            .stdin(Stdio::null())
            .output()?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else if args.contains(&"merge-tree") && out.status.code() == Some(1) {
            Err(WorktreeError::GitOutput(
                String::from_utf8_lossy(&out.stdout).trim().to_string(),
            ))
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
        let current = match fs::read_to_string(&file) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
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

    /// Commits on the agent's branch not yet in HEAD, uncommitted changes, and touched files.
    pub fn status(&self, wt: &Worktree) -> Result<BranchStatus, WorktreeError> {
        let range = format!("HEAD..{}", wt.branch);
        let ahead = self
            .run(&["rev-list", "--count", &range])?
            .parse()
            .unwrap_or(0);
        // Only tracked changes block integration; untracked files (build output, caches) are
        // reported but never merged anyway.
        let dirty = !self
            .run_in(&wt.path, &["status", "--porcelain", "--untracked-files=no"])?
            .is_empty();
        let untracked = self
            .run_in(
                &wt.path,
                &["ls-files", "--others", "--exclude-standard", "--directory"],
            )?
            .lines()
            .map(str::to_string)
            .collect();
        let base = format!("HEAD...{}", wt.branch);
        let files = self
            .run(&["diff", "--name-only", &base])?
            .lines()
            .map(str::to_string)
            .collect();
        Ok(BranchStatus {
            branch: wt.branch.clone(),
            ahead,
            dirty,
            untracked,
            files,
        })
    }

    pub fn root_is_clean(&self) -> Result<bool, WorktreeError> {
        Ok(self.run(&["status", "--porcelain"])?.is_empty())
    }

    /// Merge one branch into HEAD (`--no-ff`). A conflict is aborted, leaving HEAD unchanged.
    /// Simulate merging `branches` into HEAD in order, without touching any ref, index or
    /// working tree (`git merge-tree --write-tree`; intermediate results are unreferenced
    /// commit objects). Returns, per branch, `None` if clean or the conflicting files; stops
    /// after the first conflict, like `merge` does.
    pub fn preview_merges(
        &self,
        branches: &[String],
    ) -> Result<Vec<Option<Vec<String>>>, WorktreeError> {
        let mut base = self.run(&["rev-parse", "HEAD"])?;
        let mut out = Vec::new();
        for b in branches {
            let res = self.run(&[
                "merge-tree",
                "--write-tree",
                "--name-only",
                "--no-messages",
                &base,
                b,
            ]);
            match res {
                Ok(tree) => {
                    let tree = tree.lines().next().unwrap_or_default().to_owned();
                    base = self.run(&[
                        "-c",
                        "user.name=INA preview",
                        "-c",
                        "user.email=ina@localhost",
                        "commit-tree",
                        &tree,
                        "-p",
                        &base,
                        "-p",
                        b,
                        "-m",
                        "ina preview",
                    ])?;
                    out.push(None);
                }
                Err(WorktreeError::GitOutput(stdout)) => {
                    // Exit 1: first line is the tree, then one conflicted path per line.
                    let files = stdout
                        .lines()
                        .skip(1)
                        .filter(|l| !l.is_empty())
                        .map(str::to_string)
                        .collect();
                    out.push(Some(files));
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Files touched by `branch`'s own commits: not in HEAD, not from merges, not from the
    /// `exclude`d branches (e.g. prerequisites INA merged into it).
    pub fn own_files(
        &self,
        branch: &str,
        exclude: &[String],
    ) -> Result<Vec<String>, WorktreeError> {
        let mut args = vec![
            "log",
            "--no-merges",
            "--name-only",
            "--format=",
            branch,
            "^HEAD",
        ];
        let negated: Vec<String> = exclude.iter().map(|b| format!("^{b}")).collect();
        args.extend(negated.iter().map(String::as_str));
        let mut files: Vec<String> = self
            .run(&args)?
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        files.sort();
        files.dedup();
        Ok(files)
    }

    pub fn merge(&self, branch: &str) -> Result<MergeOutcome, WorktreeError> {
        self.merge_in(&self.root, branch)
    }

    /// Whether `branch` is already contained in the HEAD checked out at `dir`.
    pub fn is_merged_in(&self, dir: &Path, branch: &str) -> bool {
        self.run_in(dir, &["merge-base", "--is-ancestor", branch, "HEAD"])
            .is_ok()
    }

    /// Merge `branch` into the HEAD checked out at `dir` (the root or an agent worktree);
    /// a conflict is aborted so nothing changes.
    pub fn merge_in(&self, dir: &Path, branch: &str) -> Result<MergeOutcome, WorktreeError> {
        let msg = format!("ina: merge {branch}");
        match self.run_in(dir, &["merge", "--no-ff", "--no-edit", "-m", &msg, branch]) {
            Ok(_) => Ok(MergeOutcome::Merged),
            Err(e) => {
                let files: Vec<String> = self
                    .run_in(dir, &["diff", "--name-only", "--diff-filter=U"])
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_string)
                    .collect();
                if self.run_in(dir, &["merge", "--abort"]).is_err() || files.is_empty() {
                    return Err(e); // not a plain conflict: surface git's error
                }
                Ok(MergeOutcome::Conflict { files })
            }
        }
    }

    fn is_registered(&self, wt: &Worktree) -> Result<bool, WorktreeError> {
        let list = self.run(&["worktree", "list", "--porcelain"])?;
        let want = wt.path.canonicalize().ok();
        Ok(list
            .lines()
            .filter_map(|l| l.strip_prefix("worktree "))
            .any(|p| want.is_some() && Path::new(p).canonicalize().ok() == want))
    }

    /// Remove a worktree (refused by git if it has uncommitted or untracked files) and delete
    /// its branch only if it is merged (`-d`, never `-D`). Safe to retry after a partial run,
    /// e.g. when a running session kept the directory locked.
    /// Remove an agent worktree and its merged branch. With `discard_untracked`, untracked
    /// files are deleted too, but only when the branch is fully merged into HEAD and no tracked
    /// file is modified, so committed and modified work is never lost.
    pub fn cleanup(&self, wt: &Worktree, discard_untracked: bool) -> Result<(), WorktreeError> {
        self.run(&["worktree", "prune"])?; // forget worktrees whose directory is gone
        if self.is_registered(wt)? {
            let path = wt.path.to_string_lossy().to_string();
            if discard_untracked {
                if self
                    .run(&["merge-base", "--is-ancestor", &wt.branch, "HEAD"])
                    .is_err()
                {
                    return Err(WorktreeError::Unsafe(
                        "branch is not merged; nothing was removed",
                    ));
                }
                if self.status(wt)?.dirty {
                    return Err(WorktreeError::Unsafe(
                        "worktree has modified tracked files; nothing was removed",
                    ));
                }
                self.run(&["worktree", "remove", "--force", &path])?;
            } else {
                self.run(&["worktree", "remove", &path])?;
            }
        }
        if wt.path.exists() {
            fs::remove_dir(&wt.path)?; // only succeeds when empty
        }
        let exists = self
            .run(&[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", wt.branch),
            ])
            .is_ok();
        if exists {
            self.run(&["branch", "-d", &wt.branch])?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchStatus {
    pub branch: String,
    pub ahead: u32,
    pub dirty: bool,
    pub untracked: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MergeOutcome {
    Merged,
    Conflict { files: Vec<String> },
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

    fn commit_file(g: &Git, wt: &Worktree, name: &str, body: &str) {
        fs::write(wt.path.join(name), body).unwrap();
        g.run_in(&wt.path, &["add", name]).unwrap();
        g.run_in(
            &wt.path,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "-m",
                name,
            ],
        )
        .unwrap();
    }

    fn identity(g: &Git) {
        g.run(&["config", "user.name", "t"]).unwrap();
        g.run(&["config", "user.email", "t@t"]).unwrap();
    }

    #[test]
    fn status_merge_and_cleanup() {
        let g = repo("merge", true);
        identity(&g);
        let team = g.create_team("r", &["api", "ui"]).unwrap();
        commit_file(&g, &team[0], "api.txt", "a");
        fs::write(team[1].path.join("wip.txt"), "x").unwrap();

        let s = g.status(&team[0]).unwrap();
        assert_eq!(
            (s.ahead, s.dirty, s.files.clone()),
            (1, false, vec!["api.txt".into()])
        );
        let s = g.status(&team[1]).unwrap();
        assert_eq!((s.ahead, s.dirty), (0, false));
        assert_eq!(s.untracked, ["wip.txt"]);
        fs::write(team[0].path.join("api.txt"), "edited").unwrap();
        assert!(g.status(&team[0]).unwrap().dirty);
        g.run_in(&team[0].path, &["checkout", "--", "api.txt"])
            .unwrap();

        assert!(g.root_is_clean().unwrap());
        assert_eq!(g.merge(&team[0].branch).unwrap(), MergeOutcome::Merged);
        assert!(g.root.join("api.txt").exists());
        assert_eq!(g.status(&team[0]).unwrap().ahead, 0);

        g.cleanup(&team[0], false).unwrap();
        assert!(!team[0].path.exists());
        // Uncommitted work is never discarded.
        assert!(g.cleanup(&team[1], false).is_err());
        assert!(team[1].path.join("wip.txt").exists());
    }

    #[test]
    fn preview_predicts_conflicts_without_touching_anything() {
        let g = repo("preview", true);
        identity(&g);
        let team = g.create_team("r", &["a", "b", "c"]).unwrap();
        commit_file(&g, &team[0], "same.txt", "from a");
        commit_file(&g, &team[1], "same.txt", "from b");
        commit_file(&g, &team[2], "c.txt", "c");
        let head = g.run(&["rev-parse", "HEAD"]).unwrap();
        let branches: Vec<String> = team.iter().map(|w| w.branch.clone()).collect();
        let p = g.preview_merges(&branches).unwrap();
        // a is clean; b conflicts with a's change; c is not reached.
        assert_eq!(p, vec![None, Some(vec!["same.txt".to_string()])]);
        assert_eq!(g.run(&["rev-parse", "HEAD"]).unwrap(), head);
        assert!(g.root_is_clean().unwrap());
        assert!(!g.root.join("same.txt").exists());
        let p = g.preview_merges(&[branches[2].clone()]).unwrap();
        assert_eq!(p, vec![None]);
    }

    #[test]
    fn prerequisite_branch_merges_into_dependent_worktree() {
        let g = repo("prereq", true);
        identity(&g);
        let team = g.create_team("r", &["api", "ui"]).unwrap();
        commit_file(&g, &team[0], "api.txt", "a");
        assert!(!g.is_merged_in(&team[1].path, &team[0].branch));
        assert_eq!(
            g.merge_in(&team[1].path, &team[0].branch).unwrap(),
            MergeOutcome::Merged
        );
        assert!(team[1].path.join("api.txt").exists());
        assert!(g.is_merged_in(&team[1].path, &team[0].branch));
        commit_file(&g, &team[1], "ui.txt", "u");
        assert_eq!(
            g.own_files(&team[1].branch, &[team[0].branch.clone()])
                .unwrap(),
            ["ui.txt"]
        );
        // The workspace root is untouched.
        assert!(!g.root.join("api.txt").exists());
    }

    #[test]
    fn conflict_is_aborted_and_unmerged_branch_kept() {
        let g = repo("conflict", true);
        identity(&g);
        let team = g.create_team("r", &["a", "b"]).unwrap();
        commit_file(&g, &team[0], "same.txt", "from a");
        commit_file(&g, &team[1], "same.txt", "from b");
        let head = g.run(&["rev-parse", "HEAD"]).unwrap();
        assert_eq!(g.merge(&team[0].branch).unwrap(), MergeOutcome::Merged);
        let merged = g.run(&["rev-parse", "HEAD"]).unwrap();
        assert_ne!(head, merged);
        assert_eq!(
            g.merge(&team[1].branch).unwrap(),
            MergeOutcome::Conflict {
                files: vec!["same.txt".into()]
            }
        );
        assert_eq!(g.run(&["rev-parse", "HEAD"]).unwrap(), merged);
        assert!(g.root_is_clean().unwrap());
        // Branch b is unmerged: `branch -d` refuses, so its commits survive.
        assert!(g.cleanup(&team[1], false).is_err());
        assert!(g.run(&["rev-parse", "--verify", &team[1].branch]).is_ok());
    }

    #[test]
    fn cleanup_can_be_retried_after_partial_removal() {
        let g = repo("retry", true);
        identity(&g);
        let team = g.create_team("r", &["a"]).unwrap();
        commit_file(&g, &team[0], "a.txt", "a");
        assert_eq!(g.merge(&team[0].branch).unwrap(), MergeOutcome::Merged);
        // Simulate a locked directory: git unregistered it, an empty dir stayed behind.
        let path = team[0].path.to_string_lossy().to_string();
        g.run(&["worktree", "remove", &path]).unwrap();
        fs::create_dir_all(&team[0].path).unwrap();
        g.cleanup(&team[0], false).unwrap();
        assert!(!team[0].path.exists());
        assert_eq!(g.run(&["branch", "--list", "ina/*"]).unwrap(), "");
        g.cleanup(&team[0], false).unwrap(); // idempotent
    }

    #[test]
    fn forced_cleanup_only_discards_untracked_files_of_merged_branches() {
        let g = repo("force", true);
        identity(&g);
        fs::write(g.root.join("README.md"), "r").unwrap();
        g.run(&["add", "README.md"]).unwrap();
        g.run(&["commit", "-m", "readme"]).unwrap();
        let team = g.create_team("r", &["a", "b", "c"]).unwrap();
        commit_file(&g, &team[0], "a.txt", "a");
        assert_eq!(g.merge(&team[0].branch).unwrap(), MergeOutcome::Merged);
        fs::write(team[0].path.join("cache.pyc"), "x").unwrap();
        assert!(g.cleanup(&team[0], false).is_err());
        g.cleanup(&team[0], true).unwrap();
        assert!(!team[0].path.exists());

        // Unmerged branch: refused, untracked file kept.
        commit_file(&g, &team[1], "b.txt", "b");
        fs::write(team[1].path.join("wip.txt"), "x").unwrap();
        assert!(g.cleanup(&team[1], true).is_err());
        assert!(team[1].path.join("wip.txt").exists());

        // Merged (no commits) but a tracked file modified: refused.
        fs::write(team[2].path.join("README.md"), "edited").unwrap();
        assert!(g.cleanup(&team[2], true).is_err());
        assert!(team[2].path.exists());
    }
}
