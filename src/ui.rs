use std::{
    io,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::{CrosstermBackend, Terminal};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    task::JoinHandle,
};

use crate::{
    config::Config,
    session::{AgentSession, Channel, Role, TurnEvent},
    theme::{StatusKind, Theme},
};

mod commands;
mod formatting;
mod layout;
mod render;
mod settings;
mod transcript;

use self::{
    layout::app_areas,
    settings::{SETTINGS, SettingField, next_thinking_effort},
    transcript::TranscriptItem,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Chat,
    Settings,
}

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
    last_turn_elapsed: Option<Duration>,
    estimated_tokens: usize,
    history_len: usize,
    todo_details: String,
    todo_status_line: String,
    theme: Theme,
}

impl App {
    pub fn new(session: AgentSession, config_path: Option<PathBuf>) -> Self {
        let config = session.config().clone();
        let estimated_tokens = session.estimated_prompt_tokens();
        let history_len = session.history_len();
        let todo_details = session.todo_details();
        let todo_status_line = session.todo_status_line();

        Self {
            session: Some(session),
            config,
            config_path,
            transcript: vec![TranscriptItem::system(
                "Ready",
                "OH! OpenHarness is ready. Type a request, /tools, /todos, /prompt, /settings, /clear, or /quit.",
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
            last_turn_elapsed: None,
            estimated_tokens,
            history_len,
            todo_details,
            todo_status_line,
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
                let areas = app_areas(frame.area(), input_height);

                self.render_header(areas.header, frame);
                match self.view {
                    View::Chat => self.render_chat(areas.body, frame),
                    View::Settings => self.render_settings(areas.body, frame),
                }
                self.render_input(areas.input, frame);
                self.render_footer(areas.footer, frame);
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
            "/tools" => {
                if let Some(session) = &self.session {
                    self.transcript.push(TranscriptItem::system(
                        "Agent Tools",
                        session.tool_details(),
                    ));
                } else {
                    self.append_system(
                        "Busy",
                        "Tool details are unavailable while a turn is running.",
                    );
                }
                self.follow_tail = true;
                return Ok(false);
            }
            "/todos" => {
                self.transcript.push(TranscriptItem::system(
                    "Task Todos",
                    self.todo_details.clone(),
                ));
                self.follow_tail = true;
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
        self.last_turn_elapsed = None;
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

        self.last_turn_elapsed = self.busy_since.map(|started| started.elapsed());
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

        self.transcript.push(TranscriptItem::assistant_stream());
        let index = self.transcript.len() - 1;
        self.stream_item_index = Some(index);
        index
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
            self.todo_details = session.todo_details();
            self.todo_status_line = session.todo_status_line();
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
        if matches!(
            field,
            SettingField::AllowShell | SettingField::Stream | SettingField::ThinkingEffort
        ) {
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
            SettingField::ThinkingEffort => {
                config.model.thinking_effort = next_thinking_effort(&config.model.thinking_effort);
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
