//! Repaint the one borrowed control in the design's colours.
//!
//! Ported from `telegram_rust/crates/tgx-app/src/theme.rs`, trimmed to the
//! fields this window can actually reach — it borrows a single text field, not
//! a whole widget set.
//!
//! `gpui_component` keeps its colours in `Theme`, a gpui `Global`. Left alone,
//! that global is the library's own palette, and a field painted in it beside
//! our hairline rules is the most visible way two design languages can
//! disagree: rounded corners, a grey tray fill, and a blue focus ring.
//!
//! **The order is load-bearing in both directions.** Call this *after*
//! `gpui_component::init(cx)`, which is what installs the global — before it,
//! there is nothing to write to. And call it before the first frame, because a
//! component that has already painted keeps what it was measured with.
//!
//! Re-running is free: every write is an assignment, so the theme switch can
//! rebuild the palette and call this again.

use gpui::{black, App};
use tga_ui::tokens::{metrics, type_scale, Palette};

pub fn apply(palette: &Palette, cx: &mut App) {
    let dark = is_dark(palette);
    let theme = gpui_component::theme::Theme::global_mut(cx);

    // **The library branches on `mode`**, not on how light the colours it was
    // given happen to be. A dark palette under `ThemeMode::Light` gets those
    // decisions backwards while every colour looks right, which is the hardest
    // kind of mismatch to see.
    theme.mode = if dark {
        gpui_component::theme::ThemeMode::Dark
    } else {
        gpui_component::theme::ThemeMode::Light
    };

    // `metrics::RADIUS` is `px(0.0)` and says why: square corners are the
    // design, not a default. The library ships 6px and 8px, and a rounded input
    // is the single most visible way the two languages disagree.
    theme.radius = metrics::RADIUS;
    theme.radius_lg = metrics::RADIUS;
    theme.font_size = type_scale::BODY;

    // **This field is the whole window's typeface, not just the borrowed
    // control's**: `Root::render` puts it on the div that wraps our view, so
    // everything inherits from here. Set the family, not a `Font` — the library
    // assigns this to `Styled::font_family`, which cannot carry OpenType
    // features. `main.rs` adds those a level down.
    theme.font_family = tga_ui::fonts::SANS.into();

    // The page, and the ink on it. `background` is also the field's own fill,
    // so it reads as part of the page rather than as a tray sunk into it.
    theme.background = palette.bg;
    theme.foreground = palette.fg;

    // **Borders: `hairline`, not `rule`.** These are not interchangeable —
    // `hairline` is pure ink in light and a mid grey in dark, and it is what
    // `components::rule()` paints. Giving the field the soft grey would leave
    // it looking faded next to our own lines.
    theme.border = palette.hairline;
    theme.input = palette.hairline;
    theme.title_bar = palette.bg;
    theme.title_bar_border = palette.hairline;

    // **The one red says "here".** The focus ring and the caret are the same
    // statement, so they are the same colour. Selection is that red held back
    // to a wash: an opaque fill under text is a smear rather than a highlight.
    theme.ring = palette.accent;
    theme.caret = palette.accent;
    theme.selection = palette.accent.alpha(0.25);

    // `muted` is a *background* in this library — the fill of a disabled input
    // — so it takes `surface`, while `muted_foreground` takes our actual
    // secondary text grey. Swapping the two paints placeholder text in a
    // near-page grey and the disabled field in the text colour.
    theme.muted = palette.surface;
    theme.muted_foreground = palette.muted;

    theme.popover = palette.surface;
    theme.popover_foreground = palette.fg;

    // **`accent` here is not our accent.** The library uses this field for the
    // hover *fill* on menu and list items. Handing it the red would spend the
    // design's one emphasis colour on pointer movement.
    theme.accent = palette.surface;
    theme.accent_foreground = palette.fg;

    theme.primary = palette.fg;
    theme.primary_foreground = palette.bg;
    theme.primary_hover = palette.fg.opacity(0.85);
    theme.primary_active = palette.fg.opacity(0.72);

    theme.secondary = palette.surface;
    theme.secondary_foreground = palette.fg;
    theme.secondary_hover = palette.rule;
    theme.secondary_active = palette.rule;

    // **Our only red is the accent**, so danger is that red. Inventing a warmer
    // one would put two reds in a design whose whole claim is that it has one.
    theme.danger = palette.accent;
    theme.danger_foreground = palette.accent_fg;
    theme.danger_hover = palette.accent.opacity(0.85);
    theme.danger_active = palette.accent.opacity(0.72);

    // The run is the one thing this window is *doing*, and that is what the
    // accent is for.
    theme.progress_bar = palette.accent;

    // A scrim dims what is behind it, so it is ink in both appearances — `fg`
    // would paint a near-white veil over the dark theme. Dark needs the heavier
    // value because there is less light to take away.
    theme.overlay = black().alpha(if dark { 0.6 } else { 0.35 });
}

/// Which appearance this palette is.
///
/// [`Palette`] carries no field naming its own appearance, so the honest test
/// is equality against the two that exist rather than a lightness threshold,
/// which would guess wrong the moment a token moves. Anything matching neither
/// falls to dark — the same fallback `Palette::named` takes, and for the same
/// reason: a wrong guess costs a mismatched icon polarity, while the
/// alternative risks a window nobody can read.
fn is_dark(palette: &Palette) -> bool {
    *palette != Palette::light()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_appearance_is_recognised_as_itself() {
        assert!(!is_dark(&Palette::light()));
        assert!(is_dark(&Palette::dark()));
    }

    #[test]
    fn a_palette_matching_neither_appearance_falls_back_to_dark() {
        let mut odd = Palette::light();
        odd.accent = Palette::dark().accent;
        assert!(is_dark(&odd));
    }
}
