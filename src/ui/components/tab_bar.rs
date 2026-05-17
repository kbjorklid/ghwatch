use crate::app::AppMode;
use crate::ui::theme::Theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

#[must_use]
pub const fn active_tab(mode: &AppMode) -> usize {
    match mode {
        AppMode::Normal | AppMode::Search | AppMode::Follow | AppMode::Help => 0,
        AppMode::Archive => 1,
        AppMode::Settings
        | AppMode::Diagnostic
        | AppMode::LogDetail
        | AppMode::AddQueryName
        | AppMode::AddQuerySearch
        | AppMode::ConfirmQuery
        | AppMode::DeleteQueryConfirm
        | AppMode::ThemePicker => 2,
    }
}

pub fn render_tab_bar(f: &mut Frame, area: Rect, mode: &AppMode, theme: &Theme) {
    let current = active_tab(mode);
    let labels = [" PRs ", " Archive ", " Settings "];
    let sep = Span::styled(" │ ", Style::default().fg(theme.gray));

    let mut spans = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            spans.push(sep.clone());
        }
        if i == current {
            spans.push(Span::styled(
                *label,
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            spans.push(Span::styled(*label, Style::default().fg(theme.gray)));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
