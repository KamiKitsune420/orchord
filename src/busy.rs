//! Doing slow work without making the window wait.
//!
//! A window that is busy does not answer, and a screen reader that asks it something waits
//! for the answer: to the user, the screen reader has frozen. So anything that can take more
//! than a moment (asking the internet, starting another program, reading a big file) is done
//! on another thread while the window goes on answering. The wx bindings offer no way to let
//! a window answer except through a progress dialog, so one is shown, which also says what is
//! being waited for.

use std::time::{Duration, Instant};

use wxdragon::prelude::*;

/// Work that ends within this time needs no dialog; a window that slow to answer is not noticed.
const NO_DIALOG_WITHIN: Duration = Duration::from_millis(250);
const LOOK_EVERY: Duration = Duration::from_millis(20);
const ANSWER_EVERY: Duration = Duration::from_millis(100);

/// Runs `work` on another thread and returns what it returns. If it takes more than a moment,
/// a dialog saying `message` is shown until it ends, and the window keeps answering meanwhile.
pub fn wait<W: WxWidget, T: Send + 'static>(parent: &W, message: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
    let worker = std::thread::spawn(work);
    let started = Instant::now();
    while !worker.is_finished() && started.elapsed() < NO_DIALOG_WITHIN {
        std::thread::sleep(LOOK_EVERY);
    }
    if !worker.is_finished() {
        let dialog = ProgressDialog::builder(parent, "Orchord", message, 100).build();
        while !worker.is_finished() {
            // Pulsing is what lets the window answer. After the first time it changes nothing
            // on screen, so it gives a screen reader nothing new to report.
            dialog.pulse(None);
            std::thread::sleep(ANSWER_EVERY);
        }
        dialog.destroy();
    }
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
