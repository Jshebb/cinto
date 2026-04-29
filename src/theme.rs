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
            fg: Color::Rgb(202, 211, 245),          // Texto principal (suave)
            dim: Color::Rgb(128, 135, 162),         // Texto secundário/inativo
            muted: Color::Rgb(91, 96, 120),         // Bordas sutis e fundo inativo
            chrome: Color::Rgb(54, 58, 79),         // Elementos de UI pesados
            accent: Color::Rgb(245, 169, 127),      // Peach (destaques suaves)
            accent_soft: Color::Rgb(238, 212, 159), // Yellow (secundário)
            user: Color::Rgb(138, 173, 244),        // Azul pastel (Usuário)
            assistant: Color::Rgb(198, 160, 246),   // Roxo/Mauve pastel (IA)
            tool: Color::Rgb(166, 218, 149),        // Verde pastel (Ferramentas)
            success: Color::Rgb(166, 218, 149),
            warning: Color::Rgb(238, 212, 159),
            error: Color::Rgb(237, 135, 150), // Vermelho lavado
        }
    }
}

impl Theme {
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

pub const BRAND_GLYPH: &str = ">";
pub const BRAND_NAME: &str = "OH!";

pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_frame(tick: u64) -> &'static str {
    SPINNER_FRAMES[(tick as usize) % SPINNER_FRAMES.len()]
}
