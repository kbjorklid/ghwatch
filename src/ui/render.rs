use crate::domain::pr::PullRequest;
use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};
use std::io::{self, Stdout};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub struct Renderer {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn init(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        self.terminal.hide_cursor()?;
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    pub fn draw(&mut self, prs: &[PullRequest], selected_index: usize) -> Result<()> {
        self.terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                .split(f.area());

            Self::render_list(f, chunks[0], prs, selected_index);
            Self::render_detail(f, chunks[1], prs, selected_index);
        })?;
        Ok(())
    }

    fn render_list(f: &mut ratatui::Frame, area: Rect, prs: &[PullRequest], selected_index: usize) {
        let icons = crate::ui::icons::Icons::new(false); // Default to false for now
        let items: Vec<ListItem> = prs
            .iter()
            .enumerate()
            .map(|(i, pr)| {
                let style = if i == selected_index {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Line 1: ID, Title, Status
                let line1 = Line::from(vec![
                    Span::styled(format!("#{} ", pr.number), Style::default().fg(Color::Gray)),
                    Span::styled(&pr.title, style),
                ]);

                // Line 2: Author, Age, Diff, Review, Comments
                let line2 = Line::from(vec![
                    Span::styled(format!("  {} ", pr.author), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} ", pr.updated_at), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{}{} {}{} ", icons.additions(), pr.additions, icons.deletions(), pr.deletions), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} ", pr.review_status), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{} {} ", icons.comment(), pr.comment_count), Style::default().fg(Color::DarkGray)),
                ]);

                ListItem::new(vec![line1, line2])
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Pull Requests "))
            .highlight_style(Style::default().bg(Color::DarkGray));

        f.render_widget(list, area);
    }

    fn render_detail(f: &mut ratatui::Frame, area: Rect, prs: &[PullRequest], selected_index: usize) {
        if let Some(pr) = prs.get(selected_index) {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" #{} - {} ", pr.number, pr.title));
            
            let paragraph = Paragraph::new(pr.body.as_str())
                .block(block)
                .wrap(Wrap { trim: true });
            
            f.render_widget(paragraph, area);
        } else {
            let block = Block::default().borders(Borders::ALL).title(" Detail ");
            f.render_widget(block, area);
        }
    }
}
