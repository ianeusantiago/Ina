//! Final report of a finished team, in plain words: what was built and how to try it.
//! Written by a tool-less `claude -p` from the plan and the agents' own result summaries.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// What the team did, for someone who may not be a programmer.
    pub done: String,
    /// Steps to see or try the result.
    pub how_to_test: String,
}

pub const REPORT_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["done","howToTest"],"properties":{"done":{"type":"string","maxLength":1500},"howToTest":{"type":"string","maxLength":1500}}}"#;

/// One agent's part, as INA recorded it.
pub struct Part<'a> {
    pub name: &'a str,
    pub task: &'a str,
    pub summary: Option<&'a str>,
}

pub fn prompt(goal: &str, parts: &[Part]) -> String {
    let lines: Vec<String> = parts
        .iter()
        .map(|p| {
            format!(
                "- {}: task: {} | result: {}",
                p.name,
                p.task,
                p.summary.unwrap_or("(no summary)")
            )
        })
        .collect();
    format!(
        "A team of AI agents finished this goal: {goal}\n\nWhat each agent reported:\n{}\n\n\
         Write a short final report for the user, who may not be a programmer, in the language \
         the goal is written in. done: 2-4 plain sentences on what was built or changed. \
         howToTest: 1-4 short numbered steps to see or try the result (you may name one command \
         to run if it is truly needed, written exactly). No commit hashes, internal ids or \
         jargon. Do not modify any files.",
        lines.join("\n")
    )
}

pub fn args(prompt: &str) -> Vec<String> {
    crate::claude_json::args(prompt, REPORT_SCHEMA)
}

#[cfg(test)]
pub fn parse_output(stdout: &str) -> Result<Report, String> {
    validate(crate::claude_json::parse(stdout)?)
}

pub fn validate(r: Report) -> Result<Report, String> {
    if r.done.trim().is_empty() {
        return Err("empty report".into());
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_lists_every_part() {
        let p = prompt(
            "Fazer um blog",
            &[
                Part {
                    name: "Visual",
                    task: "páginas",
                    summary: Some("feito"),
                },
                Part {
                    name: "Posts",
                    task: "dados",
                    summary: None,
                },
            ],
        );
        assert!(p.contains("Fazer um blog"));
        assert!(p.contains("- Visual: task: páginas | result: feito"));
        assert!(p.contains("- Posts: task: dados | result: (no summary)"));
    }

    #[test]
    fn parses_reports() {
        let ok = r#"{"structured_output":{"done":"Fiz o blog.","howToTest":"1. Abra."}}"#;
        assert_eq!(parse_output(ok).unwrap().how_to_test, "1. Abra.");
        assert!(parse_output(r#"{"structured_output":{"done":" ","howToTest":"x"}}"#).is_err());
        assert!(parse_output(r#"{"is_error":true,"result":"boom"}"#).is_err());
        assert!(parse_output("not json").is_err());
    }
}
