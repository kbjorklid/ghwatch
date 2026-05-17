use crate::config::AppConfig;
use crate::domain::pr::PullRequest;
use crate::ui::components::{
    archive::render_archive, detail::render_detail, diagnostics::render_diagnostics,
    help::render_help, list::render_list, settings::render_settings, status_bar::render_status_bar,
    tab_bar::render_tab_bar, theme_picker::render_theme_picker,
};
use crate::ui::theme::Theme;
use anyhow::Result;
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io;

#[allow(missing_debug_implementations)]
pub struct Renderer<B: Backend> {
    terminal: Terminal<B>,
}

#[derive(Debug)]
pub struct DrawContext<'a> {
    pub prs: &'a [PullRequest],
    pub selected_index: usize,
    pub settings_selected_index: usize,
    pub diagnostic_selected_index: usize,
    pub checks: &'a [crate::domain::pr::CheckRun],
    pub timeline: &'a [crate::domain::pr::TimelineEvent],
    pub mode: &'a crate::app::AppMode,
    pub detail_focused: bool,
    pub detail_scroll: u16,
    pub input_buffer: &'a str,
    pub config: &'a AppConfig,
    pub last_refresh: Option<std::time::Instant>,
    pub error_message: Option<&'a str>,
    pub query_name_buffer: &'a str,
    pub query_search_buffer: &'a str,
    pub query_test_results: Option<&'a [PullRequest]>,
    pub query_test_error: Option<&'a str>,
    pub is_testing_query: bool,
    pub theme_picker_index: usize,
    pub editing_query_index: Option<usize>,
    pub deleting_query_index: Option<usize>,
}

impl<B: Backend> Renderer<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    pub fn new(backend: B) -> Result<Self> {
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub const fn terminal_mut(&mut self) -> &mut Terminal<B> {
        &mut self.terminal
    }

    pub fn draw(&mut self, ctx: &DrawContext<'_>) -> Result<()> {
        let theme = Theme::from_name(&ctx.config.theme);

        self.terminal.draw(|f| {
            let has_input =
                matches!(ctx.mode, crate::app::AppMode::Search | crate::app::AppMode::Follow);
            let show_status = ctx.config.show_status_bar;

            let constraints = [
                Constraint::Length(1),
                Constraint::Min(0),
                if has_input { Constraint::Length(3) } else { Constraint::Length(0) },
                if show_status { Constraint::Length(1) } else { Constraint::Length(0) },
            ];

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints.as_ref())
                .split(f.area());

            render_tab_bar(f, chunks[0], ctx.mode, &theme);

            match ctx.mode {
                crate::app::AppMode::Settings => {
                    render_settings(f, chunks[1], ctx.config, ctx.settings_selected_index, &theme);
                }
                crate::app::AppMode::Archive => {
                    render_archive(f, chunks[1], ctx.prs, ctx.selected_index, &theme);
                }
                crate::app::AppMode::Help => {
                    render_help(f, chunks[1], &theme);
                }
                crate::app::AppMode::Diagnostic => {
                    render_diagnostics(f, chunks[1], ctx.diagnostic_selected_index, &theme);
                }
                crate::app::AppMode::LogDetail => {
                    render_diagnostics(f, chunks[1], ctx.diagnostic_selected_index, &theme);
                    render_log_detail(f, f.area(), ctx.diagnostic_selected_index, &theme);
                }
                crate::app::AppMode::AddQueryName | crate::app::AppMode::AddQuerySearch => {
                    render_settings(f, chunks[1], ctx.config, ctx.settings_selected_index, &theme);
                    let is_editing = ctx.editing_query_index.is_some();
                    let title = match ctx.mode {
                        crate::app::AppMode::AddQueryName => {
                            if is_editing {
                                " Edit Query Name "
                            } else {
                                " Enter Query Name "
                            }
                        }
                        _ => {
                            if is_editing {
                                " Edit Search Query "
                            } else {
                                " Enter Search Query "
                            }
                        }
                    };
                    let buffer = if ctx.mode == &crate::app::AppMode::AddQueryName {
                        ctx.query_name_buffer
                    } else {
                        ctx.query_search_buffer
                    };

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.info))
                        .title(title)
                        .title_style(Style::default().fg(theme.title));

                    let modal_area = centered_rect(60, 20, f.area());
                    f.render_widget(ratatui::widgets::Clear, modal_area);

                    let text =
                        Paragraph::new(buffer).block(block).style(Style::default().fg(theme.text));
                    f.render_widget(text, modal_area);
                }
                crate::app::AppMode::ThemePicker => {
                    render_settings(f, chunks[1], ctx.config, ctx.settings_selected_index, &theme);
                    let modal = centered_rect(85, 90, f.area());
                    f.render_widget(ratatui::widgets::Clear, modal);
                    render_theme_picker(f, modal, ctx.theme_picker_index, &theme);
                }
                crate::app::AppMode::DeleteQueryConfirm => {
                    render_settings(f, chunks[1], ctx.config, ctx.settings_selected_index, &theme);

                    let query_name = ctx
                        .deleting_query_index
                        .and_then(|i| ctx.config.queries.get(i))
                        .map_or("this query", |q| q.name.as_str());

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.error))
                        .title(" Delete Query? ")
                        .title_style(Style::default().fg(theme.title));

                    let modal_area = centered_rect(50, 30, f.area());
                    f.render_widget(ratatui::widgets::Clear, modal_area);

                    let text = vec![
                        Line::from(""),
                        Line::from(vec![
                            Span::styled(" Delete \"", Style::default().fg(theme.text)),
                            Span::styled(
                                query_name.to_string(),
                                Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("\"?", Style::default().fg(theme.text)),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled(
                            " (y)es / (n)o ",
                            Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
                        )),
                    ];

                    let paragraph = Paragraph::new(text).block(block);
                    f.render_widget(paragraph, modal_area);
                }
                crate::app::AppMode::ConfirmQuery => {
                    render_settings(f, chunks[1], ctx.config, ctx.settings_selected_index, &theme);

                    let confirm_title = if ctx.editing_query_index.is_some() {
                        " Confirm Edit Query "
                    } else {
                        " Confirm New Query "
                    };
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.info))
                        .title(confirm_title)
                        .title_style(Style::default().fg(theme.title));

                    let modal_area = centered_rect(70, 60, f.area());
                    f.render_widget(ratatui::widgets::Clear, modal_area);

                    let mut text = Vec::new();
                    text.push(Line::from(vec![
                        Span::styled("Name: ", Style::default().fg(theme.gray)),
                        Span::styled(
                            ctx.query_name_buffer,
                            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    text.push(Line::from(vec![
                        Span::styled("Query: ", Style::default().fg(theme.gray)),
                        Span::styled(ctx.query_search_buffer, Style::default().fg(theme.info)),
                    ]));
                    text.push(Line::from(""));

                    if ctx.is_testing_query {
                        text.push(Line::from(Span::styled(
                            " Testing query... ",
                            Style::default().fg(theme.warning).add_modifier(Modifier::ITALIC),
                        )));
                    } else if let Some(err) = ctx.query_test_error {
                        text.push(Line::from(Span::styled(
                            format!(" Error: {err}"),
                            Style::default().fg(theme.error),
                        )));
                        text.push(Line::from(""));
                        text.push(Line::from(Span::styled(
                            " Press Esc to go back and edit. ",
                            Style::default().fg(theme.gray),
                        )));
                    } else if let Some(prs) = ctx.query_test_results {
                        text.push(Line::from(vec![
                            Span::styled(
                                format!(" Matches: {}", prs.len()),
                                Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
                            ),
                            if prs.len() >= 5 {
                                Span::styled(" (showing top 5)", Style::default().fg(theme.gray))
                            } else {
                                Span::raw("")
                            },
                        ]));
                        text.push(Line::from(""));

                        for pr in prs.iter().take(5) {
                            text.push(Line::from(vec![
                                Span::styled(
                                    format!(" #{} ", pr.number),
                                    Style::default().fg(theme.gray),
                                ),
                                Span::styled(&pr.title, Style::default().fg(theme.text)),
                            ]));
                        }

                        text.push(Line::from(""));
                        let action_prompt = if ctx.editing_query_index.is_some() {
                            " Save changes? "
                        } else {
                            " Add this query? "
                        };
                        text.push(Line::from(vec![
                            Span::styled(action_prompt, Style::default().fg(theme.text)),
                            Span::styled(
                                " (y)es / (n)o ",
                                Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    }

                    let paragraph = Paragraph::new(text).block(block);
                    f.render_widget(paragraph, modal_area);
                }
                _ => {
                    let area = chunks[1];
                    let direction =
                        if area.width < 120 { Direction::Vertical } else { Direction::Horizontal };
                    let main_chunks = Layout::default()
                        .direction(direction)
                        .constraints(
                            [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
                        )
                        .split(area);

                    render_list(f, main_chunks[0], ctx.prs, ctx.selected_index, ctx.config, &theme);
                    render_detail(
                        f,
                        main_chunks[1],
                        &crate::ui::components::detail::DetailProps {
                            prs: ctx.prs,
                            selected_index: ctx.selected_index,
                            checks: ctx.checks,
                            timeline: ctx.timeline,
                            config: ctx.config,
                            theme: &theme,
                            detail_focused: ctx.detail_focused,
                            detail_scroll: ctx.detail_scroll,
                        },
                    );
                }
            }

            if has_input {
                let prompt_label = match ctx.mode {
                    crate::app::AppMode::Search => "/Search: ",
                    crate::app::AppMode::Follow => "Follow (URL/Shorthand): ",
                    _ => "",
                };

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
                    .title(" Input (Esc to cancel) ")
                    .title_style(Style::default().fg(theme.title));
                let text = Paragraph::new(format!("{}{}", prompt_label, ctx.input_buffer))
                    .block(block)
                    .style(Style::default().fg(theme.text));
                f.render_widget(text, chunks[2]);
            }

            if show_status {
                render_status_bar(
                    f,
                    chunks[3],
                    ctx.mode,
                    &theme,
                    ctx.last_refresh,
                    ctx.error_message,
                );
            }
        })?;
        Ok(())
    }
}

fn render_log_detail(f: &mut Frame, area: Rect, selected_index: usize, theme: &Theme) {
    let calls = crate::logging::get_gh_calls();
    let calls_reversed: Vec<_> = calls.iter().rev().collect();

    if let Some(call) = calls_reversed.get(selected_index) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(format!(" Output: {} ", call.command))
            .title_style(Style::default().fg(theme.title));

        let modal_area = centered_rect(80, 80, area);
        f.render_widget(ratatui::widgets::Clear, modal_area);

        let text = Paragraph::new(call.output.as_str())
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(text, modal_area);
    }
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub type CrosstermRenderer = Renderer<ratatui::backend::CrosstermBackend<io::Stdout>>;

impl Renderer<ratatui::backend::CrosstermBackend<io::Stdout>> {
    pub fn init(&mut self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;
        self.terminal.hide_cursor()?;
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppMode;
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
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
            attention_state: Default::default(),
        }
    }

    #[test]
    fn test_draw_normal_mode() {
        let backend = TestBackend::new(80, 24);
        let mut renderer = Renderer::new(backend).unwrap();
        let prs = vec![create_test_pr()];
        let config = AppConfig::default();

        renderer
            .draw(&DrawContext {
                prs: &prs,
                selected_index: 0,
                settings_selected_index: 0,
                diagnostic_selected_index: 0,
                checks: &[],
                timeline: &[],
                mode: &AppMode::Normal,
                detail_focused: false,
                detail_scroll: 0,
                input_buffer: "",
                config: &config,
                last_refresh: None,
                error_message: None,
                query_name_buffer: "",
                query_search_buffer: "",
                query_test_results: None,
                query_test_error: None,
                is_testing_query: false,
                theme_picker_index: 0,
                editing_query_index: None,
                deleting_query_index: None,
            })
            .unwrap();
    }
}
