use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{CrosstermBackend, Terminal},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    task::JoinHandle,
};

use crate::{
    config::Config,
    session::{AgentSession, Channel, Message, Role, TurnEvent},
    theme::{BRAND_GLYPH, BRAND_NAME, RoleKind, StatusKind, Theme, spinner_frame},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Chat,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TranscriptRole {
    User,
    Assistant,
    Tool,
    System,
    Error,
}

#[derive(Debug, Clone)]
struct TranscriptItem {
    role: TranscriptRole,
    title: String,
    body: String,
}

#[derive(Debug, Clone, Copy)]
enum SettingField {
    Endpoint,
    Model,
    ApiKeyEnv,
    MaxTokens,
    Temperature,
    ThinkingEffort,
    Stream,
    Timeout,
    ToolTurns,
    Stop,
    Workspace,
    AllowShell,
}

const SETTINGS: [SettingField; 12] = [
    SettingField::Endpoint,
    SettingField::Model,
    SettingField::ApiKeyEnv,
    SettingField::MaxTokens,
    SettingField::Temperature,
    SettingField::ThinkingEffort,
    SettingField::Stream,
    SettingField::Timeout,
    SettingField::ToolTurns,
    SettingField::Stop,
    SettingField::Workspace,
    SettingField::AllowShell,
];

const COMMAND_TIPS: [(&str, &str); 4] = [
    ("/settings", "open API settings"),
    ("/prompt", "show Harmony prompt"),
    ("/clear", "clear chat"),
    ("/quit", "exit"),
];

type TurnTask = JoinHandle<(AgentSession, Result<()>)>;

pub struct App {
    session: Option<AgentSession>,
    config: Config,
    config_path: Option<PathBuf>,
    transcript: Vec<TranscriptItem>,
    input: String,
    status: String,
    status_kind: StatusKind,
    view: View,
    selected_setting: usize,
    setting_editor: Option<String>,
    chat_scroll: u16,
    follow_tail: bool,
    spinner_tick: u64,
    send_task: Option<TurnTask>,
    stream_rx: Option<UnboundedReceiver<TurnEvent>>,
    stream_item_index: Option<usize>,
    busy_since: Option<Instant>,
    estimated_tokens: usize,
    history_len: usize,
    theme: Theme,
}

impl App {
    pub fn new(session: AgentSession, config_path: Option<PathBuf>) -> Self {
        let config = session.config().clone();
        let estimated_tokens = session.estimated_prompt_tokens();
        let history_len = session.history_len();

        Self {
            session: Some(session),
            config,
            config_path,
            transcript: vec![TranscriptItem::system(
                "Ready",
                "OH! OpenHarness is ready. Type a request, /prompt, /settings, /clear, or /quit.",
            )],
            input: String::new(),
            status: "idle".to_string(),
            status_kind: StatusKind::Idle,
            view: View::Chat,
            selected_setting: 0,
            setting_editor: None,
            chat_scroll: 0,
            follow_tail: true,
            spinner_tick: 0,
            send_task: None,
            stream_rx: None,
            stream_item_index: None,
            busy_since: None,
            estimated_tokens,
            history_len,
            theme: Theme::default(),
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
            self.drain_stream_events();
            self.finish_completed_turn().await?;

            terminal.draw(|frame| {
                let input_height = if self.view == View::Chat && self.input.starts_with('/') {
                    4
                } else {
                    3
                };
                let [header_area, body_area, input_area, footer_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(8),
                        Constraint::Length(input_height),
                        Constraint::Length(2),
                    ])
                    .areas(frame.area());

                self.render_header(header_area, frame);
                match self.view {
                    View::Chat => self.render_chat(body_area, frame),
                    View::Settings => self.render_settings(body_area, frame),
                }
                self.render_input(input_area, frame);
                self.render_footer(footer_area, frame);
            })?;

            self.spinner_tick = self.spinner_tick.wrapping_add(1);

            if event::poll(Duration::from_millis(80))? {
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

    fn render_header(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let view = match self.view {
            View::Chat => "chat",
            View::Settings => "settings",
        };
        let busy = self.is_busy();
        let activity = if busy {
            spinner_frame(self.spinner_tick)
        } else {
            " "
        };
        let endpoint = compact(
            &self.config.model.endpoint,
            area.width.saturating_sub(34) as usize,
        );

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!(" {BRAND_NAME} "), self.theme.brand()),
                Span::styled(format!(" {BRAND_GLYPH} "), self.theme.brand_subtle()),
                Span::raw("OpenHarness"),
                Span::styled(format!("  {view}"), self.theme.dim_style()),
                Span::styled(
                    format!("  {activity}"),
                    self.theme.status_style(self.status_kind),
                ),
            ]),
            Line::from(vec![
                Span::styled("model ", self.theme.dim_style()),
                Span::raw(self.config.model.model.as_str()),
                Span::styled("  think ", self.theme.dim_style()),
                Span::raw(self.config.model.thinking_effort.as_str()),
                Span::styled("  endpoint ", self.theme.dim_style()),
                Span::raw(endpoint),
            ]),
        ])
        .block(Block::default().borders(Borders::BOTTOM));

        frame.render_widget(header, area);
    }

    fn render_chat(&mut self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let [messages_area, rail_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(48), Constraint::Length(24)])
            .areas(area);

        let lines = self.transcript_lines(messages_area.width.saturating_sub(2));
        let visible = messages_area.height.saturating_sub(2) as usize;
        let max_scroll = lines.len().saturating_sub(visible) as u16;
        if self.follow_tail {
            self.chat_scroll = max_scroll;
        } else {
            self.chat_scroll = self.chat_scroll.min(max_scroll);
        }

        let transcript = Paragraph::new(lines)
            .block(Block::default().title(" Chat ").borders(Borders::ALL))
            .wrap(Wrap { trim: false })
            .scroll((self.chat_scroll, 0));
        frame.render_widget(transcript, messages_area);

        let rail = Paragraph::new(self.context_rail_lines())
            .block(Block::default().title(" OH! ").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(rail, rail_area);
    }

    fn render_settings(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let [settings_area, help_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .areas(area);

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

        let settings = Paragraph::new(rows)
            .block(Block::default().title(" Settings ").borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        frame.render_widget(settings, settings_area);

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
        let help = Paragraph::new(vec![
            Line::styled("API Access", self.theme.brand()),
            Line::raw(""),
            Line::raw(lock_line),
            Line::raw(""),
            Line::raw("api key env stores an environment variable name, not the secret."),
            Line::raw(""),
            Line::styled("Keys", self.theme.brand()),
            Line::raw("Up/Down or j/k: select"),
            Line::raw("Enter: edit/apply"),
            Line::raw("Esc: cancel/back"),
            Line::raw("Space: toggle boolean"),
            Line::raw("s: save TOML"),
            Line::raw("Tab/F2: chat"),
            Line::raw(""),
            Line::styled("Config Path", self.theme.brand()),
            Line::raw(path),
        ])
        .block(Block::default().title(" Setup ").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
        frame.render_widget(help, help_area);
    }

    fn render_input(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let (title, content, style) = match self.view {
            View::Chat if self.is_busy() => (
                " Input ",
                vec![Line::raw("model is working; scroll remains available")],
                self.theme.dim_style(),
            ),
            View::Chat => (
                " Input ",
                self.chat_input_lines(),
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

        let input = Paragraph::new(content)
            .style(style)
            .block(Block::default().title(title).borders(Borders::ALL));
        frame.render_widget(input, area);
    }

    fn render_footer(&self, area: Rect, frame: &mut ratatui::Frame<'_>) {
        let [token_area, status_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .areas(area);
        let ratio = token_ratio(self.estimated_tokens, self.config.model.context_window);
        let token_label = format!(
            "ctx ~{} / {}",
            self.estimated_tokens, self.config.model.context_window
        );
        let gauge = Gauge::default()
            .gauge_style(self.theme.status_style(token_status_kind(ratio)))
            .ratio(ratio)
            .label(token_label);
        frame.render_widget(gauge, token_area);

        let status = Paragraph::new(Line::from(vec![
            Span::styled("Status ", self.theme.dim_style()),
            Span::styled(
                self.status.as_str(),
                self.theme.status_style(self.status_kind),
            ),
            Span::raw("   "),
            Span::styled("Keys ", Style::default().fg(self.theme.muted)),
            Span::raw("Tab/F2 settings  PgUp/PgDn scroll  Ctrl-C quit"),
        ]))
        .alignment(Alignment::Left);
        frame.render_widget(status, status_area);
    }

    async fn handle_chat_key(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Tab | KeyCode::F(2) => self.view = View::Settings,
            KeyCode::PageUp => self.scroll_up(8),
            KeyCode::PageDown => self.scroll_down(8),
            KeyCode::Home => {
                self.follow_tail = false;
                self.chat_scroll = 0;
            }
            KeyCode::End => {
                self.follow_tail = true;
            }
            KeyCode::Up => self.scroll_up(1),
            KeyCode::Down => self.scroll_down(1),
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Enter if self.is_busy() => {
                self.status = "waiting for current turn".to_string();
                self.status_kind = StatusKind::Working;
            }
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
                if self.is_busy() {
                    self.status = "cannot clear while a turn is running".to_string();
                    self.status_kind = StatusKind::Warn;
                    return Ok(false);
                }
                if let Some(session) = &mut self.session {
                    session.clear();
                }
                self.transcript.clear();
                self.append_system("Cleared", "Conversation cleared.");
                self.refresh_session_stats();
                return Ok(false);
            }
            "/prompt" => {
                if let Some(session) = &self.session {
                    self.transcript.push(TranscriptItem::system(
                        "Harmony Prompt",
                        session.render_prompt(),
                    ));
                    self.follow_tail = true;
                } else {
                    self.append_system("Busy", "Prompt is unavailable while a turn is running.");
                }
                return Ok(false);
            }
            _ => {}
        }

        self.start_async_turn(input)?;
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

    fn start_async_turn(&mut self, input: String) -> Result<()> {
        let mut session = self
            .session
            .take()
            .context("session is unavailable while a turn is already running")?;
        let (event_tx, event_rx) = unbounded_channel();

        self.transcript.push(TranscriptItem::user(input.clone()));
        self.follow_tail = true;
        self.status = "thinking".to_string();
        self.status_kind = StatusKind::Working;
        self.busy_since = Some(Instant::now());
        self.stream_rx = Some(event_rx);
        self.stream_item_index = None;

        self.send_task = Some(tokio::spawn(async move {
            let result = session.send_user_message_streaming(input, event_tx).await;
            (session, result)
        }));

        Ok(())
    }

    async fn finish_completed_turn(&mut self) -> Result<()> {
        let Some(task) = &self.send_task else {
            return Ok(());
        };
        if !task.is_finished() {
            return Ok(());
        }

        let task = self.send_task.take().expect("checked task exists");
        match task.await {
            Ok((session, result)) => {
                self.session = Some(session);
                match result {
                    Ok(()) => {
                        self.status = "idle".to_string();
                        self.status_kind = StatusKind::Ok;
                    }
                    Err(error) => {
                        self.transcript
                            .push(TranscriptItem::error(format!("{error:#}")));
                        self.status = "turn failed".to_string();
                        self.status_kind = StatusKind::Error;
                    }
                }
                self.refresh_session_stats();
            }
            Err(error) => {
                self.transcript.push(TranscriptItem::error(format!(
                    "background task failed: {error}"
                )));
                self.status = "turn task failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }

        self.busy_since = None;
        self.stream_rx = None;
        self.stream_item_index = None;
        self.follow_tail = true;
        Ok(())
    }

    fn drain_stream_events(&mut self) {
        let Some(mut rx) = self.stream_rx.take() else {
            return;
        };

        while let Ok(event) = rx.try_recv() {
            self.apply_turn_event(event);
        }

        self.stream_rx = Some(rx);
    }

    fn apply_turn_event(&mut self, event: TurnEvent) {
        match event {
            TurnEvent::AssistantDelta(delta) => {
                let index = self.ensure_stream_item();
                self.transcript[index].body.push_str(&delta);
                self.status = "streaming".to_string();
                self.status_kind = StatusKind::Working;
                self.follow_tail = true;
            }
            TurnEvent::DiscardAssistantDraft => {
                if let Some(index) = self.stream_item_index.take() {
                    if index < self.transcript.len() {
                        self.transcript.remove(index);
                    }
                }
            }
            TurnEvent::Message(message) => {
                if message.role == Role::Assistant && message.channel == Some(Channel::Final) {
                    if let Some(index) = self.stream_item_index.take() {
                        if index < self.transcript.len() {
                            self.transcript[index] = TranscriptItem::from_message(&message);
                            self.follow_tail = true;
                            return;
                        }
                    }
                }

                self.transcript.push(TranscriptItem::from_message(&message));
                self.follow_tail = true;
            }
        }
    }

    fn ensure_stream_item(&mut self) -> usize {
        if let Some(index) = self.stream_item_index {
            if index < self.transcript.len() {
                return index;
            }
        }

        self.transcript.push(TranscriptItem {
            role: TranscriptRole::Assistant,
            title: "Assistant".to_string(),
            body: String::new(),
        });
        let index = self.transcript.len() - 1;
        self.stream_item_index = Some(index);
        index
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for item in &self.transcript {
            let role = item.role.label();
            let style = item.role.style(&self.theme);
            lines.push(Line::from(vec![
                Span::styled(role, style),
                Span::styled(format!(" {}", item.title), self.theme.dim_style()),
            ]));

            for line in item.body.lines() {
                if line.is_empty() {
                    lines.push(Line::raw(""));
                } else {
                    lines.extend(wrap_text(line, width).into_iter().map(Line::raw));
                }
            }
            lines.push(Line::raw(""));
        }

        if lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines
    }

    fn context_rail_lines(&self) -> Vec<Line<'static>> {
        let auth = self
            .config
            .model
            .api_key_env
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("none");
        let elapsed = self
            .busy_since
            .map(|started| format!("{}s", started.elapsed().as_secs()))
            .unwrap_or_else(|| "-".to_string());
        let state = if self.is_busy() { "running" } else { "ready" };

        vec![
            Line::styled("Session", self.theme.brand()),
            Line::raw(format!("state: {state}")),
            Line::raw(format!("elapsed: {elapsed}")),
            Line::raw(format!("messages: {}", self.history_len)),
            Line::raw(format!(
                "ctx: ~{} / {}",
                self.estimated_tokens, self.config.model.context_window
            )),
            Line::raw(format!("tools: {}", self.config.harness.max_tool_turns)),
            Line::raw(""),
            Line::styled("Model", self.theme.brand()),
            Line::raw(compact(&self.config.model.model, 18)),
            Line::raw(format!("out: {}", self.config.model.max_tokens)),
            Line::raw(format!("temp: {:.2}", self.config.model.temperature)),
            Line::raw(format!("think: {}", self.config.model.thinking_effort)),
            Line::raw(format!("stream: {}", self.config.model.stream)),
            Line::raw(format!("auth: {auth}")),
            Line::raw(""),
            Line::styled("Commands", self.theme.brand()),
            Line::raw("/settings"),
            Line::raw("/prompt"),
            Line::raw("/clear"),
            Line::raw("/quit"),
        ]
    }

    fn chat_input_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![Line::raw(self.input.clone())];
        if !self.input.starts_with('/') {
            return lines;
        }

        let prefix = self.input.trim();
        let matches = COMMAND_TIPS
            .iter()
            .filter(|(command, _)| command.starts_with(prefix))
            .map(|(command, tip)| format!("{command} {tip}"))
            .collect::<Vec<_>>();

        let tips = if matches.is_empty() {
            "no command matches".to_string()
        } else {
            matches.join("   ")
        };
        lines.push(Line::styled(tips, self.theme.dim_style()));
        lines
    }

    fn append_system(&mut self, title: impl Into<String>, body: impl Into<String>) {
        self.transcript.push(TranscriptItem::system(title, body));
        self.status = "idle".to_string();
        self.status_kind = StatusKind::Idle;
        self.follow_tail = true;
    }

    fn refresh_session_stats(&mut self) {
        if let Some(session) = &self.session {
            self.config = session.config().clone();
            self.estimated_tokens = session.estimated_prompt_tokens();
            self.history_len = session.history_len();
        }
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
        if self.is_busy() {
            self.status = "settings locked while model is thinking".to_string();
            self.status_kind = StatusKind::Warn;
            return Ok(());
        }

        let field = SETTINGS[self.selected_setting];
        if matches!(field, SettingField::AllowShell | SettingField::Stream) {
            return self.toggle_setting();
        }

        self.setting_editor = Some(field.value(&self.config));
        Ok(())
    }

    fn toggle_setting(&mut self) -> Result<()> {
        if self.is_busy() {
            self.status = "settings locked while model is thinking".to_string();
            self.status_kind = StatusKind::Warn;
            return Ok(());
        }

        let field = SETTINGS[self.selected_setting];
        let mut config = self.config.clone();
        match field {
            SettingField::AllowShell => {
                config.harness.allow_shell = !config.harness.allow_shell;
            }
            SettingField::Stream => {
                config.model.stream = !config.model.stream;
            }
            _ => return Ok(()),
        }
        self.apply_config(config);
        Ok(())
    }

    fn apply_editor(&mut self) -> Result<()> {
        if self.is_busy() {
            self.setting_editor = None;
            self.status = "settings locked while model is thinking".to_string();
            self.status_kind = StatusKind::Warn;
            return Ok(());
        }

        let field = SETTINGS[self.selected_setting];
        let value = self.setting_editor.take().unwrap_or_default();
        let mut config = self.config.clone();

        field.apply(&mut config, value)?;
        self.apply_config(config);
        Ok(())
    }

    fn apply_config(&mut self, config: Config) {
        self.config = config.clone();
        if let Some(session) = &mut self.session {
            session.update_config(config);
            self.refresh_session_stats();
        }
        self.status = "setting updated".to_string();
        self.status_kind = StatusKind::Ok;
    }

    fn save_config(&mut self) -> Result<()> {
        if self.is_busy() {
            self.status = "wait for current turn before saving".to_string();
            self.status_kind = StatusKind::Warn;
            return Ok(());
        }

        let path = self.config.save(self.config_path.clone())?;
        self.config_path = Some(path.clone());
        self.status = format!("saved {}", path.display());
        self.status_kind = StatusKind::Ok;
        Ok(())
    }

    fn scroll_up(&mut self, amount: u16) {
        self.follow_tail = false;
        self.chat_scroll = self.chat_scroll.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: u16) {
        self.chat_scroll = self.chat_scroll.saturating_add(amount);
    }

    fn is_busy(&self) -> bool {
        self.send_task.is_some()
    }
}

impl TranscriptItem {
    fn user(body: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::User,
            title: "You".to_string(),
            body: body.into(),
        }
    }

    fn system(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::System,
            title: title.into(),
            body: body.into(),
        }
    }

    fn error(body: impl Into<String>) -> Self {
        Self {
            role: TranscriptRole::Error,
            title: "Error".to_string(),
            body: body.into(),
        }
    }

    fn from_message(message: &Message) -> Self {
        match (message.role, message.channel, &message.recipient) {
            (Role::Assistant, Some(Channel::Final), _) => Self {
                role: TranscriptRole::Assistant,
                title: "Assistant".to_string(),
                body: message.content.clone(),
            },
            (Role::Assistant, Some(Channel::Commentary), Some(recipient)) => Self {
                role: TranscriptRole::Tool,
                title: format!("Tool call {recipient}"),
                body: message.content.clone(),
            },
            (Role::Tool, _, Some(recipient)) => Self {
                role: TranscriptRole::Tool,
                title: format!("Tool result {recipient}"),
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
    fn label(self) -> &'static str {
        match self {
            TranscriptRole::User => "USER",
            TranscriptRole::Assistant => "OH",
            TranscriptRole::Tool => "TOOL",
            TranscriptRole::System => "SYS",
            TranscriptRole::Error => "ERR",
        }
    }

    fn style(self, theme: &Theme) -> Style {
        match self {
            TranscriptRole::User => theme.role_style(RoleKind::User),
            TranscriptRole::Assistant => theme.role_style(RoleKind::Assistant),
            TranscriptRole::Tool => theme.role_style(RoleKind::Tool),
            TranscriptRole::System => theme.role_style(RoleKind::System),
            TranscriptRole::Error => theme.status_style(StatusKind::Error),
        }
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
            SettingField::ThinkingEffort => "thinking",
            SettingField::Stream => "stream",
            SettingField::Timeout => "timeout secs",
            SettingField::ToolTurns => "tool turns",
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
            SettingField::ThinkingEffort => config.model.thinking_effort.clone(),
            SettingField::Stream => config.model.stream.to_string(),
            SettingField::Timeout => config.model.request_timeout_secs.to_string(),
            SettingField::ToolTurns => config.harness.max_tool_turns.to_string(),
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
            SettingField::ThinkingEffort => {
                let value = value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "none" | "low" | "medium" | "high") {
                    return Err(anyhow!("thinking must be one of none, low, medium, high"));
                }
                config.model.thinking_effort = value;
            }
            SettingField::Stream => {
                config.model.stream = value
                    .trim()
                    .parse()
                    .context("stream must be true or false")?;
            }
            SettingField::Timeout => {
                config.model.request_timeout_secs = parse_number(&value, "timeout secs")?;
            }
            SettingField::ToolTurns => {
                config.harness.max_tool_turns = parse_number(&value, "tool turns")?;
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
    let char_count = value.chars().count();
    if char_count <= max_len || max_len < 4 {
        return value.to_string();
    }

    let prefix = value.chars().take(max_len - 3).collect::<String>();
    format!("{prefix}...")
}

fn token_ratio(estimated: usize, max_tokens: u32) -> f64 {
    if max_tokens == 0 {
        return 0.0;
    }

    (estimated as f64 / max_tokens as f64).clamp(0.0, 1.0)
}

fn token_status_kind(ratio: f64) -> StatusKind {
    if ratio >= 0.9 {
        StatusKind::Error
    } else if ratio >= 0.72 {
        StatusKind::Warn
    } else {
        StatusKind::Ok
    }
}

fn wrap_text(text: &str, width: u16) -> Vec<String> {
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
