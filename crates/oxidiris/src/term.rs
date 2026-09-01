//! Terminal capability detection.
//!
//! Implements OXD-022. See spec §3.4.4.
//!
//! No two terminals are alike, and a reading tool that assumes truecolor and full Unicode will
//! render as garbage on the terminals people actually have in front of them.

/// How much colour the terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorLevel {
    /// Monochrome: `NO_COLOR` is set, or `TERM=dumb`.
    None,
    /// The 16 ANSI colours.
    Ansi16,
    /// The 256-colour cube.
    Ansi256,
    /// 24-bit colour.
    TrueColor,
}

/// Whether the window is big enough for the layout we want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeClass {
    /// Comfortable: split view is possible.
    Full,
    /// Cramped: fall back to the single-panel focus mode.
    Narrow,
    /// Unusable: ask the reader to enlarge the window rather than drawing a broken layout.
    TooSmall,
}

/// Minimum width for the full layout.
pub const MIN_WIDTH: u16 = 80;
/// Minimum height for the full layout.
pub const MIN_HEIGHT: u16 = 24;
/// Absolute minimum width below which nothing is drawn.
pub const ABS_MIN_WIDTH: u16 = 30;
/// Absolute minimum height below which nothing is drawn.
///
/// Ten rows is what the reader frame plus the status block need before they start clipping.
pub const ABS_MIN_HEIGHT: u16 = 10;

/// Classify a terminal size.
pub const fn size_class(width: u16, height: u16) -> SizeClass {
    if width < ABS_MIN_WIDTH || height < ABS_MIN_HEIGHT {
        SizeClass::TooSmall
    } else if width < MIN_WIDTH || height < MIN_HEIGHT {
        SizeClass::Narrow
    } else {
        SizeClass::Full
    }
}

/// What the current terminal can render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Colour depth available.
    pub color: ColorLevel,
    /// Whether box-drawing and arrow glyphs are safe to emit.
    pub unicode: bool,
}

impl Capabilities {
    /// Detect capabilities from the process environment.
    pub fn detect() -> Self {
        Self::from_env(|key| std::env::var(key).ok())
    }

    /// Detect capabilities from an arbitrary environment lookup, so this is testable.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        let term = get("TERM").unwrap_or_default();
        let colorterm = get("COLORTERM").unwrap_or_default().to_ascii_lowercase();

        // NO_COLOR is honoured whenever it is present and not empty.
        // https://no-color.org
        let no_color = get("NO_COLOR").is_some_and(|v| !v.is_empty());

        let color = if no_color || term == "dumb" {
            ColorLevel::None
        } else if colorterm == "truecolor" || colorterm == "24bit" {
            ColorLevel::TrueColor
        } else if term.contains("256color") {
            ColorLevel::Ansi256
        } else if term.is_empty() {
            ColorLevel::None
        } else {
            ColorLevel::Ansi16
        };

        let locale = get("LC_ALL")
            .or_else(|| get("LC_CTYPE"))
            .or_else(|| get("LANG"))
            .unwrap_or_default()
            .to_ascii_uppercase();
        let unicode = term != "dumb" && (locale.contains("UTF-8") || locale.contains("UTF8"));

        Capabilities { color, unicode }
    }

    /// Marker glyphs framing the ORP character, above and below.
    ///
    /// These are not decoration. WCAG SC 1.4.1 forbids colour as the only carrier of information,
    /// so the anchor must remain identifiable when colour is unavailable or the reader is
    /// colour-blind (spec §3.4.1).
    pub const fn markers(self) -> (&'static str, &'static str) {
        if self.unicode { ("▼", "▲") } else { ("v", "^") }
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Capabilities { color: ColorLevel::Ansi16, unicode: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
        move |key: &str| map.get(key).cloned()
    }

    #[test]
    fn no_color_disables_colour_entirely() {
        let caps = Capabilities::from_env(env(&[
            ("NO_COLOR", "1"),
            ("COLORTERM", "truecolor"),
            ("TERM", "xterm-256color"),
        ]));
        assert_eq!(caps.color, ColorLevel::None);
    }

    #[test]
    fn empty_no_color_is_ignored() {
        let caps = Capabilities::from_env(env(&[("NO_COLOR", ""), ("COLORTERM", "truecolor")]));
        assert_eq!(caps.color, ColorLevel::TrueColor);
    }

    #[test]
    fn colour_depth_is_detected_in_order_of_preference() {
        assert_eq!(
            Capabilities::from_env(env(&[("COLORTERM", "truecolor"), ("TERM", "xterm")])).color,
            ColorLevel::TrueColor
        );
        assert_eq!(
            Capabilities::from_env(env(&[("TERM", "xterm-256color")])).color,
            ColorLevel::Ansi256
        );
        assert_eq!(Capabilities::from_env(env(&[("TERM", "xterm")])).color, ColorLevel::Ansi16);
    }

    #[test]
    fn dumb_terminal_gets_no_colour_and_no_unicode() {
        let caps = Capabilities::from_env(env(&[("TERM", "dumb"), ("LANG", "en_US.UTF-8")]));
        assert_eq!(caps.color, ColorLevel::None);
        assert!(!caps.unicode);
        assert_eq!(caps.markers(), ("v", "^"));
    }

    #[test]
    fn unicode_markers_need_a_utf8_locale() {
        let utf8 = Capabilities::from_env(env(&[("TERM", "xterm"), ("LANG", "en_US.UTF-8")]));
        assert!(utf8.unicode);
        assert_eq!(utf8.markers(), ("▼", "▲"));

        let latin = Capabilities::from_env(env(&[("TERM", "xterm"), ("LANG", "en_US.ISO-8859-1")]));
        assert!(!latin.unicode);
    }

    #[test]
    fn size_classes_match_the_documented_thresholds() {
        assert_eq!(size_class(120, 40), SizeClass::Full);
        assert_eq!(size_class(80, 24), SizeClass::Full);
        assert_eq!(size_class(79, 24), SizeClass::Narrow);
        assert_eq!(size_class(80, 23), SizeClass::Narrow);
        assert_eq!(size_class(40, 10), SizeClass::Narrow);
        assert_eq!(size_class(29, 10), SizeClass::TooSmall);
        assert_eq!(size_class(40, 9), SizeClass::TooSmall);
        assert_eq!(size_class(0, 0), SizeClass::TooSmall);
    }
}
