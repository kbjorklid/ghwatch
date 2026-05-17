use ratatui::style::Color;
use ratatui_themes::ThemeName;

#[derive(Debug)]
pub struct Theme {
    pub border: Color,
    pub title: Color,
    pub text: Color,
    pub gray: Color,
    pub highlight_bg: Color,
    pub highlight_fg: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
}

impl Theme {
    #[must_use]
    pub fn dark() -> Self {
        Self::from_name("one-dark-pro")
    }

    #[must_use]
    pub fn nord() -> Self {
        Self::from_name("nord")
    }

    #[must_use]
    pub fn dracula() -> Self {
        Self::from_name("dracula")
    }

    #[must_use]
    pub fn from_name(name: &str) -> Self {
        let p = name.parse::<ThemeName>().unwrap_or_default().palette();
        Self {
            border: p.muted,
            title: p.fg,
            text: p.fg,
            gray: p.muted,
            highlight_bg: p.selection,
            highlight_fg: p.accent,
            success: p.success,
            error: p.error,
            warning: p.warning,
            info: p.info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_colors_are_distinct() {
        let theme = Theme::from_name("dracula");
        assert_ne!(theme.gray, theme.highlight_bg, "muted and selection should differ");
        assert_ne!(theme.success, theme.error, "success and error should differ");
    }

    #[test]
    fn test_from_name_known_themes_differ() {
        let nord = Theme::from_name("nord");
        let dracula = Theme::from_name("dracula");
        assert_ne!(
            nord.highlight_fg, dracula.highlight_fg,
            "nord and dracula accent colors should differ"
        );
    }

    #[test]
    fn test_from_name_unknown_falls_back_to_dracula() {
        let unknown = Theme::from_name("dark");
        let dracula = Theme::from_name("dracula");
        assert_eq!(unknown.info, dracula.info, "unknown name should fall back to Dracula");
    }

    #[test]
    fn test_all_themes_load_without_panic() {
        for name in ThemeName::all() {
            let theme = Theme::from_name(name.slug());
            assert_ne!(theme.text, theme.highlight_bg);
        }
    }
}
