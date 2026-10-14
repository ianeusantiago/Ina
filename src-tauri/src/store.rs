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
    #[error("store lock poisoned")]
    Poisoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub path: String,
}

const MIGRATIONS: &[&str] = &["CREATE TABLE workspaces (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        path TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );"];

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
}
