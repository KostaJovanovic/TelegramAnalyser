//! The window's colour and type, taken from the report's own stylesheet.
//!
//! The values are the ones `tga_report::palette` writes into every report, so
//! the window and the file it produces are visibly one product. They are
//! mirrored rather than imported as a colour type, because that module's only
//! job is to write a text file and it should not know what a GPU is; [`tests`]
//! below asserts the two never drift.
//!
//! Swiss/International: hairline rules, **square corners**, one red, and no
//! shadows. egui's defaults are none of those, so every one of them is set
//! here rather than inherited.
//!
//! **Dark only.** One appearance, no switch: a second is a second design to
//! keep in step, and this one has two colours and a red to keep in step already.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};

/// One appearance, in the order the stylesheet declares it.
pub struct Palette;

impl Palette {
    /// The page itself, behind everything.
    pub const BG: Color32 = hex(0x0a0a0a);
    /// Body text.
    pub const FG: Color32 = hex(0xe8e8e8);
    /// Secondary text.
    pub const MUTED: Color32 = hex(0x888888);
    /// The 1px rule that does all the dividing. A **mid grey**, not ink — on a
    /// near-black page a black hairline shows nothing at all.
    pub const HAIRLINE: Color32 = hex(0x333333);
    /// The softer divider.
    pub const RULE: Color32 = hex(0x262626);
    /// A raised fill.
    pub const SURFACE: Color32 = hex(0x141414);
    /// The one red.
    pub const ACCENT: Color32 = hex(0xff3347);
    /// Text on the accent.
    pub const ACCENT_FG: Color32 = hex(0xffffff);
}

/// A `#rrggbb` literal, written the way the stylesheet writes it.
///
/// `const fn`, so the palette above is a compile-time constant and a
/// transcription error is visible on the line it happens.
const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    )
}

/// The type scale, in points.
pub mod size {
    pub const BODY: f32 = 14.0;
    pub const SMALL: f32 = 13.0;
    pub const MICRO: f32 = 11.0;
}

/// The floor the layout was measured at.
pub const MIN_WINDOW: [f32; 2] = [620.0, 430.0];
pub const WINDOW: [f32; 2] = [760.0, 430.0];

/// The face names, which are ours rather than the files'.
///
/// egui takes a key we choose, unlike a platform text system that reads the
/// `name` table out of the file — so there is nothing here to get wrong and no
/// silent fallback to guard against.
pub const SANS: &str = "geist";
pub const MONO: &str = "geist-mono";
const MEDIUM: &str = "geist-medium";

/// Geist and Geist Mono, embedded in the binary.
///
/// **Latin and Cyrillic are merged into one file per weight.** Do not
/// regenerate or subset them: the merge is why a Serbian or Ukrainian folder
/// name renders in the design's typeface instead of dropping to a system
/// fallback halfway through a word.
fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();
    for (name, bytes) in [
        (SANS, &include_bytes!("../fonts/Geist-Regular.ttf")[..]),
        (MEDIUM, &include_bytes!("../fonts/Geist-Medium.ttf")[..]),
        (MONO, &include_bytes!("../fonts/GeistMono-Regular.ttf")[..]),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), FontData::from_static(bytes).into());
    }
    fonts
        .families
        .insert(FontFamily::Proportional, vec![SANS.to_owned()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec![MONO.to_owned()]);
    fonts
        .families
        .insert(FontFamily::Name(MEDIUM.into()), vec![MEDIUM.to_owned()]);
    fonts
}

/// The face the headings are set in.
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Everything about the window that is a decision rather than a default.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());

    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Body, FontId::proportional(size::BODY)),
        (TextStyle::Button, FontId::proportional(size::BODY)),
        (TextStyle::Small, FontId::proportional(size::MICRO)),
        (TextStyle::Monospace, mono(size::SMALL)),
        (TextStyle::Heading, medium(size::BODY)),
    ]
    .into();

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = Palette::BG;
    v.window_fill = Palette::BG;
    v.extreme_bg_color = Palette::SURFACE;
    v.override_text_color = Some(Palette::FG);
    v.selection.bg_fill = Palette::ACCENT.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0_f32, Palette::FG);
    v.hyperlink_color = Palette::ACCENT;

    // **Square, flat, hairline.** egui's default widget is a rounded raised
    // button with a shadow; every line below takes one of those off.
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::ZERO;
        w.bg_fill = Palette::BG;
        w.weak_bg_fill = Palette::BG;
        w.bg_stroke = Stroke::new(1.0_f32, Palette::RULE);
        w.fg_stroke = Stroke::new(1.0_f32, Palette::FG);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Palette::MUTED);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Palette::HAIRLINE);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, Palette::ACCENT);
    v.widgets.active.bg_fill = Palette::SURFACE;
    v.widgets.active.weak_bg_fill = Palette::SURFACE;
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;

    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The report's own value for a token, as it writes it into the stylesheet.
    fn from_report(name: &str) -> Color32 {
        let text = tga_report::palette::token(name);
        let rgb = u32::from_str_radix(text.trim_start_matches('#'), 16).expect("hex");
        hex(rgb)
    }

    #[test]
    fn the_window_and_the_report_name_the_same_colours() {
        // The two copies exist because the report must not drag a graphics
        // toolkit into a module whose only job is to write a text file. This is
        // what stops them becoming two designs.
        for (name, ours) in [
            ("bg", Palette::BG),
            ("fg", Palette::FG),
            ("muted", Palette::MUTED),
            ("hairline", Palette::HAIRLINE),
            ("rule", Palette::RULE),
            ("surface", Palette::SURFACE),
            ("accent", Palette::ACCENT),
            ("accent_fg", Palette::ACCENT_FG),
        ] {
            assert_eq!(
                ours,
                from_report(name),
                "{name} has drifted from the report's stylesheet"
            );
        }
    }

    #[test]
    fn a_dark_hairline_is_neither_the_page_nor_the_text() {
        // A dark theme with black hairlines shows nothing at all, and one with
        // white hairlines is a wireframe.
        let lum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
        assert!(lum(Palette::HAIRLINE) > lum(Palette::BG));
        assert!(lum(Palette::HAIRLINE) < lum(Palette::FG));
        assert!(lum(Palette::RULE) < lum(Palette::HAIRLINE));
    }

    #[test]
    fn the_accent_really_is_red() {
        let a = Palette::ACCENT;
        assert!(a.r() > a.g() && a.r() > a.b(), "{a:?}");
    }

    #[test]
    fn every_embedded_file_is_a_real_truetype_font() {
        // A zero-length or truncated file registers without complaint and then
        // renders nothing, which looks like a layout bug rather than a missing
        // asset.
        for bytes in [
            &include_bytes!("../fonts/Geist-Regular.ttf")[..],
            &include_bytes!("../fonts/Geist-Medium.ttf")[..],
            &include_bytes!("../fonts/GeistMono-Regular.ttf")[..],
        ] {
            assert!(bytes.len() > 20_000, "suspiciously small face");
            // TrueType's magic: 0x00010000, or `true`, or `OTTO`.
            let magic = &bytes[..4];
            assert!(
                magic == [0x00, 0x01, 0x00, 0x00] || magic == b"true" || magic == b"OTTO",
                "not a font: {magic:?}"
            );
        }
    }

    #[test]
    fn the_families_the_window_asks_for_are_the_ones_it_registered() {
        // egui falls back silently to an empty glyph set for a family it does
        // not have, which draws as a window with no text in it at all.
        let fonts = fonts();
        for family in [
            FontFamily::Proportional,
            FontFamily::Monospace,
            FontFamily::Name(MEDIUM.into()),
        ] {
            let names = fonts.families.get(&family).expect("family is registered");
            assert!(!names.is_empty(), "{family:?} has no face behind it");
            for name in names {
                assert!(fonts.font_data.contains_key(name), "{name} has no bytes");
            }
        }
    }
}
