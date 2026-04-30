use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use chrono::Local;

const MAX_DIFF_CHARS: usize = 24_000;
const MAX_SUGGESTION_SCAN: usize = 2_000;

pub fn diff_report(workspace: &Path) -> Result<String> {
    ensure_git_workspace(workspace)?;

    let status = git_output(workspace, &["status", "--short"])?;
    let stat = git_output(workspace, &["diff", "--stat", "HEAD", "--"])?;
    let diff = git_output(workspace, &["diff", "--binary", "HEAD", "--"])?;
    let diff = truncate_chars(&diff, MAX_DIFF_CHARS);

    let status = empty_as(&status, "clean");
    let stat = empty_as(&stat, "no tracked file changes");
    let diff = empty_as(&diff, "no tracked diff");

    Ok(format!(
        "Workspace diff\n\n**Status**\n{status}\n\n**Diff Stat**\n{stat}\n\n**Diff**\n{diff}"
    ))
}

pub fn create_checkpoint(workspace: &Path, label: Option<&str>) -> Result<String> {
    ensure_git_workspace(workspace)?;

    let checkpoint_dir = checkpoint_dir(workspace);
    fs::create_dir_all(&checkpoint_dir)
        .with_context(|| format!("failed to create {}", checkpoint_dir.display()))?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = label
        .map(sanitize_label)
        .filter(|value| !value.is_empty())
        .map(|label| format!("{stamp}-{label}"))
        .unwrap_or(stamp);
    let path = checkpoint_dir.join(format!("{name}.patch"));
    let meta_path = checkpoint_dir.join(format!("{name}.txt"));

    let status = git_output(workspace, &["status", "--short"])?;
    let stat = git_output(workspace, &["diff", "--stat", "HEAD", "--"])?;
    let diff = git_output(workspace, &["diff", "--binary", "HEAD", "--"])?;

    let metadata = format!(
        "# Cinto checkpoint\n# Created: {}\n# Workspace: {}\n# Label: {}\n\n## Status\n{}\n\n## Diff Stat\n{}\n\n## Diff\n{}",
        Local::now().to_rfc3339(),
        workspace.display(),
        label.unwrap_or(""),
        empty_as(&status, "clean"),
        empty_as(&stat, "no tracked file changes"),
        empty_as(&diff, "no tracked diff"),
    );

    fs::write(&path, diff).with_context(|| format!("failed to write {}", path.display()))?;
    fs::write(&meta_path, metadata)
        .with_context(|| format!("failed to write {}", meta_path.display()))?;

    Ok(format!(
        "Checkpoint saved\n\nPatch: {}\nMetadata: {}\n\nThis is non-destructive. The patch records tracked git changes; the metadata file records status, diff stat, and untracked file names.",
        path.display(),
        meta_path.display()
    ))
}

pub fn list_checkpoints(workspace: &Path) -> Result<String> {
    let checkpoint_dir = checkpoint_dir(workspace);
    if !checkpoint_dir.exists() {
        return Ok("No checkpoints found.".to_string());
    }

    let mut entries = fs::read_dir(&checkpoint_dir)
        .with_context(|| format!("failed to read {}", checkpoint_dir.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "patch") {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    entries.sort();

    if entries.is_empty() {
        return Ok("No checkpoints found.".to_string());
    }

    let mut output = String::from("Saved checkpoints\n\n");
    for path in entries {
        output.push_str("- ");
        output.push_str(&path.display().to_string());
        output.push('\n');
    }

    Ok(output.trim_end().to_string())
}

pub fn path_suggestions(workspace: &Path, prefix: &str, limit: usize) -> Result<Vec<String>> {
    let prefix = prefix.trim_start_matches("./");
    if prefix.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("failed to resolve workspace {}", workspace.display()))?;
    let mut suggestions = Vec::new();
    let mut scanned = 0;

    collect_path_suggestions(
        &workspace,
        &workspace,
        prefix,
        limit,
        &mut scanned,
        &mut suggestions,
    )?;

    suggestions.sort_by(|left, right| {
        let left_dir = left.ends_with('/');
        let right_dir = right.ends_with('/');
        right_dir.cmp(&left_dir).then_with(|| left.cmp(right))
    });
    suggestions.truncate(limit);
    Ok(suggestions)
}

fn ensure_git_workspace(workspace: &Path) -> Result<()> {
    let output = Command::new("git")
        .current_dir(workspace)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .with_context(|| format!("failed to run git in {}", workspace.display()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!("workspace is not inside a git repository"))
    }
}

fn git_output(workspace: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(workspace)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("git {} failed: {}", args.join(" "), stderr.trim()))
    }
}

fn checkpoint_dir(workspace: &Path) -> PathBuf {
    workspace.join(".cinto").join("checkpoints")
}

fn collect_path_suggestions(
    workspace: &Path,
    dir: &Path,
    prefix: &str,
    limit: usize,
    scanned: &mut usize,
    suggestions: &mut Vec<String>,
) -> Result<()> {
    if *scanned >= MAX_SUGGESTION_SCAN || suggestions.len() >= limit {
        return Ok(());
    }

    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        if *scanned >= MAX_SUGGESTION_SCAN || suggestions.len() >= limit {
            break;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if should_skip_entry(&file_name) {
            continue;
        }

        *scanned += 1;
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(workspace) else {
            continue;
        };
        let Some(relative) = relative.to_str() else {
            continue;
        };
        let display = relative.replace('\\', "/");
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        let suggestion = if is_dir {
            format!("{display}/")
        } else {
            display
        };

        let basename_matches = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix));
        if (suggestion.starts_with(prefix) || basename_matches) && suggestion != prefix {
            suggestions.push(suggestion);
        }

        if is_dir {
            collect_path_suggestions(workspace, &path, prefix, limit, scanned, suggestions)?;
        }
    }

    Ok(())
}

fn should_skip_entry(name: &str) -> bool {
    matches!(name, ".git" | ".cinto" | "target")
}

fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn empty_as(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("\n\n... diff truncated in UI; create a checkpoint for the full patch.");
    truncated
}
