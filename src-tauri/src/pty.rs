//! Real PTY session lifecycle (ConPTY on Windows). Backend is the sole authority over processes.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    #[error("session {0} was stopped before it started")]
    StoppedBeforeStart(String),
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
    /// Inherited environment variables removed before spawning.
    pub env_remove: Vec<std::ffi::OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtyEvent {
    Output(Vec<u8>),
    /// Process ended; `None` if the exit code could not be read.
    /// `code` is `None` if it could not be read; `killed` if `PtyManager::kill` ended it.
    Exited {
        code: Option<u32>,
        killed: bool,
    },
}

pub type EventSink = Arc<dyn Fn(&str, PtyEvent) + Send + Sync>;

struct Session {
    master: Box<dyn MasterPty + Send>,
    /// Own lock: a blocking write must not hold the session map (kill would deadlock).
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    killed: Arc<AtomicBool>,
}

type Sessions = Arc<Mutex<HashMap<String, Session>>>;

#[derive(Default)]
pub struct PtyManager {
    sessions: Sessions,
    /// Ids killed before their spawn registered them (e.g. a UI that unmounted mid-spawn):
    /// such a spawn is refused instead of leaving an orphan process.
    stopped_early: Mutex<HashSet<String>>,
}

/// Bound for `stopped_early`: ids are never reused, so stale entries are only a small leak.
const MAX_STOPPED_EARLY: usize = 1024;

/// Collected output of a child process. `complete` is false when a grandchild kept a pipe
/// open past the grace period (the output is then partial, but nothing hangs).
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub complete: bool,
}

/// Wait for a piped `child` up to `limit` (killing it on timeout) and collect its output.
/// Returns `None` as the status on timeout.
/// A non-interactive command for `spec`: its env scrub, args and cwd; no stdin, output piped.
pub fn piped_command(spec: &SpawnSpec) -> std::process::Command {
    let mut cmd = std::process::Command::new(&spec.program);
    for k in &spec.env_remove {
        cmd.env_remove(k);
    }
    cmd.args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    cmd
}

/// The last `max` characters of `s`, trimmed (char-safe).
pub fn tail(s: &str, max: usize) -> String {
    let s = s.trim();
    let start = s.char_indices().rev().nth(max).map_or(0, |(i, _)| i);
    s[start..].to_owned()
}

pub fn wait_output(
    child: &mut std::process::Child,
    limit: std::time::Duration,
) -> std::io::Result<(Option<std::process::ExitStatus>, Output)> {
    let drain = |r: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            let _ = tx.send(b);
        });
        rx
    };
    let out = drain(
        child
            .stdout
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let err = drain(
        child
            .stderr
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let status = wait_timeout(child, limit)?;
    let grace = std::time::Duration::from_secs(5);
    let (o, e) = (out.recv_timeout(grace), err.recv_timeout(grace));
    let complete = o.is_ok() && e.is_ok();
    Ok((
        status,
        Output {
            stdout: o.unwrap_or_default(),
            stderr: e.unwrap_or_default(),
            complete,
        },
    ))
}

/// Wait for `child` up to `limit`; on timeout kill and reap it and return `None`.
pub fn wait_timeout(
    child: &mut std::process::Child,
    limit: std::time::Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    let start = std::time::Instant::now();
    loop {
        if let Some(s) = child.try_wait()? {
            return Ok(Some(s));
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(std::time::Duration::from_millis(100));
    }
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
        if self.stopped_early.lock().map_err(pty_err)?.remove(id) {
            return Err(PtyError::StoppedBeforeStart(id.to_owned()));
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
        for k in &spec.env_remove {
            cmd.env_remove(k);
        }
        let mut child = pair.slave.spawn_command(cmd).map_err(pty_err)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(pty_err)?;
        let writer = pair.master.take_writer().map_err(pty_err)?;
        let killer = child.clone_killer();
        let killed = Arc::new(AtomicBool::new(false));
        sessions.insert(
            id.to_owned(),
            Session {
                master: pair.master,
                writer: Arc::new(Mutex::new(writer)),
                killer,
                killed: Arc::clone(&killed),
            },
        );
        drop(sessions);

        // Waiter: on exit, drop the session (closes the PTY so the reader hits EOF).
        let (code_tx, code_rx) = mpsc::channel::<(Option<u32>, bool)>();
        let waiter_sessions = Arc::clone(&self.sessions);
        let waiter_id = id.to_owned();
        thread::spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code());
            if let Ok(mut s) = waiter_sessions.lock() {
                s.remove(&waiter_id);
            }
            let _ = code_tx.send((code, killed.load(Ordering::SeqCst)));
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
            let (code, killed) = code_rx.recv().unwrap_or((None, false));
            sink(&reader_id, PtyEvent::Exited { code, killed });
        });
        Ok(())
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), PtyError> {
        let writer = {
            let sessions = self.sessions.lock().map_err(pty_err)?;
            let s = sessions
                .get(id)
                .ok_or_else(|| PtyError::NotFound(id.to_owned()))?;
            Arc::clone(&s.writer)
        };
        let mut w = writer.lock().map_err(pty_err)?;
        w.write_all(data)?;
        Ok(w.flush()?)
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
        let Some(s) = sessions.get_mut(id) else {
            let mut early = self.stopped_early.lock().map_err(pty_err)?;
            if early.len() >= MAX_STOPPED_EARLY {
                early.clear();
            }
            early.insert(id.to_owned());
            return Err(PtyError::NotFound(id.to_owned()));
        };
        s.killed.store(true, Ordering::SeqCst);
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
            env_remove: vec![],
        }
    }

    /// Spawn and answer ConPTY's initial cursor-position query (DSR), as xterm.js does in the app.
    fn start(m: &PtyManager, id: &str, spec: SpawnSpec, sink: EventSink) {
        m.spawn(id, spec, sink).unwrap();
        let _ = m.write(id, b"\x1b[1;1R");
    }

    fn wait_exit(rx: &mpsc::Receiver<PtyEvent>) -> (String, Option<u32>) {
        let mut out = Vec::new();
        loop {
            match rx
                .recv_timeout(Duration::from_secs(15))
                .unwrap_or_else(|e| panic!("{e:?}; got {:?}", String::from_utf8_lossy(&out)))
            {
                PtyEvent::Output(b) => out.extend(b),
                PtyEvent::Exited { code, .. } => {
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

        // Killed before it was spawned: the spawn is refused, nothing is left running.
        let (sink, _rx) = collect();
        assert!(m.kill("k3").is_err());
        assert!(matches!(
            m.spawn("k3", cmd(&["/k"]), sink),
            Err(PtyError::StoppedBeforeStart(_))
        ));
        assert!(!m.is_running("k3"));
    }

    /// Real `claude` in a PTY: must stay registered while its UI is up. Run with `--ignored`.
    #[test]
    #[ignore]
    fn real_claude_stays_registered() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let program =
            crate::policy::resolve_program(crate::policy::Program::Claude, &path).unwrap();
        let (sink, rx) = collect();
        let m = PtyManager::default();
        let spec = SpawnSpec {
            program,
            args: vec!["say hi".into()],
            cwd: std::env::temp_dir(),
            cols: 80,
            rows: 24,
            env_remove: vec![],
        };
        m.spawn("c", spec, sink).unwrap();
        std::thread::sleep(Duration::from_secs(8));
        let mut out = Vec::new();
        while let Ok(PtyEvent::Output(d)) = rx.try_recv() {
            out.extend(d);
        }
        let running = m.is_running("c");
        let _ = m.kill("c");
        println!("running={running} bytes={}", out.len());
        assert!(running);
    }

    /// Real `claude` declining the trust prompt must report `Exited`. Run with `--ignored`.
    #[test]
    #[ignore]
    fn real_claude_exit_is_reported() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let program =
            crate::policy::resolve_program(crate::policy::Program::Claude, &path).unwrap();
        let cwd = std::env::temp_dir().join(format!("ina-untrusted-{}", std::process::id()));
        std::fs::create_dir_all(&cwd).unwrap();
        let (sink, rx) = collect();
        let m = PtyManager::default();
        let spec = SpawnSpec {
            program,
            args: vec![],
            cwd,
            cols: 80,
            rows: 24,
            env_remove: vec![],
        };
        m.spawn("c", spec, sink).unwrap();
        m.write("c", b"\x1b[1;1R").unwrap(); // answer ConPTY's cursor query
        std::thread::sleep(Duration::from_secs(6));
        m.write("c", b"\r").unwrap(); // default choice: "No, exit"
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut exited = None;
        while std::time::Instant::now() < deadline {
            if let Ok(PtyEvent::Exited { code, .. }) = rx.recv_timeout(Duration::from_millis(200)) {
                exited = Some(code);
                break;
            }
        }
        println!("exited={exited:?} running={}", m.is_running("c"));
        let _ = m.kill("c");
        assert!(exited.is_some());
    }

    /// Child sees no inherited `CLAUDE*` vars. Meaningful when run from inside Claude Code.
    #[test]
    #[ignore]
    fn child_env_has_no_claude_vars() {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let cwd = std::env::temp_dir();
        let args = [
            "-NoProfile",
            "-Command",
            "(Get-ChildItem env:).Name -like 'CLAUDE*' -join ',' ; 'END'",
        ];
        let spec = crate::policy::authorize(
            crate::policy::Program::Shell,
            args.iter().map(|s| s.to_string()).collect(),
            &cwd,
            &cwd,
            0,
            &path,
        )
        .unwrap();
        assert!(
            !spec.env_remove.is_empty(),
            "run from a Claude Code session"
        );
        let (sink, rx) = collect();
        let m = PtyManager::default();
        m.spawn("e", spec, sink).unwrap();
        m.write("e", b"[1;1R").unwrap();
        let mut out = Vec::new();
        while let Ok(ev) = rx.recv_timeout(Duration::from_secs(15)) {
            match ev {
                PtyEvent::Output(d) => out.extend(d),
                PtyEvent::Exited { .. } => break,
            }
        }
        let text = String::from_utf8_lossy(&out);
        println!("{text}");
        assert!(text.contains("END") && !text.contains("CLAUDE"));
    }
}
