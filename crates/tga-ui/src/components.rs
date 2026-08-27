//! The pieces the window is built from.
//!
//! Ported from `telegram_rust/crates/tgx-ui/src/components.rs`, trimmed to what
//! this window actually paints. Everything here speaks the Swiss language:
//! hairline rules instead of boxes, square corners, letterspaced uppercase
//! micro-type, one red.
//!
//! **Letter-spacing is not a GPUI property** — not on `Styled`, not on
//! `TextStyle` — so the design's tracking is built out of layout instead; see
//! [`tracked`]. Carrying the tokens without that is how `rhythm::TRACK_*` went
//! unapplied in the exporter for as long as its components claimed otherwise.

use crate::tokens::{metrics, rhythm, type_scale, Palette};
use gpui::prelude::*;
use gpui::{div, px, relative, Div, Hsla, SharedString};

/// A 1px rule — the design's core primitive.
///
/// **It must land on a device pixel.** On a GPU-scaled surface a 1px line can
/// straddle two physical pixels and blur, which reads as a rendering fault
/// rather than a style. GPUI's `px` is in logical units and the renderer snaps
/// borders, so this is the shape to keep everything going through rather than
/// hand-rolling borders at call sites.
pub fn rule(palette: &Palette) -> Div {
    div().h(px(1.0)).w_full().bg(palette.hairline)
}

/// The softer divider, for grouping inside a panel.
pub fn soft_rule(palette: &Palette) -> Div {
    div().h(px(1.0)).w_full().bg(palette.rule)
}

/// One unit of a letterspaced run: an inked cluster, or a gap between words.
///
/// A space is a *variant* rather than an `Ink(" ")`, because a `div` whose only
/// child is a single space has no reliable width — the shaper is free to trim
/// it, and the word gap then collapses to the tracking, which reads as one long
/// word.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Glyph {
    Ink(SharedString),
    Space,
}

/// Split a label into the units [`tracked`] lays out.
///
/// **A combining mark stays with the character it marks.** Splitting on
/// `chars()` alone would give a decomposed `ć` its own box one track-space to
/// the right of the `c`, so the accent floats between two letters. The fonts
/// here are the merged Latin+Cyrillic Geist build, so the Cyrillic marks
/// (U+0483..) matter as much as the Latin ones.
fn glyphs(text: &str) -> Vec<Glyph> {
    let mut out: Vec<Glyph> = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if is_combining(c) && !current.is_empty() {
            current.push(c);
            continue;
        }
        if !current.is_empty() {
            out.push(Glyph::Ink(std::mem::take(&mut current).into()));
        }
        if c.is_whitespace() {
            out.push(Glyph::Space);
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        out.push(Glyph::Ink(current.into()));
    }
    out
}

/// The combining ranges, spelled out because `std` has no character-class query
/// and this crate takes no dependencies to get one.
fn is_combining(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F      // combining diacritical marks
        | 0x0483..=0x0489    // Cyrillic
        | 0x0591..=0x05BD    // Hebrew points
        | 0x1AB0..=0x1AFF
        | 0x1DC0..=0x1DFF
        | 0x20D0..=0x20F0    // combining marks for symbols
        | 0xFE20..=0xFE2F)
}

/// How wide a word gap is, as a fraction of the type size.
///
/// The empty spacer cannot inherit the font's own space advance, so it is set
/// here; 0.32em is close to Geist's and reads as one word gap rather than two.
const SPACE_EM: f32 = 0.32;

/// A letterspaced run of text.
///
/// One child per character, with the tracking as the flex `gap`. That is worth
/// the children because the whole design language is letterspaced uppercase
/// micro-type.
///
/// What it costs, and why these must stay **painted labels**: the text is no
/// longer one text run, so it cannot be selected, cannot be searched by the
/// platform, and will not wrap — a tracked string that outgrows its box is
/// clipped, not broken. Never put body copy or a user-supplied string of
/// unknown length through here.
///
/// The gap falls *between* glyphs and not after the last one, unlike CSS
/// `letter-spacing`, which leaves a trailing space on every label. A tracked
/// label therefore ends flush and still aligns to a rule beside it.
pub fn tracked(text: impl Into<SharedString>, size: gpui::Pixels, track_em: f32) -> Div {
    let text: SharedString = text.into();
    let track = px(f32::from(size) * track_em);
    let space = px(f32::from(size) * SPACE_EM);
    let mut row = div()
        .flex()
        .flex_row()
        .items_baseline()
        .text_size(size)
        // One line by construction. The body ratio would pad the row and push
        // the label off the baseline it shares with whatever sits next to it.
        .line_height(leading(size, rhythm::LINE_TIGHT))
        .gap(track);
    for glyph in glyphs(&text) {
        row = row.child(match glyph {
            // `flex_none` on every box: in a tight row the shrink would take
            // the letters, not the row, and the tracking would go uneven before
            // anything visibly overflowed.
            Glyph::Ink(s) => div().flex_none().child(s),
            Glyph::Space => div().flex_none().w(space),
        });
    }
    row
}

/// A letterspaced uppercase micro-heading — `MICRO` at `TRACK_MICRO`.
pub fn eyebrow(text: impl Into<SharedString>, palette: &Palette) -> Div {
    tracked(uppercase(text), type_scale::MICRO, rhythm::TRACK_MICRO).text_color(palette.muted)
}

/// Letterspaced uppercase caption at an arbitrary size — `TRACK_CAPS`.
///
/// Takes its colour rather than a palette: these label things that are
/// sometimes muted, sometimes the accent.
pub fn caps(text: impl Into<SharedString>, size: gpui::Pixels, colour: Hsla) -> Div {
    tracked(uppercase(text), size, rhythm::TRACK_CAPS).text_color(colour)
}

/// Uppercase the caller's own string.
///
/// The stylesheet does this with `text-transform`; there is no equivalent here,
/// so it is done to the text.
pub fn uppercase(text: impl Into<SharedString>) -> SharedString {
    let s: SharedString = text.into();
    s.to_uppercase().into()
}

/// A hairline square, filled when ticked.
///
/// **Disabled is a muted border, never a missing one.** A control that is off
/// and a control that is unavailable must not paint the same, or the only way
/// to tell them apart is to click and watch nothing happen.
///
/// `flex_none`, because in a row with a long label the flex shrink comes out of
/// the 12px box first and the tick vanishes before the label does.
pub fn tick_box(ticked: bool, enabled: bool, palette: &Palette) -> Div {
    let ink = if enabled { palette.fg } else { palette.muted };
    let border = if enabled {
        palette.hairline
    } else {
        palette.muted
    };
    div()
        .flex_none()
        .w(px(12.0))
        .h(px(12.0))
        .rounded(metrics::RADIUS)
        .border_1()
        .border_color(border)
        .when(ticked, |d| d.bg(ink))
}

/// The share of the track an indeterminate bar paints.
const INDETERMINATE_FILL: f32 = 0.12;

/// The fraction actually painted, given what the caller knows.
///
/// **A bar reading 0% and a bar meaning "unknown" are different states.** The
/// first says nothing has happened yet, which is true and useful; the second
/// says the run has started and its size is not known, and painting it as 0%
/// makes a working run look stuck. A non-finite fraction — an `n as f32 / total
/// as f32` with `total` zero — paints empty rather than propagating a NaN into
/// the layout.
fn bar_fill(fraction: Option<f32>) -> f32 {
    match fraction {
        None => INDETERMINATE_FILL,
        Some(f) if f.is_finite() => f.clamp(0.0, 1.0),
        Some(_) => 0.0,
    }
}

/// One progress bar. `None` is *indeterminate*.
///
/// 3px tall, as the original's `setFixedHeight(3)`: the bar is a status line,
/// not a widget, and anything taller starts competing with the type.
pub fn progress_bar(fraction: Option<f32>, palette: &Palette) -> Div {
    div()
        .w_full()
        .h(px(3.0))
        .rounded(metrics::RADIUS)
        .bg(palette.rule)
        .child(
            div()
                .h_full()
                .w(relative(bar_fill(fraction)))
                .bg(palette.accent),
        )
}

/// A square-cornered hairline button.
///
/// The primary is the one red filled; everything else is an outline. Disabled
/// is muted ink on a muted border — visible, and visibly unavailable.
pub fn button(
    label: impl Into<SharedString>,
    enabled: bool,
    primary: bool,
    palette: &Palette,
) -> Div {
    let mut b = div()
        .flex_none()
        .px(px(16.0))
        .py(px(7.0))
        .rounded(metrics::RADIUS)
        .border_1()
        .text_size(type_scale::BODY)
        .line_height(leading(type_scale::BODY, rhythm::LINE_TIGHT))
        .child(label.into());
    b = match (enabled, primary) {
        (false, _) => b.border_color(palette.muted).text_color(palette.muted),
        (true, true) => b
            .border_color(palette.accent)
            .bg(palette.accent)
            .text_color(palette.accent_fg),
        (true, false) => b.border_color(palette.hairline).text_color(palette.fg),
    };
    b
}

/// Line height in pixels for a given size.
///
/// GPUI's `line_height` takes a length and the design's rhythm is ratios. This
/// is the one place the two meet, so a hand-multiplied leading never drifts
/// from the token it came from.
pub fn leading(size: gpui::Pixels, ratio: f32) -> gpui::Pixels {
    px(f32::from(size) * ratio)
}

/// The window's floor.
pub fn min_window() -> (f32, f32) {
    metrics::MIN_WINDOW
}

/// A count, grouped for reading, as the report's own `thousands` does.
///
/// The separator is **U+00A0**: a group separator that wraps puts "6" at the
/// end of one line and "643" at the start of the next.
pub fn thousands(n: i64) -> String {
    let negative = n < 0;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if negative {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{a0}');
        }
        out.push(ch);
    }
    out
}

/// A path shown in a field too narrow for it.
///
/// **Shows the start, not the end.** A field scrolled to its caret renders
/// `N:\telegram export\UA KOLAB TELEGRAM` as `…\UA KOLAB TELEGRAM` at best and
/// as a drive letter sliced in half at worst, which reads as a typo.
pub fn elided_start(path: &str, max_chars: usize) -> String {
    let count = path.chars().count();
    if count <= max_chars {
        return path.to_string();
    }
    let kept: String = path.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combining_mark_stays_with_the_letter_it_marks() {
        // Decomposed "ć" — c followed by U+0301. Split apart, the accent gets
        // its own box one track-space to the right and floats between letters.
        let parts = glyphs("c\u{301}a");
        assert_eq!(
            parts,
            vec![Glyph::Ink("c\u{301}".into()), Glyph::Ink("a".into())]
        );
    }

    #[test]
    fn a_word_gap_is_a_spacer_and_not_a_glyph_holding_a_space() {
        // A div whose only child is a space has no reliable width: the shaper
        // may trim it, and the gap collapses to the tracking.
        assert_eq!(
            glyphs("a b"),
            vec![Glyph::Ink("a".into()), Glyph::Space, Glyph::Ink("b".into())]
        );
    }

    #[test]
    fn an_empty_label_produces_no_glyphs_at_all() {
        assert!(glyphs("").is_empty());
    }

    #[test]
    fn an_indeterminate_bar_is_not_an_empty_one() {
        // A bar reading 0% says nothing has happened; unknown says the run
        // started and its size is not known. Painting them the same makes a
        // working run look stuck.
        assert_eq!(bar_fill(None), INDETERMINATE_FILL);
        assert_eq!(bar_fill(Some(0.0)), 0.0);
    }

    #[test]
    fn a_nan_fraction_paints_empty_rather_than_poisoning_the_layout() {
        // `0 as f32 / 0 as f32` is the shape that produces this.
        assert_eq!(bar_fill(Some(f32::NAN)), 0.0);
        assert_eq!(bar_fill(Some(f32::INFINITY)), 0.0);
        assert_eq!(bar_fill(Some(2.0)), 1.0);
        assert_eq!(bar_fill(Some(-1.0)), 0.0);
    }

    #[test]
    fn a_path_is_elided_at_its_end_so_its_start_stays_readable() {
        let path = r"N:\telegram export\UA KOLAB TELEGRAM";
        let short = elided_start(path, 20);
        assert!(short.starts_with(r"N:\telegram"), "got {short}");
        assert!(short.ends_with('…'));
        assert_eq!(short.chars().count(), 20);
        assert_eq!(elided_start(path, 200), path);
    }

    #[test]
    fn a_group_separator_is_a_non_breaking_space() {
        assert_eq!(thousands(333_582), "333\u{a0}582");
        assert_eq!(thousands(0), "0");
    }

    #[test]
    fn leading_is_derived_from_the_token_rather_than_hand_multiplied() {
        assert_eq!(
            leading(type_scale::BODY, rhythm::LINE_TIGHT),
            px(f32::from(type_scale::BODY) * 1.2)
        );
    }
}
