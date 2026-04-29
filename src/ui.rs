use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Terminal},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::session::{AgentSession, Channel, Message, Role};

pub struct App {
    session: AgentSession,
    transcript: Vec<String>,
    input: String,
    status: String,
}

impl App {
    pub fn new(session: AgentSession) -> Self {
        Self {
            session,
            transcript: vec![
                "OpenHarness ready. Type a request, /prompt to inspect Harmony, /clear, or /quit."
                    .to_string(),
            ],
            input: String::new(),
            status: "idle".to_string(),
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
                let [messages_area, input_area, status_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(4),
                        Constraint::Length(3),
                        Constraint::Length(1),
                    ])
                    .areas(frame.area());

                let messages = Paragraph::new(self.transcript.join("\n\n"))
                    .block(Block::default().title("OpenHarness").borders(Borders::ALL))
                    .wrap(Wrap { trim: false });
                frame.render_widget(messages, messages_area);

                let input = Paragraph::new(self.input.as_str())
                    .block(Block::default().title("Input").borders(Borders::ALL));
                frame.render_widget(input, input_area);

                let status = Paragraph::new(Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        self.status.as_str(),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                frame.render_widget(status, status_area);
            })?;

            if event::poll(Duration::from_millis(50))? {
                let Event::Key(key) = event::read()? else {
                    continue;
                };

                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char(ch) => self.input.push(ch),
                    KeyCode::Backspace => {
                        self.input.pop();
                    }
                    KeyCode::Enter => {
                        let input = self.input.trim().to_string();
                        self.input.clear();
                        if self.handle_input(input).await? {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    async fn handle_input(&mut self, input: String) -> Result<bool> {
        if input.is_empty() {
            return Ok(false);
        }

        match input.as_str() {
            "/quit" | "/exit" => return Ok(true),
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
