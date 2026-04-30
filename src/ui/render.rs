use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use std::time::{Duration, Instant};

use super::{
    App, StreamPhase, View,
    commands::slash_command_tips,
    formatting::{
        compact, format_duration, format_elapsed_short, token_ratio, token_status_kind,
        tokens_per_second,
    },
    settings::SETTINGS,
    transcript::{is_tool_error, markdown_bold_spans, sanitize_stream_body, wrap_text},
};
use crate::theme::{
    BRAND_NAME, BRAND_WORDMARK, PhaseGlyph, StatusKind, Theme, thinking_flavor, wave_frame,
};

impl App {
    pub(super) fn render_header(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        if area.is_empty() {
            return;
        }

        let view = match self.view {
            View::Chat => "chat",
            View::Settings => "settings",
            View::Setup => "setup",
        };
        let model = short_model_name(&self.config.model.model);
        if area.height == 1 {
            let line = Paragraph::new(Line::from(vec![
                Span::styled(BRAND_NAME, self.theme.buckle()),
                Span::raw(" "),
                Span::styled(BRAND_WORDMARK, self.theme.brand()),
                Span::styled(format!(" · {model}"), Style::default().fg(self.theme.fg)),
                Span::styled(
                    format!(" · think:{}", self.config.model.thinking_effort),
                    self.theme.dim_style(),
                ),
                Span::styled(format!(" · {view}"), self.theme.dim_style()),
            ]));
            frame.render_widget(line, area);
            return;
        }

        let endpoint = compact(
            &self.config.model.endpoint,
            area.width.saturating_sub(34) as usize,
        );

        let [meta_area, logo_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(30), Constraint::Length(22)])
            .areas(area);

        let meta = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(BRAND_NAME, self.theme.buckle()),
                Span::raw(" "),
                Span::styled(BRAND_WORDMARK, self.theme.brand()),
                Span::styled(format!("  {view}"), self.theme.dim_style()),
            ]),
            Line::from(vec![
                Span::styled("model ", self.theme.dim_style()),
                Span::raw(model),
                Span::styled("  think ", self.theme.dim_style()),
                Span::raw(self.config.model.thinking_effort.as_str()),
                Span::styled("  endpoint ", self.theme.dim_style()),
                Span::raw(endpoint),
            ]),
        ]);
        frame.render_widget(
            meta,
            meta_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );

        let logo = Paragraph::new(self.theme.logo_lines()).alignment(Alignment::Right);
        frame.render_widget(
            logo,
            logo_area.inner(Margin {
                vertical: 0,
                horizontal: 1,
            }),
        );
    }

    pub(super) fn render_chat(&mut self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let show_rail = self.sidebar_visible && area.width >= 88;
        let (messages_area, rail_area) = if show_rail {
            let [messages_area, rail_area] = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(56), Constraint::Length(24)])
                .areas(area);
            (messages_area, Some(rail_area))
        } else {
            (area, None)
        };

        let messages_inner = messages_area.inner(Margin {
            vertical: 0,
            horizontal: 2,
        });
        let lines = self.transcript_lines(messages_inner.width);
        let visible = messages_inner.height as usize;
        let max_scroll = lines.len().saturating_sub(visible) as u16;
        if self.follow_tail {
            self.chat_scroll = max_scroll;
        } else {
            self.chat_scroll = self.chat_scroll.min(max_scroll);
        }

        let transcript = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((self.chat_scroll, 0));
        frame.render_widget(transcript, messages_inner);

        if let Some(rail_area) = rail_area {
            let rail_inner = rail_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            let rail = Paragraph::new(self.context_rail_lines(rail_inner.height))
                .wrap(Wrap { trim: false });
            frame.render_widget(rail, rail_inner);
        }
    }

    pub(super) fn render_settings(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let show_help = area.width >= 96;
        let (settings_area, help_area) = if show_help {
            let [settings_area, help_area] = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
                .areas(area);
            (settings_area, Some(help_area))
        } else {
            (area, None)
        };
        let settings_inner = settings_area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });

        let mut rows = Vec::new();
        let locked = self.is_busy();
        let label_prefix_width: u16 = 18;
        let value_budget = settings_inner
            .width
            .saturating_sub(label_prefix_width)
            .max(8) as usize;
        for (index, field) in SETTINGS.iter().enumerate() {
            let selected = index == self.selected_setting;
            let editing = selected && self.setting_editor.is_some();
            let value = if editing {
                self.setting_editor
                    .as_deref()
                    .unwrap_or_default()
                    .to_string()
            } else {
                truncate_for_width(&field.value(&self.config), value_budget)
            };
            let marker = if selected { ">" } else { " " };
            let label_style = if selected {
                self.theme.brand()
            } else {
                self.theme.dim_style()
            };
            let value_style = if editing {
                self.theme.status_style(StatusKind::Working)
            } else if locked {
                self.theme.chrome_style()
            } else {
                Style::default().fg(self.theme.fg)
            };

            rows.push(Line::from(vec![
                Span::styled(format!("{marker} {:<14} ", field.label()), label_style),
                Span::styled(value, value_style),
            ]));
        }

        let settings_scroll = selected_scroll_offset(self.selected_setting, settings_inner.height);
        let settings = Paragraph::new(rows).scroll((settings_scroll, 0));
        frame.render_widget(settings, settings_inner);

        if let Some(help_area) = help_area {
            let path = self
                .config_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "no config path available".to_string());
            let lock_line = if locked {
                "Settings are locked while a turn is running."
            } else {
                "Edit endpoint, model, auth env, and runtime limits here."
            };
            let help_inner = help_area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            });
            let help = Paragraph::new(settings_help_lines(
                &self.theme,
                lock_line,
                path,
                help_inner.height,
            ))
            .wrap(Wrap { trim: false });
            frame.render_widget(help, help_inner);
        }
    }

    pub(super) fn render_input(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        if area.is_empty() {
            return;
        }

        let working = self.is_busy();
        if let Some(pending) = &self.pending_tool_approval {
            let content = vec![
                Line::from(vec![
                    Span::styled("! approval ", self.theme.status_style(StatusKind::Warn)),
                    Span::styled(
                        format!("{} · {}", pending.recipient, pending.summary),
                        Style::default().fg(self.theme.fg),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("y/Enter approve", self.theme.status_style(StatusKind::Ok)),
                    Span::styled(" · ", self.theme.dim_style()),
                    Span::styled("n/Esc reject", self.theme.status_style(StatusKind::Error)),
                ]),
            ];
            frame.render_widget(Paragraph::new(content), area);
            return;
        }

        if area.height == 1 && self.view == View::Chat && !working {
            let input = Paragraph::new(Line::from(vec![
                Span::styled("❯ ", self.theme.brand()),
                Span::styled(self.input.clone(), Style::default().fg(self.theme.fg)),
            ]));
            frame.render_widget(input, area);
            self.render_input_cursor(area, false, frame);
            return;
        }

        let (title, mut content, style) = match self.view {
            View::Chat if self.is_busy() => (
                " Input ",
                vec![Line::raw("model is working; scroll remains available")],
                self.theme.dim_style(),
            ),
            View::Chat => (
                " Input ",
                self.chat_input_lines(area.width),
                Style::default().fg(self.theme.fg),
            ),
            View::Settings if self.setting_editor.is_some() => (
                " Editing Setting ",
                vec![Line::raw(
                    self.setting_editor
                        .as_deref()
                        .unwrap_or_default()
                        .to_string(),
                )],
                Style::default().fg(self.theme.fg),
            ),
            View::Settings => (
                " Settings Mode ",
                vec![Line::raw("select a setting and press Enter")],
                self.theme.dim_style(),
            ),
            View::Setup if self.setup_editor.is_some() => (
                " Editing Setup ",
                vec![Line::raw(
                    self.setup_editor.as_deref().unwrap_or_default().to_string(),
                )],
                Style::default().fg(self.theme.fg),
            ),
            View::Setup => (
                " Setup Mode ",
                vec![Line::raw("choose first-run defaults and save")],
                self.theme.dim_style(),
            ),
        };

        if working {
            content.insert(0, self.phase_indicator_line());
        }

        let has_block =
            area.height > 1 && !working && (area.height >= 4 || !self.input.starts_with('/'));
        let mut input = Paragraph::new(content).style(style);
        if has_block {
            input = input.block(Block::default().title(title).borders(Borders::ALL));
        }
        frame.render_widget(input, area);

        if !working
            && (self.view == View::Chat
                || self.setting_editor.is_some()
                || self.setup_editor.is_some())
        {
            self.render_input_cursor(area, has_block, frame);
        }
    }

    pub(super) fn render_footer(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        if area.height < 1 {
            return;
        }

        let ratio = token_ratio(self.estimated_tokens, self.config.model.context_window);
        let token_kind = token_status_kind(ratio);
        let mut status_spans = vec![];

        if self.pending_tool_approval.is_some() {
            status_spans.push(Span::styled(
                "awaiting approval",
                self.theme.status_style(StatusKind::Warn),
            ));
        } else if self.is_busy() {
            status_spans.push(Span::styled(
                self.status.as_str(),
                self.theme.status_style(self.status_kind),
            ));
            if let Some(first) = self.turn_first_token_at {
                if let Some(tps) = tokens_per_second(self.turn_token_chars, first.elapsed()) {
                    status_spans.push(Span::raw("   "));
                    status_spans.push(Span::styled(
                        format!("{tps:.0} tok/s"),
                        self.theme.status_style(StatusKind::Working),
                    ));
                }
            }
        } else {
            let last_reply = self
                .last_reply_at
                .map(|when| format_elapsed_short(when.elapsed()))
                .unwrap_or_else(|| "-".to_string());
            status_spans.push(Span::styled("▸ ready", self.theme.brand()));
            status_spans.push(Span::styled(
                format!(" · {last_reply} ago"),
                self.theme.dim_style(),
            ));
        }

        status_spans.push(Span::styled(" · ctx ", self.theme.dim_style()));
        status_spans.extend(context_meter_spans(
            &self.theme,
            token_kind,
            ratio,
            self.estimated_tokens,
            self.config.model.context_window,
        ));
        status_spans.push(Span::raw("  "));
        status_spans.push(Span::styled("Keys ", Style::default().fg(self.theme.muted)));
        let keys = match self.view {
            View::Setup => "Enter edit/apply  Space toggle  Esc skip  Ctrl-C quit",
            View::Chat | View::Settings => {
                "F2 settings  F3 sidebar  F4 header  PgUp/PgDn scroll  Ctrl-C quit"
            }
        };
        status_spans.push(Span::raw(keys));

        let status = Paragraph::new(Line::from(status_spans)).alignment(Alignment::Left);
        frame.render_widget(status, area);
    }

    fn phase_indicator_line(&self) -> Line<'static> {
        let elapsed = self
            .busy_since
            .map(|started| started.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        let waiting_long = elapsed >= Duration::from_secs(10);
        let no_tokens_yet = self.turn_token_chars == 0;

        let (glyph, label, glyph_style) =
            if no_tokens_yet || matches!(self.stream_phase, StreamPhase::WarmingUp) {
                let text = if waiting_long {
                    "still thinking… (this is normal for local models)".to_string()
                } else if self.turn_first_token_at.is_none() {
                    "warming up".to_string()
                } else {
                    "thinking".to_string()
                };
                (
                    PhaseGlyph::Thinking,
                    text,
                    self.theme.phase_style(PhaseGlyph::Thinking),
                )
            } else {
                match &self.stream_phase {
                    StreamPhase::Thinking => (
                        PhaseGlyph::Thinking,
                        "thinking".to_string(),
                        self.theme.phase_style(PhaseGlyph::Thinking),
                    ),
                    StreamPhase::CallingTool(name) => {
                        let action = match name.as_str() {
                            "read_file" => "reading file",
                            "write_file" => "writing file",
                            "list_files" => "listing files",
                            "search" => "searching",
                            "todo_write" => "writing todos",
                            "todo_read" => "reading todos",
                            _ => "calling tool",
                        };
                        (
                            PhaseGlyph::Tool,
                            format!("{action} · {name}"),
                            self.theme.phase_style(PhaseGlyph::Tool),
                        )
                    }
                    StreamPhase::Responding => (
                        PhaseGlyph::Responding,
                        "responding".to_string(),
                        self.theme.phase_style(PhaseGlyph::Responding),
                    ),
                    StreamPhase::Idle | StreamPhase::WarmingUp => (
                        PhaseGlyph::Thinking,
                        "thinking".to_string(),
                        self.theme.phase_style(PhaseGlyph::Thinking),
                    ),
                }
            };

        let mut spans = vec![
            Span::styled(format!("{} ", glyph.as_str()), glyph_style),
            Span::styled(
                wave_frame(self.spinner_tick),
                self.theme.status_style(StatusKind::Working),
            ),
            Span::raw(" "),
            Span::styled(label, self.theme.status_style(StatusKind::Working)),
            Span::styled(
                format!("  {}", format_elapsed_short(elapsed)),
                self.theme.dim_style(),
            ),
        ];

        if let Some(first) = self.turn_first_token_at {
            if let Some(tps) = tokens_per_second(self.turn_token_chars, first.elapsed()) {
                spans.push(Span::styled(
                    format!("  {tps:.0} tok/s"),
                    self.theme.dim_style(),
                ));
            }
        } else {
            spans.push(Span::styled(
                format!("  {}", thinking_flavor(self.spinner_tick)),
                self.theme.dim_style(),
            ));
        }

        Line::from(spans)
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        use super::transcript::TranscriptRole;

        let mut lines = Vec::new();
        for (index, item) in self.transcript.iter().enumerate() {
            let role = item.role.label();
            let style = item.role.style(&self.theme);
            let is_streaming = self.stream_item_index == Some(index);
            let is_diff = item.role == TranscriptRole::System && item.title == "Workspace Diff";
            let body_owned;
            let body_ref: &str = if is_streaming {
                body_owned = sanitize_stream_body(&item.body);
                &body_owned
            } else {
                &item.body
            };

            if item.role == TranscriptRole::Tool {
                lines.extend(tool_panel_lines(&self.theme, &item.title, body_ref, width));
                lines.push(Line::raw(""));
                continue;
            }

            lines.push(Line::from(vec![
                Span::styled(role, style),
                Span::styled(format!(" {}", item.title), self.theme.dim_style()),
            ]));

            if is_streaming {
                lines.push(stream_header_line(
                    &self.theme,
                    &self.stream_phase,
                    self.turn_token_chars,
                    self.turn_first_token_at,
                ));
            }

            if is_diff {
                for line in body_ref.lines() {
                    if line.is_empty() {
                        lines.push(Line::raw(""));
                    } else {
                        let style = diff_line_style(line, &self.theme);
                        for wrapped in wrap_text(line, width) {
                            lines.push(Line::from(Span::styled(wrapped, style)));
                        }
                    }
                }
            } else {
                lines.extend(markdown_body_lines(&self.theme, body_ref, width));
            }
            lines.push(Line::raw(""));
        }

        if lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines
    }

    fn context_rail_lines(&self, visible_height: u16) -> Vec<Line<'static>> {
        let elapsed = self
            .busy_since
            .map(|started| started.elapsed())
            .or(self.last_turn_elapsed)
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string());
        let (state, state_style) = if self.pending_tool_approval.is_some() {
            (
                "awaiting approval",
                self.theme.status_style(StatusKind::Warn),
            )
        } else if self.is_busy() {
            ("running", self.theme.status_style(StatusKind::Working))
        } else {
            ("ready", Style::default().fg(self.theme.fg))
        };
        let edit_mode = if self.config.harness.require_edit_approval {
            "approval"
        } else {
            "unlocked"
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(BRAND_NAME, self.theme.buckle()),
                Span::raw(" "),
                Span::styled("Session", self.theme.brand()),
            ]),
            Line::from(vec![
                Span::styled("state: ", self.theme.dim_style()),
                Span::styled(state, state_style),
                Span::styled(format!("  elapsed: {elapsed}"), self.theme.dim_style()),
            ]),
            Line::raw(format!("effort: {}", self.config.model.thinking_effort)),
            Line::raw(format!(
                "messages: {}  todos: {}",
                self.history_len, self.todo_status_line
            )),
            Line::raw(format!(
                "ctx: {} / {}",
                self.estimated_tokens, self.config.model.context_window
            )),
            Line::raw(format!("tools: {}", self.config.harness.max_tool_turns)),
            Line::raw(format!("edits: {edit_mode}")),
        ];

        if visible_height as usize >= lines.len() + 5 {
            lines.extend([
                Line::raw(""),
                Line::styled("Commands", self.theme.brand()),
                Line::raw("/git      /diff"),
                Line::raw("/stage    /unstage"),
                Line::raw("/commit   /checkpoint"),
                Line::raw("/clear     /quit"),
            ]);
        }

        lines
    }

    fn chat_input_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = vec![Line::raw(self.input.clone())];
        if self.input.starts_with('/') {
            lines.push(Line::styled(
                slash_command_tips(self.input.trim(), width),
                self.theme.dim_style(),
            ));
            return lines;
        }

        for (index, suggestion) in self.path_suggestions.iter().enumerate() {
            let marker = if index == 0 { ">" } else { " " };
            let style = if index == 0 {
                self.theme.brand()
            } else {
                self.theme.dim_style()
            };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(" ", self.theme.dim_style()),
                Span::styled(compact(suggestion, width.saturating_sub(3) as usize), style),
            ]));
        }
        lines
    }

    fn render_input_cursor(&self, area: Rect, has_block: bool, frame: &mut ratatui::Frame<'_>) {
        if area.is_empty() {
            return;
        }

        let (x_offset, y_offset, width) = if has_block {
            (1, 1, area.width.saturating_sub(2))
        } else if self.view == View::Chat {
            (2, 0, area.width.saturating_sub(2))
        } else {
            (0, 0, area.width)
        };
        if width == 0 || area.height <= y_offset {
            return;
        }

        let cursor_chars = match self.view {
            View::Chat => self.input.chars().count(),
            View::Settings => self
                .setting_editor
                .as_deref()
                .unwrap_or_default()
                .chars()
                .count(),
            View::Setup => self
                .setup_editor
                .as_deref()
                .unwrap_or_default()
                .chars()
                .count(),
        };
        let cursor_x = cursor_chars.min(width.saturating_sub(1) as usize) as u16;
        frame.set_cursor_position(Position {
            x: area.x.saturating_add(x_offset).saturating_add(cursor_x),
            y: area.y.saturating_add(y_offset),
        });
    }
}

fn diff_line_style(line: &str, theme: &Theme) -> Style {
    if line.starts_with("+++") || line.starts_with("---") {
        Style::default()
            .fg(theme.accent_soft)
            .add_modifier(Modifier::BOLD)
    } else if line.starts_with("@@") {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else if line.starts_with("diff ") || line.starts_with("index ") {
        Style::default().fg(theme.dim)
    } else if line.starts_with('+') {
        Style::default().fg(theme.success)
    } else if line.starts_with('-') {
        Style::default().fg(theme.error)
    } else {
        Style::default().fg(theme.fg)
    }
}

fn short_model_name(model: &str) -> String {
    model
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(model)
        .to_string()
}

fn context_meter_spans(
    theme: &Theme,
    token_kind: StatusKind,
    ratio: f64,
    tokens: usize,
    context_window: u32,
) -> Vec<Span<'static>> {
    const WIDTH: usize = 10;
    let filled = ((ratio * WIDTH as f64).round() as usize).min(WIDTH);
    let empty = WIDTH.saturating_sub(filled);

    vec![
        Span::styled("[", theme.chrome_style()),
        Span::styled("█".repeat(filled), theme.status_style(token_kind)),
        Span::styled("░".repeat(empty), theme.chrome_style()),
        Span::styled("] ", theme.chrome_style()),
        Span::styled(format!("{tokens}"), theme.status_style(token_kind)),
        Span::styled(format!("/{context_window}"), theme.dim_style()),
        Span::styled(format!(" ({:.0}%)", ratio * 100.0), theme.dim_style()),
    ]
}

fn markdown_body_lines(theme: &Theme, body: &str, width: u16) -> Vec<Line<'static>> {
    let mut rendered = Vec::new();
    let mut fence: Option<MarkdownFence> = None;

    for raw in body.lines() {
        if let Some(lang) = fence_marker(raw) {
            if let Some(open) = fence.take() {
                rendered.extend(fenced_block_lines(theme, &open.lang, &open.lines, width));
            } else {
                fence = Some(MarkdownFence {
                    lang,
                    lines: Vec::new(),
                });
            }
            continue;
        }

        if let Some(open) = fence.as_mut() {
            open.lines.push(raw.to_string());
            continue;
        }

        if raw.is_empty() {
            rendered.push(Line::raw(""));
        } else {
            for wrapped in wrap_text(raw, width) {
                rendered.push(Line::from(markdown_bold_spans(
                    &wrapped,
                    Style::default().fg(theme.fg),
                )));
            }
        }
    }

    if let Some(open) = fence {
        rendered.extend(fenced_block_lines(theme, &open.lang, &open.lines, width));
    }

    rendered
}

#[derive(Debug)]
struct MarkdownFence {
    lang: String,
    lines: Vec<String>,
}

fn fence_marker(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("```")?;
    Some(
        rest.trim()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string(),
    )
}

fn fenced_block_lines(
    theme: &Theme,
    lang: &str,
    content: &[String],
    width: u16,
) -> Vec<Line<'static>> {
    let kind = FenceKind::from_lang(lang);
    let panel_width = (width as usize).max(22);
    let inner_width = panel_width.max(12);
    let title = match kind {
        FenceKind::Terminal => {
            let label = if lang.trim().is_empty() {
                "bash"
            } else {
                lang.trim()
            };
            format!("$ {label}")
        }
        FenceKind::Code => {
            let label = if lang.trim().is_empty() {
                "code"
            } else {
                lang.trim()
            };
            format!("<> {label}")
        }
    };
    let title = compact(&title, inner_width.saturating_sub(2));
    let title_len = title.chars().count();
    let title_style = match kind {
        FenceKind::Terminal => theme.status_style(StatusKind::Working),
        FenceKind::Code => theme.brand(),
    };
    let border = theme.chrome_style();
    let top_fill = "─".repeat(panel_width.saturating_sub(title_len + 3));
    let mut lines = vec![Line::from(vec![
        Span::styled("─ ", border),
        Span::styled(title, title_style),
        Span::styled(format!(" {top_fill}"), border),
    ])];

    match kind {
        FenceKind::Terminal => lines.extend(terminal_block_body(theme, content, inner_width)),
        FenceKind::Code => lines.extend(code_block_body(theme, lang, content, inner_width)),
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FenceKind {
    Code,
    Terminal,
}

impl FenceKind {
    fn from_lang(lang: &str) -> Self {
        match lang.trim().to_ascii_lowercase().as_str() {
            "bash" | "sh" | "shell" | "zsh" | "fish" | "console" | "terminal" => Self::Terminal,
            _ => Self::Code,
        }
    }
}

fn terminal_block_body(
    theme: &Theme,
    content: &[String],
    inner_width: usize,
) -> Vec<Line<'static>> {
    if content.is_empty() {
        return vec![Line::raw("")];
    }

    let command_width = inner_width.saturating_sub(2).max(1);
    let mut lines = Vec::new();
    for raw in content {
        if raw.is_empty() {
            lines.push(Line::raw(""));
            continue;
        }

        for chunk in hard_wrap_preserving(raw, command_width) {
            let mut spans = vec![Span::styled("$ ", theme.brand())];
            spans.extend(shell_spans(theme, &chunk));
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn code_block_body(
    theme: &Theme,
    lang: &str,
    content: &[String],
    inner_width: usize,
) -> Vec<Line<'static>> {
    if content.is_empty() {
        return vec![Line::raw("")];
    }

    let gutter_width = content.len().to_string().len().max(2);
    let code_width = inner_width.saturating_sub(gutter_width + 3).max(1);
    let mut lines = Vec::new();
    for (index, raw) in content.iter().enumerate() {
        let chunks = hard_wrap_preserving(raw, code_width);
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            let gutter = if chunk_index == 0 {
                format!("{:>width$} │ ", index + 1, width = gutter_width)
            } else {
                format!("{:>width$} │ ", "", width = gutter_width)
            };
            let mut spans = vec![Span::styled(gutter, theme.dim_style())];
            spans.extend(code_spans(theme, lang, chunk));
            lines.push(Line::from(spans));
        }
    }
    lines
}

fn code_spans(theme: &Theme, lang: &str, text: &str) -> Vec<Span<'static>> {
    if lang.trim().eq_ignore_ascii_case("rust") {
        return rust_spans(theme, text);
    }

    vec![Span::styled(
        text.to_string(),
        Style::default().fg(theme.fg),
    )]
}

fn rust_spans(theme: &Theme, text: &str) -> Vec<Span<'static>> {
    if let Some(comment_start) = text.find("//") {
        let mut spans = rust_spans(theme, &text[..comment_start]);
        spans.push(Span::styled(
            text[comment_start..].to_string(),
            theme.dim_style(),
        ));
        return spans;
    }

    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let before = &rest[..start];
        spans.extend(rust_plain_spans(theme, before));
        let after_start = &rest[start + 1..];
        if let Some(end) = after_start.find('"') {
            let literal = &rest[start..start + end + 2];
            spans.push(Span::styled(
                literal.to_string(),
                Style::default().fg(theme.success),
            ));
            rest = &after_start[end + 1..];
        } else {
            spans.push(Span::styled(
                rest[start..].to_string(),
                Style::default().fg(theme.success),
            ));
            return spans;
        }
    }
    spans.extend(rust_plain_spans(theme, rest));
    spans
}

fn rust_plain_spans(theme: &Theme, text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut token = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
        } else {
            if !token.is_empty() {
                spans.push(rust_token_span(theme, &token));
                token.clear();
            }
            spans.push(Span::styled(ch.to_string(), Style::default().fg(theme.fg)));
        }
    }

    if !token.is_empty() {
        spans.push(rust_token_span(theme, &token));
    }

    spans
}

fn rust_token_span(theme: &Theme, token: &str) -> Span<'static> {
    let style = match token {
        "as" | "async" | "await" | "const" | "crate" | "dyn" | "else" | "enum" | "extern"
        | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod"
        | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self" | "static" | "struct"
        | "super" | "trait" | "true" | "type" | "unsafe" | "use" | "where" | "while" => {
            theme.brand()
        }
        "String" | "Vec" | "Option" | "Result" | "Some" | "None" | "Ok" | "Err" => {
            theme.brand_subtle()
        }
        _ => Style::default().fg(theme.fg),
    };
    Span::styled(token.to_string(), style)
}

fn shell_spans(theme: &Theme, text: &str) -> Vec<Span<'static>> {
    let mut parts = text.splitn(2, char::is_whitespace);
    let command = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default();
    let mut spans = Vec::new();

    if !command.is_empty() {
        spans.push(Span::styled(
            command.to_string(),
            theme.status_style(StatusKind::Working),
        ));
    }
    if !rest.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            rest.to_string(),
            Style::default().fg(theme.fg),
        ));
    }

    spans
}

fn stream_header_line(
    theme: &Theme,
    phase: &StreamPhase,
    token_chars: usize,
    first_token_at: Option<Instant>,
) -> Line<'static> {
    let channel = match phase {
        StreamPhase::Responding => "final",
        StreamPhase::CallingTool(_) => "commentary",
        StreamPhase::Idle | StreamPhase::WarmingUp | StreamPhase::Thinking => "analysis",
    };
    let tokens = token_chars / 4;
    let speed = first_token_at
        .and_then(|first| tokens_per_second(token_chars, first.elapsed()))
        .map(|tps| format!(" · {tps:.0} tok/s"))
        .unwrap_or_default();

    Line::from(vec![
        Span::styled("─── ", theme.chrome_style()),
        Span::styled(channel, theme.brand()),
        Span::styled(format!(" · {tokens} tokens{speed} "), theme.dim_style()),
        Span::styled("───", theme.chrome_style()),
    ])
}

fn tool_panel_lines(theme: &Theme, title: &str, body: &str, width: u16) -> Vec<Line<'static>> {
    let panel_width = (width as usize).max(18);
    let inner_width = panel_width.saturating_sub(4).max(10);
    let error = title.starts_with('✗') || is_tool_error(body);
    let title = compact(title, inner_width.saturating_sub(2));
    let title_len = title.chars().count();
    let title_style = if error {
        theme.status_style(StatusKind::Error)
    } else if title.starts_with('✓') {
        theme.status_style(StatusKind::Ok)
    } else {
        theme.brand()
    };
    let border = theme.chrome_style();
    let top_fill = "─".repeat(panel_width.saturating_sub(title_len + 5));
    let mut lines = vec![Line::from(vec![
        Span::styled("╭─ ", border),
        Span::styled(title, title_style),
        Span::styled(format!(" {top_fill}╮"), border),
    ])];

    if body.trim().is_empty() {
        lines.push(tool_panel_body_line(theme, "", inner_width, error));
    } else {
        for raw in body.lines() {
            for chunk in hard_wrap_preserving(raw, inner_width) {
                lines.push(tool_panel_body_line(theme, &chunk, inner_width, error));
            }
        }
    }

    lines.push(Line::from(Span::styled(
        format!("╰{}╯", "─".repeat(panel_width.saturating_sub(2))),
        border,
    )));
    lines
}

fn tool_panel_body_line(
    theme: &Theme,
    text: &str,
    inner_width: usize,
    error: bool,
) -> Line<'static> {
    let text_len = text.chars().count().min(inner_width);
    let mut spans = vec![Span::styled("│ ", theme.chrome_style())];
    spans.extend(tool_body_spans(theme, text, error));
    spans.push(Span::raw(" ".repeat(inner_width.saturating_sub(text_len))));
    spans.push(Span::styled(" │", theme.chrome_style()));
    Line::from(spans)
}

fn tool_body_spans(theme: &Theme, text: &str, error: bool) -> Vec<Span<'static>> {
    if error {
        return vec![Span::styled(
            text.to_string(),
            theme.status_style(StatusKind::Error),
        )];
    }

    let Some((label, value)) = text.split_once(':') else {
        return vec![Span::styled(
            text.to_string(),
            Style::default().fg(theme.fg),
        )];
    };

    if label.trim().is_empty() {
        return vec![Span::styled(
            text.to_string(),
            Style::default().fg(theme.fg),
        )];
    }

    let value_style = match label.trim() {
        "path" => theme.brand_subtle(),
        "query" => theme.status_style(StatusKind::Working),
        "content" => Style::default().fg(theme.fg),
        _ => Style::default().fg(theme.fg),
    };

    vec![
        Span::styled(format!("{label}:"), theme.brand()),
        Span::styled(value.to_string(), value_style),
    ]
}

fn hard_wrap_preserving(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let chars = line.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return vec![String::new()];
    }

    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

fn truncate_for_width(value: &str, budget: usize) -> String {
    let single_line = value.replace('\n', " ");
    let count = single_line.chars().count();
    if count <= budget {
        return single_line;
    }
    if budget <= 1 {
        return single_line.chars().take(budget).collect();
    }
    let take = budget.saturating_sub(1);
    let mut out: String = single_line.chars().take(take).collect();
    out.push('…');
    out
}

fn selected_scroll_offset(selected: usize, visible_height: u16) -> u16 {
    let visible = visible_height.max(1) as usize;
    selected.saturating_add(1).saturating_sub(visible) as u16
}

fn settings_help_lines(
    theme: &Theme,
    lock_line: &str,
    path: String,
    visible_height: u16,
) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::styled("API Access", theme.brand()),
        Line::raw(lock_line.to_string()),
        Line::raw("api key env stores an env var name"),
        Line::raw(""),
        Line::styled("Keys", theme.brand()),
        Line::raw("Up/Down or j/k: select"),
        Line::raw("Enter: edit/cycle"),
        Line::raw("Esc: cancel/back"),
        Line::raw("Space: toggle/cycle"),
        Line::raw("s: save TOML"),
        Line::raw("Tab/F2: chat"),
    ];

    if visible_height as usize >= lines.len() + 3 {
        lines.extend([
            Line::raw(""),
            Line::styled("Config Path", theme.brand()),
            Line::raw(path),
        ]);
    }

    lines
}
