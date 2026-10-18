//! `.ina/` protocol and shared task registry.
//!
//! INA writes each agent's assignment to `<worktree>/.ina/task.json`; the agent answers in
//! `<worktree>/.ina/result.json`. Status lives in the SQLite registry (the backend is the only
//! writer) and every change is appended to `<root>/.ina/events.jsonl` for the orchestrator.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::plan::{AgentSpec, Plan};
use crate::store::{Store, StoreError};

pub const INA_DIR: &str = ".ina";
pub const TASK_FILE: &str = "task.json";
pub const RESULT_FILE: &str = "result.json";
pub const EVENTS_FILE: &str = "events.jsonl";
const MAX_RESULT_BYTES: u64 = 64 * 1024;
const MAX_SUMMARY: usize = 2000;
const MAX_FILES: usize = 500;

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("invalid {RESULT_FILE}: {0}")]
    InvalidResult(String),
    #[error("task io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("unknown task status {0:?}")]
    UnknownStatus(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed,
    Blocked,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Result<Self, TaskError> {
        Ok(match s {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "blocked" => Self::Blocked,
            "cancelled" => Self::Cancelled,
            _ => return Err(TaskError::UnknownStatus(s.into())),
        })
    }

    /// Allowed moves. Any finished task may run again (the user reopens its terminal); a
    /// pending task is blocked while a prerequisite failed, and unblocked when it recovers.
    pub fn can_become(self, to: Self) -> bool {
        use TaskStatus::*;
        matches!(
            (self, to),
            (Pending, Running | Cancelled | Blocked)
                | (Blocked, Pending)
                | (Running, Done | Failed | Blocked | Cancelled)
                | (Done | Failed | Blocked | Cancelled, Running)
        )
    }
}

/// What an agent may report. `running`/`pending`/`cancelled` are INA's to set, not the agent's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultStatus {
    Done,
    Failed,
    Blocked,
}

impl From<ResultStatus> for TaskStatus {
    fn from(s: ResultStatus) -> Self {
        match s {
            ResultStatus::Done => Self::Done,
            ResultStatus::Failed => Self::Failed,
            ResultStatus::Blocked => Self::Blocked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentResult {
    pub status: ResultStatus,
    pub summary: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

pub fn parse_result(text: &str) -> Result<AgentResult, TaskError> {
    let r: AgentResult =
        serde_json::from_str(text).map_err(|e| TaskError::InvalidResult(e.to_string()))?;
    if r.summary.trim().is_empty() {
        return Err(TaskError::InvalidResult("summary is empty".into()));
    }
    if r.summary.len() > MAX_SUMMARY {
        return Err(TaskError::InvalidResult(format!(
            "summary longer than {MAX_SUMMARY} bytes"
        )));
    }
    if r.files.len() > MAX_FILES {
        return Err(TaskError::InvalidResult(format!(
            "more than {MAX_FILES} files"
        )));
    }
    Ok(r)
}

fn ina_dir(dir: &Path) -> PathBuf {
    dir.join(INA_DIR)
}

/// `Ok(None)` while the agent has not written a result yet.
pub fn read_result(worktree: &Path) -> Result<Option<AgentResult>, TaskError> {
    let path = ina_dir(worktree).join(RESULT_FILE);
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if meta.len() > MAX_RESULT_BYTES {
        return Err(TaskError::InvalidResult(format!(
            "larger than {MAX_RESULT_BYTES} bytes"
        )));
    }
    parse_result(&fs::read_to_string(path)?).map(Some)
}

/// A finished prerequisite, as the dependent agent sees it (its branch is already merged into
/// the dependent's worktree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prerequisite {
    pub id: String,
    pub branch: String,
    pub summary: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskFile<'a> {
    team_id: i64,
    goal: &'a str,
    agent: &'a AgentSpec,
    branch: &'a str,
    prerequisites: &'a [Prerequisite],
    result_file: String,
    result_format: serde_json::Value,
}

/// JSON shape the agent must write, shown to it in `task.json`.
pub fn result_format() -> serde_json::Value {
    serde_json::json!({
        "status": "done | failed | blocked",
        "summary": "what was done, or why it failed / what blocks you (required)",
        "files": ["changed paths, relative to the worktree"],
        "notes": "optional"
    })
}

/// Set aside a result left by a previous run, so it is not mistaken for the new one.
pub fn archive_result(worktree: &Path) -> Result<(), TaskError> {
    let dir = ina_dir(worktree);
    let result = dir.join(RESULT_FILE);
    if result.exists() {
        fs::rename(&result, dir.join("result.prev.json"))?;
    }
    Ok(())
}

/// Write (or refresh) the agent's assignment.
pub fn write_task(
    worktree: &Path,
    team: i64,
    plan: &Plan,
    agent: &AgentSpec,
    branch: &str,
    prerequisites: &[Prerequisite],
) -> Result<(), TaskError> {
    let dir = ina_dir(worktree);
    fs::create_dir_all(&dir)?;
    let task = TaskFile {
        team_id: team,
        goal: &plan.summary,
        agent,
        branch,
        prerequisites,
        result_file: format!("{INA_DIR}/{RESULT_FILE}"),
        result_format: result_format(),
    };
    let json = serde_json::to_string_pretty(&task).map_err(std::io::Error::other)?;
    fs::write(dir.join(TASK_FILE), json)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Event<'a> {
    at: u64,
    team_id: i64,
    agent_id: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
}

pub fn events_path(root: &Path) -> PathBuf {
    ina_dir(root).join(EVENTS_FILE)
}

fn append_event(root: &Path, e: &Event<'_>) -> Result<(), TaskError> {
    fs::create_dir_all(ina_dir(root))?;
    let mut line = serde_json::to_string(e).map_err(std::io::Error::other)?;
    line.push('\n');
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_path(root))?
        .write_all(line.as_bytes())?;
    Ok(())
}

/// The registry for one workspace: store rows plus the event log in its root.
pub struct Registry<'a> {
    pub store: &'a Store,
    pub root: &'a Path,
    pub team: i64,
    pub plan: &'a Plan,
}

/// Detail prefix of a block INA set because of a prerequisite (and may lift again).
const PREREQ_BLOCK: &str = "prerequisite ";

impl Registry<'_> {
    pub fn register(&self, agents: &[&str]) -> Result<(), TaskError> {
        self.store
            .insert_tasks(self.team, agents, TaskStatus::Pending.as_str())?;
        for a in agents {
            self.log(a, TaskStatus::Pending, None);
        }
        Ok(())
    }

    pub fn status(&self, agent: &str) -> Result<Option<TaskStatus>, TaskError> {
        self.store
            .task_status(self.team, agent)?
            .map(|s| TaskStatus::parse(&s))
            .transpose()
    }

    /// Move `agent` to `to` if allowed from its current status. Returns whether it changed.
    pub fn transition(
        &self,
        agent: &str,
        to: TaskStatus,
        summary: Option<&str>,
        detail: Option<&str>,
    ) -> Result<bool, TaskError> {
        let Some(from) = self.status(agent)? else {
            return Ok(false); // orchestrator, or a team approved before the registry existed
        };
        if !from.can_become(to) {
            return Ok(false);
        }
        let changed = self.store.update_task(
            self.team,
            agent,
            from.as_str(),
            to.as_str(),
            summary,
            detail,
        )?;
        if changed {
            self.log(agent, to, detail.or(summary));
            self.propagate(agent, to)?;
        }
        Ok(changed)
    }

    /// Pending dependents of a failed/blocked agent become blocked; once it is done again,
    /// dependents INA blocked go back to pending if no other prerequisite is broken. A cancel
    /// does not block: dependents just keep waiting for it.
    fn propagate(&self, agent: &str, to: TaskStatus) -> Result<(), TaskError> {
        use TaskStatus::*;
        let dependents: Vec<&str> = self
            .plan
            .agents
            .iter()
            .filter(|a| self.plan.prerequisites(&a.id).contains(&agent))
            .map(|a| a.id.as_str())
            .collect();
        for d in dependents {
            match (to, self.status(d)?) {
                (Failed | Blocked, Some(Pending)) => {
                    let why = format!("{PREREQ_BLOCK}{agent} is {}", to.as_str());
                    self.transition(d, Blocked, None, Some(&why))?;
                }
                (Done, Some(Blocked)) if self.blocked_by_prerequisite(d)? => {
                    let broken = self
                        .plan
                        .prerequisites(d)
                        .into_iter()
                        .any(|p| matches!(self.status(p), Ok(Some(Failed | Blocked))));
                    if !broken {
                        let why = format!("{PREREQ_BLOCK}{agent} is done");
                        self.transition(d, Pending, None, Some(&why))?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn blocked_by_prerequisite(&self, agent: &str) -> Result<bool, TaskError> {
        Ok(self
            .store
            .tasks(self.team)?
            .into_iter()
            .find(|t| t.agent_id == agent)
            .and_then(|t| t.detail)
            .is_some_and(|d| d.starts_with(PREREQ_BLOCK)))
    }

    /// The event log is a mirror for the orchestrator; the store is the source of truth, so a
    /// failed append is reported but does not undo the change.
    fn log(&self, agent: &str, status: TaskStatus, detail: Option<&str>) {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let e = Event {
            at,
            team_id: self.team,
            agent_id: agent,
            status: status.as_str(),
            detail,
        };
        if let Err(err) = append_event(self.root, &e) {
            eprintln!("events.jsonl append failed: {err}");
        }
    }

    /// Apply the agent's `result.json` if it is running and wrote one. An invalid file is
    /// returned as an error and leaves the status unchanged, since the agent may still fix it.
    pub fn apply_result(&self, agent: &str, worktree: &Path) -> Result<bool, TaskError> {
        if self.status(agent)? != Some(TaskStatus::Running) {
            return Ok(false);
        }
        match read_result(worktree)? {
            Some(r) => self.apply(agent, &r),
            None => Ok(false),
        }
    }

    fn apply(&self, agent: &str, r: &AgentResult) -> Result<bool, TaskError> {
        self.transition(agent, r.status.into(), Some(&r.summary), Some(RESULT_FILE))
    }

    /// The agent's process ended (or was lost): take its result, or record that it has none.
    pub fn finish(&self, agent: &str, worktree: &Path, why: &str) -> Result<(), TaskError> {
        if self.status(agent)? != Some(TaskStatus::Running) {
            return Ok(()); // already cancelled or reported
        }
        let detail = match read_result(worktree) {
            Ok(Some(r)) => return self.apply(agent, &r).map(|_| ()),
            Ok(None) => format!("{why} without {RESULT_FILE}"),
            Err(e) => format!("{why}; {e}"),
        };
        self.transition(agent, TaskStatus::Failed, None, Some(&detail))
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Mode;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ina-tasks-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn agent() -> AgentSpec {
        AgentSpec {
            id: "api".into(),
            name: "API".into(),
            role: "backend".into(),
            scope: "src/api".into(),
            task: "add endpoint".into(),
            acceptance: vec!["tests pass".into()],
            depends_on: vec![],
        }
    }

    fn plan() -> Plan {
        Plan {
            summary: "goal".into(),
            mode: Mode::Parallel,
            agents: vec![agent()],
        }
    }

    fn write_result(dir: &Path, json: &str) {
        fs::create_dir_all(dir.join(INA_DIR)).unwrap();
        fs::write(dir.join(INA_DIR).join(RESULT_FILE), json).unwrap();
    }

    #[test]
    fn transitions() {
        use TaskStatus::*;
        assert!(Pending.can_become(Running));
        assert!(Running.can_become(Done));
        assert!(Done.can_become(Running));
        assert!(!Pending.can_become(Done));
        assert!(!Done.can_become(Failed));
        assert!(!Cancelled.can_become(Done));
        for s in [Pending, Running, Done, Failed, Blocked, Cancelled] {
            assert_eq!(TaskStatus::parse(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn result_is_validated() {
        let ok = parse_result(r#"{"status":"done","summary":"did it","files":["a.rs"]}"#).unwrap();
        assert_eq!((ok.status, ok.files.len()), (ResultStatus::Done, 1));
        for bad in [
            r#"{"status":"running","summary":"x"}"#,
            r#"{"status":"done","summary":"  "}"#,
            r#"{"status":"done"}"#,
            r#"{"status":"done","summary":"x","extra":1}"#,
            "not json",
        ] {
            assert!(parse_result(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn task_file_written_and_stale_result_set_aside() {
        let dir = tempdir("taskfile");
        write_result(&dir, r#"{"status":"done","summary":"old"}"#);
        archive_result(&dir).unwrap();
        let pre = [Prerequisite {
            id: "db".into(),
            branch: "ina/r/db".into(),
            summary: Some("schema".into()),
        }];
        write_task(&dir, 7, &plan(), &agent(), "ina/r/api", &pre).unwrap();
        let task: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join(".ina/task.json")).unwrap()).unwrap();
        assert_eq!(task["teamId"], 7);
        assert_eq!(task["agent"]["id"], "api");
        assert_eq!(task["resultFile"], ".ina/result.json");
        assert_eq!(task["prerequisites"][0]["branch"], "ina/r/db");
        assert!(read_result(&dir).unwrap().is_none());
        assert!(dir.join(".ina/result.prev.json").exists());
    }

    #[test]
    fn assignment_to_result_lifecycle() {
        let root = tempdir("life");
        let wt = root.join("wt");
        let store = Store::in_memory().unwrap();
        let ws = store.add_workspace(&root).unwrap();
        let team = store.insert_team(ws.id, "{}").unwrap();
        let p = plan();
        let reg = Registry {
            store: &store,
            root: &root,
            team,
            plan: &p,
        };
        reg.register(&["api", "ui"]).unwrap();
        assert_eq!(reg.status("api").unwrap(), Some(TaskStatus::Pending));
        assert_eq!(reg.status("orchestrator").unwrap(), None);

        // Running, no result yet: nothing applied.
        assert!(
            reg.transition("api", TaskStatus::Running, None, None)
                .unwrap()
        );
        assert!(!reg.apply_result("api", &wt).unwrap());
        // Invalid result: error, status unchanged.
        write_result(&wt, r#"{"status":"done"}"#);
        assert!(reg.apply_result("api", &wt).is_err());
        assert_eq!(reg.status("api").unwrap(), Some(TaskStatus::Running));
        // Valid result while running is applied.
        write_result(&wt, r#"{"status":"blocked","summary":"needs ui"}"#);
        assert!(reg.apply_result("api", &wt).unwrap());
        assert_eq!(reg.status("api").unwrap(), Some(TaskStatus::Blocked));

        // Exit without result → failed; cancel wins over a later exit.
        reg.transition("ui", TaskStatus::Running, None, None)
            .unwrap();
        reg.finish("ui", &root.join("none"), "exited").unwrap();
        assert_eq!(reg.status("ui").unwrap(), Some(TaskStatus::Failed));
        reg.transition("ui", TaskStatus::Running, None, None)
            .unwrap();
        reg.transition("ui", TaskStatus::Cancelled, None, Some("stopped"))
            .unwrap();
        reg.finish("ui", &root.join("none"), "exited").unwrap();
        assert_eq!(reg.status("ui").unwrap(), Some(TaskStatus::Cancelled));

        let log = fs::read_to_string(events_path(&root)).unwrap();
        let statuses: Vec<String> = log
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["status"].to_string())
            .collect();
        assert_eq!(statuses.len(), 8, "{log}");
        assert!(log.contains("exited without result.json"));
    }

    #[test]
    fn failure_blocks_dependents_and_recovery_unblocks_them() {
        let root = tempdir("propagate");
        let store = Store::in_memory().unwrap();
        let ws = store.add_workspace(&root).unwrap();
        let team = store.insert_team(ws.id, "{}").unwrap();
        let mk = |id: &str, deps: &[&str]| AgentSpec {
            id: id.into(),
            depends_on: deps.iter().map(|d| d.to_string()).collect(),
            ..agent()
        };
        let p = Plan {
            summary: "g".into(),
            mode: Mode::Parallel,
            agents: vec![mk("api", &[]), mk("ui", &["api"]), mk("e2e", &["ui"])],
        };
        let reg = Registry {
            store: &store,
            root: &root,
            team,
            plan: &p,
        };
        reg.register(&["api", "ui", "e2e"]).unwrap();
        reg.transition("api", TaskStatus::Running, None, None)
            .unwrap();
        reg.transition("api", TaskStatus::Failed, None, Some("boom"))
            .unwrap();
        assert_eq!(reg.status("ui").unwrap(), Some(TaskStatus::Blocked));
        assert_eq!(reg.status("e2e").unwrap(), Some(TaskStatus::Blocked));

        reg.transition("api", TaskStatus::Running, None, None)
            .unwrap();
        reg.transition("api", TaskStatus::Done, Some("ok"), None)
            .unwrap();
        assert_eq!(reg.status("ui").unwrap(), Some(TaskStatus::Pending));
        // e2e waits on ui, which is pending (not done): stays blocked until ui recovers.
        assert_eq!(reg.status("e2e").unwrap(), Some(TaskStatus::Blocked));
    }
}
