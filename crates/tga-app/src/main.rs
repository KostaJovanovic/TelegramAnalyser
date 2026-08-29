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

mod app;
mod job;
mod open;
mod state;
mod theme;

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
/// telling someone their machine needs working graphics drivers when the app
/// has been running all afternoon sends them to fix something that is not
/// broken. The hook says which kind of failure it is, and it can only know that
/// by being told — the exporter carried this flag for a while without anything
/// ever setting it, so every panic got the wrong message.
static WINDOW_OPENED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn panic_message(window_opened: bool, panic: &str) -> String {
    if window_opened {
        // The renderer plainly works — it has been drawing. Naming the GPU here
        // would send someone to update a driver that is fine.
        format!(
            "{} stopped unexpectedly: {panic}\n\
             Nothing was sent anywhere and the export on disk is untouched; \
             this app only ever reads it.",
            app::TITLE
        )
    } else {
        format!(
            "{} could not start: {panic}\n\
             This build draws with OpenGL and needs working graphics drivers.",
            app::TITLE
        )
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        use std::sync::atomic::Ordering;
        let message = panic_message(WINDOW_OPENED.load(Ordering::Relaxed), &info.to_string());
        report_startup_failure(&message);
    }));

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(app::TITLE)
            .with_inner_size(theme::WINDOW)
            // Stated rather than left to chance: the exporter's window drifted
            // under its own floor and squeezed the only column that named the
            // rows down to nothing.
            .with_min_inner_size(theme::MIN_WINDOW)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    let started = eframe::run_native(
        app::TITLE,
        options,
        Box::new(|cc| {
            // The earliest point at which "the window opened" is true — past
            // here a panic is not a startup failure. See `panic_message`.
            WINDOW_OPENED.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(Box::new(app::Shell::new(&cc.egui_ctx)))
        }),
    );
    if let Err(e) = started {
        report_startup_failure(&format!("could not open a window: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_before_the_window_opens_blames_the_graphics_stack() {
        let message = panic_message(false, "boom");
        assert!(message.contains("could not start"));
        assert!(message.contains("graphics drivers"));
    }

    #[test]
    fn a_panic_after_the_window_opened_does_not_blame_the_graphics_stack() {
        // The branch that was unreachable in the exporter for a long time,
        // because nothing ever set the flag: a panic deep into a run was
        // reported as a graphics driver problem.
        let message = panic_message(true, "boom");
        assert!(!message.contains("graphics drivers"));
        assert!(message.contains("stopped unexpectedly"));
        assert!(message.contains("untouched"));
    }

    #[test]
    fn the_flag_starts_false_so_a_startup_panic_gets_the_driver_message() {
        assert!(!WINDOW_OPENED.load(std::sync::atomic::Ordering::Relaxed));
    }
}
