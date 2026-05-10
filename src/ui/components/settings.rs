use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::Theme;
use crate::config::AppConfig;

pub enum SettingAction {
    None,
    ToggleNerdFonts,
    ToggleStatusBar,
    CycleTheme,
    ToggleColumn(crate::config::Column),
    ToggleQuery(usize),
}

pub fn get_setting_action(_config: &AppConfig, index: usize) -> SettingAction {
    match index {
        0 | 1 | 5 => SettingAction::None,
        2 => SettingAction::ToggleNerdFonts,
        3 => SettingAction::ToggleStatusBar,
        4 => SettingAction::CycleTheme,
        idx if (6..11).contains(&idx) => {
            let cols = [
                crate::config::Column::Author,
                crate::config::Column::Age,
                crate::config::Column::Diff,
                crate::config::Column::Review,
                crate::config::Column::Comments,
            ];
            SettingAction::ToggleColumn(cols[idx - 6].clone())
        }
        idx if idx >= 11 => {
            SettingAction::ToggleQuery(idx - 11)
        }
        _ => SettingAction::None,
    }
}

pub fn render_settings(f: &mut Frame, area: Rect, config: &AppConfig, selected_idx: usize, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Settings (Esc to exit) ")
        .title_style(Style::default().fg(theme.title));
    let mut text = Vec::new();

    let items = [
        ("Current User", config.current_user.clone()),
        ("Polling Interval", format!("{}ms", config.polling_interval_ms)),
        ("Nerd Fonts", if config.use_nerd_fonts { "Enabled".to_string() } else { "Disabled".to_string() }),
        ("Status Bar", if config.show_status_bar { "Visible".to_string() } else { "Hidden".to_string() }),
        ("Theme", config.theme.clone()),
        ("Unfollow Timeout", format!("{} mins", config.unfollow_timeout_mins)),
    ];

    for (i, (label, value)) in items.iter().enumerate() {
        let style = if i == selected_idx { Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg) } else { Style::default().fg(theme.text) };
        text.push(Line::from(vec![
            Span::styled(format!(" {:<20}: ", label), style),
            Span::styled(value, style),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(" Columns (Space/Enter to toggle):", Style::default().fg(theme.title).add_modifier(Modifier::BOLD))));
    
    let all_cols = [
        (crate::config::Column::Author, "Author"),
        (crate::config::Column::Age, "Age/Staleness"),
        (crate::config::Column::Diff, "Diff Size"),
        (crate::config::Column::Review, "Review Status"),
        (crate::config::Column::Comments, "Comment Count"),
    ];

    for (i, (col, label)) in all_cols.iter().enumerate() {
        let idx = i + 6;
        let style = if idx == selected_idx { Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg) } else { Style::default().fg(theme.text) };
        let is_visible = config.visible_columns.contains(col);
        text.push(Line::from(vec![
            Span::styled(if is_visible { " [x] " } else { " [ ] " }, style),
            Span::styled(format!("{:<20}", label), style),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(" Queries (Space/Enter to toggle):", Style::default().fg(theme.title).add_modifier(Modifier::BOLD))));

    for (i, query) in config.queries.iter().enumerate() {
        let idx = i + 11;
        let style = if idx == selected_idx { Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg) } else { Style::default().fg(theme.text) };
        text.push(Line::from(vec![
            Span::styled(if query.enabled { " [x] " } else { " [ ] " }, style),
            Span::styled(format!("{}: ", query.name), style.fg(theme.info)),
            Span::styled(&query.search, style),
            Span::styled(format!(" ({})", query.interval), style.fg(theme.gray)),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(" Tip: Space/Enter to toggle settings. Edit config.toml for full control.", Style::default().fg(theme.warning))));

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}
