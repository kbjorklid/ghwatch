use comrak::{Arena, Options, nodes::NodeValue, parse_document};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[must_use]
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let arena = Arena::new();
    let root = parse_document(&arena, text, &Options::default());

    let mut lines = Vec::new();
    let mut current_line = Vec::new();

    fn walk<'a>(
        node: &'a comrak::nodes::AstNode<'a>,
        mut style: Style,
        current_line: &mut Vec<Span<'static>>,
        lines: &mut Vec<Line<'static>>,
    ) {
        match &node.data.borrow().value {
            NodeValue::Text(t) => {
                current_line.push(Span::styled(t.clone(), style));
            }
            NodeValue::Code(c) => {
                current_line.push(Span::styled(c.literal.clone(), style.bg(Color::DarkGray)));
            }
            NodeValue::Strong => {
                style = style.add_modifier(Modifier::BOLD);
            }
            NodeValue::Emph => {
                style = style.add_modifier(Modifier::ITALIC);
            }
            NodeValue::Heading(h) => {
                current_line.push(Span::styled(
                    "# ".repeat(h.level as usize),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
                style = style.add_modifier(Modifier::BOLD);
            }
            NodeValue::Link(_l) => {
                style = style.fg(Color::Blue);
            }
            NodeValue::Item(_i) => {
                current_line.push(Span::raw(" • "));
            }
            _ => {}
        }

        for child in node.children() {
            walk(child, style, current_line, lines);
        }

        match &node.data.borrow().value {
            NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::Item(_) => {
                if !current_line.is_empty() {
                    lines.push(Line::from(current_line.clone()));
                    current_line.clear();
                }

                // Add an empty line after paragraphs and headings, but not items
                if matches!(&node.data.borrow().value, NodeValue::Paragraph | NodeValue::Heading(_))
                {
                    // Check if the parent is an item to avoid double spacing in lists
                    let parent_is_item = node
                        .parent()
                        .is_some_and(|p| matches!(p.data.borrow().value, NodeValue::Item(_)));
                    if !parent_is_item {
                        lines.push(Line::from(""));
                    }
                }
            }
            _ => {}
        }
    }

    walk(root, Style::default(), &mut current_line, &mut lines);

    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn test_render_plain_text() {
        let lines = render_markdown("Hello world");
        assert!(!lines.is_empty());
        assert_eq!(lines[0].spans[0].content, "Hello world");
    }

    #[test]
    fn test_render_heading() {
        let lines = render_markdown("# Title");
        assert!(!lines.is_empty());
        assert_eq!(lines[0].spans[0].content, "# ");
        assert_eq!(lines[0].spans[1].content, "Title");
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Yellow));
        assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_render_bold() {
        let lines = render_markdown("Hello **bold** world");
        let bold_span =
            lines[0].spans.iter().find(|s| s.content == "bold").expect("bold span not found");
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_render_list() {
        let lines = render_markdown("- Item 1\n- Item 2");
        let has_bullet = lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("•") || s.content.contains('-')));
        assert!(has_bullet, "List items should have bullets or markers");
    }

    #[test]
    fn test_render_link() {
        let lines = render_markdown("[Google](https://google.com)");
        let link_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("Google"))
            .expect("link text not found");
        assert_eq!(link_span.style.fg, Some(Color::Blue));
    }
}
