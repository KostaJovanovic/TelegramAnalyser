//! The Telegram export analyser.
//!
//! **One executable, two front ends.** Run it with arguments and it behaves as
//! the old `tga.exe` did — read an export, write a report, print what it found.
//! Run it with none and the window opens. There was never a difference between
//! the two beyond how they were launched, and shipping that difference as a
//! second file meant two things to copy and keep in step.

// **A release build is a GUI binary, so double-clicking it does not open a
// console window behind the app.** Without this the exe defaults to the console
// subsystem and Windows allocates one, which looks like the program started
// something else.
//
// Debug builds keep the console, because that is where `cargo run` lives and
// the startup diagnostics below are worth more than the tidiness.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
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

/// Borrow the console of whatever launched us, if there was one.
///
/// **A release build is a GUI-subsystem binary, and a GUI binary starts with no
/// stdout at all** — so `println!` from the command half would write into
/// nothing and the program would look like it had silently done nothing. Windows
/// hands the parent's console over on request, and `AttachConsole` is that
/// request. It fails when there is no parent console (double-clicked, or piped),
/// and failing is fine: the output goes where it was already going.
///
/// This is the whole price of shipping one executable instead of two, and it is
/// paid once, here.
#[cfg(windows)]
fn attach_parent_console() {
    // Declared rather than pulled in with the `windows` crate: one FFI line
    // against a stable kernel32 export does not need a dependency tree.
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn main() {
    // **Arguments mean the command, none means the window.** This used to be
    // two executables; the only difference between them was ever how they were
    // started, which is a thing argv already records.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        attach_parent_console();
        if let Err(e) = cli::run(args) {
            // `{e:#}` so anyhow's context chain prints, not just the outermost
            // message -- "Not a folder" without the path it tried is no help.
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
        return;
    }

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
