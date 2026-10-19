//! Reviewer before merging: a tool-less `claude -p` reads each agent's changes (diff against the
//! workspace's branch) and answers in plain words whether it is fine to merge.

use serde::{Deserialize, Serialize};

/// Diff text budget for the whole prompt (it goes on the command line; Windows caps it at ~32K).
const DIFF_BUDGET: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub agent: String,
    /// "high" | "medium" | "low"
    pub severity: String,
    pub problem: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    /// "ok" (safe to merge) or "issues".
    pub verdict: String,
    pub summary: String,
    pub findings: Vec<Finding>,
    /// Some diffs were cut to fit; filled in by INA, not the model.
    #[serde(default)]
    pub truncated: bool,
}

pub const REVIEW_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["verdict","summary","findings"],"properties":{"verdict":{"type":"string","enum":["ok","issues"]},"summary":{"type":"string","maxLength":800},"findings":{"type":"array","maxItems":10,"items":{"type":"object","additionalProperties":false,"required":["agent","severity","problem"],"properties":{"agent":{"type":"string"},"severity":{"type":"string","enum":["high","medium","low"]},"problem":{"type":"string","maxLength":500}}}}}}"#;

pub struct Part<'a> {
    pub name: &'a str,
    pub task: &'a str,
    pub stat: &'a str,
    pub diff: &'a str,
}

/// Prompt with every agent's changes; diffs share `DIFF_BUDGET`. Returns (prompt, truncated).
pub fn prompt(goal: &str, parts: &[Part]) -> (String, bool) {
    let share = DIFF_BUDGET / parts.len().max(1);
    let mut truncated = false;
    let blocks: Vec<String> = parts
        .iter()
        .map(|p| {
            let diff = match p.diff.char_indices().nth(share) {
                Some((i, _)) => {
                    truncated = true;
                    format!("{}\n[... diff cut to fit ...]", &p.diff[..i])
                }
                None => p.diff.to_string(),
            };
            format!(
                "### Agent: {}\nTask: {}\nFiles changed:\n{}\nDiff:\n{}",
                p.name,
                p.task,
                p.stat.trim(),
                diff.trim()
            )
        })
        .collect();
    let text = format!(
        "You review the work of a team of AI coding agents before it is merged. Goal: {goal}\n\n\
         {}\n\nCheck each agent's changes against its task: bugs, missing parts, broken or \
         unsafe code, leftover debug code, changes outside the task. Say verdict \"ok\" only if it \
         is safe to merge. Write summary and each problem in the language of the goal, in plain \
         words for someone who may not be a programmer (a file name is fine when it helps). Do \
         not invent problems; an empty findings list is fine. Do not modify any files.",
        blocks.join("\n\n")
    );
    (text, truncated)
}

pub fn args(prompt: &str) -> Vec<String> {
    [
        "-p",
        prompt,
        "--output-format",
        "json",
        "--json-schema",
        REVIEW_SCHEMA,
        "--tools",
        "",
        "--strict-mcp-config",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn parse_output(stdout: &str) -> Result<Review, String> {
    #[derive(Deserialize)]
    struct Envelope {
        #[serde(default)]
        is_error: bool,
        #[serde(default)]
        result: Option<String>,
        structured_output: Option<Review>,
    }
    let env: Envelope =
        serde_json::from_str(stdout.trim()).map_err(|e| format!("unparseable output: {e}"))?;
    if env.is_error {
        return Err(env.result.unwrap_or_else(|| "unknown error".into()));
    }
    let r = env.structured_output.ok_or("no structured output")?;
    if r.verdict != "ok" && r.verdict != "issues" {
        return Err(format!("unknown verdict {:?}", r.verdict));
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_parts_and_cuts_big_diffs() {
        let big = "+x\n".repeat(20_000);
        let (p, cut) = prompt(
            "Blog",
            &[
                Part {
                    name: "A",
                    task: "pages",
                    stat: " a.html | 2 +",
                    diff: "+<h1>",
                },
                Part {
                    name: "B",
                    task: "posts",
                    stat: " b.js | 1 +",
                    diff: &big,
                },
            ],
        );
        assert!(cut);
        assert!(p.contains("### Agent: A") && p.contains("+<h1>"));
        assert!(p.contains("[... diff cut to fit ...]"));
        assert!(
            p.len() < 30_000,
            "prompt too long for the command line: {}",
            p.len()
        );
        let (_, cut) = prompt(
            "x",
            &[Part {
                name: "A",
                task: "t",
                stat: "",
                diff: "small",
            }],
        );
        assert!(!cut);
    }

    #[test]
    fn parses_reviews() {
        let ok = r#"{"structured_output":{"verdict":"ok","summary":"Tudo certo.","findings":[]}}"#;
        assert_eq!(parse_output(ok).unwrap().verdict, "ok");
        let issues = r#"{"structured_output":{"verdict":"issues","summary":"s","findings":[{"agent":"A","severity":"high","problem":"quebra"}]}}"#;
        assert_eq!(parse_output(issues).unwrap().findings[0].severity, "high");
        assert!(
            parse_output(r#"{"structured_output":{"verdict":"maybe","summary":"","findings":[]}}"#)
                .is_err()
        );
        assert!(parse_output(r#"{"is_error":true,"result":"x"}"#).is_err());
    }
}
