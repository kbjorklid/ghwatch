use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::ui::theme::Theme;
use crate::app::AppMode;

pub fn render_status_bar(f: &mut Frame, area: Rect, mode: &AppMode, theme: &Theme, last_refresh: Option<std::time::Instant>, error: Option<&str>) {
    let keys = match mode {
        AppMode::Normal => "j/k: Nav | Tab: Focus | o: Open | y: Copy URL | s: Sort | f: Follow | /: Filter | m: Mark | u: Unfollow | A: Arch | S: Set | q: Quit",
        AppMode::Search => "Enter: Filter | Esc: Cancel | Backspace: Delete",
        AppMode::Follow => "Enter: Follow | Esc: Cancel",
        AppMode::Settings => "j/k: Navigate | Enter: Toggle | D: Diag | Esc: Back",
        AppMode::Archive => "j/k: Navigate | Esc: Back",
        AppMode::Diagnostic => "j/k: Navigate | Enter: View Output | y: Copy Cmd | Esc: Back",
        AppMode::LogDetail => "Esc/Enter: Back",
        AppMode::Help => "Esc/q: Back",
        AppMode::AddQueryName | AppMode::AddQuerySearch => "Enter: Next | Esc: Cancel",
        AppMode::ConfirmQuery => "y: Accept | n/Esc: Back",
    };


    let refresh_text = match last_refresh {
        Some(t) => format!("Refreshed {}s ago", t.elapsed().as_secs()),
        None => "Never refreshed".to_string(),
    };

    let mut spans = vec![
        Span::styled(" ghwatch ", Style::default().bg(theme.info).fg(Color::Black).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", keys), Style::default().fg(theme.gray)),
    ];

    if let Some(msg) = error {
        spans.push(Span::styled(format!(" ERROR: {} ", msg), Style::default().bg(theme.error).fg(Color::White).add_modifier(Modifier::BOLD)));
    } else {
        spans.push(Span::styled(format!(" {} ", refresh_text), Style::default().fg(theme.gray)));
    }

    let status = Line::from(spans);
    f.render_widget(Paragraph::new(status), area);
}
