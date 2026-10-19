//! Tokens a team's Claude sessions used, summed from their session logs (`message.usage`).
//! Claude Code writes one log line per content block, each repeating the message's usage, so
//! every message id is counted once.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Session logs found and read.
    pub sessions: u32,
}

impl TeamUsage {
    pub fn add_line(&mut self, v: &Value, seen: &mut HashSet<String>) {
        if v["type"].as_str() != Some("assistant") {
            return;
        }
        let msg = &v["message"];
        let Some(u) = msg.get("usage") else { return };
        if let Some(id) = msg["id"].as_str()
            && !seen.insert(id.to_owned())
        {
            return;
        }
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        self.input += n("input_tokens");
        self.output += n("output_tokens");
        self.cache_read += n("cache_read_input_tokens");
        self.cache_write += n("cache_creation_input_tokens");
    }

    pub fn add_log(&mut self, file: &Path) -> std::io::Result<()> {
        let mut seen = HashSet::new();
        for line in BufReader::new(std::fs::File::open(file)?).lines() {
            if let Ok(v) = serde_json::from_str::<Value>(&line?) {
                self.add_line(&v, &mut seen);
            }
        }
        self.sessions += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_each_message_once() {
        let mut u = TeamUsage::default();
        let mut seen = HashSet::new();
        let msg = |id: &str, out: u64| {
            json!({"type":"assistant","message":{"id":id,"usage":{
                "input_tokens":2,"output_tokens":out,
                "cache_read_input_tokens":10,"cache_creation_input_tokens":5}}})
        };
        u.add_line(&msg("m1", 100), &mut seen);
        u.add_line(&msg("m1", 100), &mut seen); // same message, another content block
        u.add_line(&msg("m2", 50), &mut seen);
        u.add_line(
            &json!({"type":"user","message":{"content":"hi"}}),
            &mut seen,
        );
        assert_eq!(
            u,
            TeamUsage {
                input: 4,
                output: 150,
                cache_read: 20,
                cache_write: 10,
                sessions: 0
            }
        );
    }

    #[test]
    fn reads_a_log_file() {
        let path = std::env::temp_dir().join(format!("ina-usage-{}.jsonl", std::process::id()));
        std::fs::write(
            &path,
            "{\"type\":\"assistant\",\"message\":{\"id\":\"a\",\"usage\":{\"output_tokens\":7}}}\nnot json\n",
        )
        .unwrap();
        let mut u = TeamUsage::default();
        u.add_log(&path).unwrap();
        assert_eq!((u.output, u.sessions), (7, 1));
        std::fs::remove_file(&path).unwrap();
    }
}
