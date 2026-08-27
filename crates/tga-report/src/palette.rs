//! The report's colour, taken from the exporter's own design language.
//!
//! Ported from `analyser/palette.py`.
//!
//! `telegram_rust`'s `tgx-ui/src/tokens.rs` holds the same set of tokens for
//! the window, so the report and the app that produced it look like one
//! product. The values are mirrored rather than imported, because this module's
//! only job is to write a text file and it should not drag a GPU toolkit in to
//! do it; `tga-ui` mirrors them in the other direction and
//! `tga_ui::tokens::tests` asserts the two never drift.
//!
//! **One hue does all the work.** The design has two colours and one red, which
//! rules out a categorical palette — and that turns out to cost nothing,
//! because almost every figure in this report is *magnitude*, whose correct
//! encoding is a single-hue sequential ramp anyway. Where something genuinely
//! needs telling apart by identity, the report uses small multiples on a shared
//! axis rather than inventing hues the design does not have.
//!
//! The ramp is not eyeballed. It is generated in OKLab, holding the accent's
//! hue, with chroma clamped to the sRGB gamut at each step so the lightness
//! steps come out even, and it passes four ordinal checks in both modes:
//!
//! ```text
//! ramp             light                          dark
//! monotone L       yes                            yes
//! adjacent dL      0.073 (floor 0.06)             0.082
//! end vs surface   2.37:1 (floor 2:1)             2.33:1
//! hue spread       1 degree                       1 degree
//! ```
//!
//! The end step is what sets the span. On white, anything paler than 2:1 stops
//! being a mark and becomes surface; on near-black the same is true going down.
//! That is why the ramp runs past the accent into deeper red rather than
//! starting at it — the accent is a step of the hue's ramp, not its end.
//!
//! `tests` below re-derives all four from the hex values, so an edit that
//! breaks one fails the suite instead of quietly shipping a ramp nobody can
//! read.

/// One theme's tokens, **in emission order**.
///
/// A slice of pairs rather than a map: the stylesheet is written by walking
/// this, and a `BTreeMap` would sort the custom properties alphabetically. That
/// changes no pixel and every line of the parity diff.
pub type Tokens = &'static [(&'static str, &'static str)];

/// Mirrors `app/ui/theme.py::PALETTES`, plus the chart-only tokens.
pub const LIGHT: Tokens = &[
    ("bg", "#ffffff"),
    ("fg", "#0a0a0a"),
    ("muted", "#6b6b6b"),
    ("hairline", "#0a0a0a"),
    ("rule", "#e6e6e6"),
    ("surface", "#f4f4f4"),
    ("accent", "#e60023"),
    ("accent_fg", "#ffffff"),
    // Chart-only. `track` is an empty cell — a heatmap needs somewhere for zero
    // to live that is not the lightest step of the ramp, or a silent day and a
    // quiet day look the same.
    ("track", "#efefef"),
    // Not called `grid`: the exporter's palette uses that name for the window's
    // backdrop texture, which is a different thing at a different value, and
    // the drift check compares shared names.
    ("gridline", "#ececec"),
];

pub const DARK: Tokens = &[
    ("bg", "#0a0a0a"),
    ("fg", "#e8e8e8"),
    ("muted", "#888888"),
    ("hairline", "#333333"),
    ("rule", "#262626"),
    ("surface", "#141414"),
    ("accent", "#ff3347"),
    ("accent_fg", "#ffffff"),
    ("track", "#1c1c1c"),
    ("gridline", "#222222"),
];

/// Light-to-dark, magnitude increasing. Validated; see the module docs.
pub const RAMP_LIGHT: &[&str] = &["#e8918a", "#de7068", "#d24b47", "#c41422", "#a10a19"];
pub const RAMP_DARK: &[&str] = &["#813435", "#a83f41", "#d14a4e", "#fc555b", "#fd8b88"];

pub const DEFAULT: &str = "dark";

/// The named theme, falling back to [`DEFAULT`] for anything else.
pub fn tokens(theme: &str) -> Tokens {
    match theme {
        "light" => LIGHT,
        _ => DARK,
    }
}

pub fn ramp(theme: &str) -> &'static [&'static str] {
    match theme {
        "light" => RAMP_LIGHT,
        _ => RAMP_DARK,
    }
}

/// Whether a name is a theme this module has.
pub fn known(theme: &str) -> bool {
    theme == "light" || theme == "dark"
}

pub fn token(theme: &str, name: &str) -> &'static str {
    tokens(theme)
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
        .unwrap_or("#000000")
}

/// The ramp colour for `value` out of `top`.
///
/// Zero is the track, never the ramp's first step: a day nobody spoke and a day
/// one person spoke are different facts and must not share a colour.
pub fn step(theme: &str, value: f64, top: f64) -> &'static str {
    if value <= 0.0 || top <= 0.0 {
        return token(theme, "track");
    }
    let ramp = ramp(theme);
    // Rank by share of the maximum, floored into the first step so a single
    // message is always visible. Linear: a heatmap that quietly applies a
    // square root reports a busy cell as busier than it was.
    let index = ((value / top * ramp.len() as f64) as usize).min(ramp.len() - 1);
    ramp[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(hex: &str) -> (f64, f64, f64) {
        let n = u32::from_str_radix(hex.trim_start_matches('#'), 16).expect("hex");
        (
            ((n >> 16) & 0xff) as f64 / 255.0,
            ((n >> 8) & 0xff) as f64 / 255.0,
            (n & 0xff) as f64 / 255.0,
        )
    }

    fn linear(c: f64) -> f64 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// sRGB -> OKLab, Björn Ottosson's matrices.
    fn oklab(hex: &str) -> (f64, f64, f64) {
        let (r, g, b) = rgb(hex);
        let (r, g, b) = (linear(r), linear(g), linear(b));
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        (
            0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
        )
    }

    fn hue_degrees(hex: &str) -> f64 {
        let (_, a, b) = oklab(hex);
        b.atan2(a).to_degrees().rem_euclid(360.0)
    }

    fn relative_luminance(hex: &str) -> f64 {
        let (r, g, b) = rgb(hex);
        0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let (x, y) = (relative_luminance(a), relative_luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// The four checks the module docs claim, re-derived from the hex values.
    ///
    /// Stated as a test rather than as prose because the numbers are the whole
    /// argument for the ramp being this and not something prettier, and prose
    /// does not fail.
    fn check_ramp(theme: &str) {
        let ramp = ramp(theme);
        let surface = token(theme, "bg");

        let lightness: Vec<f64> = ramp.iter().map(|c| oklab(c).0).collect();

        // 1. Monotone L. Light runs pale-to-deep, dark runs deep-to-pale;
        //    either way it may never turn around, or two adjacent steps stop
        //    being orderable.
        let descending = lightness[0] > lightness[lightness.len() - 1];
        for pair in lightness.windows(2) {
            if descending {
                assert!(
                    pair[0] > pair[1],
                    "{theme}: L is not monotone: {lightness:?}"
                );
            } else {
                assert!(
                    pair[0] < pair[1],
                    "{theme}: L is not monotone: {lightness:?}"
                );
            }
        }

        // 2. Adjacent dL, floor 0.06. Below this two steps read as one.
        for pair in lightness.windows(2) {
            let delta = (pair[0] - pair[1]).abs();
            assert!(
                delta >= 0.06,
                "{theme}: adjacent dL {delta:.3} is under 0.06"
            );
        }

        // 3. The end step against the surface, floor 2:1. Below it the mark
        //    stops being a mark and becomes background.
        let end = contrast(ramp[ramp.len() - 1], surface);
        assert!(
            end >= 2.0,
            "{theme}: end vs surface is {end:.2}:1, under 2:1"
        );
        // And the other end, which is the one that actually binds: on white the
        // palest step is the one at risk.
        let start = contrast(ramp[0], surface);
        assert!(
            start >= 2.0,
            "{theme}: first step vs surface is {start:.2}:1, under 2:1"
        );

        // 4. Hue spread. This is a *single-hue* ramp; a step that drifts in hue
        //    encodes a category the data does not have.
        let hues: Vec<f64> = ramp.iter().map(|c| hue_degrees(c)).collect();
        let spread = hues.iter().cloned().fold(f64::MIN, f64::max)
            - hues.iter().cloned().fold(f64::MAX, f64::min);
        assert!(spread <= 12.0, "{theme}: hue spreads {spread:.1} degrees");
    }

    #[test]
    fn the_light_ramp_passes_all_four_ordinal_checks() {
        check_ramp("light");
    }

    #[test]
    fn the_dark_ramp_passes_all_four_ordinal_checks() {
        check_ramp("dark");
    }

    #[test]
    fn both_themes_carry_the_same_token_names_in_the_same_order() {
        // A token present in one appearance and not the other is a report that
        // renders in light and falls apart in dark, and the stylesheet emits
        // both blocks from this list.
        let light: Vec<&str> = LIGHT.iter().map(|(k, _)| *k).collect();
        let dark: Vec<&str> = DARK.iter().map(|(k, _)| *k).collect();
        assert_eq!(light, dark);
    }

    #[test]
    fn zero_is_the_track_and_never_the_ramps_first_step() {
        // The stated property. A day nobody spoke and a day one person spoke
        // are different facts.
        assert_eq!(step("dark", 0.0, 100.0), token("dark", "track"));
        assert_eq!(step("dark", 5.0, 0.0), token("dark", "track"));
        assert_ne!(step("dark", 1.0, 100.0), token("dark", "track"));
    }

    #[test]
    fn the_top_value_lands_on_the_last_step_rather_than_past_it() {
        // `value / top * len` is exactly `len` at the maximum, which indexes
        // one past the end. The clamp is what stops that being a panic.
        assert_eq!(step("light", 100.0, 100.0), RAMP_LIGHT[4]);
        assert_eq!(step("light", 1.0, 100.0), RAMP_LIGHT[0]);
        assert_eq!(step("light", 50.0, 100.0), RAMP_LIGHT[2]);
    }

    #[test]
    fn an_unknown_theme_falls_back_to_the_default() {
        assert!(!known("solarized"));
        assert_eq!(tokens("solarized"), tokens(DEFAULT));
    }
}
