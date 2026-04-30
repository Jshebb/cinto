use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};

use std::time::Duration;

use super::{
    App, StreamPhase, View,
    commands::slash_command_tips,
    formatting::{
        compact, format_duration, format_elapsed_short, token_ratio, token_status_kind,
        tokens_per_second,
    },
    settings::SETTINGS,
    transcript::{markdown_bold_spans, sanitize_stream_body, wrap_text},
};
use crate::theme::{
    BRAND_NAME, BRAND_WORDMARK, PhaseGlyph, StatusKind, Theme, thinking_flavor, wave_frame,
};

impl App {
    pub(super) fn render_header(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let view = match self.view {
            View::Chat => "chat",
            View::Settings => "settings",
        };
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
                Span::raw(self.config.model.model.as_str()),
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
        let show_rail = area.width >= 88;
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
            vertical: 1,
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
        for (index, field) in SETTINGS.iter().enumerate() {
            let selected = index == self.selected_setting;
            let editing = selected && self.setting_editor.is_some();
            let value = if editing {
                self.setting_editor
                    .as_deref()
                    .unwrap_or_default()
                    .to_string()
            } else {
                field.value(&self.config)
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
        let settings = Paragraph::new(rows)
            .wrap(Wrap { trim: false })
            .scroll((settings_scroll, 0));
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
        };

        if working {
            content.insert(0, self.phase_indicator_line());
        }

        if area.height == 1 && self.view == View::Chat && !working {
            if self.input.starts_with('/') {
                content = vec![Line::styled(
                    slash_command_tips(self.input.trim(), area.width),
                    self.theme.dim_style(),
                )];
            } else if let Some(suggestion) = self.path_suggestions.first() {
                content = vec![Line::styled(
                    format!("complete: {suggestion}"),
                    self.theme.dim_style(),
                )];
            }
        }

        let has_block = area.height >= 4 || !self.input.starts_with('/');
        let mut input = Paragraph::new(content).style(style);
        if has_block {
            input = input.block(Block::default().title(title).borders(Borders::ALL));
        }
        frame.render_widget(input, area);

        if !working && (self.view == View::Chat || self.setting_editor.is_some()) {
            self.render_input_cursor(area, has_block, frame);
        }
    }

    pub(super) fn render_footer(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        if area.height < 2 {
            return;
        }

        let [token_area, status_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .areas(area);

        let ratio = token_ratio(self.estimated_tokens, self.config.model.context_window);
        let label_text = format!(
            "ctx {} / {}",
            self.estimated_tokens, self.config.model.context_window
        );
        let label_width = (label_text.chars().count() as u16).saturating_add(2);
        let [label_area, gauge_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(label_width), Constraint::Min(8)])
            .areas(token_area);

        let token_kind = token_status_kind(ratio);
        let label = Paragraph::new(Line::from(vec![
            Span::styled("ctx ", self.theme.dim_style()),
            Span::styled(
                format!("{}", self.estimated_tokens),
                self.theme.status_style(token_kind),
            ),
            Span::styled(
                format!(" / {}", self.config.model.context_window),
                self.theme.dim_style(),
            ),
            Span::raw(" "),
        ]));
        frame.render_widget(label, label_area);

        let gauge = Gauge::default()
            .gauge_style(self.theme.status_style(token_kind))
            .ratio(ratio)
            .label(format!("{:.0}%", ratio * 100.0));
        frame.render_widget(gauge, gauge_area);

        let mut status_spans = vec![
            Span::styled("Status ", self.theme.dim_style()),
            Span::styled(
                self.status.as_str(),
                self.theme.status_style(self.status_kind),
            ),
        ];

        if self.is_busy() {
            if let Some(first) = self.turn_first_token_at {
                if let Some(tps) = tokens_per_second(self.turn_token_chars, first.elapsed()) {
                    status_spans.push(Span::raw("   "));
                    status_spans.push(Span::styled(
                        format!("{tps:.0} tok/s"),
                        self.theme.status_style(StatusKind::Working),
                    ));
                }
            }
        }

        status_spans.push(Span::raw("   "));
        status_spans.push(Span::styled("Keys ", Style::default().fg(self.theme.muted)));
        status_spans.push(Span::raw("Tab/F2 settings  PgUp/PgDn scroll  Ctrl-C quit"));

        let status = Paragraph::new(Line::from(status_spans)).alignment(Alignment::Left);
        frame.render_widget(status, status_area);
    }

    fn phase_indicator_line(&self) -> Line<'static> {
        let elapsed = self
            .busy_since
            .map(|started| started.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        let waiting_long = elapsed >= Duration::from_secs(10);
        let no_tokens_yet = self.turn_token_chars == 0;

        let (glyph, label, glyph_style) = if no_tokens_yet
            || matches!(self.stream_phase, StreamPhase::WarmingUp)
        {
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
            let is_tool = item.role == TranscriptRole::Tool;
            let rail = if is_tool { "│ " } else { "" };
            let rail_style = self.theme.dim_style();
            let inner_width = width.saturating_sub(rail.chars().count() as u16);

            lines.push(Line::from(vec![
                Span::styled(role, style),
                Span::styled(format!(" {}", item.title), self.theme.dim_style()),
            ]));

            let is_streaming = self.stream_item_index == Some(index);
            let is_diff = item.role == TranscriptRole::System && item.title == "Workspace Diff";
            let body_owned;
            let body_ref: &str = if is_streaming {
                body_owned = sanitize_stream_body(&item.body);
                &body_owned
            } else {
                &item.body
            };

            for line in body_ref.lines() {
                if line.is_empty() {
                    if is_tool {
                        lines.push(Line::from(vec![Span::styled(rail, rail_style)]));
                    } else {
                        lines.push(Line::raw(""));
                    }
                } else if is_diff {
                    let style = diff_line_style(line, &self.theme);
                    for wrapped in wrap_text(line, inner_width) {
                        lines.push(Line::from(Span::styled(wrapped, style)));
                    }
                } else {
                    for wrapped in wrap_text(line, inner_width) {
                        let mut spans = Vec::new();
                        if is_tool {
                            spans.push(Span::styled(rail, rail_style));
                        }
                        spans.extend(markdown_bold_spans(
                            &wrapped,
                            Style::default().fg(self.theme.fg),
                        ));
                        lines.push(Line::from(spans));
                    }
                }
            }
            lines.push(Line::raw(""));
        }

        if lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines
    }

    fn context_rail_lines(&self, visible_height: u16) -> Vec<Line<'static>> {
        let auth = self
            .config
            .model
            .api_key_env
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("none");
        let elapsed = self
            .busy_since
            .map(|started| started.elapsed())
            .or(self.last_turn_elapsed)
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string());
        let state = if self.is_busy() { "running" } else { "ready" };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(BRAND_NAME, self.theme.buckle()),
                Span::raw(" "),
                Span::styled(BRAND_WORDMARK, self.theme.brand()),
            ]),
            Line::raw(""),
            Line::styled("Session", self.theme.brand()),
            Line::raw(format!("state: {state}  elapsed: {elapsed}")),
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
            Line::raw(""),
            Line::styled("Model", self.theme.brand()),
            Line::raw(compact(&self.config.model.model, 18)),
            Line::raw(format!(
                "out: {}  temp: {:.2}",
                self.config.model.max_tokens, self.config.model.temperature
            )),
            Line::raw(format!(
                "stream: {}  auth: {auth}",
                self.config.model.stream
            )),
        ];

        if visible_height as usize >= lines.len() + 5 {
            lines.extend([
                Line::raw(""),
                Line::styled("Commands", self.theme.brand()),
                Line::raw("/settings  /prompt"),
                Line::raw("/tools     /todos"),
                Line::raw("/diff      /checkpoint"),
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
