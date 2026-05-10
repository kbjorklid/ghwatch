use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::Theme;

pub fn render_diagnostics(f: &mut Frame, area: Rect, selected_index: usize, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Diagnostics (GH Call Log) (j/k: Navigate | Enter: View Output | Esc: Exit) ")
        .title_style(Style::default().fg(theme.title));
    
    let calls = crate::logging::get_gh_calls();
    let mut text = Vec::new();

    if calls.is_empty() {
        text.push(Line::from("No GitHub CLI calls recorded yet."));
    } else {
        // Log is stored in chronological order, we want to show newest first for the list
        // but selected_index should map to the list displayed.
        let calls_reversed: Vec<_> = calls.iter().rev().collect();
        for (i, call) in calls_reversed.iter().enumerate() {
            let color = if call.exit_code == 0 { theme.success } else { theme.error };
            let mut style = Style::default().fg(theme.text);
            let mut bg_style = Style::default();
            
            if i == selected_index {
                bg_style = bg_style.bg(theme.highlight_bg);
                style = style.fg(theme.highlight_fg).add_modifier(ratatui::style::Modifier::BOLD);
            }

            text.push(Line::from(vec![
                Span::styled(format!("[{}] ", call.timestamp.format("%H:%M:%S")), Style::default().fg(theme.gray)),
                Span::styled(format!("{:>4}ms ", call.duration_ms), Style::default().fg(theme.info)),
                Span::styled(format!("exit={:<2} ", call.exit_code), Style::default().fg(color)),
                Span::styled(&call.command, style),
            ]).patch_style(bg_style));
        }
    }

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}
