use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

use crate::{
    session::{Channel, Message, Role},
    theme::{RoleKind, StatusKind, Theme},
};

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
                body: message.content.clone(),
            },
            (Role::Tool, _, Some(recipient)) => Self {
                role: TranscriptRole::Tool,
                title: format!("Tool result {}", clean_recipient(recipient)),
                body: message.content.clone(),
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
