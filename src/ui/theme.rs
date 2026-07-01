use ratatui::style::{Color, Style};
use rustui::style::{ColorToken, Design, Palette, Role, style as rustui_style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UiRole {
    Text,
    Muted,
    Accent,
    Success,
    Warning,
    Danger,
    Border,
    FocusedBorder,
}

pub(super) fn style(role: UiRole) -> Style {
    design().role_style(role.framework_role())
}

pub(super) fn bold(role: UiRole) -> Style {
    let design = design();

    rustui_style()
        .fg(design.color(role.color_token()))
        .bold()
        .build()
}

pub(super) fn underline(role: UiRole) -> Style {
    let design = design();

    rustui_style()
        .fg(design.color(role.color_token()))
        .underlined()
        .build()
}

pub(super) fn selected() -> Style {
    design().role_style(Role::ListItemSelected)
}

impl UiRole {
    fn framework_role(self) -> Role {
        match self {
            UiRole::Text => Role::Text,
            UiRole::Muted => Role::TextMuted,
            UiRole::Accent => Role::HeaderBrand,
            UiRole::Success => Role::StatusSuccess,
            UiRole::Warning => Role::StatusWarning,
            UiRole::Danger => Role::StatusDanger,
            UiRole::Border => Role::Panel,
            UiRole::FocusedBorder => Role::PanelFocused,
        }
    }

    fn color_token(self) -> ColorToken {
        match self {
            UiRole::Text => ColorToken::Text,
            UiRole::Muted => ColorToken::Muted,
            UiRole::Accent => ColorToken::Accent,
            UiRole::Success => ColorToken::Success,
            UiRole::Warning => ColorToken::Warning,
            UiRole::Danger => ColorToken::Danger,
            UiRole::Border => ColorToken::Border,
            UiRole::FocusedBorder => ColorToken::Warning,
        }
    }
}

fn design() -> Design {
    let palette = palette();

    Design::new(palette)
        .role(Role::Text, rustui_style().fg(palette.text))
        .role(Role::TextMuted, rustui_style().fg(palette.muted))
        .role(Role::HeaderBrand, rustui_style().fg(palette.accent).bold())
        .role(Role::StatusSuccess, rustui_style().fg(palette.success))
        .role(Role::StatusWarning, rustui_style().fg(palette.warning))
        .role(Role::StatusDanger, rustui_style().fg(palette.danger))
        .role(Role::Panel, rustui_style().fg(palette.border))
        .role(Role::PanelFocused, rustui_style().fg(palette.warning))
        .role(
            Role::ListItemSelected,
            rustui_style().fg(palette.text).bg(palette.selection),
        )
}

fn palette() -> Palette {
    Palette {
        surface0: Color::Black,
        surface1: Color::Rgb(18, 22, 28),
        surface2: Color::Rgb(29, 34, 42),
        border: Color::DarkGray,
        text: Color::White,
        muted: Color::Gray,
        accent: Color::Cyan,
        accent_low: Color::Blue,
        success: Color::Green,
        warning: Color::Yellow,
        danger: Color::Red,
        selection: Color::DarkGray,
    }
}
