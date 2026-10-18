//! Structured messages between INA, the user, the orchestrator and agents.
//!
//! A session writes JSON files to `<dir>/.ina/outbox/`; INA validates them, stores them and
//! moves each file to `outbox/done/` (or `outbox/rejected/`). INA delivers messages to an agent
//! as `<worktree>/.ina/inbox/<id>.json` and tells its terminal with one fixed line.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tasks::INA_DIR;

const OUTBOX: &str = "outbox";
const INBOX: &str = "inbox";
const MAX_MESSAGE_BYTES: u64 = 16 * 1024;
const MAX_TEXT: usize = 4000;
/// Files ingested per poll, so a runaway writer cannot stall INA.
const MAX_PER_POLL: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Progress,
    Question,
    Blocked,
    Handoff,
    /// Orchestrator → agent; delivered only after the user approves.
    Instruction,
    /// User → agent (an answer or a direct instruction).
    Reply,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Progress => "progress",
            Self::Question => "question",
            Self::Blocked => "blocked",
            Self::Handoff => "handoff",
            Self::Instruction => "instruction",
            Self::Reply => "reply",
        }
    }
}

/// What a session may write to its outbox.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outgoing {
    #[serde(rename = "type")]
    pub kind: Kind,
    pub text: String,
    /// Target agent id (handoff, instruction). Absent means the user / orchestrator.
    #[serde(default)]
    pub to: Option<String>,
}

/// Validate an outbox message. Agents may send progress/question/blocked/handoff; the
/// orchestrator only instructions, addressed to a known agent.
pub fn parse_outgoing(
    text: &str,
    from_orchestrator: bool,
    agents: &[&str],
) -> Result<Outgoing, String> {
    let m: Outgoing = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if m.text.trim().is_empty() || m.text.len() > MAX_TEXT {
        return Err(format!("text is empty or longer than {MAX_TEXT} bytes"));
    }
    let allowed = if from_orchestrator {
        m.kind == Kind::Instruction
    } else {
        matches!(
            m.kind,
            Kind::Progress | Kind::Question | Kind::Blocked | Kind::Handoff
        )
    };
    if !allowed {
        return Err(format!("type {:?} is not allowed here", m.kind.as_str()));
    }
    match (&m.to, m.kind) {
        (Some(to), _) if !agents.contains(&to.as_str()) => Err(format!("unknown agent {to:?}")),
        (None, Kind::Instruction) => Err("an instruction needs `to`".into()),
        _ => Ok(m),
    }
}

/// One outbox file, read and moved out of the way.
pub struct Ingested {
    pub file: String,
    pub message: Result<Outgoing, String>,
}

/// Read up to `MAX_PER_POLL` outbox files of `dir`, oldest name first, moving each to
/// `done/` or `rejected/` so it is ingested once.
pub fn ingest(
    dir: &Path,
    from_orchestrator: bool,
    agents: &[&str],
) -> std::io::Result<Vec<Ingested>> {
    // Overlapping polls (two panels, a remount) would read a file twice before moving it.
    static INGESTING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = INGESTING
        .lock()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let outbox = dir.join(INA_DIR).join(OUTBOX);
    let entries = match fs::read_dir(&outbox) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files.truncate(MAX_PER_POLL);
    let mut out = Vec::new();
    for path in files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let message = match fs::metadata(&path) {
            Ok(m) if m.len() > MAX_MESSAGE_BYTES => {
                Err(format!("larger than {MAX_MESSAGE_BYTES} bytes"))
            }
            Ok(_) => fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| parse_outgoing(&t, from_orchestrator, agents)),
            Err(e) => Err(e.to_string()),
        };
        let dest = outbox.join(if message.is_ok() { "done" } else { "rejected" });
        fs::create_dir_all(&dest)?;
        fs::rename(&path, dest.join(&name))?;
        out.push(Ingested {
            file: name,
            message,
        });
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InboxFile<'a> {
    id: i64,
    from: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
    text: &'a str,
    reply_to: Option<i64>,
}

/// Deliver a message to an agent's inbox; returns its path relative to the worktree.
pub fn write_inbox(
    worktree: &Path,
    id: i64,
    from: &str,
    kind: Kind,
    text: &str,
    reply_to: Option<i64>,
) -> std::io::Result<String> {
    let dir = worktree.join(INA_DIR).join(INBOX);
    fs::create_dir_all(&dir)?;
    let name = format!("{id}.json");
    let json = serde_json::to_string_pretty(&InboxFile {
        id,
        from,
        kind: kind.as_str(),
        text,
        reply_to,
    })
    .map_err(std::io::Error::other)?;
    fs::write(dir.join(&name), json)?;
    Ok(format!("{INA_DIR}/{INBOX}/{name}"))
}

/// The only text INA types into an agent's terminal: a pointer, never the message itself.
pub fn notice(inbox_path: &str) -> String {
    format!("INA: new message in {inbox_path}. Read it and continue.\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ina-msg-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn outgoing_rules() {
        let agents = ["api", "ui"];
        assert!(
            parse_outgoing(r#"{"type":"question","text":"which db?"}"#, false, &agents).is_ok()
        );
        assert!(
            parse_outgoing(
                r#"{"type":"handoff","text":"api ready","to":"ui"}"#,
                false,
                &agents
            )
            .is_ok()
        );
        assert!(
            parse_outgoing(
                r#"{"type":"instruction","text":"x","to":"ui"}"#,
                true,
                &agents
            )
            .is_ok()
        );
        for (json, orch) in [
            (r#"{"type":"instruction","text":"x","to":"ui"}"#, false),
            (r#"{"type":"question","text":"x"}"#, true),
            (r#"{"type":"instruction","text":"x"}"#, true),
            (r#"{"type":"handoff","text":"x","to":"ghost"}"#, false),
            (r#"{"type":"progress","text":"  "}"#, false),
            (r#"{"type":"progress","text":"x","extra":1}"#, false),
            (r#"{"type":"reply","text":"x"}"#, false),
        ] {
            assert!(parse_outgoing(json, orch, &agents).is_err(), "{json}");
        }
    }

    #[test]
    fn ingest_moves_files_once() {
        let dir = tempdir("ingest");
        let outbox = dir.join(".ina/outbox");
        fs::create_dir_all(&outbox).unwrap();
        fs::write(
            outbox.join("1.json"),
            r#"{"type":"progress","text":"half"}"#,
        )
        .unwrap();
        fs::write(outbox.join("2.json"), "nope").unwrap();
        fs::write(outbox.join("note.txt"), "ignored").unwrap();
        let got = ingest(&dir, false, &["api"]).unwrap();
        assert_eq!(got.len(), 2);
        assert!(got[0].message.is_ok() && got[1].message.is_err());
        assert!(outbox.join("done/1.json").exists());
        assert!(outbox.join("rejected/2.json").exists());
        assert!(ingest(&dir, false, &["api"]).unwrap().is_empty());
    }

    #[test]
    fn inbox_and_notice() {
        let dir = tempdir("inbox");
        let rel = write_inbox(&dir, 9, "user", Kind::Reply, "use sqlite", Some(3)).unwrap();
        assert_eq!(rel, ".ina/inbox/9.json");
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join(&rel)).unwrap()).unwrap();
        assert_eq!(
            (v["type"].as_str(), v["replyTo"].as_i64()),
            (Some("reply"), Some(3))
        );
        assert!(!notice(&rel).contains("sqlite"));
    }
}
