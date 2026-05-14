use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow};
use tokio::sync::oneshot;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EditDirective {
    pub path: String,
    pub mode: EditMode,
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum EditMode {
    Replace,
    ReplaceFunction(String),
    Prepend,
    Append,
}

pub struct ApplyRequest {
    pub directive: EditDirective,
    pub preview: String,
    pub response_tx: oneshot::Sender<bool>,
}

#[derive(Debug, Default)]
pub struct PatchResult {
    pub directives_found: usize,
    pub files_written: Vec<String>,
    pub rejected: Vec<String>,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse `<EDIT path="..." mode="...">content</EDIT>` blocks from a CRP
/// FILE_EDITS slot or plain model output.
pub fn parse_edit_directives(text: &str) -> Vec<EditDirective> {
    let mut directives = Vec::new();
    let mut remaining = text;

    while let Some(rel) = remaining.find("<EDIT ") {
        remaining = &remaining[rel..];

        let Some(tag_end) = remaining.find('>') else {
            break;
        };
        let tag = &remaining[..tag_end + 1];
        remaining = &remaining[tag_end + 1..];

        let path = match extract_attr(tag, "path") {
            Some(p) => p,
            None => continue,
        };
        let mode_str = extract_attr(tag, "mode").unwrap_or_else(|| "replace".into());
        let mode = parse_mode(&mode_str);

        let Some(close) = remaining.find("</EDIT>") else {
            break;
        };
        let content = remaining[..close].trim().to_string();
        remaining = &remaining[close + "</EDIT>".len()..];

        if !content.is_empty() {
            directives.push(EditDirective { path, mode, content });
        }
    }

    // Terse @@ format fallback (Appendix B of CRP spec)
    if directives.is_empty() {
        directives = parse_terse_format(text);
    }

    directives
}

fn parse_terse_format(text: &str) -> Vec<EditDirective> {
    let mut directives = Vec::new();
    let mut current: Option<(String, EditMode, Vec<&str>)> = None;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            if let Some((path, mode, lines)) = current.take() {
                let content = lines.join("\n").trim().to_string();
                if !content.is_empty() {
                    directives.push(EditDirective { path, mode, content });
                }
            }
            let mut parts = rest.splitn(2, ' ');
            let path = parts.next().unwrap_or("").to_string();
            let mode_str = parts.next().unwrap_or("replace");
            let mode = parse_mode(mode_str);
            if !path.is_empty() {
                current = Some((path, mode, Vec::new()));
            }
        } else if let Some((_, _, ref mut lines)) = current {
            lines.push(line);
        }
    }

    if let Some((path, mode, lines)) = current {
        let content = lines.join("\n").trim().to_string();
        if !content.is_empty() {
            directives.push(EditDirective { path, mode, content });
        }
    }

    directives
}

fn parse_mode(s: &str) -> EditMode {
    if s.starts_with("replace_function:") {
        let name = s["replace_function:".len()..].trim().to_string();
        return EditMode::ReplaceFunction(name);
    }
    match s.trim() {
        "prepend" | "prepend_to_existing" => EditMode::Prepend,
        "append" | "append_to_existing" => EditMode::Append,
        _ => EditMode::Replace,
    }
}

fn extract_attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

/// Apply a single directive to the workspace. Returns a short diff summary.
pub fn apply_directive(workspace: &Path, directive: &EditDirective) -> Result<String> {
    let path = resolve(workspace, &directive.path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create parent of {}", directive.path))?;
    }

    match &directive.mode {
        EditMode::Replace => {
            let old_lines = fs::read_to_string(&path).unwrap_or_default().lines().count();
            let new_lines = directive.content.lines().count();
            fs::write(&path, &directive.content)
                .with_context(|| format!("cannot write {}", directive.path))?;
            Ok(format!(
                "{}: replaced ({old_lines} → {new_lines} lines)",
                directive.path
            ))
        }

        EditMode::ReplaceFunction(name) => {
            let original = fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", directive.path))?;
            let updated = replace_function_body(&original, name, &directive.content)
                .ok_or_else(|| anyhow!("function '{}' not found in {}", name, directive.path))?;
            fs::write(&path, &updated)
                .with_context(|| format!("cannot write {}", directive.path))?;
            Ok(format!("{}: replaced function '{name}'", directive.path))
        }

        EditMode::Prepend => {
            let existing = fs::read_to_string(&path).unwrap_or_default();
            let updated = format!("{}\n{}", directive.content.trim_end(), existing);
            fs::write(&path, &updated)
                .with_context(|| format!("cannot write {}", directive.path))?;
            Ok(format!("{}: prepended {} lines", directive.path, directive.content.lines().count()))
        }

        EditMode::Append => {
            let existing = fs::read_to_string(&path).unwrap_or_default();
            let updated = format!("{}\n{}", existing.trim_end(), directive.content);
            fs::write(&path, &updated)
                .with_context(|| format!("cannot write {}", directive.path))?;
            Ok(format!("{}: appended {} lines", directive.path, directive.content.lines().count()))
        }
    }
}

/// Build a human-readable preview for the approval prompt.
pub fn preview(workspace: &Path, directive: &EditDirective) -> String {
    let current = workspace
        .join(&directive.path)
        .pipe(|p| fs::read_to_string(p).unwrap_or_default());
    let current_lines = current.lines().count();
    let new_lines = directive.content.lines().count();

    let mode_label = match &directive.mode {
        EditMode::Replace => format!("replace entire file ({current_lines} → {new_lines} lines)"),
        EditMode::ReplaceFunction(name) => format!("replace function '{name}'"),
        EditMode::Prepend => format!("prepend {new_lines} lines"),
        EditMode::Append => format!("append {new_lines} lines"),
    };

    // Show a compact snippet of what will be written
    let snippet: String = directive
        .content
        .lines()
        .take(8)
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    let ellipsis = if new_lines > 8 {
        format!("\n  ... ({} more lines)", new_lines - 8)
    } else {
        String::new()
    };

    format!("{} — {mode_label}\n{snippet}{ellipsis}", directive.path)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve(workspace: &Path, rel_path: &str) -> Result<std::path::PathBuf> {
    // New files don't exist yet — only validate the path isn't escaping.
    let joined = workspace.join(rel_path);
    // Resolve as much as exists without requiring the file itself.
    let canonical_ws = workspace
        .canonicalize()
        .with_context(|| format!("cannot resolve workspace {}", workspace.display()))?;
    // Normalise the path manually to detect traversal without requiring existence.
    let mut parts = Vec::new();
    for component in joined.components() {
        match component {
            std::path::Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(anyhow!("path escapes workspace: {rel_path}"));
                }
            }
            std::path::Component::CurDir => {}
            c => parts.push(c),
        }
    }
    let normalised: std::path::PathBuf = parts.iter().collect();
    if !normalised.starts_with(&canonical_ws) {
        return Err(anyhow!("path escapes workspace: {rel_path}"));
    }
    Ok(normalised)
}

/// Replace a named function (or struct/impl/trait) body in `source`.
/// Uses brace-depth tracking — language-agnostic and good enough for MVP.
fn replace_function_body(source: &str, name: &str, new_body: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();

    // Find the line that declares the target symbol.
    let start_idx = lines.iter().position(|l| {
        let t = l.trim();
        t.contains(name)
            && (t.contains("fn ")
                || t.contains("struct ")
                || t.contains("impl ")
                || t.contains("class ")
                || t.contains("def ")
                || t.contains("func "))
    })?;

    // Walk forward to find the opening brace / colon and matching close.
    let mut depth: i32 = 0;
    let mut end_idx = start_idx;
    let mut found_open = false;

    for (i, line) in lines[start_idx..].iter().enumerate() {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                found_open = true;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        if found_open && depth == 0 {
            end_idx = start_idx + i;
            break;
        }
    }

    if !found_open {
        return None;
    }

    let mut result = lines[..start_idx].join("\n");
    if start_idx > 0 {
        result.push('\n');
    }
    result.push_str(new_body.trim_end());
    result.push('\n');
    if end_idx + 1 < lines.len() {
        result.push('\n');
        result.push_str(&lines[end_idx + 1..].join("\n"));
        result.push('\n');
    }

    Some(result)
}

// Small helper to avoid a `let` binding just for piping.
trait Pipe: Sized {
    fn pipe<F: FnOnce(Self) -> R, R>(self, f: F) -> R { f(self) }
}
impl<T> Pipe for T {}
