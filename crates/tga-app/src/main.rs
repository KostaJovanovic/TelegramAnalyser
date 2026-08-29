//! The Telegram export analyser.
//!
//! `tga` is the same program with the window taken off.

// **A release build is a GUI binary, so double-clicking it does not open a
// console window behind the app.** Without this the exe defaults to the console
// subsystem and Windows allocates one, which looks like the program started
// something else.
//
// Debug builds keep the console, because that is where `cargo run` lives and
// the startup diagnostics below are worth more than the tidiness.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod job;
mod open;
mod shell;
mod theme;

use gpui::{
    px, size, App, AppContext, Application, Bounds, ParentElement, Styled, TitlebarOptions,
    WindowBounds, WindowOptions,
};

/// Record a failure somewhere a user can actually find it.
///
/// A GUI-subsystem binary has no stderr unless it was launched from a terminal,
/// and **a blank window is the worst possible failure precisely because it says
/// nothing**. So the message goes to stderr, which reaches a developer running
/// `cargo run`, and to a file beside the executable, which is the only one that
/// survives a double-click.
fn report_startup_failure(message: &str) {
    eprintln!("{message}");
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = std::fs::write(dir.join("startup-error.log"), message);
        }
    }
}

/// Whether the window ever opened.
///
/// **A panic six seconds into a large export is not a startup failure**, and
/// telling someone their machine needs DirectX drivers when the app has been
/// running all afternoon sends them to fix something that is not broken. The
/// hook says which kind of failure it is, and it can only know that by being
/// told — the exporter carried this flag for a while without anything ever
/// setting it, so every panic got the wrong message.
static WINDOW_OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn panic_message(window_opened: bool, panic: &str) -> String {
    if window_opened {
        // The renderer plainly works — it has been drawing. Naming the GPU here
        // would send someone to update a driver that is fine.
        format!(
            "{} stopped unexpectedly: {panic}\n\
             Nothing was sent anywhere and the export on disk is untouched; \
             this app only ever reads it.",
            shell::TITLE
        )
    } else {
        format!(
            "{} could not start: {panic}\n\
             This build needs a GPU with working DirectX drivers.",
            shell::TITLE
        )
    }
}

/// One element between `Root` and the shell, carrying nothing but the typeface.
///
/// `Root` already sets `Theme::font_family` on its own div, which is why
/// `theme::apply` is where the *family* is chosen. But it sets the family
/// alone, and the design also wants Geist's `tnum`: the face's default figures
/// are proportional, so a count ticking from 199 to 200 shifts everything
/// beside it sideways. Only `Styled::font` carries [`gpui::FontFeatures`].
struct Typeface(gpui::AnyView);

impl gpui::Render for Typeface {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
            .size_full()
            .font(tga_ui::fonts::sans())
            .child(self.0.clone())
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        use std::sync::atomic::Ordering;
        let message = panic_message(WINDOW_OPENED.load(Ordering::Relaxed), &info.to_string());
        report_startup_failure(&message);
    }));

    Application::new().run(|cx: &mut App| {
        // **Before `gpui_component::init`**, because `theme::apply` below hands
        // the library `tga_ui::fonts::SANS` and a family that is not registered
        // yet resolves to the system face and is then cached under that name.
        // A failure here is reported and stepped over: the window is set in the
        // wrong typeface, which is ugly, while refusing to start would cost the
        // user a report over a font.
        if let Err(e) = tga_ui::fonts::register(cx) {
            report_startup_failure(&format!("{e}"));
        }
        // Must run before any gpui-component feature is used.
        gpui_component::init(cx);
        // And immediately after it, before the borrowed field can paint once in
        // the library's own colours. See `theme.rs` for why the order is
        // load-bearing in both directions.
        theme::apply(&tga_ui::tokens::Palette::dark(), cx);

        let (w, h) = tga_ui::components::min_window();
        // The Python opens at 760x430; the floor is what the layout was
        // measured at and is stated so the window cannot be dragged under it.
        let bounds = Bounds::centered(None, size(px(760.0), px(h)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(w), px(h))),
                titlebar: Some(TitlebarOptions {
                    title: Some(shell::TITLE.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| shell::Shell::new(window, cx));
                let typeface = cx.new(|_| Typeface(gpui::AnyView::from(view)));
                // The first level in the window has to be a Root, so the
                // component library's overlays have somewhere to go. `Typeface`
                // goes *inside* it rather than around it for that reason.
                cx.new(|cx| gpui_component::Root::new(gpui::AnyView::from(typeface), window, cx))
            },
        );
        if let Err(e) = opened {
            report_startup_failure(&format!("could not open a window: {e}"));
            return;
        }
        // Past this point a panic is not a startup failure — see
        // `panic_message`. `open_window` returning `Ok` is the earliest point
        // that is true.
        WINDOW_OPENED.store(true, std::sync::atomic::Ordering::Relaxed);
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_before_the_window_opens_blames_the_gpu() {
        let message = panic_message(false, "boom");
        assert!(message.contains("could not start"));
        assert!(message.contains("DirectX"));
    }

    #[test]
    fn a_panic_after_the_window_opened_does_not_blame_the_gpu() {
        // The branch that was unreachable in the exporter for a long time,
        // because nothing ever set the flag: a panic deep into a run was
        // reported as a graphics driver problem.
        let message = panic_message(true, "boom");
        assert!(!message.contains("DirectX"));
        assert!(message.contains("stopped unexpectedly"));
        assert!(message.contains("untouched"));
    }

    #[test]
    fn the_flag_starts_false_so_a_startup_panic_gets_the_gpu_message() {
        assert!(!WINDOW_OPENED.load(std::sync::atomic::Ordering::Relaxed));
    }
}
