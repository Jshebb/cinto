use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub fg: Color,
    pub dim: Color,
    pub muted: Color,
    pub chrome: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub user: Color,
    pub assistant: Color,
    pub tool: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            fg: Color::Rgb(218, 226, 232),
            dim: Color::Rgb(132, 145, 154),
            muted: Color::Rgb(82, 96, 106),
            chrome: Color::Rgb(43, 53, 60),
            accent: Color::Rgb(92, 214, 205),
            accent_soft: Color::Rgb(132, 186, 255),
            user: Color::Rgb(132, 186, 255),
            assistant: Color::Rgb(92, 214, 205),
            tool: Color::Rgb(154, 214, 128),
            success: Color::Rgb(154, 214, 128),
            warning: Color::Rgb(228, 204, 112),
            error: Color::Rgb(236, 118, 130),
        }
    }
}

impl Theme {
    pub fn logo_lines(&self) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::text::{Line, Span};

        vec![
            Line::from(vec![Span::styled("█▀▀ █ █▄ █ ▀█▀ █▀█", self.brand())]),
            Line::from(vec![Span::styled(
                "█▄▄ █ █ ▀█  █  █▄█",
                self.brand_subtle(),
            )]),
            Line::from(vec![
                Span::styled("╾══════", self.dim_style()),
                Span::styled("[C]", self.buckle()),
                Span::styled("══════╼", self.dim_style()),
            ]),
        ]
    }

    pub fn large_logo_lines(&self) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::text::{Line, Span};

        vec![
            Line::from(vec![Span::styled(
                " ██████╗██╗███╗   ██╗████████╗ ██████╗ ",
                self.brand(),
            )]),
            Line::from(vec![Span::styled(
                "██╔════╝██║████╗  ██║╚══██╔══╝██╔═══██╗",
                self.brand(),
            )]),
            Line::from(vec![Span::styled(
                "██║     ██║██╔██╗ ██║   ██║   ██║   ██║",
                self.brand_subtle(),
            )]),
            Line::from(vec![Span::styled(
                "██║     ██║██║╚██╗██║   ██║   ██║   ██║",
                self.brand_subtle(),
            )]),
            Line::from(vec![Span::styled(
                "╚██████╗██║██║ ╚████║   ██║   ╚██████╔╝",
                self.brand(),
            )]),
            Line::from(vec![Span::styled(
                " ╚═════╝╚═╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ",
                self.dim_style(),
            )]),
            Line::from(vec![
                Span::styled("╾══════════════════", self.dim_style()),
                Span::styled("[C]", self.buckle()),
                Span::styled("══════════════════╼", self.dim_style()),
            ]),
        ]
    }

    pub fn buckle(&self) -> Style {
        Style::default()
            .fg(self.warning)
            .add_modifier(Modifier::BOLD)
    }

    pub fn brand(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn brand_subtle(&self) -> Style {
        Style::default().fg(self.accent_soft)
    }

    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn chrome_style(&self) -> Style {
        Style::default().fg(self.chrome)
    }

    pub fn role_style(&self, role: RoleKind) -> Style {
        let color = match role {
            RoleKind::User => self.user,
            RoleKind::Assistant => self.assistant,
            RoleKind::Tool => self.tool,
            RoleKind::System => self.dim,
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }

    pub fn status_style(&self, status: StatusKind) -> Style {
        let color = match status {
            StatusKind::Idle => self.dim,
            StatusKind::Working => self.accent,
            StatusKind::Ok => self.success,
            StatusKind::Warn => self.warning,
            StatusKind::Error => self.error,
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RoleKind {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, Copy)]
pub enum StatusKind {
    Idle,
    Working,
    Ok,
    Warn,
    Error,
}

pub const BRAND_NAME: &str = "[C]";
pub const BRAND_WORDMARK: &str = "Cinto";

pub const WAVE_FRAMES: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "▆", "▅", "▄", "▃", "▂"];

pub const THINKING_FLAVOR: &[&str] = &[
    "Reading the room",
    "Tracing the thread",
    "Checking the shape of it",
    "Holding the context",
    "Following the clues",
    "Letting the tokens settle",
    "Looking for the clean move",
    "Keeping the harness warm",
];

pub fn wave_frame(tick: u64) -> &'static str {
    WAVE_FRAMES[(tick as usize) % WAVE_FRAMES.len()]
}

pub fn thinking_flavor(tick: u64) -> &'static str {
    THINKING_FLAVOR[((tick / 8) as usize) % THINKING_FLAVOR.len()]
}

#[derive(Debug, Clone, Copy)]
pub enum PhaseGlyph {
    Thinking,
    Tool,
    Responding,
}

impl PhaseGlyph {
    pub fn as_str(self) -> &'static str {
        match self {
            PhaseGlyph::Thinking => "◦",
            PhaseGlyph::Tool => "⚙",
            PhaseGlyph::Responding => "▸",
        }
    }
}

impl Theme {
    pub fn phase_style(&self, glyph: PhaseGlyph) -> Style {
        let color = match glyph {
            PhaseGlyph::Thinking => self.accent_soft,
            PhaseGlyph::Tool => self.tool,
            PhaseGlyph::Responding => self.accent,
        };
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    }
}
