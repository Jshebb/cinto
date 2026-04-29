use std::{io, path::PathBuf, time::Duration};

use anyhow::{Context, Result, anyhow};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{CrosstermBackend, Terminal},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    config::Config,
    session::{AgentSession, Channel, Message, Role},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Chat,
    Settings,
}

#[derive(Debug, Clone, Copy)]
enum SettingField {
    Endpoint,
    Model,
    ApiKeyEnv,
    MaxTokens,
    Temperature,
    Timeout,
    Stop,
    Workspace,
    AllowShell,
}

const SETTINGS: [SettingField; 9] = [
    SettingField::Endpoint,
    SettingField::Model,
    SettingField::ApiKeyEnv,
    SettingField::MaxTokens,
    SettingField::Temperature,
    SettingField::Timeout,
    SettingField::Stop,
    SettingField::Workspace,
    SettingField::AllowShell,
];

pub struct App {
    session: AgentSession,
    config_path: Option<PathBuf>,
    transcript: Vec<String>,
    input: String,
    status: String,
    view: View,
    selected_setting: usize,
    setting_editor: Option<String>,
}

impl App {
    pub fn new(session: AgentSession, config_path: Option<PathBuf>) -> Self {
        Self {
            session,
            config_path,
            transcript: vec![
                "OH! OpenHarness ready. Type a request, /prompt, /settings, /clear, or /quit."
                    .to_string(),
            ],
            input: String::new(),
            status: "idle".to_string(),
            view: View::Chat,
            selected_setting: 0,
            setting_editor: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal).await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            terminal.draw(|frame| {
                let [header_area, body_area, input_area, status_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(5),
                        Constraint::Min(6),
                        Constraint::Length(3),
                        Constraint::Length(1),
                    ])
                    .areas(frame.area());

                self.render_header(frame.area().width, header_area, frame);
                match self.view {
                    View::Chat => self.render_chat(body_area, frame),
                    View::Settings => self.render_settings(body_area, frame),
                }
                self.render_input(input_area, frame);
                self.render_status(status_area, frame);
            })?;

            if event::poll(Duration::from_millis(50))? {
                let Event::Key(key) = event::read()? else {
                    continue;
                };

                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(());
                }

                match self.view {
                    View::Chat => {
                        if self.handle_chat_key(key.code).await? {
                            return Ok(());
                        }
                    }
                    View::Settings => self.handle_settings_key(key.code)?,
                }
            }
        }
    }

    fn render_header(&self, width: u16, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let config = self.session.config();
        let active_view = match self.view {
            View::Chat => "Chat",
            View::Settings => "Settings",
        };
        let endpoint = compact(&config.model.endpoint, width.saturating_sub(30) as usize);

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    " OH! ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  OpenHarness"),
                Span::styled(
                    format!("  {active_view}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Model ", Style::default().fg(Color::DarkGray)),
                Span::raw(config.model.model.as_str()),
                Span::styled("  Endpoint ", Style::default().fg(Color::DarkGray)),
                Span::raw(endpoint),
            ]),
            Line::from(vec![
                Span::styled("Tab/F2", Style::default().fg(Color::Yellow)),
                Span::raw(" switch  "),
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(" send/edit  "),
                Span::styled("s", Style::default().fg(Color::Yellow)),
                Span::raw(" save settings  "),
                Span::styled("Ctrl-C", Style::default().fg(Color::Yellow)),
                Span::raw(" quit"),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(header, area);
    }

    fn render_chat(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let [transcript_area, side_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(36), Constraint::Length(30)])
            .areas(area);

        let transcript = Paragraph::new(self.transcript.join("\n\n"))
            .block(Block::default().title("Transcript").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(transcript, transcript_area);

        let config = self.session.config();
        let auth = config
            .model
            .api_key_env
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("none");
        let panel = Paragraph::new(vec![
            Line::styled("Session", Style::default().add_modifier(Modifier::BOLD)),
            Line::raw(""),
            Line::raw(format!("max tokens: {}", config.model.max_tokens)),
            Line::raw(format!("temperature: {:.2}", config.model.temperature)),
            Line::raw(format!("timeout: {}s", config.model.request_timeout_secs)),
            Line::raw(format!("auth env: {auth}")),
            Line::raw(""),
            Line::styled("Commands", Style::default().add_modifier(Modifier::BOLD)),
            Line::raw("/settings"),
            Line::raw("/prompt"),
            Line::raw("/clear"),
            Line::raw("/quit"),
        ])
        .block(Block::default().title("OH! Control").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
        frame.render_widget(panel, side_area);
    }

    fn render_settings(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let [settings_area, help_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .areas(area);

        let config = self.session.config();
        let mut rows = Vec::new();
        for (index, field) in SETTINGS.iter().enumerate() {
            let selected = index == self.selected_setting;
            let editing = selected && self.setting_editor.is_some();
            let label = field.label();
            let value = if editing {
                self.setting_editor
                    .as_deref()
                    .unwrap_or_default()
                    .to_string()
            } else {
                field.value(config)
            };
            let marker = if selected { ">" } else { " " };
            let style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            rows.push(Line::from(vec![
                Span::styled(format!("{marker} {label:<14} "), style),
                Span::styled(
                    value,
                    if editing {
                        Style::default().fg(Color::Yellow)
                    } else {
                        style
                    },
                ),
            ]));
        }

        let settings = Paragraph::new(rows)
            .block(Block::default().title("Settings").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(settings, settings_area);

        let path = self
            .config_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "no config path available".to_string());
        let help = Paragraph::new(vec![
            Line::styled("API Access", Style::default().add_modifier(Modifier::BOLD)),
            Line::raw(""),
            Line::raw("Endpoint should point at an OpenAI-compatible completions route."),
            Line::raw(""),
            Line::raw(
                "Set api key env to the name of an environment variable, not the secret itself.",
            ),
            Line::raw(""),
            Line::styled("Keys", Style::default().add_modifier(Modifier::BOLD)),
            Line::raw("Up/Down: select"),
            Line::raw("Enter: edit/apply"),
            Line::raw("Esc: cancel"),
            Line::raw("Space: toggle boolean"),
            Line::raw("s: save TOML"),
            Line::raw(""),
            Line::styled("Config Path", Style::default().add_modifier(Modifier::BOLD)),
            Line::raw(path),
        ])
        .block(Block::default().title("OH! Setup").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
        frame.render_widget(help, help_area);
    }

    fn render_input(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let title = match self.view {
            View::Chat => "Input",
            View::Settings if self.setting_editor.is_some() => "Editing Setting",
            View::Settings => "Settings Mode",
        };
        let content = match self.view {
            View::Chat => self.input.as_str(),
            View::Settings if self.setting_editor.is_some() => {
                self.setting_editor.as_deref().unwrap_or_default()
            }
            View::Settings => "Select a setting and press Enter.",
        };
        let input =
            Paragraph::new(content).block(Block::default().title(title).borders(Borders::ALL));
        frame.render_widget(input, area);
    }

    fn render_status(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let status = Paragraph::new(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                self.status.as_str(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        frame.render_widget(status, area);
    }

    async fn handle_chat_key(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Tab | KeyCode::F(2) => self.view = View::Settings,
            KeyCode::Char(ch) => self.input.push(ch),
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Enter => {
                let input = self.input.trim().to_string();
                self.input.clear();
                if self.handle_chat_input(input).await? {
                    return Ok(true);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_chat_input(&mut self, input: String) -> Result<bool> {
        if input.is_empty() {
            return Ok(false);
        }

        match input.as_str() {
            "/quit" | "/exit" => return Ok(true),
            "/settings" => {
                self.view = View::Settings;
                return Ok(false);
            }
            "/clear" => {
                self.session.clear();
                self.transcript.clear();
                self.status = "conversation cleared".to_string();
                return Ok(false);
            }
            "/prompt" => {
                self.transcript
                    .push(format!("Harmony prompt:\n{}", self.session.render_prompt()));
                return Ok(false);
            }
            _ => {}
        }

        self.transcript.push(format!("You:\n{input}"));
        self.status = "thinking".to_string();
        match self.session.send_user_message(input).await {
            Ok(messages) => {
                for message in messages {
                    self.transcript.push(format_message(&message));
                }
                self.status = "idle".to_string();
            }
            Err(error) => {
                self.transcript.push(format!("Error:\n{error:#}"));
                self.status = "error".to_string();
            }
        }

        Ok(false)
    }

    fn handle_settings_key(&mut self, code: KeyCode) -> Result<()> {
        if self.setting_editor.is_some() {
            return self.handle_editor_key(code);
        }

        match code {
            KeyCode::Tab | KeyCode::F(2) => self.view = View::Chat,
            KeyCode::Esc => self.view = View::Chat,
            KeyCode::Up => self.select_previous_setting(),
            KeyCode::Down => self.select_next_setting(),
            KeyCode::Char('k') => self.select_previous_setting(),
            KeyCode::Char('j') => self.select_next_setting(),
            KeyCode::Char('s') => self.save_config()?,
            KeyCode::Char(' ') => self.toggle_setting()?,
            KeyCode::Enter => self.begin_edit_or_toggle()?,
            _ => {}
        }

        Ok(())
    }

    fn handle_editor_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc => self.setting_editor = None,
            KeyCode::Backspace => {
                if let Some(editor) = &mut self.setting_editor {
                    editor.pop();
                }
            }
            KeyCode::Enter => self.apply_editor()?,
            KeyCode::Char(ch) => {
                if let Some(editor) = &mut self.setting_editor {
                    editor.push(ch);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn select_previous_setting(&mut self) {
        self.selected_setting = self
            .selected_setting
            .checked_sub(1)
            .unwrap_or(SETTINGS.len() - 1);
    }

    fn select_next_setting(&mut self) {
        self.selected_setting = (self.selected_setting + 1) % SETTINGS.len();
    }

    fn begin_edit_or_toggle(&mut self) -> Result<()> {
        let field = SETTINGS[self.selected_setting];
        if matches!(field, SettingField::AllowShell) {
            return self.toggle_setting();
        }

        self.setting_editor = Some(field.value(self.session.config()));
        Ok(())
    }

    fn toggle_setting(&mut self) -> Result<()> {
        let field = SETTINGS[self.selected_setting];
        if !matches!(field, SettingField::AllowShell) {
            return Ok(());
        }

        let mut config = self.session.config().clone();
        config.harness.allow_shell = !config.harness.allow_shell;
        self.session.update_config(config);
        self.status = "setting updated".to_string();
        Ok(())
    }

    fn apply_editor(&mut self) -> Result<()> {
        let field = SETTINGS[self.selected_setting];
        let value = self.setting_editor.take().unwrap_or_default();
        let mut config = self.session.config().clone();

        field.apply(&mut config, value)?;
        self.session.update_config(config);
        self.status = "setting updated".to_string();
        Ok(())
    }

    fn save_config(&mut self) -> Result<()> {
        let path = self.session.config().save(self.config_path.clone())?;
        self.config_path = Some(path.clone());
        self.status = format!("saved {}", path.display());
        Ok(())
    }
}

impl SettingField {
    fn label(self) -> &'static str {
        match self {
            SettingField::Endpoint => "endpoint",
            SettingField::Model => "model",
            SettingField::ApiKeyEnv => "api key env",
            SettingField::MaxTokens => "max tokens",
            SettingField::Temperature => "temperature",
            SettingField::Timeout => "timeout secs",
            SettingField::Stop => "stop",
            SettingField::Workspace => "workspace",
            SettingField::AllowShell => "allow shell",
        }
    }

    fn value(self, config: &Config) -> String {
        match self {
            SettingField::Endpoint => config.model.endpoint.clone(),
            SettingField::Model => config.model.model.clone(),
            SettingField::ApiKeyEnv => config.model.api_key_env.clone().unwrap_or_default(),
            SettingField::MaxTokens => config.model.max_tokens.to_string(),
            SettingField::Temperature => format!("{:.2}", config.model.temperature),
            SettingField::Timeout => config.model.request_timeout_secs.to_string(),
            SettingField::Stop => config.model.stop.join(","),
            SettingField::Workspace => config.harness.workspace.display().to_string(),
            SettingField::AllowShell => config.harness.allow_shell.to_string(),
        }
    }

    fn apply(self, config: &mut Config, value: String) -> Result<()> {
        match self {
            SettingField::Endpoint => config.model.endpoint = non_empty(value, "endpoint")?,
            SettingField::Model => config.model.model = non_empty(value, "model")?,
            SettingField::ApiKeyEnv => {
                let value = value.trim().to_string();
                config.model.api_key_env = if value.is_empty() { None } else { Some(value) };
            }
            SettingField::MaxTokens => {
                config.model.max_tokens = parse_number(&value, "max tokens")?;
            }
            SettingField::Temperature => {
                config.model.temperature = value
                    .trim()
                    .parse()
                    .context("temperature must be a number")?;
            }
            SettingField::Timeout => {
                config.model.request_timeout_secs = parse_number(&value, "timeout secs")?;
            }
            SettingField::Stop => {
                config.model.stop = value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(ToString::to_string)
                    .collect();
            }
            SettingField::Workspace => {
                config.harness.workspace = PathBuf::from(non_empty(value, "workspace")?);
            }
            SettingField::AllowShell => {
                config.harness.allow_shell = value
                    .trim()
                    .parse()
                    .context("allow shell must be true or false")?;
            }
        }

        Ok(())
    }
}

fn non_empty(value: String, label: &str) -> Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }
    Ok(value)
}

fn parse_number<T>(value: &str, label: &str) -> Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value
        .trim()
        .parse()
        .with_context(|| format!("{label} must be a number"))
}

fn compact(value: &str, max_len: usize) -> String {
    if value.len() <= max_len || max_len < 4 {
        return value.to_string();
    }

    format!("{}...", &value[..max_len - 3])
}

fn format_message(message: &Message) -> String {
    match (message.role, message.channel, &message.recipient) {
        (Role::Assistant, Some(Channel::Final), _) => {
            format!("Assistant:\n{}", message.content)
        }
        (Role::Assistant, Some(Channel::Commentary), Some(recipient)) => {
            format!("Tool call {recipient}:\n{}", message.content)
        }
        (Role::Tool, _, Some(recipient)) => {
            format!("Tool result {recipient}:\n{}", message.content)
        }
        _ => message.content.clone(),
    }
}
