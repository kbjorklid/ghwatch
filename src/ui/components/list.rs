use crate::config::AppConfig;
use crate::domain::attention::DotColor;
use crate::domain::pr::PullRequest;
use crate::domain::pr_list::get_grouped_items;
use crate::ui::icons::Icons;
use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn render_list(
    f: &mut Frame,
    area: Rect,
    prs: &[PullRequest],
    selected_index: usize,
    config: &AppConfig,
    theme: &Theme,
) {
    let icons = Icons::new(config.use_nerd_fonts);
    let grouped_items = get_grouped_items(prs, config);

    let mut items = Vec::new();
    let mut current_idx = 0;
    let mut list_selected_idx = 0;

    for group in grouped_items {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            format!(" ▼ {} ", group.name),
            Style::default().bg(theme.info).fg(Color::Black).add_modifier(Modifier::BOLD),
        )])));

        for pr in group.prs {
            let is_selected = current_idx == selected_index;
            if is_selected {
                list_selected_idx = items.len();
            }

            let lines = render_pr_item(pr, is_selected, config, theme, &icons, area.width);
            items.push(ListItem::new(lines));
            current_idx += 1;
        }
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(" Pull Requests ")
            .title_style(Style::default().fg(theme.title)),
    );

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(list_selected_idx));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_pr_item(
    pr: &PullRequest,
    is_selected: bool,
    config: &AppConfig,
    theme: &Theme,
    icons: &Icons,
    width: u16,
) -> Vec<Line<'static>> {
    let dot_color = pr.attention_state.dot_color(&pr.updated_at);

    let mut style = Style::default().fg(theme.text);
    let bg_style = Style::default();
    if is_selected {
        style = style.fg(theme.highlight_fg);
    }

    match &dot_color {
        Some(DotColor::Red) => style = style.fg(theme.error).add_modifier(Modifier::BOLD),
        Some(DotColor::Blue) => style = style.add_modifier(Modifier::BOLD),
        None => {}
    }

    let dot_marker = if dot_color.is_some() { "● " } else { "  " };
    let dot_style = match &dot_color {
        Some(DotColor::Red) => bg_style.fg(theme.error).add_modifier(Modifier::BOLD),
        Some(DotColor::Blue) => bg_style.fg(theme.info),
        None => bg_style,
    };

    let selection_bar = if is_selected {
        Span::styled("┃", Style::default().fg(Color::Yellow))
    } else {
        Span::raw(" ")
    };

    let status_color = match pr.status {
        crate::domain::pr::PRStatus::Open => theme.success,
        crate::domain::pr::PRStatus::Merged => theme.info,
        crate::domain::pr::PRStatus::Closed => theme.gray,
    };

    let review_status_color = match pr.review_status {
        crate::domain::pr::ReviewStatus::Approved => theme.success,
        crate::domain::pr::ReviewStatus::ChangesRequested => theme.error,
        crate::domain::pr::ReviewStatus::Pending => theme.warning,
    };

    let (mergeable_badge, mergeable_color) = match pr.mergeable {
        crate::domain::pr::MergeableStatus::Mergeable => (" [M] ", theme.success),
        crate::domain::pr::MergeableStatus::Conflicting => (" [C] ", theme.error),
        crate::domain::pr::MergeableStatus::Unknown => (" [?] ", theme.gray),
    };

    let status_str = format!(" {} ", pr.status);
    let review_status_str = format!(" {} ", pr.review_status);
    let id_str = format!("#{} ", pr.number);

    // area.width - 2 (borders) - 1 (selection bar) - 2 (dot) - id_str - status - review - mergeable
    let used_width = 2
        + 1
        + 2
        + id_str.len()
        + status_str.len()
        + review_status_str.len()
        + mergeable_badge.len();
    let available_title_width = width.saturating_sub(used_width as u16);

    let display_title = if pr.title.chars().count() > available_title_width as usize {
        let mut t: String =
            pr.title.chars().take(available_title_width.saturating_sub(1) as usize).collect();
        t.push('…');
        t
    } else {
        pr.title.clone()
    };

    let padding_len =
        available_title_width.saturating_sub(display_title.chars().count() as u16) as usize;
    let line1 = Line::from(vec![
        selection_bar.clone(),
        Span::styled(dot_marker, dot_style),
        Span::styled(id_str, bg_style.fg(theme.gray)),
        Span::styled(display_title, bg_style.patch(style)),
        Span::styled(" ".repeat(padding_len), bg_style.patch(style)),
        Span::styled(
            status_str,
            Style::default().bg(status_color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            review_status_str,
            Style::default().bg(review_status_color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            mergeable_badge,
            Style::default().bg(mergeable_color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut line2_spans = vec![selection_bar, Span::styled("  ", bg_style)];
    for col in &config.visible_columns {
        match col {
            crate::config::Column::Author => {
                line2_spans.push(Span::styled(format!("{} ", pr.author), bg_style.fg(theme.gray)));
            }
            crate::config::Column::Age => {
                line2_spans.push(Span::styled(
                    format!("{} ", crate::ui::format::format_relative_time(&pr.updated_at)),
                    bg_style.fg(theme.gray),
                ));
            }
            crate::config::Column::Diff => {
                line2_spans.push(Span::styled(
                    format!("{}{} ", icons.additions(), pr.additions),
                    bg_style.fg(theme.success),
                ));
                line2_spans.push(Span::styled(
                    format!("{}{} ", icons.deletions(), pr.deletions),
                    bg_style.fg(theme.error),
                ));
            }
            crate::config::Column::Comments => {
                let unresolved_new = if pr.last_seen_at.is_some() {
                    (pr.total_resolvable_count.saturating_sub(pr.last_seen_total_resolvable_count))
                        .min(pr.unresolved_count)
                } else {
                    0
                };
                let unresolved_old = pr.unresolved_count.saturating_sub(unresolved_new);

                let conversational_new = if pr.last_seen_at.is_some() {
                    pr.conversational_count.saturating_sub(pr.last_seen_conversational_count)
                } else {
                    0
                };
                let conversational_old = pr.conversational_count.saturating_sub(conversational_new);

                let unresolved_spans = if unresolved_new > 0 {
                    vec![
                        Span::styled(unresolved_old.to_string(), bg_style.fg(theme.gray)),
                        Span::styled(
                            format!("+{unresolved_new}"),
                            bg_style.fg(theme.info).add_modifier(Modifier::BOLD),
                        ),
                    ]
                } else {
                    vec![Span::styled(unresolved_old.to_string(), bg_style.fg(theme.gray))]
                };

                let conversational_spans = if conversational_new > 0 {
                    vec![
                        Span::styled(conversational_old.to_string(), bg_style.fg(theme.gray)),
                        Span::styled(
                            format!("+{conversational_new}"),
                            bg_style.fg(theme.info).add_modifier(Modifier::BOLD),
                        ),
                    ]
                } else {
                    vec![Span::styled(conversational_old.to_string(), bg_style.fg(theme.gray))]
                };

                line2_spans
                    .push(Span::styled(format!("{} ", icons.comment()), bg_style.fg(theme.gray)));
                line2_spans.extend(unresolved_spans);
                line2_spans.push(Span::styled(
                    format!("/{} (", pr.total_resolvable_count),
                    bg_style.fg(theme.gray),
                ));
                line2_spans.extend(conversational_spans);
                line2_spans.push(Span::styled(") ", bg_style.fg(theme.gray)));
            }
        }
    }

    vec![line1, Line::from(line2_spans)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, Column};
    use crate::domain::attention::TriggerReason;
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus};
    use crate::ui::theme::Theme;
    use std::collections::HashSet;

    #[test]
    fn test_render_pr_item_selected_has_vertical_bar() {
        let pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "author".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 10,
            deletions: 5,
            review_status: ReviewStatus::Approved,
            comment_count: 2,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 2,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "main".to_string(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };
        let config = AppConfig {
            current_user: "me".to_string(),
            visible_columns: vec![Column::Author],
            ..Default::default()
        };
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, true, &config, &theme, &icons, 100);

        // Check line 1
        assert_eq!(
            lines[0].spans[0].content, "┃",
            "Line 1 should start with vertical bar when selected"
        );
        assert_eq!(
            lines[0].spans[0].style.fg,
            Some(Color::Yellow),
            "Vertical bar should be yellow"
        );

        // Check line 2
        assert_eq!(
            lines[1].spans[0].content, "┃",
            "Line 2 should start with vertical bar when selected"
        );
        assert_eq!(
            lines[1].spans[0].style.fg,
            Some(Color::Yellow),
            "Vertical bar should be yellow"
        );
    }

    #[test]
    fn test_render_pr_item_unselected_has_no_vertical_bar() {
        let pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "author".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 10,
            deletions: 5,
            review_status: ReviewStatus::Approved,
            comment_count: 2,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 2,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "main".to_string(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };
        let config = AppConfig {
            current_user: "me".to_string(),
            visible_columns: vec![Column::Author],
            ..Default::default()
        };
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, false, &config, &theme, &icons, 100);

        // Check line 1 first span is unread marker or selection placeholder
        assert_ne!(
            lines[0].spans[0].content, "┃",
            "Line 1 should not start with vertical bar when unselected"
        );
    }

    #[test]
    fn test_render_pr_item_default_columns_no_review_status_on_second_line() {
        let pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "author".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 10,
            deletions: 5,
            review_status: ReviewStatus::Pending,
            comment_count: 2,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 2,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "main".to_string(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };
        let config = AppConfig::default();
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, false, &config, &theme, &icons, 100);

        // Line 2 should NOT contain "Pending"
        let line2_content: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !line2_content.contains("Pending"),
            "Line 2 should not contain review status 'Pending'. Content: {line2_content}"
        );
    }

    #[test]
    fn test_render_pr_item_red_dot_shows_error_color() {
        let mut pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "other".to_string(), // not current_user → old needs_attention stays false
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: String::new(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };
        pr.attention_state.active_reasons = HashSet::from([TriggerReason::CiFailed]);

        let config = AppConfig {
            current_user: "me".to_string(),
            visible_columns: vec![],
            ..Default::default()
        };
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, false, &config, &theme, &icons, 100);

        assert_eq!(lines[0].spans[1].content, "● ", "Red dot should show ● marker");
        assert_eq!(lines[0].spans[1].style.fg, Some(theme.error), "Red dot should use error color");
    }

    #[test]
    fn test_render_pr_item_no_dot_when_attention_seen_and_clear() {
        use chrono::Utc;
        let mut pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "other".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: String::new(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None, // old field still None → old code would show ● (is_unread=true)
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };
        // attention_state seen after updated_at → no dot
        pr.attention_state.last_seen_at = Some(
            chrono::DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        );

        let config = AppConfig {
            current_user: "me".to_string(),
            visible_columns: vec![],
            ..Default::default()
        };
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, false, &config, &theme, &icons, 100);

        assert_eq!(lines[0].spans[1].content, "  ", "No dot when attention_state is clear");
    }

    #[test]
    fn test_render_pr_item_no_separate_attention_marker() {
        let pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "me".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::ChangesRequested, // triggers old needs_attention
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: String::new(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };

        let config = AppConfig {
            current_user: "me".to_string(),
            visible_columns: vec![],
            ..Default::default()
        };
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, false, &config, &theme, &icons, 100);

        let line1_content: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            !line1_content.contains("! "),
            "Old '! ' attention marker should not appear in new design. Got: {line1_content}"
        );
    }

    #[test]
    fn test_render_comment_count_with_delta() {
        let pr = PullRequest {
            id: "1".to_string(),
            number: 1,
            title: "Test PR".to_string(),
            author: "author".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 10,
            deletions: 5,
            review_status: ReviewStatus::Pending,
            comment_count: 4,
            unresolved_count: 3,
            total_resolvable_count: 3,
            conversational_count: 1,
            ci_status: CIStatus::Passing,
            mergeable: MergeableStatus::Unknown,
            head_ref: "main".to_string(),
            body: String::new(),
            url: String::new(),
            requested_reviewers: vec![],
            reviewers: vec![],
            is_draft: false,
            last_seen_at: Some("some-time".to_string()),
            last_seen_unresolved_count: 1,
            last_seen_total_resolvable_count: 1,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        };
        // Note: unresolved_new = total_resolvable - last_seen_total = 3 - 1 = 2
        // unresolved_old = unresolved_total - unresolved_new = 3 - 2 = 1
        // conversational_new = conversational - last_seen_conversational = 1 - 0 = 1
        // Expected string: "1+2/3 (0+1) " (plus icon)

        let config = AppConfig { visible_columns: vec![Column::Comments], ..Default::default() };
        let theme = Theme::dark();
        let icons = Icons::new(false);

        let lines = render_pr_item(&pr, false, &config, &theme, &icons, 100);
        let line2_content: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(
            line2_content.contains("1+2/3 (0+1)"),
            "Comment string should contain delta. Got: {line2_content}"
        );
    }
}
