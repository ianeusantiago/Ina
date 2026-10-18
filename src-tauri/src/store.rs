//! Persistence of metadata only (workspaces, later sessions/tasks). Never terminal output or secrets.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("not a directory: {0}")]
    NotADirectory(String),
    #[error("workspace already added: {0}")]
    DuplicateWorkspace(String),
    #[error("workspace not found: {0}")]
    WorkspaceNotFound(i64),
    #[error("team not found: {0}")]
    TeamNotFound(i64),
    #[error("team was already approved: {0}")]
    TeamAlreadyApproved(i64),
    #[error("store lock poisoned")]
    Poisoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub path: String,
}

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE workspaces (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // Team plans. `worktrees` is NULL until approved; `closed` once cleaned up.
    "CREATE TABLE teams (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        workspace_id INTEGER NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
        plan TEXT NOT NULL,
        worktrees TEXT,
        closed INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // Shared task registry: one row per agent of an approved team, plus its history.
    "CREATE TABLE tasks (
        team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
        agent_id TEXT NOT NULL,
        status TEXT NOT NULL,
        summary TEXT,
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (team_id, agent_id)
    );
    CREATE TABLE task_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
        agent_id TEXT NOT NULL,
        status TEXT NOT NULL,
        detail TEXT,
        at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
    // Structured messages. `status`: received | proposed | delivered | rejected.
    "CREATE TABLE messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
        from_agent TEXT NOT NULL,
        to_agent TEXT,
        kind TEXT NOT NULL,
        text TEXT NOT NULL,
        status TEXT NOT NULL,
        reply_to INTEGER REFERENCES messages(id),
        at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
];

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS
            .iter()
            .enumerate()
            .skip(usize::try_from(version).unwrap_or(0))
        {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(
                None,
                "user_version",
                i64::try_from(i + 1).unwrap_or(i64::MAX),
            )?;
            tx.commit()?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }

    /// Adds an existing directory, stored as its canonical path.
    pub fn add_workspace(&self, dir: &Path) -> Result<Workspace, StoreError> {
        let not_dir = || StoreError::NotADirectory(dir.display().to_string());
        let canonical = dir.canonicalize().map_err(|_| not_dir())?;
        if !canonical.is_dir() {
            return Err(not_dir());
        }
        let path = crate::policy::simplify(canonical.clone())
            .display()
            .to_string();
        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        let conn = self.conn()?;
        let exists: Option<i64> = conn
            .query_row("SELECT id FROM workspaces WHERE path = ?1", [&path], |r| {
                r.get(0)
            })
            .optional()?;
        if exists.is_some() {
            return Err(StoreError::DuplicateWorkspace(path));
        }
        conn.execute(
            "INSERT INTO workspaces (name, path) VALUES (?1, ?2)",
            params![name, path],
        )?;
        Ok(Workspace {
            id: conn.last_insert_rowid(),
            name,
            path,
        })
    }

    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, name, path FROM workspaces ORDER BY name, id")?;
        let rows = stmt.query_map([], |r| {
            Ok(Workspace {
                id: r.get(0)?,
                name: r.get(1)?,
                path: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get_workspace(&self, id: i64) -> Result<Workspace, StoreError> {
        self.conn()?
            .query_row(
                "SELECT id, name, path FROM workspaces WHERE id = ?1",
                [id],
                |r| {
                    Ok(Workspace {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        path: r.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or(StoreError::WorkspaceNotFound(id))
    }

    /// Removes the record only; never touches the directory on disk.
    pub fn remove_workspace(&self, id: i64) -> Result<(), StoreError> {
        match self
            .conn()?
            .execute("DELETE FROM workspaces WHERE id = ?1", [id])?
        {
            0 => Err(StoreError::WorkspaceNotFound(id)),
            _ => Ok(()),
        }
    }
}

/// A stored team: plan and worktrees as JSON (shapes owned by `plan`/`worktree`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRow {
    pub id: i64,
    pub workspace_id: i64,
    pub plan: String,
    pub worktrees: Option<String>,
    pub closed: bool,
}

const TEAM_COLS: &str = "id, workspace_id, plan, worktrees, closed";

fn team_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TeamRow> {
    Ok(TeamRow {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        plan: r.get(2)?,
        worktrees: r.get(3)?,
        closed: r.get(4)?,
    })
}

impl Store {
    pub fn insert_team(&self, workspace_id: i64, plan: &str) -> Result<i64, StoreError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO teams (workspace_id, plan) VALUES (?1, ?2)",
            params![workspace_id, plan],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_team(&self, id: i64) -> Result<TeamRow, StoreError> {
        self.conn()?
            .query_row(
                &format!("SELECT {TEAM_COLS} FROM teams WHERE id = ?1"),
                [id],
                team_row,
            )
            .optional()?
            .ok_or(StoreError::TeamNotFound(id))
    }

    /// Atomically record approval; fails if the team was approved already.
    pub fn approve_team(&self, id: i64, worktrees: &str) -> Result<(), StoreError> {
        let n = self.conn()?.execute(
            "UPDATE teams SET worktrees = ?2 WHERE id = ?1 AND worktrees IS NULL",
            params![id, worktrees],
        )?;
        match n {
            0 => {
                self.get_team(id)?;
                Err(StoreError::TeamAlreadyApproved(id))
            }
            _ => Ok(()),
        }
    }

    pub fn close_team(&self, id: i64) -> Result<(), StoreError> {
        match self
            .conn()?
            .execute("UPDATE teams SET closed = 1 WHERE id = ?1", [id])?
        {
            0 => Err(StoreError::TeamNotFound(id)),
            _ => Ok(()),
        }
    }

    /// The newest unapproved plan of a workspace, if made in the last 30 minutes and after its
    /// newest approved team (older drafts were abandoned by planning again or approving).
    pub fn latest_draft(&self, workspace_id: i64) -> Result<Option<TeamRow>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                &format!(
                    "SELECT {TEAM_COLS} FROM teams WHERE workspace_id = ?1 AND worktrees IS NULL \
                     AND closed = 0 AND created_at >= datetime('now', '-30 minutes') \
                     AND id > (SELECT COALESCE(MAX(id), 0) FROM teams \
                               WHERE workspace_id = ?1 AND worktrees IS NOT NULL) \
                     ORDER BY id DESC LIMIT 1"
                ),
                [workspace_id],
                team_row,
            )
            .optional()?)
    }

    /// Approved, not yet closed teams of a workspace, newest first.
    pub fn open_teams(&self, workspace_id: i64) -> Result<Vec<TeamRow>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {TEAM_COLS} FROM teams WHERE workspace_id = ?1 AND worktrees IS NOT NULL              AND closed = 0 ORDER BY id DESC"
        ))?;
        let rows = stmt.query_map([workspace_id], team_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// A task registry row; `status` strings are owned by `tasks::TaskStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub agent_id: String,
    pub status: String,
    pub summary: Option<String>,
    /// Detail of the latest history entry.
    pub detail: Option<String>,
    pub updated_at: String,
}

/// Task columns plus the latest history entry's detail.
const TASK_SELECT: &str = "SELECT agent_id, status, summary, updated_at, (SELECT detail FROM task_events e \
     WHERE e.team_id = t.team_id AND e.agent_id = t.agent_id ORDER BY e.id DESC LIMIT 1) FROM tasks t";

fn task_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        agent_id: r.get(0)?,
        status: r.get(1)?,
        summary: r.get(2)?,
        updated_at: r.get(3)?,
        detail: r.get(4)?,
    })
}

impl Store {
    /// Register every agent of a team as `status`, in one transaction.
    pub fn insert_tasks(&self, team: i64, agents: &[&str], status: &str) -> Result<(), StoreError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        for a in agents {
            tx.execute(
                "INSERT INTO tasks (team_id, agent_id, status) VALUES (?1, ?2, ?3)",
                params![team, a, status],
            )?;
            tx.execute(
                "INSERT INTO task_events (team_id, agent_id, status) VALUES (?1, ?2, ?3)",
                params![team, a, status],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn tasks(&self, team: i64) -> Result<Vec<TaskRow>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(&format!("{TASK_SELECT} WHERE team_id = ?1 ORDER BY rowid"))?;
        let rows = stmt.query_map([team], task_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn task(&self, team: i64, agent: &str) -> Result<Option<TaskRow>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                &format!("{TASK_SELECT} WHERE team_id = ?1 AND agent_id = ?2"),
                params![team, agent],
                task_row,
            )
            .optional()?)
    }

    pub fn task_status(&self, team: i64, agent: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn()?
            .query_row(
                "SELECT status FROM tasks WHERE team_id = ?1 AND agent_id = ?2",
                params![team, agent],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Change a task's status only if it is still `from`, recording the change in its history.
    /// `summary` belongs to the current run: it is replaced (None clears it).
    /// Returns false (and changes nothing) when the status moved on meanwhile.
    pub fn update_task(
        &self,
        team: i64,
        agent: &str,
        from: &str,
        to: &str,
        summary: Option<&str>,
        detail: Option<&str>,
    ) -> Result<bool, StoreError> {
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE tasks SET status = ?4, summary = ?5,              updated_at = datetime('now') WHERE team_id = ?1 AND agent_id = ?2 AND status = ?3",
            params![team, agent, from, to, summary],
        )?;
        if n == 0 {
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO task_events (team_id, agent_id, status, detail) VALUES (?1, ?2, ?3, ?4)",
            params![team, agent, to, detail],
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Status history of one agent, oldest first: (status, detail, at).
    #[cfg(test)]
    pub fn task_history(
        &self,
        team: i64,
        agent: &str,
    ) -> Result<Vec<(String, Option<String>, String)>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT status, detail, at FROM task_events WHERE team_id = ?1 AND agent_id = ?2              ORDER BY id",
        )?;
        let rows = stmt.query_map(params![team, agent], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// A stored message; `kind`/`status` strings are owned by `messages`/lib.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    pub id: i64,
    pub from_agent: String,
    pub to_agent: Option<String>,
    pub kind: String,
    pub text: String,
    pub status: String,
    pub reply_to: Option<i64>,
    pub at: String,
}

/// A message to store.
pub struct NewMessage<'a> {
    pub from: &'a str,
    pub to: Option<&'a str>,
    pub kind: &'a str,
    pub text: &'a str,
    pub status: &'a str,
    pub reply_to: Option<i64>,
}

impl Store {
    pub fn insert_message(&self, team: i64, m: &NewMessage<'_>) -> Result<i64, StoreError> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO messages (team_id, from_agent, to_agent, kind, text, status, reply_to)              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![team, m.from, m.to, m.kind, m.text, m.status, m.reply_to],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn messages(&self, team: i64) -> Result<Vec<MessageRow>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, from_agent, to_agent, kind, text, status, reply_to, at FROM messages              WHERE team_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map([team], |r| {
            Ok(MessageRow {
                id: r.get(0)?,
                from_agent: r.get(1)?,
                to_agent: r.get(2)?,
                kind: r.get(3)?,
                text: r.get(4)?,
                status: r.get(5)?,
                reply_to: r.get(6)?,
                at: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Change a message's status only if it is still `from`; false when it moved on.
    pub fn update_message_status(
        &self,
        team: i64,
        id: i64,
        from: &str,
        to: &str,
    ) -> Result<bool, StoreError> {
        Ok(self.conn()?.execute(
            "UPDATE messages SET status = ?4 WHERE team_id = ?1 AND id = ?2 AND status = ?3",
            params![team, id, from, to],
        )? > 0)
    }
}

impl Workspace {
    pub fn root(&self) -> PathBuf {
        PathBuf::from(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ina-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn add_list_get_remove_workspace() {
        let s = Store::in_memory().unwrap();
        let dir = tempdir("crud");
        let w = s.add_workspace(&dir.join(".").join("")).unwrap();
        assert!(w.name.starts_with("ina-store-crud"));
        assert!(!w.path.starts_with(r"\\?\"));
        assert_eq!(s.list_workspaces().unwrap(), vec![w.clone()]);
        assert_eq!(s.get_workspace(w.id).unwrap(), w);
        assert!(matches!(
            s.add_workspace(&dir),
            Err(StoreError::DuplicateWorkspace(_))
        ));
        s.remove_workspace(w.id).unwrap();
        assert!(s.list_workspaces().unwrap().is_empty());
        assert!(
            dir.is_dir(),
            "removing a workspace must not delete the directory"
        );
        assert!(matches!(
            s.remove_workspace(w.id),
            Err(StoreError::WorkspaceNotFound(_))
        ));
    }

    #[test]
    fn rejects_missing_dir_and_files() {
        let s = Store::in_memory().unwrap();
        let dir = tempdir("reject");
        assert!(matches!(
            s.add_workspace(&dir.join("nope")),
            Err(StoreError::NotADirectory(_))
        ));
        fs::write(dir.join("f.txt"), "").unwrap();
        assert!(matches!(
            s.add_workspace(&dir.join("f.txt")),
            Err(StoreError::NotADirectory(_))
        ));
    }

    #[test]
    fn persists_across_reopen_and_migrates_once() {
        let dir = tempdir("reopen");
        let db = dir.join("ina.db");
        let ws = tempdir("reopen-ws");
        Store::open(&db).unwrap().add_workspace(&ws).unwrap();
        let s = Store::open(&db).unwrap();
        assert_eq!(s.list_workspaces().unwrap().len(), 1);
    }

    #[test]
    fn latest_draft_is_newest_unapproved_after_approvals() {
        let dir = tempdir("drafts");
        let s = Store::open(&dir.join("ina.db")).unwrap();
        let ws = s.add_workspace(&dir).unwrap();
        assert!(s.latest_draft(ws.id).unwrap().is_none());
        let old = s.insert_team(ws.id, "{old}").unwrap();
        let new = s.insert_team(ws.id, "{new}").unwrap();
        assert_eq!(s.latest_draft(ws.id).unwrap().unwrap().id, new);
        s.approve_team(old, "{wt}").unwrap();
        assert_eq!(s.latest_draft(ws.id).unwrap().unwrap().id, new);
        s.approve_team(new, "{wt}").unwrap();
        assert!(s.latest_draft(ws.id).unwrap().is_none(), "approved");
        let stale = s.insert_team(ws.id, "{stale}").unwrap();
        s.conn()
            .unwrap()
            .execute(
                "UPDATE teams SET created_at = datetime('now', '-31 minutes') WHERE id = ?1",
                [stale],
            )
            .unwrap();
        assert!(s.latest_draft(ws.id).unwrap().is_none(), "too old");
    }

    #[test]
    fn team_lifecycle_persists() {
        let dir = tempdir("teams");
        let db = dir.join("ina.db");
        let ws = {
            let s = Store::open(&db).unwrap();
            let ws = s.add_workspace(&dir).unwrap();
            let id = s.insert_team(ws.id, "{plan}").unwrap();
            assert!(
                s.open_teams(ws.id).unwrap().is_empty(),
                "unapproved is not listed"
            );
            s.approve_team(id, "{wt}").unwrap();
            assert!(matches!(
                s.approve_team(id, "{x}"),
                Err(StoreError::TeamAlreadyApproved(_))
            ));
            assert!(matches!(
                s.approve_team(id + 9, "{x}"),
                Err(StoreError::TeamNotFound(_))
            ));
            ws
        };
        let s = Store::open(&db).unwrap(); // reopen: survives restart
        let open = s.open_teams(ws.id).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].worktrees.as_deref(), Some("{wt}"));
        s.close_team(open[0].id).unwrap();
        assert!(s.open_teams(ws.id).unwrap().is_empty());
        s.remove_workspace(ws.id).unwrap();
        assert!(matches!(
            s.get_team(open[0].id),
            Err(StoreError::TeamNotFound(_))
        ));
    }

    #[test]
    fn task_updates_are_conditional_and_logged() {
        let s = Store::in_memory().unwrap();
        let w = s.add_workspace(&tempdir("tasks")).unwrap();
        let t = s.insert_team(w.id, "{}").unwrap();
        s.insert_tasks(t, &["a", "b"], "pending").unwrap();
        assert!(
            s.update_task(t, "a", "pending", "running", None, None)
                .unwrap()
        );
        // Stale `from`: nothing changes.
        assert!(
            !s.update_task(t, "a", "pending", "done", None, None)
                .unwrap()
        );
        assert!(
            s.update_task(t, "a", "running", "done", Some("ok"), Some("result.json"))
                .unwrap()
        );
        let rows = s.tasks(t).unwrap();
        assert_eq!(
            rows.iter()
                .map(|r| (r.agent_id.as_str(), r.status.as_str(), r.summary.as_deref()))
                .collect::<Vec<_>>(),
            [("a", "done", Some("ok")), ("b", "pending", None)]
        );
        let h: Vec<String> = s
            .task_history(t, "a")
            .unwrap()
            .into_iter()
            .map(|e| e.0)
            .collect();
        assert_eq!(h, ["pending", "running", "done"]);
    }

    #[test]
    fn messages_are_stored_and_decided_once() {
        let s = Store::in_memory().unwrap();
        let w = s.add_workspace(&tempdir("msgs")).unwrap();
        let t = s.insert_team(w.id, "{}").unwrap();
        let id = s
            .insert_message(
                t,
                &NewMessage {
                    from: "orchestrator",
                    to: Some("ui"),
                    kind: "instruction",
                    text: "use flexbox",
                    status: "proposed",
                    reply_to: None,
                },
            )
            .unwrap();
        assert!(
            s.update_message_status(t, id, "proposed", "delivered")
                .unwrap()
        );
        assert!(
            !s.update_message_status(t, id, "proposed", "rejected")
                .unwrap()
        );
        let m = &s.messages(t).unwrap()[0];
        assert_eq!(
            (m.status.as_str(), m.to_agent.as_deref()),
            ("delivered", Some("ui"))
        );
    }
}
