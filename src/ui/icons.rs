pub struct Icons {
    pub use_nerd_fonts: bool,
}

impl Icons {
    pub fn new(use_nerd_fonts: bool) -> Self {
        Self { use_nerd_fonts }
    }

    pub fn comment(&self) -> &str {
        if self.use_nerd_fonts { "󰆈" } else { "💬" }
    }

    pub fn check_ok(&self) -> &str {
        if self.use_nerd_fonts { "󰄬" } else { "✓" }
    }

    pub fn check_err(&self) -> &str {
        if self.use_nerd_fonts { "󰅖" } else { "✗" }
    }

    pub fn additions(&self) -> &str {
        "+"
    }

    pub fn deletions(&self) -> &str {
        "-"
    }
}
