use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::{
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
            (Role::Assistant, Some(Channel::Final), _) => Self {
                role: TranscriptRole::Assistant,
                title: "Assistant".to_string(),
                body: message.content.clone(),
            },
            (Role::Assistant, Some(Channel::Commentary), Some(recipient)) => Self {
                role: TranscriptRole::Tool,
                title: format!("Tool call {}", clean_recipient(recipient)),
                body: truncate_tool_body(&message.content, ToolPreviewKind::Call),
            },
            (Role::Tool, _, Some(recipient)) => Self {
                role: TranscriptRole::Tool,
                title: format!("Tool result {}", clean_recipient(recipient)),
                body: truncate_tool_body(&message.content, ToolPreviewKind::Result),
            },
            _ => Self {
                role: TranscriptRole::System,
                title: "Message".to_string(),
                body: message.content.clone(),
            },
        }
    }
}

impl TranscriptRole {
    pub(super) fn label(self) -> &'static str {
        match self {
            TranscriptRole::User => "USER",
            TranscriptRole::Assistant => "OH",
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

#[derive(Debug, Clone, Copy)]
enum ToolPreviewKind {
    Call,
    Result,
}

impl ToolPreviewKind {
    fn label(self) -> &'static str {
        match self {
            Self::Call => "Tool call",
            Self::Result => "Tool output",
        }
    }

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

    let mut output = preview_header(kind, line_count, char_count, head_len, tail_len);
    output.push_str("\n--- first lines ---\n");
    for line in lines.iter().take(head_len) {
        output.push_str(&clip_preview_line(line));
        output.push('\n');
    }

    output.push_str(&format!("--- omitted {omitted} lines ---\n"));

    if tail_len > 0 {
        output.push_str("--- last lines ---\n");
        for line in lines.iter().skip(lines.len() - tail_len) {
            output.push_str(&clip_preview_line(line));
            output.push('\n');
        }
    }

    output
}

fn preview_header(
    kind: ToolPreviewKind,
    line_count: usize,
    char_count: usize,
    head_len: usize,
    tail_len: usize,
) -> String {
    format!(
        "**{} truncated for display**\noriginal: {line_count} lines / {char_count} chars\npreview: first {head_len} lines + last {tail_len} lines\nfull content remains in session context\n",
        kind.label()
    )
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
        "**{} truncated for display**\noriginal: 1 line / {char_count} chars\npreview: first {head_chars} chars + last {tail_chars} chars\nfull content remains in session context\n\n--- preview ---\n{}\n--- omitted {omitted} chars ---\n{}",
        kind.label(),
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
