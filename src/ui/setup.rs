use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    config::{self, PRESETS},
    model::ModelClient,
};

use super::{
    App, StatusKind, View,
    settings::{next_format, next_reasoning_protocol},
};

/// Index into `config::PRESETS`, or `PRESETS.len()` for "Custom".
pub(super) fn preset_index_from_endpoint(endpoint: &str) -> usize {
    for (index, preset) in PRESETS.iter().enumerate() {
        if let Ok(config) = preset.to_config() {
            if endpoint.contains(&strip_scheme(&config.model.endpoint)) {
                return index;
            }
        }
    }
    PRESETS.len() // Custom
}

fn strip_scheme(url: &str) -> String {
    url.trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string()
}

fn preset_label(index: usize) -> String {
    if index < PRESETS.len() {
        format!("{} — {}", PRESETS[index].label, PRESETS[index].description)
    } else {
        "Custom".to_string()
    }
}

fn next_preset(index: usize) -> usize {
    (index + 1) % (PRESETS.len() + 1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupField {
    Preset,
    Endpoint,
    Model,
    Format,
    ReasoningProtocol,
    Workspace,
    EditApproval,
    Shell,
    ContextCompression,
    SaveStart,
}

const SETUP_FIELDS: [SetupField; 10] = [
    SetupField::Preset,
    SetupField::Endpoint,
    SetupField::Model,
    SetupField::Format,
    SetupField::ReasoningProtocol,
    SetupField::Workspace,
    SetupField::EditApproval,
    SetupField::Shell,
    SetupField::ContextCompression,
    SetupField::SaveStart,
];

impl App {
    pub(super) fn handle_setup_key(&mut self, code: KeyCode) -> Result<()> {
        if self.setup_editor.is_some() {
            return self.handle_setup_editor_key(code);
        }

        match code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::F(2) => {
                self.view = View::Chat;
                self.status = "setup skipped".to_string();
                self.status_kind = StatusKind::Warn;
            }
            KeyCode::Up => {
                self.setup_selected = self
                    .setup_selected
                    .checked_sub(1)
                    .unwrap_or(SETUP_FIELDS.len() - 1);
            }
            KeyCode::Down => {
                self.setup_selected = (self.setup_selected + 1) % SETUP_FIELDS.len();
            }
            KeyCode::PageUp => {
                self.setup_selected = self.setup_selected.saturating_sub(4);
            }
            KeyCode::PageDown => {
                self.setup_selected = (self.setup_selected + 4).min(SETUP_FIELDS.len() - 1);
            }
            KeyCode::Home => {
                self.setup_selected = 0;
            }
            KeyCode::End => {
                self.setup_selected = SETUP_FIELDS.len() - 1;
            }
            KeyCode::Char(' ') | KeyCode::Enter => self.activate_setup_field()?,
            _ => {}
        }

        Ok(())
    }

    fn handle_setup_editor_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc => self.setup_editor = None,
            KeyCode::Backspace => {
                if let Some(editor) = &mut self.setup_editor {
                    editor.pop();
                }
            }
            KeyCode::Enter => self.apply_setup_editor()?,
            KeyCode::Char(ch) => {
                if let Some(editor) = &mut self.setup_editor {
                    editor.push(ch);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn activate_setup_field(&mut self) -> Result<()> {
        match SETUP_FIELDS[self.setup_selected] {
            SetupField::Preset => {
                self.setup_preset = next_preset(self.setup_preset);
                self.apply_setup_preset();
            }
            SetupField::Endpoint | SetupField::Model | SetupField::Workspace => {
                self.setup_editor = Some(self.setup_value(SETUP_FIELDS[self.setup_selected]));
            }
            SetupField::Format => {
                let mut config = self.config.clone();
                config.model.format = next_format(&config.model.format);
                self.apply_config(config);
            }
            SetupField::ReasoningProtocol => {
                let mut config = self.config.clone();
                config.harness.reasoning_protocol =
                    next_reasoning_protocol(&config.harness.reasoning_protocol);
                self.apply_config(config);
            }
            SetupField::EditApproval => {
                let mut config = self.config.clone();
                config.harness.require_edit_approval = !config.harness.require_edit_approval;
                self.apply_config(config);
            }
            SetupField::Shell => {
                let mut config = self.config.clone();
                config.harness.allow_shell = !config.harness.allow_shell;
                self.apply_config(config);
            }
            SetupField::ContextCompression => {
                let mut config = self.config.clone();
                config.harness.auto_context_compression = !config.harness.auto_context_compression;
                self.apply_config(config);
            }
            SetupField::SaveStart => self.finish_setup()?,
        }

        Ok(())
    }

    fn apply_setup_editor(&mut self) -> Result<()> {
        let field = SETUP_FIELDS[self.setup_selected];
        let value = self.setup_editor.take().unwrap_or_default();
        let mut config = self.config.clone();

        match field {
            SetupField::Endpoint => {
                config.model.endpoint = non_empty(value, "endpoint")?;
                self.setup_preset = preset_index_from_endpoint(&config.model.endpoint);
            }
            SetupField::Model => config.model.model = non_empty(value, "model")?,
            SetupField::Workspace => {
                config.harness.workspace = PathBuf::from(non_empty(value, "workspace")?)
            }
            _ => {}
        }

        self.apply_config(config);
        Ok(())
    }

    fn apply_setup_preset(&mut self) {
        if self.setup_preset >= PRESETS.len() {
            // Custom — don't change anything
            return;
        }

        let preset = &PRESETS[self.setup_preset];
        let Ok(preset_config) = preset.to_config() else {
            return;
        };

        let mut config = self.config.clone();
        // Apply model settings from preset, keep workspace and prompts
        config.model.endpoint = preset_config.model.endpoint;
        config.model.model = preset_config.model.model;
        config.model.format = preset_config.model.format;
        config.model.max_tokens = preset_config.model.max_tokens;
        config.model.temperature = preset_config.model.temperature;
        config.model.thinking_effort = preset_config.model.thinking_effort;
        config.model.stream = preset_config.model.stream;
        config.model.stop = preset_config.model.stop;
        config.model.request_timeout_secs = preset_config.model.request_timeout_secs;
        config.model.context_window = preset_config.model.context_window;
        // Apply harness settings from preset, keep workspace and prompts
        config.harness.reasoning_protocol = preset_config.harness.reasoning_protocol;
        config.harness.crp_retry_budget = preset_config.harness.crp_retry_budget;
        config.harness.max_tool_turns = preset_config.harness.max_tool_turns;
        config.harness.auto_context_compression = preset_config.harness.auto_context_compression;
        config.harness.require_edit_approval = preset_config.harness.require_edit_approval;
        config.harness.allow_shell = preset_config.harness.allow_shell;

        self.apply_config(config);
    }

    fn finish_setup(&mut self) -> Result<()> {
        let old_window = self.config.model.context_window;
        let detected_ctx = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = ModelClient::new(self.config.model.clone());
                tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    client.detect_context_window(),
                )
                .await
                .ok()
                .and_then(|result| result.ok())
                .flatten()
            })
        });

        if let Some(detected_window) = detected_ctx
            && detected_window > 0
            && detected_window != old_window
        {
            self.config.model.context_window = detected_window;
            self.append_system(
                "Context Detection",
                format!("Updated context window from {old_window} to {detected_window} tokens."),
            );
        }

        let path = self
            .config
            .save(self.config_path.clone())
            .context("failed to save setup config")?;
        self.config_path = Some(path.clone());
        self.view = View::Chat;
        self.status = "setup saved".to_string();
        self.status_kind = StatusKind::Ok;
        self.append_system(
            "Setup Complete",
            format!(
                "Saved config to {}.\nYou can rerun this screen with /setup.",
                path.display()
            ),
        );
        Ok(())
    }

    fn setup_value(&self, field: SetupField) -> String {
        match field {
            SetupField::Preset => preset_label(self.setup_preset),
            SetupField::Endpoint => self.config.model.endpoint.clone(),
            SetupField::Model => self.config.model.model.clone(),
            SetupField::Format => self.config.model.format.clone(),
            SetupField::ReasoningProtocol => self.config.harness.reasoning_protocol.clone(),
            SetupField::Workspace => self.config.harness.workspace.display().to_string(),
            SetupField::EditApproval => bool_label(self.config.harness.require_edit_approval),
            SetupField::Shell => bool_label(self.config.harness.allow_shell),
            SetupField::ContextCompression => {
                bool_label(self.config.harness.auto_context_compression)
            }
            SetupField::SaveStart => "save config and enter chat".to_string(),
        }
    }

    pub(super) fn setup_input_height(&self) -> u16 {
        if self.setup_editor.is_some() { 3 } else { 2 }
    }

    pub(super) fn render_setup(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(Clear, area);

        let [hero_area, fields_area] = if area.width >= 100 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .areas(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(setup_hero_height(area.height)),
                    Constraint::Min(8),
                ])
                .areas(area)
        };

        self.render_setup_hero(hero_area, frame);
        self.render_setup_fields(fields_area, frame);
    }

    fn render_setup_hero(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        let mut lines = if inner.width >= 52 && inner.height >= 8 {
            self.theme.large_logo_lines()
        } else {
            self.theme.logo_lines()
        };

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("First run setup", self.theme.brand()),
            Span::styled("  choose a preset, then chat", self.theme.dim_style()),
        ]));

        if inner.height >= 13 {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("workspace ", self.theme.dim_style()),
                Span::raw(self.config.harness.workspace.display().to_string()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("server    ", self.theme.dim_style()),
                Span::raw(self.config.model.endpoint.clone()),
            ]));
            lines.push(Line::from(vec![
                Span::styled("model     ", self.theme.dim_style()),
                Span::raw(self.config.model.model.clone()),
            ]));

            if inner.height >= 16 {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled("Presets: ", self.theme.dim_style()),
                    Span::raw(
                        config::PRESETS
                            .iter()
                            .map(|p| p.label)
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    Span::styled(", Custom", self.theme.dim_style()),
                ]));
            }
        }

        let hero = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });
        frame.render_widget(hero, inner);
    }

    fn render_setup_fields(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let inner = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        let mut lines = vec![Line::from(vec![
            Span::styled("Setup", self.theme.brand().add_modifier(Modifier::BOLD)),
            Span::styled("  install-time configuration", self.theme.dim_style()),
        ])];
        lines.push(Line::raw(""));

        for (index, field) in SETUP_FIELDS.iter().enumerate() {
            let selected = index == self.setup_selected;
            let editing = selected && self.setup_editor.is_some();
            let marker = if selected { ">" } else { " " };
            let label_style = if selected {
                self.theme.brand()
            } else {
                self.theme.dim_style()
            };
            let value_style = if editing {
                self.theme.status_style(StatusKind::Working)
            } else if matches!(field, SetupField::SaveStart) {
                self.theme.status_style(StatusKind::Ok)
            } else {
                Style::default().fg(self.theme.fg)
            };
            let value = if editing {
                self.setup_editor.clone().unwrap_or_default()
            } else {
                self.setup_value(*field)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{marker} {:<18} ", field.label()), label_style),
                Span::styled(value, value_style),
            ]));
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Enter", self.theme.status_style(StatusKind::Ok)),
            Span::raw(" edit/apply  "),
            Span::styled("Space", self.theme.status_style(StatusKind::Ok)),
            Span::raw(" toggle  "),
            Span::styled("PgUp/PgDn", self.theme.status_style(StatusKind::Ok)),
            Span::raw(" jump  "),
            Span::styled("Esc", self.theme.status_style(StatusKind::Warn)),
            Span::raw(" skip"),
        ]));

        let scroll = setup_scroll_offset(self.setup_selected, inner.height);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.chrome))
            .border_type(BorderType::Rounded)
            .title(" Initial Setup ");
        let setup = Paragraph::new(lines)
            .block(block)
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false });
        frame.render_widget(setup, inner);

        if let Some(editor) = &self.setup_editor {
            let selected_line = self.setup_selected + 2;
            let visible_y = selected_line.saturating_sub(scroll as usize);
            let content_height = inner.height.saturating_sub(2) as usize;
            if visible_y < content_height {
                let cursor_x = 21usize
                    .saturating_add(editor.chars().count())
                    .min(inner.width.saturating_sub(2) as usize);
                frame.set_cursor_position(Position {
                    x: inner.x.saturating_add(1).saturating_add(cursor_x as u16),
                    y: inner.y.saturating_add(1).saturating_add(visible_y as u16),
                });
            }
        }
    }
}

impl SetupField {
    fn label(self) -> &'static str {
        match self {
            Self::Preset => "server preset",
            Self::Endpoint => "endpoint",
            Self::Model => "model",
            Self::Format => "format",
            Self::ReasoningProtocol => "reasoning",
            Self::Workspace => "workspace",
            Self::EditApproval => "edit approval",
            Self::Shell => "shell tools",
            Self::ContextCompression => "context compress",
            Self::SaveStart => "start",
        }
    }
}

fn bool_label(value: bool) -> String {
    if value {
        "on".to_string()
    } else {
        "off".to_string()
    }
}

fn non_empty(value: String, label: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        anyhow::bail!("{label} cannot be empty");
    }
    Ok(value)
}

fn setup_hero_height(area_height: u16) -> u16 {
    if area_height >= 24 {
        12
    } else if area_height >= 16 {
        8
    } else {
        4
    }
}

fn setup_scroll_offset(selected: usize, visible_height: u16) -> u16 {
    let content_height = visible_height.saturating_sub(2) as usize;
    if content_height == 0 {
        return 0;
    }

    let selected_line = selected + 2;
    let top_margin = 2;
    if selected_line < top_margin {
        0
    } else {
        selected_line
            .saturating_sub(content_height.saturating_sub(2))
            .min(selected_line) as u16
    }
}
