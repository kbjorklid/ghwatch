use crate::config::AppConfig;
use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

#[derive(Debug)]
pub enum SettingAction {
    None,
    ToggleNerdFonts,
    ToggleStatusBar,
    CycleTheme,
    ToggleColumn(crate::config::Column),
    ToggleQuery(usize),
    AddQuery,
}

#[must_use]
pub fn get_setting_action(config: &AppConfig, index: usize) -> SettingAction {
    match index {
        0 | 1 | 5 => SettingAction::None,
        2 => SettingAction::ToggleNerdFonts,
        3 => SettingAction::ToggleStatusBar,
        4 => SettingAction::CycleTheme,
        idx if (6..10).contains(&idx) => {
            let cols = [
                crate::config::Column::Author,
                crate::config::Column::Age,
                crate::config::Column::Diff,
                crate::config::Column::Comments,
            ];
            SettingAction::ToggleColumn(cols[idx - 6].clone())
        }
        idx if idx >= 10 && idx < 10 + config.queries.len() => SettingAction::ToggleQuery(idx - 10),
        idx if idx == 10 + config.queries.len() => SettingAction::AddQuery,
        _ => SettingAction::None,
    }
}

pub fn render_settings(
    f: &mut Frame,
    area: Rect,
    config: &AppConfig,
    selected_idx: usize,
    theme: &Theme,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(" Settings (Esc to exit) ")
        .title_style(Style::default().fg(theme.title));
    let mut text = Vec::new();

    let items = [
        ("Current User", config.current_user.clone()),
        ("Polling Interval", format!("{}ms", config.polling_interval_ms)),
        (
            "Nerd Fonts",
            if config.use_nerd_fonts { "Enabled".to_string() } else { "Disabled".to_string() },
        ),
        (
            "Status Bar",
            if config.show_status_bar { "Visible".to_string() } else { "Hidden".to_string() },
        ),
        ("Theme", config.theme.clone()),
        ("Unfollow Timeout", format!("{} mins", config.unfollow_timeout_mins)),
    ];

    for (i, (label, value)) in items.iter().enumerate() {
        let style = if i == selected_idx {
            Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg)
        } else {
            Style::default().fg(theme.text)
        };
        text.push(Line::from(vec![
            Span::styled(format!(" {label:<20}: "), style),
            Span::styled(value, style),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        " Columns (Space/Enter to toggle):",
        Style::default().fg(theme.title).add_modifier(Modifier::BOLD),
    )));

    let all_cols = [
        (crate::config::Column::Author, "Author"),
        (crate::config::Column::Age, "Age/Staleness"),
        (crate::config::Column::Diff, "Diff Size"),
        (crate::config::Column::Comments, "Comment Count"),
    ];

    for (i, (col, label)) in all_cols.iter().enumerate() {
        let idx = i + 6;
        let style = if idx == selected_idx {
            Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg)
        } else {
            Style::default().fg(theme.text)
        };
        let is_visible = config.visible_columns.contains(col);
        text.push(Line::from(vec![
            Span::styled(if is_visible { " [x] " } else { " [ ] " }, style),
            Span::styled(format!("{label:<20}"), style),
        ]));
    }

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        " Queries (Space/Enter to toggle):",
        Style::default().fg(theme.title).add_modifier(Modifier::BOLD),
    )));

    for (i, query) in config.queries.iter().enumerate() {
        let idx = i + 10;
        let style = if idx == selected_idx {
            Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg)
        } else {
            Style::default().fg(theme.text)
        };
        text.push(Line::from(vec![
            Span::styled(if query.enabled { " [x] " } else { " [ ] " }, style),
            Span::styled(format!("{}: ", query.name), style.fg(theme.info)),
            Span::styled(&query.search, style),
            Span::styled(format!(" ({})", query.interval), style.fg(theme.gray)),
        ]));
    }

    // Add Query button
    let add_query_idx = 10 + config.queries.len();
    let style = if add_query_idx == selected_idx {
        Style::default().bg(theme.highlight_bg).fg(theme.highlight_fg)
    } else {
        Style::default().fg(theme.text)
    };
    text.push(Line::from(vec![Span::styled(
        " [+] Add New Query...",
        style.fg(theme.success).add_modifier(Modifier::BOLD),
    )]));

    text.push(Line::from(""));
    text.push(Line::from(Span::styled(
        " Tip: Space/Enter to toggle settings. Edit config.toml for full control.",
        Style::default().fg(theme.warning),
    )));

    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_get_setting_action() {
        let mut config = AppConfig::default();
        config.queries = vec![crate::config::QueryConfig {
            name: "test".to_string(),
            search: "search".to_string(),
            interval: "1m".to_string(),
            enabled: true,
        }];

        assert!(matches!(get_setting_action(&config, 0), SettingAction::None));
        assert!(matches!(get_setting_action(&config, 2), SettingAction::ToggleNerdFonts));
        assert!(matches!(get_setting_action(&config, 4), SettingAction::CycleTheme));
        assert!(matches!(
            get_setting_action(&config, 6),
            SettingAction::ToggleColumn(crate::config::Column::Author)
        ));
        assert!(matches!(get_setting_action(&config, 10), SettingAction::ToggleQuery(0)));
        assert!(matches!(get_setting_action(&config, 11), SettingAction::AddQuery));
        assert!(matches!(get_setting_action(&config, 999), SettingAction::None));
    }

    #[test]
    fn test_render_settings() {
        let backend = TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let config = AppConfig::default();
        let theme = Theme::dark();

        terminal
            .draw(|f| {
                render_settings(f, f.area(), &config, 0, &theme);
            })
            .unwrap();
    }
}
