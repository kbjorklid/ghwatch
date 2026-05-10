use crate::domain::pr::PullRequest;
use anyhow::Result;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
    Terminal,
    Frame,
};
use std::io;
use crate::config::AppConfig;
use crate::ui::theme::Theme;
use crate::ui::components::{
    list::render_list,
    detail::render_detail,
    settings::render_settings,
    help::render_help,
    diagnostics::render_diagnostics,
    status_bar::render_status_bar,
    archive::render_archive,
};

pub struct Renderer<B: Backend> {
    terminal: Terminal<B>,
}

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
}

impl<B: Backend> Renderer<B> 
where 
    B::Error: std::error::Error + Send + Sync + 'static 
{
    pub fn new(backend: B) -> Result<Self> {
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<B> {
        &mut self.terminal
    }

    pub fn draw(&mut self, ctx: DrawContext) -> Result<()> {
        let theme = Theme::from_name(&ctx.config.theme);

        self.terminal.draw(|f| {
            let has_input = matches!(ctx.mode, crate::app::AppMode::Search | crate::app::AppMode::Follow);
            let show_status = ctx.config.show_status_bar;
            
            let constraints = [
                Constraint::Min(0),
                if has_input { Constraint::Length(3) } else { Constraint::Length(0) },
                if show_status { Constraint::Length(1) } else { Constraint::Length(0) },
            ];

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints.as_ref())
                .split(f.area());

            match ctx.mode {
                crate::app::AppMode::Settings => {
                    render_settings(f, chunks[0], ctx.config, ctx.settings_selected_index, &theme);
                }
                crate::app::AppMode::Archive => {
                    render_archive(f, chunks[0], ctx.prs, ctx.selected_index, &theme);
                }
                crate::app::AppMode::Help => {
                    render_help(f, chunks[0], &theme);
                }
                crate::app::AppMode::Diagnostic => {
                    render_diagnostics(f, chunks[0], ctx.diagnostic_selected_index, &theme);
                }
                crate::app::AppMode::LogDetail => {
                    render_diagnostics(f, chunks[0], ctx.diagnostic_selected_index, &theme);
                    render_log_detail(f, f.area(), ctx.diagnostic_selected_index, &theme);
                }
                _ => {
                    let area = chunks[0];
                    let direction = if area.width < 120 { Direction::Vertical } else { Direction::Horizontal };
                    let main_chunks = Layout::default()
                        .direction(direction)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                        .split(area);

                    render_list(f, main_chunks[0], ctx.prs, ctx.selected_index, ctx.config, &theme);
                    render_detail(f, main_chunks[1], crate::ui::components::detail::DetailProps {
                        prs: ctx.prs,
                        selected_index: ctx.selected_index,
                        checks: ctx.checks,
                        timeline: ctx.timeline,
                        config: ctx.config,
                        theme: &theme,
                        detail_focused: ctx.detail_focused,
                        detail_scroll: ctx.detail_scroll,
                    });
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
                f.render_widget(text, chunks[1]);
            }

            if show_status {
                render_status_bar(f, chunks[2], ctx.mode, &theme, ctx.last_refresh, ctx.error_message);
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
        self.terminal.hide_cursor()?;
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen, crossterm::event::DisableMouseCapture)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use crate::domain::pr::{PullRequest, PRStatus, ReviewStatus, CIStatus};
    use crate::app::AppMode;

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
            ci_status: CIStatus::Passing,
            head_ref: "sha123".to_string(),
            body: "Body text".to_string(),
            url: "https://github.com/org/repo/pull/1".to_string(),
            requested_reviewers: vec![],
            reviewers: vec![],
            last_seen_at: None,
        }
    }

    #[test]
    fn test_draw_normal_mode() {
        let backend = TestBackend::new(80, 24);
        let mut renderer = Renderer::new(backend).unwrap();
        let prs = vec![create_test_pr()];
        let config = AppConfig::default();
        
        renderer.draw(DrawContext {
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
        }).unwrap();
    }
}
