use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::ui::theme::Theme;
use crate::app::AppMode;

pub fn render_status_bar(f: &mut Frame, area: Rect, mode: &AppMode, theme: &Theme, last_refresh: Option<std::time::Instant>) {
    let keys = match mode {
        AppMode::Normal => "j/k: Nav | Tab: Focus | o: Open | s: Sort | f: Follow | /: Filter | m: Mark | u: Unfollow | A: Arch | S: Set | q: Quit",
        AppMode::Search => "Enter: Filter | Esc: Cancel | Backspace: Delete",
        AppMode::Follow => "Enter: Follow | Esc: Cancel",
        AppMode::Settings => "j/k: Navigate | Enter: Toggle | D: Diag | Esc: Back",
        AppMode::Archive => "j/k: Navigate | Esc: Back",
        AppMode::Help => "Esc/q: Back",
        AppMode::Diagnostic => "Esc/q: Back",
    };

    let refresh_text = match last_refresh {
        Some(t) => format!("Refreshed {}s ago", t.elapsed().as_secs()),
        None => "Never refreshed".to_string(),
    };

    let status = Line::from(vec![
        Span::styled(" ghnotify ", Style::default().bg(theme.info).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", keys), Style::default().bg(theme.gray).fg(theme.text)),
        Span::styled(format!(" {} ", refresh_text), Style::default().bg(theme.border).fg(theme.gray)),
    ]);

    f.render_widget(Paragraph::new(status), area);
}
