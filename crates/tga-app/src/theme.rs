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
//! **Dark only**, so this runs once at startup and never again. It is still
//! written as a function of a `Palette` rather than against constants: the
//! mapping from *our* token to *their* field is the part worth reading, and
//! inlining `#0a0a0a` twenty times would bury it.

use gpui::{black, App};
use tga_ui::tokens::{metrics, type_scale, Palette};

pub fn apply(palette: &Palette, cx: &mut App) {
    let theme = gpui_component::theme::Theme::global_mut(cx);

    // **The library branches on `mode`**, not on how light the colours it was
    // given happen to be — icon polarity and a handful of `is_dark()` calls
    // inside its own components. A dark palette under `ThemeMode::Light` gets
    // those decisions backwards while every colour looks right, which is the
    // hardest kind of mismatch to see. There is one appearance here, so this is
    // a constant rather than a branch.
    theme.mode = gpui_component::theme::ThemeMode::Dark;

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
    // `hairline` is the mid grey `components::rule()` paints, the structural
    // 1px line the whole layout is divided by; `rule` is the softer divider
    // used *inside* a panel. Giving the field the soft grey would leave it
    // looking faded next to our own lines.
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

    // A scrim dims what is behind it, so it is ink rather than `fg` — the
    // latter would paint a near-white veil over the page.
    theme.overlay = black().alpha(0.6);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_this_is_handed_really_is_the_dark_one() {
        // `apply` writes `ThemeMode::Dark` as a constant rather than deriving it
        // from the colours, which is only correct while there is one appearance.
        // This is the assertion that would fail if a light palette ever came
        // back — the library uses `mode` for icon polarity, so a mismatch looks
        // right and behaves wrong.
        let p = Palette::dark();
        assert!(p.bg.l < p.fg.l, "the page must be darker than the ink");
        assert!(p.hairline.l > p.bg.l, "and the hairline visible against it");
    }
}
