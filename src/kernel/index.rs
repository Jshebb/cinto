use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

const SKIP_DIRS: &[&str] = &[
    "target",
    ".git",
    ".cinto",
    "node_modules",
    ".venv",
    "venv",
    "dist",
    "build",
    "out",
    "__pycache__",
    ".next",
    ".nuxt",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "vendor",
];
const MAX_FILE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMap {
    pub root: String,
    pub generated_at: String,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub lines: usize,
    pub extension: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub line: usize,
    pub kind: String,
    pub name: String,
}

pub type SymbolIndex = HashMap<String, Vec<Symbol>>;

pub struct IndexResult {
    pub project_map: ProjectMap,
    pub symbol_index: SymbolIndex,
    pub file_count: usize,
    pub symbol_count: usize,
}

pub fn index_repo(workspace: &Path) -> Result<IndexResult> {
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("cannot resolve workspace: {}", workspace.display()))?;

    let mut files = Vec::new();
    let mut symbol_index = SymbolIndex::new();

    walk(&workspace, &workspace, &mut files, &mut symbol_index)?;

    let symbol_count = symbol_index.values().map(Vec::len).sum();
    let file_count = files.len();

    let project_map = ProjectMap {
        root: workspace.to_string_lossy().into_owned(),
        generated_at: Utc::now().to_rfc3339(),
        files,
    };

    Ok(IndexResult {
        project_map,
        symbol_index,
        file_count,
        symbol_count,
    })
}

pub fn save(workspace: &Path, result: &IndexResult) -> Result<()> {
    let dir = workspace.join(".cinto");
    fs::create_dir_all(&dir)
        .with_context(|| format!("cannot create {}", dir.display()))?;

    let map_json = serde_json::to_string_pretty(&result.project_map)
        .context("failed to serialize project map")?;
    fs::write(dir.join("project_map.json"), map_json)?;

    let sym_json = serde_json::to_string_pretty(&result.symbol_index)
        .context("failed to serialize symbol index")?;
    fs::write(dir.join("symbol_index.json"), sym_json)?;

    Ok(())
}

fn walk(
    workspace: &Path,
    dir: &Path,
    files: &mut Vec<FileEntry>,
    symbols: &mut SymbolIndex,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if ft.is_dir() {
            if SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            walk(workspace, &path, files, symbols)?;
            continue;
        }

        if !ft.is_file() {
            continue;
        }

        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }

        let rel = path
            .strip_prefix(workspace)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let language = language_for(&ext);

        let (line_count, file_symbols) = match &language {
            Some(lang) => match fs::read_to_string(&path) {
                Ok(content) => (content.lines().count(), extract_symbols(&content, lang)),
                Err(_) => (0, vec![]),
            },
            None => (0, vec![]),
        };

        if !file_symbols.is_empty() {
            symbols.insert(rel.clone(), file_symbols);
        }

        files.push(FileEntry {
            path: rel,
            size_bytes: meta.len(),
            lines: line_count,
            extension: ext,
            language,
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Symbol extraction
// ---------------------------------------------------------------------------

pub fn extract_symbols(content: &str, language: &str) -> Vec<Symbol> {
    match language {
        "rust" => rust_symbols(content),
        "python" => python_symbols(content),
        "javascript" | "typescript" => js_symbols(content),
        "go" => go_symbols(content),
        _ => vec![],
    }
}

fn rust_symbols(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let t = line.trim_start();
        let Some((kind, rest)) = rust_def(t) else {
            continue;
        };
        let name = ident(rest);
        if !name.is_empty() {
            out.push(sym(i + 1, kind, name));
        }
    }
    out
}

fn rust_def(s: &str) -> Option<(&'static str, &str)> {
    const FN: &[&str] = &[
        "pub(crate) async fn ",
        "pub async fn ",
        "async fn ",
        "pub(crate) fn ",
        "pub fn ",
        "fn ",
    ];
    const STRUCT: &[&str] = &["pub(crate) struct ", "pub struct ", "struct "];
    const ENUM: &[&str] = &["pub(crate) enum ", "pub enum ", "enum "];
    const TRAIT: &[&str] = &["pub(crate) trait ", "pub trait ", "trait "];
    const TYPE: &[&str] = &["pub(crate) type ", "pub type ", "type "];
    const MOD: &[&str] = &["pub(crate) mod ", "pub mod ", "mod "];

    strip_any(s, FN)
        .map(|r| ("fn", r))
        .or_else(|| strip_any(s, STRUCT).map(|r| ("struct", r)))
        .or_else(|| strip_any(s, ENUM).map(|r| ("enum", r)))
        .or_else(|| strip_any(s, TRAIT).map(|r| ("trait", r)))
        .or_else(|| strip_any(s, TYPE).map(|r| ("type", r)))
        .or_else(|| strip_any(s, MOD).map(|r| ("mod", r)))
        .or_else(|| s.strip_prefix("impl ").map(|r| ("impl", r)))
}

fn python_symbols(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let t = line.trim_start();
        let (kind, rest) = if let Some(r) = t.strip_prefix("async def ") {
            ("fn", r)
        } else if let Some(r) = t.strip_prefix("def ") {
            ("fn", r)
        } else if let Some(r) = t.strip_prefix("class ") {
            ("class", r)
        } else {
            continue;
        };
        let name = ident(rest);
        if !name.is_empty() {
            out.push(sym(i + 1, kind, name));
        }
    }
    out
}

fn js_symbols(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let t = line.trim_start();
        let Some((kind, rest)) = js_def(t) else {
            continue;
        };
        let name = ident(rest);
        if !name.is_empty() {
            out.push(sym(i + 1, kind, name));
        }
    }
    out
}

fn js_def(s: &str) -> Option<(&'static str, &str)> {
    const FN: &[&str] = &[
        "export default async function ",
        "export async function ",
        "export default function ",
        "export function ",
        "async function ",
        "function ",
    ];
    const CLASS: &[&str] = &["export default class ", "export class ", "class "];
    const IFACE: &[&str] = &["export interface ", "interface "];

    strip_any(s, FN)
        .map(|r| ("fn", r))
        .or_else(|| strip_any(s, CLASS).map(|r| ("class", r)))
        .or_else(|| strip_any(s, IFACE).map(|r| ("interface", r)))
}

fn go_symbols(content: &str) -> Vec<Symbol> {
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("func ") {
            let name = if rest.starts_with('(') {
                // receiver method: `func (recv Type) Name(`
                rest.find(')')
                    .map(|j| ident(rest[j + 1..].trim_start()).to_string())
                    .unwrap_or_default()
            } else {
                ident(rest).to_string()
            };
            if !name.is_empty() {
                out.push(sym(i + 1, "fn", &name));
            }
        } else if let Some(rest) = t.strip_prefix("type ") {
            let name = ident(rest);
            if !name.is_empty() {
                out.push(sym(i + 1, "type", name));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn strip_any<'a>(s: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes.iter().find_map(|p| s.strip_prefix(p))
}

fn ident(s: &str) -> &str {
    let end = s
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(s.len());
    &s[..end]
}

fn sym(line: usize, kind: &str, name: &str) -> Symbol {
    Symbol {
        line,
        kind: kind.to_string(),
        name: name.to_string(),
    }
}

pub fn language_for(ext: &str) -> Option<String> {
    Some(
        match ext {
            "rs" => "rust",
            "py" | "pyw" => "python",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "ts" | "tsx" | "mts" | "cts" => "typescript",
            "go" => "go",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" => "cpp",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "rb" => "ruby",
            "sh" | "bash" => "shell",
            "toml" => "toml",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "md" | "mdx" => "markdown",
            _ => return None,
        }
        .to_string(),
    )
}
