use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::ui::theme::Theme;
use crate::domain::pr::PullRequest;

pub fn render_archive(f: &mut Frame, area: Rect, prs: &[PullRequest], selected_index: usize, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Archive (Esc to exit) ")
        .title_style(Style::default().fg(theme.title));
    if prs.is_empty() {
        let text = vec![
            Line::from("No archived PRs found."),
            Line::from("Use 'u' in the main list to archive a PR."),
        ];
        let paragraph = Paragraph::new(text).block(block).style(Style::default().fg(theme.text));
        f.render_widget(paragraph, area);
    } else {
        let items: Vec<ListItem> = prs
            .iter()
            .enumerate()
            .map(|(i, pr)| {
                let is_selected = i == selected_index;
                let mut style = Style::default().fg(theme.text);
                if is_selected {
                    style = style.fg(theme.highlight_fg);
                }

                let line1 = Line::from(vec![
                    Span::styled(format!("#{} ", pr.number), Style::default().fg(theme.gray)),
                    Span::styled(&pr.title, style),
                ]);

                let line2 = Line::from(vec![
                    Span::styled(format!("  {} ", pr.author), Style::default().fg(theme.gray)),
                    Span::styled(format!("{} ", pr.repo), Style::default().fg(theme.gray)),
                    Span::styled(format!("Status: {} ", pr.status), Style::default().fg(theme.gray)),
                ]);

                ListItem::new(vec![line1, line2])
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(theme.highlight_bg));

        f.render_widget(list, area);
    }
}
