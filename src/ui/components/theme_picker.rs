use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use ratatui_themes::{ThemeName, ThemePicker};

pub fn render_theme_picker(f: &mut Frame, area: Rect, selected_index: usize, theme: &Theme) {
    let themes = ThemeName::all();
    let clamped = selected_index.min(themes.len().saturating_sub(1));

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let items: Vec<ListItem> = themes.iter().map(|t| ListItem::new(t.display_name())).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title(" Theme (Enter: select, Esc: cancel) ")
                .title_style(Style::default().fg(theme.title)),
        )
        .highlight_style(Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg));

    let mut list_state = ListState::default();
    list_state.select(Some(clamped));
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    let preview_name = themes.get(clamped).copied().unwrap_or_default();
    let picker = ThemePicker::new(preview_name)
        .title(format!(" {} ", preview_name.display_name()))
        .instructions(" color preview ");
    f.render_widget(picker, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_theme_picker() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::from_name("dracula");

        terminal
            .draw(|f| {
                render_theme_picker(f, f.area(), 0, &theme);
            })
            .unwrap();
    }

    #[test]
    fn test_render_theme_picker_out_of_bounds_index() {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = Theme::from_name("dracula");

        terminal
            .draw(|f| {
                render_theme_picker(f, f.area(), 999, &theme);
            })
            .unwrap();
    }
}
