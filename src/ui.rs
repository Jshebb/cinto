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
use ratatui::prelude::{CrosstermBackend, Terminal};
use tokio::{
    sync::{
        mpsc::{UnboundedReceiver, unbounded_channel},
        oneshot,
    },
    task::JoinHandle,
};

use crate::{
    config::Config,
    kernel::worker::{WorkerEvent, WorkerLoop},
    model::ModelClient,
    session::{AgentSession, Channel, Role, TurnEvent},
    theme::{StatusKind, Theme},
    workspace,
};

mod commands;
mod formatting;
mod layout;
mod render;
mod settings;
mod setup;
mod transcript;

use self::{
    layout::app_areas,
    settings::{
        SETTINGS, SettingField, next_format, next_reasoning_protocol, next_thinking_effort,
    },
    setup::preset_index_from_endpoint,
    transcript::{TranscriptItem, sanitize_stream_body},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Chat,
    Settings,
    Setup,
}

type TurnTask = JoinHandle<(AgentSession, Result<()>)>;
type KernelTask = JoinHandle<Result<String>>;
const TURN_INTERRUPTED: &str = "turn interrupted";

struct PendingToolApproval {
    recipient: String,
    summary: String,
    response_tx: oneshot::Sender<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StreamPhase {
    Idle,
    WarmingUp,
    Thinking,
    CrpSlot(String),
    CallingTool(String),
    Responding,
    KernelStage(String),
}

pub struct App {
    session: Option<AgentSession>,
    config: Config,
    config_path: Option<PathBuf>,
    transcript: Vec<TranscriptItem>,
    input: String,
    path_suggestions: Vec<String>,
    path_suggestion_range: Option<(usize, usize)>,
    status: String,
    status_kind: StatusKind,
    view: View,
    selected_setting: usize,
    setting_editor: Option<String>,
    setup_selected: usize,
    setup_editor: Option<String>,
    setup_preset: usize,
    sidebar_visible: bool,
    header_expanded: bool,
    chat_scroll: u16,
    follow_tail: bool,
    spinner_tick: u64,
    kernel_mode: bool,
    kernel_task: Option<KernelTask>,
    kernel_rx: Option<UnboundedReceiver<WorkerEvent>>,
    send_task: Option<TurnTask>,
    cancel_tx: Option<oneshot::Sender<()>>,
    stream_rx: Option<UnboundedReceiver<TurnEvent>>,
    stream_item_index: Option<usize>,
    pending_tool_approval: Option<PendingToolApproval>,
    busy_since: Option<Instant>,
    last_turn_elapsed: Option<Duration>,
    last_reply_at: Option<Instant>,
    stream_phase: StreamPhase,
    turn_first_token_at: Option<Instant>,
    turn_token_chars: usize,
    estimated_tokens: usize,
    history_len: usize,
    todo_details: String,
    todo_status_line: String,
    theme: Theme,
}

impl App {
    pub fn new(session: AgentSession, config_path: Option<PathBuf>, first_run: bool) -> Self {
        let config = session.config().clone();
        let estimated_tokens = session.estimated_prompt_tokens();
        let history_len = session.history_len();
        let todo_details = session.todo_details();
        let todo_status_line = session.todo_status_line();
        let setup_preset = preset_index_from_endpoint(&config.model.endpoint);

        Self {
            session: Some(session),
            config,
            config_path,
            transcript: vec![TranscriptItem::system(
                "Ready",
                "Cinto is ready. Type a request, /tools, /todos, /prompt, /settings, /clear, or /quit.",
            )],
            input: String::new(),
            path_suggestions: Vec::new(),
            path_suggestion_range: None,
            status: "idle".to_string(),
            status_kind: StatusKind::Idle,
            view: if first_run { View::Setup } else { View::Chat },
            selected_setting: 0,
            setting_editor: None,
            setup_selected: 0,
            setup_editor: None,
            setup_preset,
            sidebar_visible: true,
            header_expanded: false,
            chat_scroll: 0,
            follow_tail: true,
            spinner_tick: 0,
            kernel_mode: false,
            kernel_task: None,
            kernel_rx: None,
            send_task: None,
            cancel_tx: None,
            stream_rx: None,
            stream_item_index: None,
            pending_tool_approval: None,
            busy_since: None,
            last_turn_elapsed: None,
            last_reply_at: None,
            stream_phase: StreamPhase::Idle,
            turn_first_token_at: None,
            turn_token_chars: 0,
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
                if self.view == View::Setup {
                    self.render_setup(frame.area(), frame);
                    return;
                }

                let header_height = self.header_height();
                let input_height = self.input_height();
                let areas = app_areas(frame.area(), header_height, input_height, 1);

                self.render_header(areas.header, frame);
                match self.view {
                    View::Chat => self.render_chat(areas.body, frame),
                    View::Settings => self.render_settings(areas.body, frame),
                    View::Setup => {}
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
                    if self.is_busy() {
                        self.interrupt_turn();
                        continue;
                    }
                    return Ok(());
                }
                if self.handle_approval_key(key.code) {
                    continue;
                }
                if key.code == KeyCode::F(3) {
                    self.sidebar_visible = !self.sidebar_visible;
                    continue;
                }
                if key.code == KeyCode::F(4) {
                    self.header_expanded = !self.header_expanded;
                    continue;
                }
                if key.code == KeyCode::F(5) {
                    self.toggle_kernel_mode();
                    continue;
                }

                match self.view {
                    View::Chat => {
                        if self.handle_chat_key(key.code).await? {
                            return Ok(());
                        }
                    }
                    View::Settings => self.handle_settings_key(key.code)?,
                    View::Setup => self.handle_setup_key(key.code)?,
                }
            }
        }
    }

    async fn handle_chat_key(&mut self, code: KeyCode) -> Result<bool> {
        match code {
            KeyCode::Tab | KeyCode::F(2) => self.view = View::Settings,
            KeyCode::Right if self.accept_path_suggestion() => {}
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
            KeyCode::Esc if self.is_busy() => self.interrupt_turn(),
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Enter if self.is_busy() => {
                self.status = "waiting for current turn; Esc interrupts".to_string();
                self.status_kind = StatusKind::Working;
            }
            KeyCode::Char(ch) => {
                self.input.push(ch);
                self.refresh_path_suggestions();
            }
            KeyCode::Backspace => {
                self.input.pop();
                self.refresh_path_suggestions();
            }
            KeyCode::Enter => {
                let input = self.input.trim().to_string();
                self.input.clear();
                self.clear_path_suggestions();
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
            "/kernel" => {
                self.toggle_kernel_mode();
                return Ok(false);
            }
            "/settings" => {
                self.view = View::Settings;
                self.clear_path_suggestions();
                return Ok(false);
            }
            "/setup" => {
                self.view = View::Setup;
                self.clear_path_suggestions();
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
                        "Model Prompt",
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
            "/diff" => {
                self.show_workspace_diff();
                return Ok(false);
            }
            "/git" | "/changes" => {
                self.show_git_changes();
                return Ok(false);
            }
            "/checkpoints" => {
                self.show_checkpoints();
                return Ok(false);
            }
            "/context" => {
                self.detect_context_window().await?;
                return Ok(false);
            }
            _ => {}
        }

        if input == "/checkpoint" || input.starts_with("/checkpoint ") {
            let label = input
                .strip_prefix("/checkpoint")
                .map(str::trim)
                .unwrap_or_default();
            self.create_checkpoint(label);
            return Ok(false);
        }

        if input == "/stage" || input.starts_with("/stage ") {
            let spec = input
                .strip_prefix("/stage")
                .map(str::trim)
                .unwrap_or_default();
            self.stage_paths(spec);
            return Ok(false);
        }

        if input == "/unstage" || input.starts_with("/unstage ") {
            let spec = input
                .strip_prefix("/unstage")
                .map(str::trim)
                .unwrap_or_default();
            self.unstage_paths(spec);
            return Ok(false);
        }

        if input == "/commit" || input.starts_with("/commit ") {
            let message = input
                .strip_prefix("/commit")
                .map(str::trim)
                .unwrap_or_default();
            self.commit_staged(message);
            return Ok(false);
        }

        self.start_async_turn(input)?;
        Ok(false)
    }

    fn handle_settings_key(&mut self, code: KeyCode) -> Result<()> {
        if self.setting_editor.is_some() {
            return self.handle_editor_key(code);
        }

        match code {
            KeyCode::Tab | KeyCode::F(2) => {
                self.view = View::Chat;
                self.refresh_path_suggestions();
            }
            KeyCode::Esc => {
                self.view = View::Chat;
                self.refresh_path_suggestions();
            }
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
        if self.kernel_mode {
            return self.start_kernel_turn(input);
        }

        let mut session = self
            .session
            .take()
            .context("session is unavailable while a turn is already running")?;
        let (event_tx, event_rx) = unbounded_channel();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        self.transcript.push(TranscriptItem::user(input.clone()));
        self.follow_tail = true;
        self.status = "thinking".to_string();
        self.status_kind = StatusKind::Working;
        self.busy_since = Some(Instant::now());
        self.last_turn_elapsed = None;
        self.stream_phase = StreamPhase::WarmingUp;
        self.turn_first_token_at = None;
        self.turn_token_chars = 0;
        self.stream_rx = Some(event_rx);
        self.cancel_tx = Some(cancel_tx);
        self.stream_item_index = None;

        self.send_task = Some(tokio::spawn(async move {
            let result = tokio::select! {
                result = session.send_user_message_streaming(input, event_tx) => result,
                _ = cancel_rx => {
                    session.discard_trailing_user_message();
                    Err(anyhow!(TURN_INTERRUPTED))
                }
            };
            (session, result)
        }));

        Ok(())
    }

    fn start_kernel_turn(&mut self, input: String) -> Result<()> {
        if self.kernel_task.is_some() {
            self.status = "kernel turn already running".to_string();
            self.status_kind = StatusKind::Warn;
            return Ok(());
        }

        let (event_tx, event_rx) = unbounded_channel::<WorkerEvent>();
        let workspace = self.config.harness.workspace.clone();
        let config = self.config.clone();

        self.transcript.push(TranscriptItem::user(input.clone()));
        self.follow_tail = true;
        self.status = "kernel: starting".to_string();
        self.status_kind = StatusKind::Working;
        self.busy_since = Some(Instant::now());
        self.last_turn_elapsed = None;
        self.stream_phase = StreamPhase::WarmingUp;
        self.kernel_rx = Some(event_rx);

        self.kernel_task = Some(tokio::spawn(async move {
            WorkerLoop::new(&workspace, config, input, event_tx)
                .run_bugfix()
                .await
        }));

        Ok(())
    }

    fn apply_kernel_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::StageStarted { stage } => {
                self.stream_phase = StreamPhase::KernelStage(stage.clone());
                self.status = format!("kernel: {stage}");
                self.status_kind = StatusKind::Working;
            }
            WorkerEvent::ContextPackReady { stage, chars_used, budget } => {
                self.status = format!("kernel: {stage} [{chars_used}/{budget} chars]");
                self.status_kind = StatusKind::Working;
            }
            WorkerEvent::StageCompleted { stage, crp_valid } => {
                let marker = if crp_valid { "✓" } else { "~ (plain text)" };
                self.status = format!("kernel: {stage} {marker}");
                self.status_kind = StatusKind::Working;
            }
            WorkerEvent::StageRetry { stage, attempt, reason } => {
                self.transcript.push(TranscriptItem::system(
                    format!("Kernel retry — {stage} (attempt {attempt})"),
                    reason,
                ));
            }
            WorkerEvent::StageFailed { stage, error } => {
                self.transcript.push(TranscriptItem::error(
                    format!("Kernel stage {stage} failed: {error}"),
                ));
            }
            WorkerEvent::WorkflowComplete { final_response } => {
                let mut item = TranscriptItem::assistant_stream();
                item.body = final_response;
                self.transcript.push(item);
                self.follow_tail = true;
            }
            WorkerEvent::WorkflowFailed { error } => {
                self.transcript.push(TranscriptItem::error(
                    format!("Kernel workflow failed: {error}"),
                ));
            }
            WorkerEvent::PatchApprovalRequested { path, preview, response_tx } => {
                self.pending_tool_approval = Some(PendingToolApproval {
                    recipient: path.clone(),
                    summary: format!("write {path}"),
                    response_tx,
                });
                self.transcript.push(TranscriptItem::system(
                    "Patch Approval",
                    format!("{preview}\nPress y to apply or n to skip."),
                ));
                self.status = "patch approval needed".to_string();
                self.status_kind = StatusKind::Warn;
                self.follow_tail = true;
            }
            WorkerEvent::PatchApplied { files_changed } => {
                self.transcript.push(TranscriptItem::system(
                    "Patch Applied",
                    files_changed.join("\n"),
                ));
                self.follow_tail = true;
            }
            // Trace events carry raw prompt/response pairs for dataset generation.
            // The TUI has no use for them — only the batch runner consumes them.
            WorkerEvent::StageSkipped { stage, reason } => {
                self.transcript.push(TranscriptItem::system(
                    format!("Kernel skipped — {stage}"),
                    format!("{reason}\nContinuing with search-based context."),
                ));
            }
            WorkerEvent::StageTrace { .. } => {}
            WorkerEvent::StageOutput { .. } => {}
        }
    }

    fn toggle_kernel_mode(&mut self) {
        if self.is_busy() {
            self.status = "cannot switch mode while a turn is running".to_string();
            self.status_kind = StatusKind::Warn;
            return;
        }
        self.kernel_mode = !self.kernel_mode;
        let mode = if self.kernel_mode { "kernel" } else { "core" };
        self.append_system(
            "Mode",
            format!("Switched to {mode} mode. {}", if self.kernel_mode {
                "Tasks run as a staged pipeline. F5 or /kernel to switch back."
            } else {
                "Tasks run as a conversational agent. F5 or /kernel to switch back."
            }),
        );
        self.status = format!("mode: {mode}");
        self.status_kind = StatusKind::Idle;
    }

    async fn finish_completed_turn(&mut self) -> Result<()> {
        // ── Kernel task ──────────────────────────────────────────────────────
        if let Some(task) = &self.kernel_task {
            if task.is_finished() {
                let task = self.kernel_task.take().expect("checked");
                match task.await {
                    Ok(Ok(_)) => {
                        self.status = "kernel: done".to_string();
                        self.status_kind = StatusKind::Ok;
                    }
                    Ok(Err(e)) => {
                        self.transcript.push(TranscriptItem::error(format!("{e:#}")));
                        self.status = "kernel: failed".to_string();
                        self.status_kind = StatusKind::Error;
                    }
                    Err(e) => {
                        self.transcript.push(TranscriptItem::error(format!("kernel task: {e}")));
                        self.status = "kernel: task error".to_string();
                        self.status_kind = StatusKind::Error;
                    }
                }
                self.last_turn_elapsed = self.busy_since.map(|s| s.elapsed());
                self.last_reply_at = Some(Instant::now());
                self.busy_since = None;
                self.stream_phase = StreamPhase::Idle;
                self.kernel_rx = None;
                self.follow_tail = true;
            }
        }

        // ── Core task ────────────────────────────────────────────────────────
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
                        self.status = "ready".to_string();
                        self.status_kind = StatusKind::Ok;
                    }
                    Err(error) if error.to_string() == TURN_INTERRUPTED => {
                        self.transcript.push(TranscriptItem::system(
                            "Interrupted",
                            "Stopped the in-flight model turn.",
                        ));
                        self.status = "interrupted".to_string();
                        self.status_kind = StatusKind::Warn;
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
        self.last_reply_at = Some(Instant::now());
        self.busy_since = None;
        self.cancel_tx = None;
        self.stream_phase = StreamPhase::Idle;
        self.turn_first_token_at = None;
        self.turn_token_chars = 0;
        self.stream_rx = None;
        self.stream_item_index = None;
        self.pending_tool_approval = None;
        self.follow_tail = true;
        Ok(())
    }

    fn drain_stream_events(&mut self) {
        // Core events
        if let Some(mut rx) = self.stream_rx.take() {
            while let Ok(event) = rx.try_recv() {
                self.apply_turn_event(event);
            }
            self.stream_rx = Some(rx);
        }

        // Kernel events
        if let Some(mut rx) = self.kernel_rx.take() {
            while let Ok(event) = rx.try_recv() {
                self.apply_kernel_event(event);
            }
            self.kernel_rx = Some(rx);
        }
    }

    fn apply_turn_event(&mut self, event: TurnEvent) {
        match event {
            TurnEvent::AssistantDelta(delta) => {
                let index = self.ensure_stream_item();
                self.transcript[index].body.push_str(&delta);
                if self.turn_first_token_at.is_none() {
                    self.turn_first_token_at = Some(Instant::now());
                }
                self.turn_token_chars = self.turn_token_chars.saturating_add(delta.chars().count());
                self.stream_phase = derive_stream_phase(&self.transcript[index].body);
                self.status = "streaming".to_string();
                self.status_kind = StatusKind::Working;
                self.follow_tail = true;
            }
            TurnEvent::DiscardAssistantDraft => {
                if let Some(index) = self.stream_item_index.take()
                    && index < self.transcript.len()
                {
                    self.transcript.remove(index);
                }
                self.turn_first_token_at = None;
                self.turn_token_chars = 0;
                self.stream_phase = StreamPhase::Thinking;
            }
            TurnEvent::ToolApprovalRequested {
                recipient,
                summary,
                response_tx,
            } => {
                self.pending_tool_approval = Some(PendingToolApproval {
                    recipient,
                    summary: summary.clone(),
                    response_tx,
                });
                self.transcript.push(TranscriptItem::system(
                    "Approval Required",
                    format!("{summary}\nPress y to approve or n to reject."),
                ));
                self.status = "approval needed".to_string();
                self.status_kind = StatusKind::Warn;
                self.follow_tail = true;
            }
            TurnEvent::ContextCompacted {
                original_messages,
                kept_messages,
                estimated_tokens,
                threshold_tokens,
            } => {
                self.transcript.push(TranscriptItem::system(
                    "Context Compressed",
                    format!(
                        "Compacted earlier model context at ~{estimated_tokens}/{threshold_tokens} tokens. Kept {kept_messages} recent messages from {original_messages} total."
                    ),
                ));
                self.status = "context compressed".to_string();
                self.status_kind = StatusKind::Ok;
                self.follow_tail = true;
            }
            TurnEvent::EmptyModelResponse { format, model } => {
                if let Some(index) = self.stream_item_index.take()
                    && index < self.transcript.len()
                {
                    self.transcript.remove(index);
                }
                self.transcript.push(TranscriptItem::system(
                    "Empty Model Response",
                    format!(
                        "The model server returned no assistant text and no tool call after the last request.\nmodel: {model}\nformat: {format}\n\nThis usually means the local model stopped early, emitted an unsupported response shape, or needs a more direct follow-up. Try asking again, switching `model.format`, or checking the server logs."
                    ),
                ));
                self.status = "empty model response".to_string();
                self.status_kind = StatusKind::Warn;
                self.follow_tail = true;
            }
            TurnEvent::ModelRetryRequested {
                attempt,
                budget,
                reason,
            } => {
                if let Some(index) = self.stream_item_index.take()
                    && index < self.transcript.len()
                {
                    self.transcript.remove(index);
                }
                self.transcript.push(TranscriptItem::system(
                    format!("Model Retry {attempt}/{budget}"),
                    reason,
                ));
                self.status = "model retry".to_string();
                self.status_kind = StatusKind::Warn;
                self.follow_tail = true;
            }
            TurnEvent::CrpRetryRequested {
                attempt,
                budget,
                invalid_slots,
                parse_error,
            } => {
                if let Some(index) = self.stream_item_index.take()
                    && index < self.transcript.len()
                {
                    self.transcript.remove(index);
                }
                let detail = if let Some(error) = parse_error {
                    format!("CRP parse failed: {error}. Asking the model to re-emit a valid trace.")
                } else if invalid_slots.is_empty() {
                    "CRP validation failed. Asking the model to retry.".to_string()
                } else {
                    format!(
                        "Invalid slots: {}. Asking the model to retry.",
                        invalid_slots.join(", ")
                    )
                };
                self.transcript.push(TranscriptItem::system(
                    format!("CRP Retry {attempt}/{budget}"),
                    detail,
                ));
                self.status = "crp retry".to_string();
                self.status_kind = StatusKind::Warn;
                self.follow_tail = true;
            }
            TurnEvent::CrpRetryExhausted {
                budget,
                invalid_slots,
            } => {
                let detail = if invalid_slots.is_empty() {
                    format!(
                        "CRP retry budget ({budget}) exhausted. Surfacing the best-effort response with warnings."
                    )
                } else {
                    format!(
                        "CRP retry budget ({budget}) exhausted with invalid slots: {}. Surfacing the best-effort response.",
                        invalid_slots.join(", ")
                    )
                };
                self.transcript
                    .push(TranscriptItem::system("CRP Retry Exhausted", detail));
                self.status = "crp retry exhausted".to_string();
                self.status_kind = StatusKind::Warn;
                self.follow_tail = true;
            }
            TurnEvent::Message(message) => {
                if message.role == Role::Assistant
                    && message.channel == Some(Channel::Final)
                    && let Some(index) = self.stream_item_index.take()
                    && index < self.transcript.len()
                {
                    self.transcript[index] = TranscriptItem::from_message(&message);
                    self.follow_tail = true;
                    return;
                }

                self.transcript.push(TranscriptItem::from_message(&message));
                self.follow_tail = true;
            }
        }
    }

    fn ensure_stream_item(&mut self) -> usize {
        if let Some(index) = self.stream_item_index
            && index < self.transcript.len()
        {
            return index;
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

    fn input_height(&self) -> u16 {
        if self.pending_tool_approval.is_some() {
            2
        } else if self.view == View::Setup {
            self.setup_input_height()
        } else if self.view == View::Chat && self.is_busy() {
            2
        } else if self.view == View::Chat && self.input.is_empty() {
            1
        } else if self.view == View::Chat && self.input.starts_with('/') {
            4
        } else if self.view == View::Chat && !self.path_suggestions.is_empty() {
            3 + self.path_suggestions.len().min(5) as u16
        } else {
            3
        }
    }

    fn handle_approval_key(&mut self, code: KeyCode) -> bool {
        let approve = match code {
            KeyCode::Enter => Some(true),
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'y') => Some(true),
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'n') => Some(false),
            KeyCode::Esc => Some(false),
            KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End => {
                return false;
            }
            _ if self.pending_tool_approval.is_some() => {
                self.status = "approve with y/Enter or reject with n/Esc".to_string();
                self.status_kind = StatusKind::Warn;
                return true;
            }
            _ => None,
        };

        let Some(approve) = approve else {
            return false;
        };
        let Some(pending) = self.pending_tool_approval.take() else {
            return false;
        };

        let summary = pending.summary.clone();
        let _ = pending.response_tx.send(approve);
        if approve {
            self.append_system("Approved", format!("Allowed {summary}."));
            self.status = "tool approved".to_string();
            self.status_kind = StatusKind::Ok;
        } else {
            self.append_system("Rejected", format!("Blocked {summary}."));
            self.status = "tool rejected".to_string();
            self.status_kind = StatusKind::Warn;
        }
        true
    }

    fn header_height(&self) -> u16 {
        if self.header_expanded || self.last_reply_at.is_none() {
            4
        } else {
            1
        }
    }

    fn refresh_path_suggestions(&mut self) {
        if self.view != View::Chat || self.input.starts_with('/') {
            self.clear_path_suggestions();
            return;
        }

        let Some((range, prefix)) = current_path_prefix(&self.input) else {
            self.clear_path_suggestions();
            return;
        };

        match workspace::path_suggestions(&self.config.harness.workspace, &prefix, 5) {
            Ok(suggestions) => {
                self.path_suggestions = suggestions;
                self.path_suggestion_range = Some(range);
            }
            Err(_) => self.clear_path_suggestions(),
        }
    }

    fn clear_path_suggestions(&mut self) {
        self.path_suggestions.clear();
        self.path_suggestion_range = None;
    }

    fn accept_path_suggestion(&mut self) -> bool {
        if self.is_busy() {
            return false;
        }
        let Some((start, end)) = self.path_suggestion_range else {
            return false;
        };
        let Some(suggestion) = self.path_suggestions.first().cloned() else {
            return false;
        };
        if start > end || end > self.input.len() {
            self.clear_path_suggestions();
            return false;
        }

        self.input.replace_range(start..end, &suggestion);
        self.refresh_path_suggestions();
        true
    }

    fn show_workspace_diff(&mut self) {
        match workspace::diff_report(&self.config.harness.workspace) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Workspace Diff", report));
                self.status = "diff ready".to_string();
                self.status_kind = StatusKind::Ok;
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "diff failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    fn show_git_changes(&mut self) {
        match workspace::changes_report(&self.config.harness.workspace) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Git Changes", report));
                self.status = "git changes ready".to_string();
                self.status_kind = StatusKind::Ok;
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "git changes failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    fn stage_paths(&mut self, spec: &str) {
        if self.is_busy() {
            self.status = "wait for current turn before staging".to_string();
            self.status_kind = StatusKind::Warn;
            return;
        }

        match workspace::stage_paths(&self.config.harness.workspace, spec) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Git Stage", report));
                self.status = "changes staged".to_string();
                self.status_kind = StatusKind::Ok;
                self.show_git_changes();
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "stage failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    fn unstage_paths(&mut self, spec: &str) {
        if self.is_busy() {
            self.status = "wait for current turn before unstaging".to_string();
            self.status_kind = StatusKind::Warn;
            return;
        }

        match workspace::unstage_paths(&self.config.harness.workspace, spec) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Git Unstage", report));
                self.status = "changes unstaged".to_string();
                self.status_kind = StatusKind::Ok;
                self.show_git_changes();
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "unstage failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    fn commit_staged(&mut self, message: &str) {
        if self.is_busy() {
            self.status = "wait for current turn before committing".to_string();
            self.status_kind = StatusKind::Warn;
            return;
        }

        match workspace::commit_staged(&self.config.harness.workspace, message) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Git Commit", report));
                self.status = "commit created".to_string();
                self.status_kind = StatusKind::Ok;
                self.show_git_changes();
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "commit failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    fn create_checkpoint(&mut self, label: &str) {
        let label = if label.is_empty() { None } else { Some(label) };
        match workspace::create_checkpoint(&self.config.harness.workspace, label) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Checkpoint", report));
                self.status = "checkpoint saved".to_string();
                self.status_kind = StatusKind::Ok;
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "checkpoint failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    fn show_checkpoints(&mut self) {
        match workspace::list_checkpoints(&self.config.harness.workspace) {
            Ok(report) => {
                self.transcript
                    .push(TranscriptItem::system("Checkpoints", report));
                self.status = "checkpoints listed".to_string();
                self.status_kind = StatusKind::Ok;
            }
            Err(error) => {
                self.transcript
                    .push(TranscriptItem::error(format!("{error:#}")));
                self.status = "checkpoints failed".to_string();
                self.status_kind = StatusKind::Error;
            }
        }
        self.follow_tail = true;
    }

    async fn detect_context_window(&mut self) -> Result<()> {
        if self.is_busy() {
            self.status = "wait for current turn before detecting context".to_string();
            self.status_kind = StatusKind::Warn;
            return Ok(());
        }

        let old_window = self.config.model.context_window;
        self.transcript.push(TranscriptItem::system(
            "Context Detection",
            "Checking the model server for the served context window...",
        ));
        self.status = "detecting context".to_string();
        self.status_kind = StatusKind::Working;
        self.follow_tail = true;

        let client = ModelClient::new(self.config.model.clone());
        match tokio::time::timeout(Duration::from_secs(15), client.detect_context_window()).await {
            Ok(Ok(Some(detected_window))) if detected_window > 0 => {
                let mut config = self.config.clone();
                config.model.context_window = detected_window;
                self.apply_config(config);
                let path = self.config.save(self.config_path.clone())?;
                self.config_path = Some(path.clone());

                let detail = if detected_window == old_window {
                    format!("Confirmed {detected_window} tokens.")
                } else {
                    format!(
                        "Updated context window from {old_window} to {detected_window} tokens.\nSaved config to {}.",
                        path.display()
                    )
                };
                self.transcript
                    .push(TranscriptItem::system("Context Detection", detail));
                self.status = "context updated".to_string();
                self.status_kind = StatusKind::Ok;
            }
            Ok(Ok(_)) => {
                self.transcript.push(TranscriptItem::system(
                    "Context Detection",
                    "The server did not expose a context window for this model.",
                ));
                self.status = "context unavailable".to_string();
                self.status_kind = StatusKind::Warn;
            }
            Ok(Err(error)) => {
                self.transcript.push(TranscriptItem::error(format!(
                    "Context detection failed: {error:#}"
                )));
                self.status = "context detection failed".to_string();
                self.status_kind = StatusKind::Error;
            }
            Err(_) => {
                self.transcript.push(TranscriptItem::system(
                    "Context Detection",
                    "Timed out while checking the model server.",
                ));
                self.status = "context detection timed out".to_string();
                self.status_kind = StatusKind::Warn;
            }
        }

        self.follow_tail = true;
        Ok(())
    }

    fn interrupt_turn(&mut self) {
        let Some(cancel_tx) = self.cancel_tx.take() else {
            self.status = "interrupt already requested".to_string();
            self.status_kind = StatusKind::Warn;
            return;
        };

        let _ = cancel_tx.send(());
        self.transcript.push(TranscriptItem::system(
            "Interrupt Requested",
            "Stopping the in-flight model turn...",
        ));
        self.status = "interrupting".to_string();
        self.status_kind = StatusKind::Warn;
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
            SettingField::AllowShell
                | SettingField::EditApproval
                | SettingField::Stream
                | SettingField::AutoContextCompression
                | SettingField::WorkspaceInstructions
                | SettingField::ThinkingEffort
                | SettingField::ReasoningProtocol
                | SettingField::Format
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
            SettingField::EditApproval => {
                config.harness.require_edit_approval = !config.harness.require_edit_approval;
            }
            SettingField::Stream => {
                config.model.stream = !config.model.stream;
            }
            SettingField::AutoContextCompression => {
                config.harness.auto_context_compression = !config.harness.auto_context_compression;
            }
            SettingField::WorkspaceInstructions => {
                config.harness.load_workspace_instructions =
                    !config.harness.load_workspace_instructions;
            }
            SettingField::ThinkingEffort => {
                config.model.thinking_effort = next_thinking_effort(&config.model.thinking_effort);
            }
            SettingField::ReasoningProtocol => {
                config.harness.reasoning_protocol =
                    next_reasoning_protocol(&config.harness.reasoning_protocol);
            }
            SettingField::Format => {
                config.model.format = next_format(&config.model.format);
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
        self.refresh_path_suggestions();
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
        self.send_task.is_some() || self.kernel_task.is_some()
    }
}

fn current_path_prefix(input: &str) -> Option<((usize, usize), String)> {
    let token_start = input
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    let token = input[token_start..].trim_matches('"');
    if token.len() < 2 || token.starts_with('-') {
        return None;
    }

    let token_offset = input[token_start..].find(token).unwrap_or(0);
    let start = token_start + token_offset + usize::from(token.starts_with('@'));
    let end = token_start + token_offset + token.len();
    let prefix = token.strip_prefix('@').unwrap_or(token);

    if !is_path_like(prefix) {
        return None;
    }

    Some(((start, end), prefix.to_string()))
}

fn derive_stream_phase(body: &str) -> StreamPhase {
    let visible = visible_stream_body(body);
    if visible.trim_start().starts_with('<')
        && let Some(slot) = crate::crp::active_slot(&visible)
    {
        return StreamPhase::CrpSlot(slot);
    }

    const MARKER: &str = "<|channel|>";
    let Some(idx) = body.rfind(MARKER) else {
        return if body.is_empty() {
            StreamPhase::WarmingUp
        } else {
            StreamPhase::Responding
        };
    };
    let after = &body[idx + MARKER.len()..];

    if let Some(rest) = after.strip_prefix("commentary") {
        if let Some(to_idx) = rest.find(" to=") {
            let after_to = &rest[to_idx + " to=".len()..];
            let end = after_to
                .find(|c: char| c.is_whitespace() || c == '<')
                .unwrap_or(after_to.len());
            let recipient = after_to[..end].trim();
            let name = recipient
                .strip_prefix("functions.")
                .unwrap_or(recipient)
                .to_string();
            if name.is_empty() {
                StreamPhase::Thinking
            } else {
                StreamPhase::CallingTool(name)
            }
        } else {
            StreamPhase::Thinking
        }
    } else if after.starts_with("final") {
        StreamPhase::Responding
    } else {
        StreamPhase::Thinking
    }
}

fn visible_stream_body(body: &str) -> String {
    let sanitized = sanitize_stream_body(body);
    if sanitized.is_empty() && !body.contains("<|") {
        body.to_string()
    } else {
        sanitized
    }
}

fn is_path_like(token: &str) -> bool {
    token.len() >= 2
        && !token.starts_with('/')
        && (token.contains('/')
            || token.contains('.')
            || token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_crp_slot_from_plain_stream() {
        assert_eq!(
            derive_stream_phase("<PROPOSED_APPROACH>\n- Add CRP UI."),
            StreamPhase::CrpSlot("PROPOSED_APPROACH".to_string())
        );
    }

    #[test]
    fn derives_crp_slot_from_harmony_final_stream() {
        assert_eq!(
            derive_stream_phase("<|channel|>final<|message|><FILE_EDITS>\n@@ src/main.rs prepend"),
            StreamPhase::CrpSlot("FILE_EDITS".to_string())
        );
    }

    #[test]
    fn ignores_uppercase_markup_when_stream_is_not_crp() {
        assert_eq!(
            derive_stream_phase("Use <TOKEN> as placeholder."),
            StreamPhase::Responding
        );
    }
}
