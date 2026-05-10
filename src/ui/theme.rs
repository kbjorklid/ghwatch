use ratatui::style::Color;

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
    pub fn dark() -> Self {
        Self {
            border: Color::Gray,
            title: Color::White,
            text: Color::White,
            gray: Color::DarkGray,
            highlight_bg: Color::DarkGray,
            highlight_fg: Color::Yellow,
            success: Color::Green,
            error: Color::Red,
            warning: Color::Yellow,
            info: Color::Cyan,
        }
    }

    pub fn nord() -> Self {
        Self {
            border: Color::Rgb(76, 86, 106),
            title: Color::Rgb(236, 239, 244),
            text: Color::Rgb(216, 222, 233),
            gray: Color::Rgb(76, 86, 106),
            highlight_bg: Color::Rgb(59, 66, 82),
            highlight_fg: Color::Rgb(136, 192, 208),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(191, 97, 106),
            warning: Color::Rgb(235, 203, 139),
            info: Color::Rgb(129, 161, 193),
        }
    }

    pub fn dracula() -> Self {
        Self {
            border: Color::Rgb(98, 114, 164),
            title: Color::Rgb(248, 248, 242),
            text: Color::Rgb(248, 248, 242),
            gray: Color::Rgb(98, 114, 164),
            highlight_bg: Color::Rgb(68, 71, 90),
            highlight_fg: Color::Rgb(189, 147, 249),
            success: Color::Rgb(80, 250, 123),
            error: Color::Rgb(255, 85, 85),
            warning: Color::Rgb(241, 250, 140),
            info: Color::Rgb(139, 233, 253),
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            _ => Self::dark(),
        }
    }
}
