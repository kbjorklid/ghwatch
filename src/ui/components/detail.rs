use crate::config::AppConfig;
use crate::domain::pr::{CheckRun, PullRequest, TimelineEvent};
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
