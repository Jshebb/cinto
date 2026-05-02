use ratatui::{
    style::{Modifier, Style},
    text::Span,
};
use serde_json::Value;

use crate::{
    crp,
    session::{Channel, Message, Role},
    theme::{RoleKind, StatusKind, Theme},
};

const MAX_TOOL_CALL_DISPLAY_CHARS: usize = 900;
const MAX_TOOL_CALL_DISPLAY_LINES: usize = 18;
const MAX_TOOL_RESULT_DISPLAY_CHARS: usize = 1_600;
const MAX_TOOL_RESULT_DISPLAY_LINES: usize = 36;
const MAX_TOOL_PREVIEW_LINE_CHARS: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TranscriptRole {
    User,
    Assistant,
    Tool,
    System,
    Error,
}

#[derive(Debug, Clone)]
pub(super) struct TranscriptItem {
    pub(super) role: TranscriptRole,
    pub(super) title: String,
    pub(super) body: String,
}

impl TranscriptItem {
    pub(super) fn user(body: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::User,
            title: "You".to_string(),
            body: body.into(),
        }
    }

    pub(super) fn system(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::System,
            title: title.into(),
            body: body.into(),
        }
    }

    pub(super) fn error(body: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::Error,
            title: "Error".to_string(),
            body: body.into(),
        }
    }

    pub(super) fn assistant_stream() -> Self {
        Self {
            role: TranscriptRole::Assistant,
            title: "Assistant".to_string(),
            body: String::new(),
        }
    }

    pub(super) fn from_message(message: &Message) -> Self {
        match (message.role, message.channel, &message.recipient) {
            (Role::Assistant, Some(Channel::Final), _) => {
                if let Some(item) = Self::from_crp_content(&message.content) {
                    item
                } else {
                    Self {
                        role: TranscriptRole::Assistant,
                        title: "Assistant".to_string(),
                        body: message.content.clone(),
                    }
                }
            }
            (Role::Assistant, Some(Channel::Commentary), Some(recipient)) => {
                let name = clean_recipient(recipient);
                let short = strip_functions_prefix(&name);
                let body = format_tool_call_body(short, &message.content);
                Self {
                    role: TranscriptRole::Tool,
                    title: format!("⚙ {short}"),
                    body,
                }
            }
            (Role::Tool, _, Some(recipient)) => {
                let name = clean_recipient(recipient);
                let short = strip_functions_prefix(&name);
                let (title_suffix, body) = format_tool_result(short, &message.content);
                let glyph = if is_tool_error(&body) { "✗" } else { "✓" };
                Self {
                    role: TranscriptRole::Tool,
                    title: format!("{glyph} {short}{title_suffix}"),
                    body,
                }
            }
            _ => Self {
                role: TranscriptRole::System,
                title: "Message".to_string(),
                body: message.content.clone(),
            },
        }
    }

    fn from_crp_content(content: &str) -> Option<Self> {
        match crp::parse(content) {
            Ok(trace) => Some(Self {
                role: TranscriptRole::Assistant,
                title: format!("CRP Trace · {} slots", trace.slots.len()),
                body: format_crp_trace(&trace),
            }),
            Err(error) if looks_like_crp(content) => Some(Self {
                role: TranscriptRole::Error,
                title: "CRP Parse Error".to_string(),
                body: format!(
                    "{error}\n\nraw preview:\n{}",
                    truncate_tool_body(content.trim(), ToolPreviewKind::Result)
                ),
            }),
            Err(_) => None,
        }
    }
}

impl TranscriptRole {
    pub(super) fn label(self) -> &'static str {
        match self {
            TranscriptRole::User => "USER",
            TranscriptRole::Assistant => "CINTO",
            TranscriptRole::Tool => "TOOL",
            TranscriptRole::System => "SYS",
            TranscriptRole::Error => "ERR",
        }
    }

    pub(super) fn style(self, theme: &Theme) -> Style {
        match self {
            TranscriptRole::User => theme.role_style(RoleKind::User),
            TranscriptRole::Assistant => theme.role_style(RoleKind::Assistant),
            TranscriptRole::Tool => theme.role_style(RoleKind::Tool),
            TranscriptRole::System => theme.role_style(RoleKind::System),
            TranscriptRole::Error => theme.status_style(StatusKind::Error),
        }
    }
}

pub(super) fn sanitize_stream_body(body: &str) -> String {
    const MSG: &str = "<|message|>";
    let Some(idx) = body.rfind(MSG) else {
        return String::new();
    };
    let tail = &body[idx + MSG.len()..];

    let mut out = String::with_capacity(tail.len());
    let mut rest = tail;
    while let Some(start) = rest.find("<|") {
        out.push_str(&rest[..start]);
        match rest[start..].find("|>") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

pub(super) fn wrap_text(text: &str, width: u16) -> Vec<String> {
    let width = width.max(12) as usize;
    if text.chars().count() <= width {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let next_len =
            current.chars().count() + word.chars().count() + usize::from(!current.is_empty());
        if next_len > width && !current.is_empty() {
            lines.push(current);
            current = String::new();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }

    if current.is_empty() {
        lines.push(text.chars().take(width).collect());
    } else {
        lines.push(current);
    }
    lines
}

pub(super) fn markdown_bold_spans(text: &str, base: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;
    let bold = base.add_modifier(Modifier::BOLD);

    while let Some(start) = rest.find("**") {
        let before = &rest[..start];
        if !before.is_empty() {
            spans.push(Span::styled(before.to_string(), base));
        }

        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("**") else {
            spans.push(Span::styled(rest[start..].to_string(), base));
            return spans;
        };

        let bold_text = &after_start[..end];
        if !bold_text.is_empty() {
            spans.push(Span::styled(bold_text.to_string(), bold));
        }
        rest = &after_start[end + 2..];
    }

    if !rest.is_empty() {
        spans.push(Span::styled(rest.to_string(), base));
    }

    if spans.is_empty() {
        spans.push(Span::raw(""));
    }
    spans
}

fn clean_recipient(recipient: &str) -> String {
    recipient
        .split("<|")
        .next()
        .unwrap_or(recipient)
        .split_whitespace()
        .next()
        .unwrap_or(recipient)
        .trim()
        .to_string()
}

fn strip_functions_prefix(name: &str) -> &str {
    name.strip_prefix("functions.").unwrap_or(name)
}

fn looks_like_crp(content: &str) -> bool {
    let trimmed = content.trim_start();
    let Some(after_open) = trimmed.strip_prefix('<') else {
        return false;
    };
    after_open.starts_with('/')
        || after_open
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
}

fn format_crp_trace(trace: &crp::Trace) -> String {
    let mut lines = vec![format!(
        "CRP {} · {} slots",
        crp::VERSION,
        trace.slots.len()
    )];

    for slot in &trace.slots {
        lines.push(String::new());
        lines.push(format_crp_slot_heading(slot));

        let content = truncate_tool_body(&slot.content, ToolPreviewKind::Result);
        if content.trim().is_empty() {
            lines.push("(empty)".to_string());
        } else {
            lines.extend(content.lines().map(ToString::to_string));
        }
    }

    lines.join("\n")
}

fn format_crp_slot_heading(slot: &crp::Slot) -> String {
    let mut details = vec![crp_slot_summary(slot)];
    if !slot.attributes.is_empty() {
        details.push(format_crp_attributes(&slot.attributes));
    }

    format!("**{}** · {}", slot.name, details.join(" · "))
}

fn crp_slot_summary(slot: &crp::Slot) -> String {
    match slot.name.as_str() {
        "TASK_INTERPRETATION" => "task understanding".to_string(),
        "ASSUMPTIONS" => pluralize(count_list_items(&slot.content), "assumption"),
        "RELEVANT_FILES" => pluralize(count_list_items(&slot.content), "file"),
        "PROPOSED_APPROACH" => pluralize(count_list_items(&slot.content), "step"),
        "RISKS" => pluralize(count_list_items(&slot.content), "risk"),
        "DELIVERABLE_SPEC" => "success criteria".to_string(),
        "FILE_EDITS" => pluralize(count_edit_blocks(&slot.content), "edit block"),
        "COMMAND_PROPOSALS" => pluralize(count_list_items(&slot.content), "command"),
        "CHECKPOINTS" => pluralize(count_list_items(&slot.content), "checkpoint"),
        "CLARIFICATION_REQUEST" => "needs user input".to_string(),
        "FINAL_RESPONSE" => "user-facing response".to_string(),
        "SKILLS_USED" => pluralize(count_list_items(&slot.content), "skill"),
        _ => format!("{} lines", slot.content.lines().count()),
    }
}

fn format_crp_attributes(attributes: &[crp::Attribute]) -> String {
    attributes
        .iter()
        .map(|attribute| format!("{}=\"{}\"", attribute.name, attribute.value))
        .collect::<Vec<_>>()
        .join(" ")
}

fn count_list_items(content: &str) -> usize {
    let count = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ") || trimmed.starts_with("* ")
        })
        .count();

    if count == 0 && !content.trim().is_empty() {
        1
    } else {
        count
    }
}

fn count_edit_blocks(content: &str) -> usize {
    let edit_tags = content.matches("<EDIT").count();
    let terse_blocks = content
        .lines()
        .filter(|line| line.trim_start().starts_with("@@ "))
        .count();
    let count = edit_tags + terse_blocks;

    if count == 0 && !content.trim().is_empty() {
        1
    } else {
        count
    }
}

fn pluralize(count: usize, label: &str) -> String {
    if count == 1 {
        format!("1 {label}")
    } else {
        format!("{count} {label}s")
    }
}

fn format_tool_call_body(tool: &str, raw_args: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(raw_args.trim()).ok();
    match (tool, parsed.as_ref()) {
        ("write_file", Some(v)) => {
            let path = v.get("path").and_then(Value::as_str).unwrap_or("?");
            let mut lines = vec![format_arg("path", &Value::String(path.to_string()))];
            if let Some(content) = v
                .get("content")
                .or_else(|| v.get("contents"))
                .and_then(Value::as_str)
            {
                lines.push(format_arg(
                    "content",
                    &Value::String(preview_multiline(content)),
                ));
            }
            lines.join("\n")
        }
        ("read_file" | "list_files" | "delete_file", Some(v)) => {
            let path = v.get("path").and_then(Value::as_str).unwrap_or(".");
            format_arg("path", &Value::String(path.to_string()))
        }
        ("search", Some(v)) => {
            let query = v.get("query").and_then(Value::as_str).unwrap_or("?");
            let path = v.get("path").and_then(Value::as_str).unwrap_or(".");
            [
                format_arg("query", &Value::String(query.to_string())),
                format_arg("path", &Value::String(path.to_string())),
            ]
            .join("\n")
        }
        ("todo_write", Some(v)) => {
            let count = v
                .get("items")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0);
            let plural = if count == 1 { "item" } else { "items" };
            format!("items:   {count} {plural}")
        }
        ("todo_read", _) => "read".to_string(),
        (_, Some(Value::Object(map))) => format_arg_map(map),
        (_, Some(value)) => format_arg("value", value),
        (_, None) => {
            let trimmed = raw_args.trim();
            truncate_tool_body(trimmed, ToolPreviewKind::Call)
        }
    }
}

fn format_tool_result(_tool: &str, raw: &str) -> (String, String) {
    const SENTINEL: &str = "<CINTO_TOOL_OUTPUT_END>";
    let stripped = raw.replace(SENTINEL, "");
    let body = stripped.trim_end_matches(|c: char| c.is_whitespace());
    let line_count = body.lines().count();
    let char_count = body.chars().count();

    let stats = format!(
        "  {} · {}",
        format_lines(line_count),
        humanize_size(char_count)
    );

    if line_count <= MAX_TOOL_RESULT_DISPLAY_LINES && char_count <= MAX_TOOL_RESULT_DISPLAY_CHARS {
        return (stats, body.to_string());
    }

    let truncated = truncate_tool_body(body, ToolPreviewKind::Result);
    (format!("{stats}  · truncated"), truncated)
}

pub(super) fn is_tool_error(body: &str) -> bool {
    let lower = body.trim_start().to_ascii_lowercase();
    lower.starts_with("tool error:")
        || lower.starts_with("unsupported tool:")
        || lower.starts_with("tool blocked:")
        || lower.contains("\nunsupported tool:")
        || lower.contains("\ntool error:")
        || lower.contains("\ntool blocked:")
}

fn format_arg_map(map: &serde_json::Map<String, Value>) -> String {
    map.iter()
        .map(|(key, value)| format_arg(key, value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_arg(key: &str, value: &Value) -> String {
    const LABEL_WIDTH: usize = 8;
    let rendered = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(items) => {
            if items.is_empty() {
                "[]".to_string()
            } else {
                format!(
                    "{} item{}",
                    items.len(),
                    if items.len() == 1 { "" } else { "s" }
                )
            }
        }
        Value::Object(fields) => {
            if fields.is_empty() {
                "{}".to_string()
            } else {
                format!(
                    "{} field{}",
                    fields.len(),
                    if fields.len() == 1 { "" } else { "s" }
                )
            }
        }
    };
    let rendered = truncate_tool_body(&rendered, ToolPreviewKind::Call);
    let mut lines = rendered.lines();
    let first = lines.next().unwrap_or_default();
    let indent = " ".repeat(LABEL_WIDTH + 2);
    let mut output = format!("{key:<width$}: {first}", width = LABEL_WIDTH);
    for line in lines {
        output.push('\n');
        output.push_str(&indent);
        output.push_str(line);
    }
    output
}

fn preview_multiline(value: &str) -> String {
    truncate_tool_body(value.trim_end(), ToolPreviewKind::Call)
}

fn format_lines(count: usize) -> String {
    if count == 1 {
        "1 line".to_string()
    } else {
        format!("{count} lines")
    }
}

fn humanize_size(chars: usize) -> String {
    if chars < 1024 {
        format!("{chars}B")
    } else if chars < 1024 * 1024 {
        format!("{:.1}KB", chars as f64 / 1024.0)
    } else {
        format!("{:.1}MB", chars as f64 / (1024.0 * 1024.0))
    }
}

#[derive(Debug, Clone, Copy)]
enum ToolPreviewKind {
    Call,
    Result,
}

impl ToolPreviewKind {
    fn max_chars(self) -> usize {
        match self {
            Self::Call => MAX_TOOL_CALL_DISPLAY_CHARS,
            Self::Result => MAX_TOOL_RESULT_DISPLAY_CHARS,
        }
    }

    fn max_lines(self) -> usize {
        match self {
            Self::Call => MAX_TOOL_CALL_DISPLAY_LINES,
            Self::Result => MAX_TOOL_RESULT_DISPLAY_LINES,
        }
    }

    fn head_lines(self) -> usize {
        match self {
            Self::Call => 8,
            Self::Result => 14,
        }
    }

    fn tail_lines(self) -> usize {
        match self {
            Self::Call => 3,
            Self::Result => 5,
        }
    }
}

fn truncate_tool_body(body: &str, kind: ToolPreviewKind) -> String {
    let line_count = body.lines().count();
    let char_count = body.chars().count();
    if line_count <= kind.max_lines() && char_count <= kind.max_chars() {
        return body.to_string();
    }

    if line_count <= 1 {
        return single_line_tool_preview(body, kind, char_count);
    }

    let lines = body.lines().collect::<Vec<_>>();
    let head_len = kind.head_lines().min(lines.len());
    let tail_len = kind.tail_lines().min(lines.len().saturating_sub(head_len));
    let omitted = lines.len().saturating_sub(head_len + tail_len);

    let mut output = String::new();
    for line in lines.iter().take(head_len) {
        output.push_str(&clip_preview_line(line));
        output.push('\n');
    }

    output.push_str(&format!("… {omitted} lines hidden …\n"));

    if tail_len > 0 {
        for line in lines.iter().skip(lines.len() - tail_len) {
            output.push_str(&clip_preview_line(line));
            output.push('\n');
        }
    }

    output.trim_end().to_string()
}

fn single_line_tool_preview(body: &str, kind: ToolPreviewKind, char_count: usize) -> String {
    let head_chars = (kind.max_chars() * 3 / 4).min(char_count);
    let tail_chars = (kind.max_chars() / 4).min(char_count.saturating_sub(head_chars));
    let head = body.chars().take(head_chars).collect::<String>();
    let tail = body
        .chars()
        .skip(char_count.saturating_sub(tail_chars))
        .collect::<String>();
    let omitted = char_count.saturating_sub(head_chars + tail_chars);

    format!(
        "{}\n… {omitted} chars hidden …\n{}",
        clip_preview_line(&head),
        clip_preview_line(&tail)
    )
}

fn clip_preview_line(line: &str) -> String {
    let char_count = line.chars().count();
    if char_count <= MAX_TOOL_PREVIEW_LINE_CHARS {
        return line.to_string();
    }

    let keep = MAX_TOOL_PREVIEW_LINE_CHARS.saturating_sub(24);
    let preview = line.chars().take(keep).collect::<String>();
    format!("{preview} ... [line clipped]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_valid_crp_final_response_as_trace() {
        let item = TranscriptItem::from_message(&Message::assistant_final(
            r#"<TASK_INTERPRETATION>
Parse CRP and show it in the UI.
</TASK_INTERPRETATION>

<FILE_EDITS>
@@ src/main.rs prepend
fn hello() {}
</FILE_EDITS>

<FINAL_RESPONSE>
Done.
</FINAL_RESPONSE>"#,
        ));

        assert_eq!(item.role, TranscriptRole::Assistant);
        assert_eq!(item.title, "CRP Trace · 3 slots");
        assert!(item.body.contains("CRP 1.0-draft · 3 slots"));
        assert!(item.body.contains("**FILE_EDITS** · 1 edit block"));
    }

    #[test]
    fn formats_malformed_crp_final_response_as_parse_error() {
        let item = TranscriptItem::from_message(&Message::assistant_final(
            "<FINAL_RESPONSE>\nMissing close",
        ));

        assert_eq!(item.role, TranscriptRole::Error);
        assert_eq!(item.title, "CRP Parse Error");
        assert!(item.body.contains("missing CRP closing tag"));
        assert!(item.body.contains("raw preview:"));
    }
}
