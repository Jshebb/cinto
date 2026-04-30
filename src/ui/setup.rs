use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{App, StatusKind, View, settings::next_format};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupPreset {
    LmStudio,
    Ollama,
    Custom,
}

impl SetupPreset {
    pub(super) fn from_endpoint(endpoint: &str) -> Self {
        if endpoint.contains("127.0.0.1:1234") || endpoint.contains("localhost:1234") {
            Self::LmStudio
        } else if endpoint.contains("127.0.0.1:11434") || endpoint.contains("localhost:11434") {
            Self::Ollama
        } else {
            Self::Custom
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::LmStudio => "LM Studio",
            Self::Ollama => "Ollama",
            Self::Custom => "Custom",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::LmStudio => Self::Ollama,
            Self::Ollama => Self::Custom,
            Self::Custom => Self::LmStudio,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetupField {
    Preset,
    Endpoint,
    Model,
    Format,
    Workspace,
    EditApproval,
    Shell,
    ContextCompression,
    SaveStart,
}

const SETUP_FIELDS: [SetupField; 9] = [
    SetupField::Preset,
    SetupField::Endpoint,
    SetupField::Model,
    SetupField::Format,
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
                self.setup_preset = self.setup_preset.next();
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
                self.setup_preset = SetupPreset::from_endpoint(&config.model.endpoint);
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
        let mut config = self.config.clone();
        match self.setup_preset {
            SetupPreset::LmStudio => {
                config.model.endpoint = "http://127.0.0.1:1234".to_string();
                config.model.model = "openai/gpt-oss-20b".to_string();
                config.model.format = "harmony".to_string();
                config.model.thinking_effort = "medium".to_string();
            }
            SetupPreset::Ollama => {
                config.model.endpoint = "http://127.0.0.1:11434".to_string();
                config.model.model = "qwen2.5-coder:7b-instruct".to_string();
                config.model.format = "openai-tools".to_string();
                config.model.thinking_effort = "none".to_string();
                config.model.stop.clear();
            }
            SetupPreset::Custom => {}
        }
        self.apply_config(config);
    }

    fn finish_setup(&mut self) -> Result<()> {
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
            SetupField::Preset => self.setup_preset.label().to_string(),
            SetupField::Endpoint => self.config.model.endpoint.clone(),
            SetupField::Model => self.config.model.model.clone(),
            SetupField::Format => self.config.model.format.clone(),
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

        let [hero_area, fields_area] = if area.width >= 100 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
                .areas(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(10), Constraint::Min(10)])
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
            Span::styled(" for a local coding agent harness", self.theme.dim_style()),
        ]));
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
            Span::styled("Setup", self.theme.brand()),
            Span::styled("  choose defaults, save, then chat", self.theme.dim_style()),
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
            Span::styled("Esc", self.theme.status_style(StatusKind::Warn)),
            Span::raw(" skip"),
        ]));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.chrome))
            .title(" Initial Setup ");
        let setup = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(setup, inner);
    }
}

impl SetupField {
    fn label(self) -> &'static str {
        match self {
            Self::Preset => "server preset",
            Self::Endpoint => "endpoint",
            Self::Model => "model",
            Self::Format => "format",
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
