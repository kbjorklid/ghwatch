use comrak::{markdown_to_commonmark, Options};

pub fn render_markdown(text: &str) -> String {
    // For Phase 1, we just return the text.
    // In a real implementation, we would parse with comrak and map AST to Ratatui Text.
    // For now, let's just ensure comrak is used to at least "format" it slightly.
    let mut options = Options::default();
    options.extension.autolink = true;
    
    // Just a placeholder for now.
    text.to_string()
}
