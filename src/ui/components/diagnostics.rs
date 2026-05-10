use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::ui::theme::Theme;

pub fn render_diagnostics(f: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Diagnostics (GH Call Log) (Esc to exit) ")
        .title_style(Style::default().fg(theme.title));
    
    let calls = crate::logging::get_gh_calls();
    let mut text = Vec::new();

    if calls.is_empty() {
        text.push(Line::from("No GitHub CLI calls recorded yet."));
    } else {
        for call in calls.iter().rev() {
            let color = if call.exit_code == 0 { theme.success } else { theme.error };
            text.push(Line::from(vec![
                Span::styled(format!("[{}] ", call.timestamp.format("%H:%M:%S")), Style::default().fg(theme.gray)),
                Span::styled(format!("{}ms ", call.duration_ms), Style::default().fg(theme.info)),
                Span::styled(format!("exit={} ", call.exit_code), Style::default().fg(color)),
                Span::styled(&call.command, Style::default().fg(theme.text)),
            ]));
        }
    }

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}
