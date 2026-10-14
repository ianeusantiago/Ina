//! Real PTY session lifecycle (ConPTY on Windows). Backend is the sole authority over processes.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("session already exists: {0}")]
    Duplicate(String),
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("pty error: {0}")]
    Pty(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Already-validated spawn request (see `policy`). No shell strings: program + argv only.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyEvent {
    Output(Vec<u8>),
    /// Process ended; `None` if the exit code could not be read.
    Exited(Option<u32>),
}

pub type EventSink = Arc<dyn Fn(&str, PtyEvent) + Send + Sync>;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

type Sessions = Arc<Mutex<HashMap<String, Session>>>;

#[derive(Default)]
pub struct PtyManager {
    sessions: Sessions,
}

fn pty_err(e: impl std::fmt::Display) -> PtyError {
    PtyError::Pty(e.to_string())
}

impl PtyManager {
    pub fn spawn(&self, id: &str, spec: SpawnSpec, sink: EventSink) -> Result<(), PtyError> {
        let mut sessions = self.sessions.lock().map_err(pty_err)?;
        if sessions.contains_key(id) {
            return Err(PtyError::Duplicate(id.to_owned()));
        }

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(pty_err)?;
        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        let mut child = pair.slave.spawn_command(cmd).map_err(pty_err)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(pty_err)?;
        let writer = pair.master.take_writer().map_err(pty_err)?;
        let killer = child.clone_killer();
        sessions.insert(
            id.to_owned(),
            Session {
                master: pair.master,
                writer,
                killer,
            },
        );
        drop(sessions);

        // Waiter: on exit, drop the session (closes the PTY so the reader hits EOF).
        let (code_tx, code_rx) = mpsc::channel::<Option<u32>>();
        let waiter_sessions = Arc::clone(&self.sessions);
        let waiter_id = id.to_owned();
        thread::spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code());
            if let Ok(mut s) = waiter_sessions.lock() {
                s.remove(&waiter_id);
            }
            let _ = code_tx.send(code);
        });

        // Reader: forward output in order, then report exit once output is drained.
        let reader_id = id.to_owned();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink(&reader_id, PtyEvent::Output(buf[..n].to_vec())),
                }
            }
            sink(&reader_id, PtyEvent::Exited(code_rx.recv().ok().flatten()));
        });
        Ok(())
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), PtyError> {
        let mut sessions = self.sessions.lock().map_err(pty_err)?;
        let s = sessions
            .get_mut(id)
            .ok_or_else(|| PtyError::NotFound(id.to_owned()))?;
        s.writer.write_all(data)?;
        Ok(s.writer.flush()?)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), PtyError> {
        let sessions = self.sessions.lock().map_err(pty_err)?;
        let s = sessions
            .get(id)
            .ok_or_else(|| PtyError::NotFound(id.to_owned()))?;
        s.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(pty_err)
    }

    pub fn kill(&self, id: &str) -> Result<(), PtyError> {
        let mut sessions = self.sessions.lock().map_err(pty_err)?;
        let s = sessions
            .get_mut(id)
            .ok_or_else(|| PtyError::NotFound(id.to_owned()))?;
        let res = s.killer.kill();
        // portable-pty 0.9.0 (pinned) inverts TerminateProcess's result on Windows:
        // Err means success, Ok means failure. Remove when upstream is fixed.
        #[cfg(windows)]
        let res = match res {
            Err(_) => Ok(()),
            Ok(()) => Err(PtyError::Pty("TerminateProcess failed".into())),
        };
        #[cfg(not(windows))]
        let res = res.map_err(PtyError::from);
        res
    }

    pub fn count(&self) -> usize {
        self.sessions.lock().map(|s| s.len()).unwrap_or(usize::MAX)
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.sessions
            .lock()
            .map(|s| s.contains_key(id))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn collect() -> (EventSink, mpsc::Receiver<PtyEvent>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        (
            Arc::new(move |_id: &str, ev| {
                let _ = tx.lock().unwrap().send(ev);
            }),
            rx,
        )
    }

    fn cmd(args: &[&str]) -> SpawnSpec {
        SpawnSpec {
            program: PathBuf::from("cmd.exe"),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd: std::env::temp_dir(),
            cols: 80,
            rows: 24,
        }
    }

    /// Spawn and answer ConPTY's initial cursor-position query (DSR), as xterm.js does in the app.
    fn start(m: &PtyManager, id: &str, spec: SpawnSpec, sink: EventSink) {
        m.spawn(id, spec, sink).unwrap();
        let _ = m.write(id, b"[1;1R");
    }

    fn wait_exit(rx: &mpsc::Receiver<PtyEvent>) -> (String, Option<u32>) {
        let mut out = Vec::new();
        loop {
            match rx
                .recv_timeout(Duration::from_secs(15))
                .unwrap_or_else(|e| panic!("{e:?}; got {:?}", String::from_utf8_lossy(&out)))
            {
                PtyEvent::Output(b) => out.extend(b),
                PtyEvent::Exited(code) => {
                    return (String::from_utf8_lossy(&out).into_owned(), code);
                }
            }
        }
    }

    #[test]
    fn runs_process_and_reports_output_and_exit() {
        let m = PtyManager::default();
        let (sink, rx) = collect();
        start(&m, "a", cmd(&["/c", "echo ina-ok && exit 3"]), sink);
        let (out, code) = wait_exit(&rx);
        assert!(out.contains("ina-ok"), "output: {out:?}");
        assert_eq!(code, Some(3));
        assert!(!m.is_running("a"));
    }

    #[test]
    fn rejects_duplicate_id() {
        let m = PtyManager::default();
        let (sink, rx) = collect();
        start(&m, "d", cmd(&["/k"]), sink.clone());
        assert!(matches!(
            m.spawn("d", cmd(&["/k"]), sink),
            Err(PtyError::Duplicate(_))
        ));
        m.kill("d").unwrap();
        wait_exit(&rx);
    }

    #[test]
    fn write_drives_interactive_session_and_kill_ends_it() {
        let m = PtyManager::default();
        let (sink, rx) = collect();
        start(&m, "k", cmd(&["/k"]), sink);
        m.resize("k", 120, 40).unwrap();
        m.write("k", b"exit 5\r\n").unwrap();
        let (_, code) = wait_exit(&rx);
        assert_eq!(code, Some(5));

        let (sink, rx) = collect();
        start(&m, "k2", cmd(&["/k"]), sink);
        m.kill("k2").unwrap();
        wait_exit(&rx);
        assert!(!m.is_running("k2"));
        assert!(matches!(m.kill("k2"), Err(PtyError::NotFound(_))));
    }
}
