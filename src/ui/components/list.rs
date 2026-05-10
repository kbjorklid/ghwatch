use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use crate::ui::theme::Theme;
use crate::domain::pr::PullRequest;
use crate::config::AppConfig;
use crate::ui::icons::Icons;
use crate::domain::pr_list::get_grouped_items;

pub fn render_list(f: &mut Frame, area: Rect, prs: &[PullRequest], selected_index: usize, config: &AppConfig, theme: &Theme) {
    let icons = Icons::new(config.use_nerd_fonts);
    let grouped_items = get_grouped_items(prs, config);
    
    let mut items = Vec::new();
    let mut current_idx = 0;
    let mut list_selected_idx = 0;

    for group in grouped_items {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!(" ▼ {} ", group.name), Style::default().bg(theme.info).fg(Color::Black).add_modifier(Modifier::BOLD)),
        ])));
        
        for pr in group.prs {
            let is_selected = current_idx == selected_index;
            if is_selected {
                list_selected_idx = items.len();
            }

            let is_unread = crate::domain::lifecycle::is_unread(pr);
            let needs_attention = crate::domain::rules::needs_attention(pr, &config.current_user);

            let mut style = Style::default().fg(theme.text);
            let mut bg_style = Style::default();
            if is_selected {
                style = style.fg(theme.highlight_fg);
                bg_style = bg_style.bg(theme.highlight_bg);
            }
            
            if is_unread {
                style = style.add_modifier(Modifier::BOLD);
            }

            if needs_attention {
                style = style.fg(theme.error);
            }

            let unread_marker = if is_unread { "● " } else { "  " };
            let attention_marker = if needs_attention { "! " } else { "  " };

            let status_color = match pr.status {
                crate::domain::pr::PRStatus::Open => theme.success,
                crate::domain::pr::PRStatus::Merged => theme.info,
                crate::domain::pr::PRStatus::Closed => theme.gray,
            };

            let status_str = format!(" {} ", pr.status);
            let id_str = format!("#{} ", pr.number);
            
            // Calculate available width for title
            // area.width - 2 (borders) - 2 (unread) - 2 (attention) - id_str.len() - status_str.len()
            let used_width = 2 + 2 + 2 + id_str.len() + status_str.len();
            let available_title_width = area.width.saturating_sub(used_width as u16);
            
            let display_title = if pr.title.chars().count() > available_title_width as usize {
                let mut t: String = pr.title.chars().take(available_title_width.saturating_sub(1) as usize).collect();
                t.push('…');
                t
            } else {
                pr.title.clone()
            };

            let padding_len = available_title_width.saturating_sub(display_title.chars().count() as u16) as usize;
            let line1 = Line::from(vec![
                Span::styled(unread_marker, bg_style.fg(theme.info)),
                Span::styled(attention_marker, bg_style.fg(theme.error).add_modifier(Modifier::BOLD)),
                Span::styled(id_str, bg_style.fg(theme.gray)),
                Span::styled(display_title, bg_style.patch(style)),
                Span::styled(" ".repeat(padding_len), bg_style.patch(style)),
                Span::styled(status_str, Style::default().bg(status_color).fg(Color::Black).add_modifier(Modifier::BOLD)),
            ]);

            let mut line2_spans = vec![Span::styled("  ", bg_style)];
            for col in &config.visible_columns {
                match col {
                    crate::config::Column::Author => {
                        line2_spans.push(Span::styled(format!("{} ", pr.author), bg_style.fg(theme.gray)));
                    }
                    crate::config::Column::Age => {
                        line2_spans.push(Span::styled(format!("{} ", pr.updated_at), bg_style.fg(theme.gray)));
                    }
                    crate::config::Column::Diff => {
                        line2_spans.push(Span::styled(format!("{}{} ", icons.additions(), pr.additions), bg_style.fg(theme.success)));
                        line2_spans.push(Span::styled(format!("{}{} ", icons.deletions(), pr.deletions), bg_style.fg(theme.error)));
                    }
                    crate::config::Column::Review => {
                        line2_spans.push(Span::styled(format!("{} ", pr.review_status), bg_style.fg(theme.gray)));
                    }
                    crate::config::Column::Comments => {
                        line2_spans.push(Span::styled(format!("{} {} ", icons.comment(), pr.comment_count), bg_style.fg(theme.gray)));
                    }
                }
            }

            items.push(ListItem::new(vec![line1, Line::from(line2_spans)]));
            current_idx += 1;
        }
    }

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(" Pull Requests ")
            .title_style(Style::default().fg(theme.title)));
        
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(list_selected_idx));

    f.render_stateful_widget(list, area, &mut state);
}
