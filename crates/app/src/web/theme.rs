/// Theme chosen by the user. `Auto` follows the OS via CSS `prefers-color-scheme`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Auto,
    Light,
    Dark,
}

impl Theme {
    /// Parse the `theme` cookie value; anything unknown => Auto.
    pub fn from_cookie(value: Option<&str>) -> Self {
        match value {
            Some("light") => Theme::Light,
            Some("dark") => Theme::Dark,
            _ => Theme::Auto,
        }
    }
    /// The `data-theme` attribute value, or empty for Auto (CSS handles OS default).
    pub fn data_attr(self) -> &'static str {
        match self {
            Theme::Auto => "",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_cookie() {
        assert_eq!(Theme::from_cookie(Some("dark")), Theme::Dark);
        assert_eq!(Theme::from_cookie(Some("zzz")), Theme::Auto);
        assert_eq!(Theme::from_cookie(None), Theme::Auto);
    }
}
