use std::{
    fs,
    io::Write,
    path::Path,
};

use anyhow::Result;
use chrono::Local;

const MAX_LOAD_CHARS: usize = 2_000;

/// Append a decision record to `.cinto/memory/decisions.md`.
pub fn save_decision(workspace: &Path, task: &str, decision: &str) -> Result<()> {
    let dir = workspace.join(".cinto").join("memory");
    fs::create_dir_all(&dir)?;

    let path = dir.join("decisions.md");
    let entry = format!(
        "\n## {}\n**Task:** {}\n\n{}\n",
        Local::now().format("%Y-%m-%d %H:%M"),
        task.lines().next().unwrap_or("").trim(),
        decision.trim(),
    );

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(entry.as_bytes())?;
    Ok(())
}

/// Load recent decisions for inclusion in a context pack.
/// Returns the last MAX_LOAD_CHARS of decisions.md, or an empty string.
pub fn load_recent_decisions(workspace: &Path) -> String {
    let path = workspace.join(".cinto").join("memory").join("decisions.md");
    let content = fs::read_to_string(path).unwrap_or_default();
    if content.is_empty() {
        return String::new();
    }
    if content.len() <= MAX_LOAD_CHARS {
        content
    } else {
        // Start from a clean entry boundary
        let tail = &content[content.len() - MAX_LOAD_CHARS..];
        tail.find("\n## ")
            .map(|i| tail[i..].to_string())
            .unwrap_or_else(|| tail.to_string())
    }
}
