use std::{fs, path::Path};

use anyhow::Result;

use super::{
    index::{FileEntry, ProjectMap, SymbolIndex, extract_symbols, language_for},
    read::{AroundParams, read_around, read_range},
    search::{SearchParams, search},
};

/// ~4192 tokens at 4 chars/token — mirrors the kernel budget doc.
pub const DEFAULT_CODE_BUDGET: usize = 16_000;
const REPO_MAP_BUDGET: usize = 800;
const SYMBOLS_BUDGET_PER_FILE: usize = 600;
const MAX_REPO_MAP_FILES: usize = 20;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ContextHints {
    /// Files to show symbol tables for.
    pub files: Vec<String>,
    /// Explicit line ranges: (file, start, end).
    pub ranges: Vec<(String, usize, usize)>,
    /// Search terms to run and include as code snippets.
    pub search_terms: Vec<String>,
}

pub struct ContextPack {
    pub chars_used: usize,
    pub chars_budget: usize,
    pub truncated: bool,
    pub formatted: String,
}

pub struct ContextPackBuilder {
    workspace: std::path::PathBuf,
    project_map: Option<ProjectMap>,
    symbol_index: Option<SymbolIndex>,
    code_budget: usize,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

impl ContextPackBuilder {
    pub fn new(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            project_map: None,
            symbol_index: None,
            code_budget: DEFAULT_CODE_BUDGET,
        }
    }

    pub fn with_budget(mut self, chars: usize) -> Self {
        self.code_budget = chars;
        self
    }

    /// Load project_map.json and symbol_index.json from .cinto/ if present.
    /// Degrades gracefully when index is missing.
    pub fn with_index(mut self) -> Self {
        let dir = self.workspace.join(".cinto");
        self.project_map = load_json(&dir.join("project_map.json"));
        self.symbol_index = load_json(&dir.join("symbol_index.json"));
        self
    }

    pub fn build(self, task: &str, hints: &ContextHints) -> Result<ContextPack> {
        let mut out = String::new();
        let mut code_used: usize = 0;
        let mut truncated = false;

        // ── TASK (always included, not counted against code budget) ─────────
        section(&mut out, "TASK");
        out.push_str(task.trim());
        out.push_str("\n\n");

        // ── REPO MAP ────────────────────────────────────────────────────────
        if let Some(ref map) = self.project_map {
            let repo_map = build_repo_map(map, hints, REPO_MAP_BUDGET);
            if !repo_map.is_empty() {
                section(
                    &mut out,
                    &format!("REPO MAP — {} files total", map.files.len()),
                );
                out.push_str(&repo_map);
                out.push('\n');
            }
        }

        // ── SYMBOLS (per hinted file) ────────────────────────────────────────
        for file in &hints.files {
            if code_used >= self.code_budget {
                truncated = true;
                break;
            }
            let block = self.symbols_block(file);
            if fits(&block, code_used, self.code_budget, SYMBOLS_BUDGET_PER_FILE) {
                section(&mut out, &format!("SYMBOLS  {file}"));
                out.push_str(&block);
                out.push('\n');
                code_used += block.len();
            } else {
                truncated = true;
            }
        }

        // ── CODE (explicit ranges) ───────────────────────────────────────────
        for (file, start, end) in &hints.ranges {
            if code_used >= self.code_budget {
                truncated = true;
                break;
            }
            match read_range(&self.workspace, file, *start, *end) {
                Ok(snippet) => {
                    let available = self.code_budget.saturating_sub(code_used);
                    let (snippet, cut) = budget_cut(snippet, available);
                    if cut {
                        truncated = true;
                    }
                    section(&mut out, &format!("CODE  {file}  lines {start}–{end}"));
                    out.push_str(&snippet);
                    out.push('\n');
                    code_used += snippet.len();
                }
                Err(e) => {
                    out.push_str(&format!("[could not read {file}: {e}]\n\n"));
                }
            }
        }

        // ── CODE (search results) ────────────────────────────────────────────
        for term in &hints.search_terms {
            if code_used >= self.code_budget {
                truncated = true;
                break;
            }
            let params = SearchParams::new(term);
            match search(&self.workspace, params) {
                Ok(result) => {
                    if result.match_count == 0 {
                        continue;
                    }
                    let available = self.code_budget.saturating_sub(code_used);
                    let (snippet, cut) = budget_cut(result.formatted, available);
                    if cut {
                        truncated = true;
                    }
                    section(&mut out, &format!("SEARCH  {term:?}"));
                    out.push_str(&snippet);
                    out.push('\n');
                    code_used += snippet.len();
                }
                Err(_) => {}
            }
        }

        // ── BUDGET FOOTER ────────────────────────────────────────────────────
        if truncated {
            out.push_str("[context truncated — some sections omitted to respect budget]\n");
        }
        out.push_str(&format!(
            "[budget: {code_used} / {} chars]\n",
            self.code_budget
        ));

        Ok(ContextPack {
            chars_used: code_used,
            chars_budget: self.code_budget,
            truncated,
            formatted: out,
        })
    }

    /// Extract symbols for a file — from symbol_index if available, otherwise live.
    fn symbols_block(&self, rel_path: &str) -> String {
        // Try index first (faster)
        if let Some(ref idx) = self.symbol_index {
            if let Some(syms) = idx.get(rel_path) {
                return format_symbols(syms.iter().map(|s| (s.line, s.kind.as_str(), s.name.as_str())));
            }
        }

        // Fall back to live extraction
        let path = self.workspace.join(rel_path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let Some(lang) = language_for(ext) else {
            return String::new();
        };
        let Ok(content) = fs::read_to_string(&path) else {
            return String::new();
        };
        let syms = extract_symbols(&content, &lang);
        format_symbols(syms.iter().map(|s| (s.line, s.kind.as_str(), s.name.as_str())))
    }
}

// ---------------------------------------------------------------------------
// Repo map
// ---------------------------------------------------------------------------

fn build_repo_map(map: &ProjectMap, hints: &ContextHints, budget: usize) -> String {
    let mut out = String::new();

    // Hinted files first, then others up to MAX_REPO_MAP_FILES
    let mut ordered: Vec<&FileEntry> = Vec::new();

    for hint in &hints.files {
        if let Some(entry) = map.files.iter().find(|f| &f.path == hint) {
            ordered.push(entry);
        }
    }
    for entry in &map.files {
        if ordered.len() >= MAX_REPO_MAP_FILES {
            break;
        }
        if !ordered.iter().any(|e| e.path == entry.path) {
            ordered.push(entry);
        }
    }

    for entry in ordered {
        let lang = entry.language.as_deref().unwrap_or("");
        let line = format!(
            "  {:<50}  {:>5} lines  {}\n",
            entry.path, entry.lines, lang
        );
        if out.len() + line.len() > budget {
            out.push_str("  ...\n");
            break;
        }
        out.push_str(&line);
    }

    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn section(out: &mut String, title: &str) {
    out.push_str(&format!("[{title}]\n"));
}

fn format_symbols<'a>(
    iter: impl Iterator<Item = (usize, &'a str, &'a str)>,
) -> String {
    let mut out = String::new();
    for (line, kind, name) in iter {
        out.push_str(&format!("  {:5}  {:<8}  {}\n", line, kind, name));
    }
    out
}

fn fits(block: &str, used: usize, budget: usize, per_section_cap: usize) -> bool {
    block.len() <= per_section_cap && used + block.len() <= budget
}

/// Trim a string to `max_chars`, appending a truncation note.
fn budget_cut(mut s: String, max_chars: usize) -> (String, bool) {
    if s.len() <= max_chars {
        return (s, false);
    }
    s.truncate(max_chars);
    s.push_str("\n... truncated to fit budget\n");
    (s, true)
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
