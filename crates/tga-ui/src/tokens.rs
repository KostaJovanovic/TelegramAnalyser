//! The design tokens, in GPUI's colour type.
//!
//! Ported from `telegram_rust/crates/tgx-ui/src/tokens.rs`, and the values are
//! the ones `tga_report::palette` writes into the report's stylesheet — so the
//! window and the file it produces are visibly one product. `tests` below
//! asserts the two never drift, which is the same guard `test_analyser.py`
//! keeps over the Python pair.
//!
//! Swiss/International: black on white, hairline rules, square corners, one
//! red.
//!
//! **Both appearances carry the same keys**, enforced by the type rather than
//! by a test — a palette is a struct, so a colour that exists in light and not
//! in dark will not compile.

use gpui::{hsla, Hsla};

/// One appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// The page itself, behind everything.
    pub bg: Hsla,
    /// Body text.
    pub fg: Hsla,
    /// Secondary text.
    pub muted: Hsla,
    /// The 1px rule that does all the dividing. **Pure ink in light, mid grey
    /// in dark** — a dark theme with black hairlines shows nothing at all.
    pub hairline: Hsla,
    /// The softer divider.
    pub rule: Hsla,
    /// A raised fill.
    pub surface: Hsla,
    /// The one red.
    pub accent: Hsla,
    /// Text on the accent.
    pub accent_fg: Hsla,
}

/// Convert a `#rrggbb` literal to GPUI's colour type.
///
/// Written over the hex the stylesheet actually contains, so the tokens below
/// read as the CSS does and a transcription error is visible on the line it
/// happens.
fn hex(rgb: u32) -> Hsla {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    rgb_to_hsla(r, g, b, 1.0)
}

fn rgb_to_hsla(r: f32, g: f32, b: f32, a: f32) -> Hsla {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d.abs() < f32::EPSILON {
        return hsla(0.0, 0.0, l, a);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    hsla(h, s, l, a)
}

impl Palette {
    pub fn light() -> Self {
        Self {
            bg: hex(0xffffff),
            fg: hex(0x0a0a0a),
            muted: hex(0x6b6b6b),
            hairline: hex(0x0a0a0a),
            rule: hex(0xe6e6e6),
            surface: hex(0xf4f4f4),
            accent: hex(0xe60023),
            accent_fg: hex(0xffffff),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: hex(0x0a0a0a),
            fg: hex(0xe8e8e8),
            muted: hex(0x888888),
            hairline: hex(0x333333),
            rule: hex(0x262626),
            surface: hex(0x141414),
            accent: hex(0xff3347),
            accent_fg: hex(0xffffff),
        }
    }

    /// Anything but `light` falls back to dark, so a bad name cannot leave the
    /// window unreadable.
    pub fn named(name: &str) -> Self {
        if name == "light" {
            Self::light()
        } else {
            Self::dark()
        }
    }
}

/// The type scale.
pub mod type_scale {
    use gpui::{px, Pixels};

    pub const H1: Pixels = px(56.0);
    pub const H2: Pixels = px(28.0);
    pub const H3: Pixels = px(16.0);
    pub const BODY: Pixels = px(14.0);
    pub const SMALL: Pixels = px(13.0);
    pub const TINY: Pixels = px(11.0);
    pub const MICRO: Pixels = px(10.0);
}

/// Rhythm and tracking.
pub mod rhythm {
    pub const LINE_TIGHT: f32 = 1.2;
    pub const LINE_BODY: f32 = 1.5;
    pub const LINE_PROSE: f32 = 1.62;
    /// In em. GPUI has no letter-spacing property, so tracking is built out of
    /// layout — see `components::tracked`.
    pub const TRACK_CAPS: f32 = 0.08;
    pub const TRACK_MICRO: f32 = 0.18;
}

/// Metrics.
pub mod metrics {
    use gpui::{px, Pixels};

    pub const GAP: Pixels = px(24.0);
    /// `--radius: 0`. Square corners are the design, not a default.
    pub const RADIUS: Pixels = px(0.0);
    /// The floor the layout was measured at. The exporter's window drifted
    /// below its own and squeezed the only column that named the rows down to
    /// nothing; this one is small, but a floor costs nothing to state.
    pub const MIN_WINDOW: (f32, f32) = (620.0, 430.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The report's own value for a token, as it writes it into the stylesheet.
    fn from_report(theme: &str, name: &str) -> Hsla {
        let text = tga_report::palette::token(theme, name);
        hex(u32::from_str_radix(text.trim_start_matches('#'), 16).expect("hex"))
    }

    #[test]
    fn the_window_and_the_report_name_the_same_colours() {
        // The two copies exist because the report must not drag a GPU toolkit
        // into a module whose only job is to write a text file. This is what
        // stops them becoming two designs.
        for (theme, palette) in [("light", Palette::light()), ("dark", Palette::dark())] {
            for (name, ours) in [
                ("bg", palette.bg),
                ("fg", palette.fg),
                ("muted", palette.muted),
                ("hairline", palette.hairline),
                ("rule", palette.rule),
                ("surface", palette.surface),
                ("accent", palette.accent),
                ("accent_fg", palette.accent_fg),
            ] {
                assert_eq!(
                    ours,
                    from_report(theme, name),
                    "{theme}/{name} has drifted from the report's stylesheet"
                );
            }
        }
    }

    #[test]
    fn the_two_appearances_are_genuinely_different() {
        let (l, d) = (Palette::light(), Palette::dark());
        assert_ne!(l.bg, d.bg);
        assert_ne!(l.fg, d.fg);
        assert_ne!(l.accent, d.accent);
    }

    #[test]
    fn a_dark_hairline_is_not_black() {
        // A dark theme with black hairlines shows nothing at all.
        let (l, d) = (Palette::light(), Palette::dark());
        assert_eq!(l.hairline, l.fg, "light hairlines are the ink colour");
        assert!(
            d.hairline.l > d.bg.l,
            "the dark hairline must be lighter than the page"
        );
        assert!(d.hairline.l < d.fg.l, "and darker than the text");
    }

    #[test]
    fn hex_conversion_leaves_the_greys_achromatic() {
        let white = hex(0xffffff);
        assert!((white.l - 1.0).abs() < 1e-4, "got {white:?}");
        assert!(white.s.abs() < 1e-4, "white gained saturation: {white:?}");
    }

    #[test]
    fn the_accent_really_is_red() {
        let a = Palette::light().accent;
        assert!(a.h < 0.03 || a.h > 0.97, "hue was {}", a.h);
        assert!(a.s > 0.9, "saturation was {}", a.s);
    }

    #[test]
    fn an_unknown_theme_name_falls_back_rather_than_breaking() {
        assert_eq!(Palette::named("chartreuse"), Palette::dark());
        assert_eq!(Palette::named("light"), Palette::light());
    }

    #[test]
    fn corners_are_square() {
        assert_eq!(metrics::RADIUS, gpui::px(0.0));
    }
}
