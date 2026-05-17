use crate::config::AppConfig;
use crate::domain::attention::DotColor;
use crate::domain::pr::{CheckRun, MergeableStatus, PullRequest, TimelineEvent};
use crate::ui::icons::Icons;
use crate::ui::markdown::render_markdown;
use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

#[derive(Debug)]
pub struct DetailProps<'a> {
    pub prs: &'a [PullRequest],
    pub selected_index: usize,
    pub checks: &'a [CheckRun],
    pub timeline: &'a [TimelineEvent],
    pub config: &'a AppConfig,
    pub theme: &'a Theme,
    pub detail_focused: bool,
    pub detail_scroll: u16,
}

pub fn render_detail(f: &mut Frame, area: Rect, props: &DetailProps<'_>) {
    if let Some(pr) = props.prs.get(props.selected_index) {
        let icons = Icons::new(props.config.use_nerd_fonts);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if props.detail_focused {
                props.theme.highlight_fg
            } else {
                props.theme.border
            }))
            .title(format!(
                " #{} - {} {} ",
                pr.number,
                pr.title,
                if props.detail_focused { "(Focused)" } else { "" }
            ))
            .title_style(Style::default().fg(props.theme.title));

        let mut detail_text = Vec::new();

        // Header Info
        detail_text.push(Line::from(vec![
            Span::styled("Author: ", Style::default().fg(props.theme.gray)),
            Span::styled(&pr.author, Style::default().fg(props.theme.text)),
            Span::raw(" | "),
            Span::styled("Repo: ", Style::default().fg(props.theme.gray)),
            Span::styled(&pr.repo, Style::default().fg(props.theme.text)),
        ]));

        let status_color = match pr.status {
            crate::domain::pr::PRStatus::Open => props.theme.success,
            crate::domain::pr::PRStatus::Merged => props.theme.info,
            crate::domain::pr::PRStatus::Closed => props.theme.gray,
        };

        let review_status_color = match pr.review_status {
            crate::domain::pr::ReviewStatus::Approved => props.theme.success,
            crate::domain::pr::ReviewStatus::ChangesRequested => props.theme.error,
            crate::domain::pr::ReviewStatus::Pending => props.theme.warning,
        };

        let (mergeable_label, mergeable_color) = match pr.mergeable {
            MergeableStatus::Mergeable => (" Mergeable ", props.theme.success),
            MergeableStatus::Conflicting => (" Conflicting ", props.theme.error),
            MergeableStatus::Unknown => (" Unknown ", props.theme.gray),
        };

        detail_text.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(props.theme.gray)),
            Span::styled(
                format!(" {} ", pr.status),
                Style::default()
                    .bg(status_color)
                    .fg(ratatui::style::Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("Review: ", Style::default().fg(props.theme.gray)),
            Span::styled(
                format!(" {} ", pr.review_status),
                Style::default()
                    .bg(review_status_color)
                    .fg(ratatui::style::Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled("CI: ", Style::default().fg(props.theme.gray)),
            Span::styled(format!("{} ", pr.ci_status), Style::default().fg(props.theme.text)),
            Span::raw(" "),
            Span::styled("Merge: ", Style::default().fg(props.theme.gray)),
            Span::styled(
                mergeable_label,
                Style::default()
                    .bg(mergeable_color)
                    .fg(ratatui::style::Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        detail_text.push(Line::from(vec![
            Span::styled("Diff: ", Style::default().fg(props.theme.gray)),
            Span::styled(
                format!("{}{} ", icons.additions(), pr.additions),
                Style::default().fg(props.theme.success),
            ),
            Span::styled(
                format!("{}{} ", icons.deletions(), pr.deletions),
                Style::default().fg(props.theme.error),
            ),
            Span::raw(" | "),
            Span::styled("Reviewers: ", Style::default().fg(props.theme.gray)),
            Span::styled(
                if pr.reviewers.is_empty() {
                    "None".to_string()
                } else {
                    pr.reviewers
                        .iter()
                        .map(|r| format!("{}({})", r.login, r.status))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                Style::default().fg(props.theme.text),
            ),
        ]));

        // Attention reasons
        if pr.attention_state.is_red() {
            let heading_color = match pr.attention_state.dot_color(&pr.updated_at) {
                Some(DotColor::Red) => props.theme.error,
                _ => props.theme.info,
            };
            detail_text.push(Line::from(Span::styled(
                "Attention:",
                Style::default().fg(heading_color).add_modifier(Modifier::BOLD),
            )));
            let mut reasons: Vec<_> = pr.attention_state.active_reasons.iter().collect();
            reasons.sort_by_key(ToString::to_string);
            for reason in reasons {
                detail_text.push(Line::from(vec![
                    Span::raw("  ● "),
                    Span::styled(reason.to_string(), Style::default().fg(props.theme.error)),
                ]));
            }
        }

        detail_text.push(Line::from(""));

        // CI Checks
        if !props.checks.is_empty() {
            detail_text.push(Line::from(Span::styled(
                "CI Checks:",
                Style::default().fg(props.theme.title).add_modifier(Modifier::BOLD),
            )));
            for check in props.checks {
                let color = match check.conclusion.as_deref() {
                    Some("success") => props.theme.success,
                    Some("failure" | "error") => props.theme.error,
                    _ => props.theme.warning,
                };
                let icon = match check.conclusion.as_deref() {
                    Some("success") => icons.check_ok(),
                    Some("failure" | "error") => icons.check_err(),
                    _ => "○",
                };
                detail_text.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{icon} "), Style::default().fg(color)),
                    Span::styled(
                        format!("{} ", check.conclusion.as_deref().unwrap_or(&check.status)),
                        Style::default().fg(color),
                    ),
                    Span::styled(&check.name, Style::default().fg(props.theme.text)),
                ]));
            }
            detail_text.push(Line::from(""));
        }

        // Timeline
        if !props.timeline.is_empty() {
            detail_text.push(Line::from(Span::styled(
                "Activity:",
                Style::default().fg(props.theme.title).add_modifier(Modifier::BOLD),
            )));
            for event in props.timeline.iter().take(10) {
                // Limit to 10 for now
                let mut spans = vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{} ", event.event_type),
                        Style::default().fg(props.theme.info),
                    ),
                    Span::styled(
                        format!("by {} ", event.actor),
                        Style::default().fg(props.theme.text),
                    ),
                ];

                if let Some(content) = &event.content {
                    let truncated = if content.len() > 100 {
                        format!("{}...", &content[..97])
                    } else {
                        content.clone()
                    };
                    spans.push(Span::styled(
                        format!(": {truncated} "),
                        Style::default().fg(props.theme.text),
                    ));
                }

                spans.push(Span::styled(&event.created_at, Style::default().fg(props.theme.gray)));

                detail_text.push(Line::from(spans));
            }
            detail_text.push(Line::from(""));
        }

        // Body
        detail_text.push(Line::from(Span::styled(
            "Description:",
            Style::default().fg(props.theme.title).add_modifier(Modifier::BOLD),
        )));
        detail_text.extend(render_markdown(&pr.body));

        let paragraph = Paragraph::new(detail_text)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((props.detail_scroll, 0))
            .style(Style::default().fg(props.theme.text));

        f.render_widget(paragraph, area);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(props.theme.border))
            .title(" Detail ")
            .title_style(Style::default().fg(props.theme.title));
        f.render_widget(block, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::attention::{AttentionState, TriggerReason};
    use crate::domain::pr::{CIStatus, MergeableStatus, PRStatus, PullRequest, ReviewStatus};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::HashSet;

    fn make_test_pr() -> PullRequest {
        PullRequest {
            id: "1".to_string(),
            number: 42,
            title: "Test PR".to_string(),
            author: "author".to_string(),
            repo: "org/repo".to_string(),
            status: PRStatus::Open,
            created_at: "1h".to_string(),
            updated_at: "1h".to_string(),
            additions: 10,
            deletions: 5,
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
            attention_state: AttentionState::default(),
        }
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal.backend().buffer().content().iter().map(ratatui::buffer::Cell::symbol).collect()
    }

    #[test]
    fn test_detail_shows_active_attention_reasons() {
        let mut pr = make_test_pr();
        pr.attention_state.active_reasons =
            HashSet::from([TriggerReason::CiFailed, TriggerReason::Approved]);

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::dark();
        let config = AppConfig::default();

        terminal
            .draw(|f| {
                let area = f.area();
                let props = DetailProps {
                    prs: &[pr],
                    selected_index: 0,
                    checks: &[],
                    timeline: &[],
                    config: &config,
                    theme: &theme,
                    detail_focused: false,
                    detail_scroll: 0,
                };
                render_detail(f, area, &props);
            })
            .unwrap();

        let content = buffer_text(&terminal);
        assert!(
            content.contains("CI failed"),
            "Detail should show 'CI failed' reason. Content: {content}"
        );
        assert!(
            content.contains("Approved"),
            "Detail should show 'Approved' reason. Content: {content}"
        );
    }

    #[test]
    fn test_detail_shows_no_attention_section_when_clear() {
        let pr = make_test_pr(); // no active reasons, default state

        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::dark();
        let config = AppConfig::default();

        terminal
            .draw(|f| {
                let area = f.area();
                let props = DetailProps {
                    prs: &[pr],
                    selected_index: 0,
                    checks: &[],
                    timeline: &[],
                    config: &config,
                    theme: &theme,
                    detail_focused: false,
                    detail_scroll: 0,
                };
                render_detail(f, area, &props);
            })
            .unwrap();

        let content = buffer_text(&terminal);
        assert!(
            !content.contains("Attention:"),
            "Detail should not show Attention section when no reasons active"
        );
    }
}
