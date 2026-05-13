use std::{path::Path, process::Command};

use anyhow::{Context, Result};
use serde::Deserialize;

const DEFAULT_MAX_RESULTS: usize = 15;
const MAX_RESULTS_HARD_CAP: usize = 20;
const DEFAULT_CONTEXT_LINES: usize = 2;
const MAX_CONTEXT_LINES: usize = 4;
const MAX_MATCHES_PER_FILE: usize = 3;
const MAX_OUTPUT_CHARS: usize = 3_000;

pub struct SearchParams {
    pub query: String,
    /// ripgrep glob pattern, e.g. `*.rs`
    pub file_glob: Option<String>,
    pub max_results: usize,
    pub context_lines: usize,
}

impl SearchParams {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            file_glob: None,
            max_results: DEFAULT_MAX_RESULTS,
            context_lines: DEFAULT_CONTEXT_LINES,
        }
    }
}

pub struct SearchResult {
    pub query: String,
    pub match_count: usize,
    pub truncated: bool,
    /// Ready-to-use text block for the model's context pack.
    pub formatted: String,
}

pub fn search(workspace: &Path, params: SearchParams) -> Result<SearchResult> {
    let max_results = params.max_results.min(MAX_RESULTS_HARD_CAP);
    let context_lines = params.context_lines.min(MAX_CONTEXT_LINES);

    let mut cmd = Command::new("rg");
    cmd.current_dir(workspace)
        .arg("--json")
        .arg("--smart-case")
        .arg("--max-count")
        .arg(MAX_MATCHES_PER_FILE.to_string())
        .arg("--context")
        .arg(context_lines.to_string())
        .arg("--max-filesize")
        .arg("524288");

    if let Some(glob) = &params.file_glob {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg("--").arg(&params.query);

    let output = cmd
        .output()
        .context("failed to run ripgrep — is rg installed and on PATH?")?;

    // exit code 2 = real error; exit code 1 = no matches (ok)
    if output.status.code() == Some(2) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("ripgrep error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = parse_rg_json(&stdout, max_results);
    let match_count: usize = files.iter().map(|f| f.match_count).sum();
    let truncated = match_count >= max_results;
    let formatted = format_output(&params.query, &files, truncated);

    Ok(SearchResult {
        query: params.query,
        match_count,
        truncated,
        formatted,
    })
}

// ---------------------------------------------------------------------------
// rg --json parsing
// ---------------------------------------------------------------------------

struct FileLine {
    line_number: usize,
    text: String,
    is_match: bool,
}

struct FileResult {
    file: String,
    lines: Vec<FileLine>,
    match_count: usize,
}

#[derive(Deserialize)]
struct RgMessage {
    #[serde(rename = "type")]
    kind: String,
    data: RgData,
}

#[derive(Deserialize)]
struct RgData {
    path: Option<RgText>,
    line_number: Option<usize>,
    lines: Option<RgText>,
}

#[derive(Deserialize)]
struct RgText {
    text: Option<String>,
}

fn parse_rg_json(stdout: &str, max_results: usize) -> Vec<FileResult> {
    let mut results: Vec<FileResult> = Vec::new();
    let mut current: Option<FileResult> = None;
    let mut total_matches: usize = 0;

    for line in stdout.lines() {
        if total_matches >= max_results {
            break;
        }

        let Ok(msg) = serde_json::from_str::<RgMessage>(line) else {
            continue;
        };

        match msg.kind.as_str() {
            "begin" => {
                if let Some(prev) = current.take() {
                    if prev.match_count > 0 {
                        results.push(prev);
                    }
                }
                let file = msg.data.path.and_then(|p| p.text).unwrap_or_default();
                current = Some(FileResult {
                    file,
                    lines: vec![],
                    match_count: 0,
                });
            }
            "match" => {
                let line_number = msg.data.line_number.unwrap_or(0);
                let text = msg
                    .data
                    .lines
                    .and_then(|l| l.text)
                    .unwrap_or_default()
                    .trim_end_matches('\n')
                    .to_string();
                if let Some(r) = &mut current {
                    r.lines.push(FileLine {
                        line_number,
                        text,
                        is_match: true,
                    });
                    r.match_count += 1;
                    total_matches += 1;
                }
            }
            "context" => {
                let line_number = msg.data.line_number.unwrap_or(0);
                let text = msg
                    .data
                    .lines
                    .and_then(|l| l.text)
                    .unwrap_or_default()
                    .trim_end_matches('\n')
                    .to_string();
                if let Some(r) = &mut current {
                    r.lines.push(FileLine {
                        line_number,
                        text,
                        is_match: false,
                    });
                }
            }
            "end" => {
                if let Some(r) = current.take() {
                    if r.match_count > 0 {
                        results.push(r);
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(r) = current {
        if r.match_count > 0 {
            results.push(r);
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn format_output(query: &str, files: &[FileResult], truncated: bool) -> String {
    let total: usize = files.iter().map(|f| f.match_count).sum();

    let mut out = String::new();

    out.push_str(&format!("search: {:?}\n", query));
    if total == 0 {
        out.push_str("no matches\n");
        return out;
    }

    out.push_str(&format!("matches: {total}"));
    if truncated {
        out.push_str(" (capped — refine query or use a file glob)");
    }
    out.push_str("\n\n");

    for file in files {
        out.push_str(&file.file);
        out.push('\n');

        let mut prev_line: Option<usize> = None;
        for fl in &file.lines {
            if let Some(prev) = prev_line {
                if fl.line_number > prev + 1 {
                    out.push_str("  ...\n");
                }
            }
            let marker = if fl.is_match { '>' } else { ' ' };
            out.push_str(&format!("{marker} {:4} | {}\n", fl.line_number, fl.text));
            prev_line = Some(fl.line_number);
        }
        out.push('\n');

        if out.len() >= MAX_OUTPUT_CHARS {
            out.push_str("... output limit reached\n");
            break;
        }
    }

    out
}
