use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_help(f: &mut Frame, area: Rect, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Help (Esc to exit) ")
        .title_style(Style::default().fg(theme.title));
    let text = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j/k, Down/Up : Navigate list (or scroll detail if focused)"),
        Line::from("  Tab          : Toggle detail pane focus"),
        Line::from("  g            : Go to top"),
        Line::from("  G            : Go to bottom"),
        Line::from("  Enter        : Refresh selected PR details"),
        Line::from("  o            : Open current PR in browser"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  /            : Filter/Search PRs"),
        Line::from("  f            : Follow PR by URL or shorthand (owner/repo#123)"),
        Line::from("  m            : Mark selected PR as read"),
        Line::from("  M            : Mark ALL PRs as read"),
        Line::from("  u            : Unfollow selected PR (moves to archive)"),
        Line::from("  s            : Cycle sort mode (Updated, Created, Priority, Repo)"),
        Line::from("  Ctrl+g       : Cycle group mode (None, Repo, Author, Status, MyVsOther)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Views",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  S            : Open Settings screen"),
        Line::from("  A            : Open Archive view"),
        Line::from("  ?            : Open this Help overlay"),
        Line::from("  q            : Quit application"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Archive View",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  d            : Permanently delete selected PR"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Settings",
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  j/k, Down/Up : Navigate settings"),
        Line::from("  Space/Enter  : Toggle boolean setting or query"),
        Line::from(""),
        Line::from(Span::styled(
            "Tip: In narrow terminals, the layout automatically switches to vertical.",
            Style::default().fg(theme.warning),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme.text));
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_help() {
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::dark();

        terminal
            .draw(|f| {
                render_help(f, f.area(), &theme);
            })
            .unwrap();
    }
}
