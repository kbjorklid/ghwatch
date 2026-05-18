#[derive(Debug)]
pub struct Icons {
    pub use_nerd_fonts: bool,
}

impl Icons {
    #[must_use]
    pub const fn new(use_nerd_fonts: bool) -> Self {
        Self { use_nerd_fonts }
    }

    #[must_use]
    pub const fn comment(&self) -> &str {
        if self.use_nerd_fonts { "󰆈" } else { "💬" }
    }

    #[must_use]
    pub const fn check_ok(&self) -> &str {
        if self.use_nerd_fonts { "󰄬" } else { "✓" }
    }

    #[must_use]
    pub const fn check_err(&self) -> &str {
        if self.use_nerd_fonts { "󰅖" } else { "✗" }
    }

    #[must_use]
    pub const fn additions(&self) -> &'static str {
        "+"
    }

    #[must_use]
    pub const fn deletions(&self) -> &'static str {
        "-"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icons_standard() {
        let icons = Icons::new(false);
        assert_eq!(icons.comment(), "💬");
        assert_eq!(icons.check_ok(), "✓");
        assert_eq!(icons.check_err(), "✗");
    }

    #[test]
    fn test_icons_nerd() {
        let icons = Icons::new(true);
        assert_eq!(icons.comment(), "󰆈");
        assert_eq!(icons.check_ok(), "󰄬");
        assert_eq!(icons.check_err(), "󰅖");
    }

    #[test]
    fn test_icons_diff() {
        let icons = Icons::new(false);
        assert_eq!(icons.additions(), "+");
        assert_eq!(icons.deletions(), "-");
    }
}
