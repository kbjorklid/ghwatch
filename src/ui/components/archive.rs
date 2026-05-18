use crate::domain::pr::PullRequest;
use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

pub fn render_archive(
    f: &mut Frame,
    area: Rect,
    prs: &[PullRequest],
    selected_index: usize,
    theme: &Theme,
) {
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
                    Span::styled(
                        format!("Status: {} ", pr.status),
                        Style::default().fg(theme.gray),
                    ),
                ]);

                ListItem::new(vec![line1, line2])
            })
            .collect();

        let list =
            List::new(items).block(block).highlight_style(Style::default().bg(theme.highlight_bg));

        f.render_widget(list, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::attention::AttentionState;
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus};
    use ratatui::backend::TestBackend;

    fn create_test_pr() -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "alice".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "2024-05-01T10:00:00Z".to_string(),
            updated_at: "2024-05-01T10:00:00Z".to_string(),
            additions: 10,
            deletions: 5,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "sha123".to_string(),
            body: "Body text".to_string(),
            url: "https://github.com/org/repo/pull/1".to_string(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            matched_queries: Vec::new(),
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: AttentionState::default(),
        }
    }

    #[test]
    fn test_render_archive_empty() {
        let backend = TestBackend::new(80, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let theme = Theme::dark();

        terminal
            .draw(|f| {
                render_archive(f, f.area(), &[], 0, &theme);
            })
            .unwrap();
    }

    #[test]
    fn test_render_archive_with_prs() {
        let prs = vec![create_test_pr()];
        let backend = TestBackend::new(80, 40);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let theme = Theme::dark();

        terminal
            .draw(|f| {
                render_archive(f, f.area(), &prs, 0, &theme);
            })
            .unwrap();
    }
}
