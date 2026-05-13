use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};

use super::index::{Symbol, extract_symbols, language_for};

const MAX_RANGE_LINES: usize = 150;
const DEFAULT_AROUND_BEFORE: usize = 5;
const DEFAULT_AROUND_AFTER: usize = 5;
const MAX_AROUND_LINES: usize = 20;
const MAX_AROUND_OCCURRENCES: usize = 3;
const MAX_OUTPUT_CHARS: usize = 4_000;

// ---------------------------------------------------------------------------
// read_range
// ---------------------------------------------------------------------------

/// Read a specific line range from a file (1-indexed, inclusive).
/// Capped at MAX_RANGE_LINES lines.
pub fn read_range(workspace: &Path, rel_path: &str, start: usize, end: usize) -> Result<String> {
    if start == 0 {
        return Err(anyhow!("start line must be >= 1"));
    }
    if end < start {
        return Err(anyhow!("end line must be >= start line"));
    }

    let path = resolve(workspace, rel_path)?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("cannot read {rel_path}"))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    let lo = (start - 1).min(total);
    let hi = end.min(total);
    let requested = hi.saturating_sub(lo);

    let (hi, truncated) = if requested > MAX_RANGE_LINES {
        (lo + MAX_RANGE_LINES, true)
    } else {
        (hi, false)
    };

    let mut out = String::new();
    out.push_str(&format!(
        "{rel_path}  lines {start}–{}\n\n",
        lo + (hi - lo)
    ));

    for (i, line) in lines[lo..hi].iter().enumerate() {
        out.push_str(&format!("  {:4} | {line}\n", lo + i + 1));
    }

    if truncated {
        out.push_str(&format!(
            "\n... capped at {MAX_RANGE_LINES} lines (requested {requested})\n"
        ));
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// read_around
// ---------------------------------------------------------------------------

pub struct AroundParams {
    pub before: usize,
    pub after: usize,
}

impl Default for AroundParams {
    fn default() -> Self {
        Self {
            before: DEFAULT_AROUND_BEFORE,
            after: DEFAULT_AROUND_AFTER,
        }
    }
}

/// Find up to MAX_AROUND_OCCURRENCES of `query` in `file` and return a
/// windowed snippet around each match. Case-insensitive.
pub fn read_around(
    workspace: &Path,
    rel_path: &str,
    query: &str,
    params: AroundParams,
) -> Result<String> {
    if query.is_empty() {
        return Err(anyhow!("query must not be empty"));
    }

    let before = params.before.min(MAX_AROUND_LINES);
    let after = params.after.min(MAX_AROUND_LINES);

    let path = resolve(workspace, rel_path)?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("cannot read {rel_path}"))?;

    let lines: Vec<&str> = content.lines().collect();
    let query_lower = query.to_lowercase();

    // Collect match line indices (0-based)
    let mut match_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.to_lowercase().contains(&query_lower))
        .map(|(i, _)| i)
        .collect();

    if match_indices.is_empty() {
        return Ok(format!("{rel_path}  no occurrences of {:?}\n", query));
    }

    let total = match_indices.len();
    match_indices.truncate(MAX_AROUND_OCCURRENCES);

    let mut out = String::new();
    out.push_str(&format!(
        "{rel_path}  {} occurrence{} of {:?}",
        total,
        if total == 1 { "" } else { "s" },
        query
    ));
    if total > MAX_AROUND_OCCURRENCES {
        out.push_str(&format!(
            " (showing first {MAX_AROUND_OCCURRENCES})"
        ));
    }
    out.push_str("\n\n");

    let n = lines.len();
    for (occurrence, &match_idx) in match_indices.iter().enumerate() {
        let lo = match_idx.saturating_sub(before);
        let hi = (match_idx + after + 1).min(n);

        out.push_str(&format!(
            "match {} — line {}\n",
            occurrence + 1,
            match_idx + 1
        ));

        for (i, line) in lines[lo..hi].iter().enumerate() {
            let line_num = lo + i + 1;
            let marker = if lo + i == match_idx { '>' } else { ' ' };
            out.push_str(&format!("{marker} {:4} | {line}\n", line_num));
        }
        out.push('\n');

        if out.len() >= MAX_OUTPUT_CHARS {
            out.push_str("... output limit reached\n");
            break;
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// list_symbols
// ---------------------------------------------------------------------------

/// Extract and list all symbols from a file. Always re-reads from disk
/// so results are fresh regardless of index state.
pub fn list_symbols(workspace: &Path, rel_path: &str) -> Result<String> {
    let path = resolve(workspace, rel_path)?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let lang = language_for(ext);

    let symbols: Vec<Symbol> = match &lang {
        Some(l) => {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("cannot read {rel_path}"))?;
            extract_symbols(&content, l)
        }
        None => vec![],
    };

    let mut out = String::new();

    if symbols.is_empty() {
        out.push_str(&format!(
            "{rel_path}  no symbols (language: {})\n",
            lang.as_deref().unwrap_or("unknown")
        ));
        return Ok(out);
    }

    out.push_str(&format!(
        "{rel_path}  {} symbol{}\n\n",
        symbols.len(),
        if symbols.len() == 1 { "" } else { "s" }
    ));

    for sym in &symbols {
        out.push_str(&format!(
            "  {:5}  {:<8}  {}\n",
            sym.line, sym.kind, sym.name
        ));
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve(workspace: &Path, rel_path: &str) -> Result<PathBuf> {
    let path = workspace.join(rel_path);
    let canonical = path
        .canonicalize()
        .with_context(|| format!("file not found: {rel_path}"))?;
    if !canonical.starts_with(workspace) {
        return Err(anyhow!("path escapes workspace: {rel_path}"));
    }
    Ok(canonical)
}
