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
            if is_selected {
                style = style.fg(theme.highlight_fg);
            }
            
            if is_unread {
                style = style.add_modifier(Modifier::BOLD);
            }

            if needs_attention {
                style = style.fg(theme.error);
            }

            let unread_marker = if is_unread { "● " } else { "  " };
            let attention_marker = if needs_attention { "! " } else { "  " };

            let line1 = Line::from(vec![
                Span::styled(unread_marker, Style::default().fg(theme.info)),
                Span::styled(attention_marker, Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
                Span::styled(format!("#{} ", pr.number), Style::default().fg(theme.gray)),
                Span::styled(&pr.title, style),
            ]);

            let mut line2_spans = vec![Span::raw("  ")];
            for col in &config.visible_columns {
                match col {
                    crate::config::Column::Author => {
                        line2_spans.push(Span::styled(format!("{} ", pr.author), Style::default().fg(theme.gray)));
                    }
                    crate::config::Column::Age => {
                        line2_spans.push(Span::styled(format!("{} ", pr.updated_at), Style::default().fg(theme.gray)));
                    }
                    crate::config::Column::Diff => {
                        line2_spans.push(Span::styled(format!("{}{} {}{} ", icons.additions(), pr.additions, icons.deletions(), pr.deletions), Style::default().fg(theme.gray)));
                    }
                    crate::config::Column::Review => {
                        line2_spans.push(Span::styled(format!("{} ", pr.review_status), Style::default().fg(theme.gray)));
                    }
                    crate::config::Column::Comments => {
                        line2_spans.push(Span::styled(format!("{} {} ", icons.comment(), pr.comment_count), Style::default().fg(theme.gray)));
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
            .title_style(Style::default().fg(theme.title)))
        .highlight_style(Style::default().bg(theme.highlight_bg));
        
    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(list_selected_idx));

    f.render_stateful_widget(list, area, &mut state);
}
