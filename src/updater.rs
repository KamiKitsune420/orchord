//! Checking for a newer Orchord and installing it: the window's side, with its questions and
//! its progress dialog. The work itself is in `orchord_core::update`.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use orchord_core::update::{self, Release};
use wxdragon::prelude::*;

use crate::busy;

const TITLE: &str = "Orchord Update";
/// Release notes are shown in the question; a long list would bury the question itself.
const NOTES_LIMIT: usize = 1200;
const MEGABYTE: u64 = 1 << 20;
const BAR_MOVES_EVERY: Duration = Duration::from_secs(1);

/// Asks GitHub in the background whether there is a newer version. Only a newer version is
/// ever sent: having no internet connection when Orchord starts is not news.
pub fn check_in_background() -> Receiver<Release> {
    let (sender, news) = mpsc::channel();
    std::thread::spawn(move || {
        if let Ok(Some(release)) = update::newer_release() {
            let _ = sender.send(release);
        }
    });
    news
}

/// File > Check for Updates. Returns whether a new version was installed and started, in
/// which case this copy of Orchord should close.
pub fn check_now(frame: Frame, machine_running: bool) -> bool {
    let release = match busy::wait(&frame, "Checking for a newer version of Orchord.", update::newer_release) {
        Err(error) => {
            tell(frame, &format!("Orchord could not find out whether there is a newer version. {error:#}."), MessageDialogStyle::IconWarning);
            return false;
        }
        Ok(None) => {
            tell(frame, &format!("This is the newest version of Orchord, {}.", update::current_version()), MessageDialogStyle::IconInformation);
            return false;
        }
        Ok(Some(release)) => release,
    };
    if machine_running {
        // The boot folder a running Mac starts from is among the files an update replaces.
        tell(frame, &format!("Orchord {} is available. A virtual machine is running; stop it, then choose Check for Updates again.", release.version), MessageDialogStyle::IconInformation);
        return false;
    }
    let question = format!(
        "Orchord {} is available; this is version {}.\n\n{}\n\nDownload and install it now? It is about {} megabytes, and Orchord restarts afterwards.",
        release.version,
        update::current_version(),
        plain_notes(&release.notes),
        (release.zip_bytes / MEGABYTE).max(1)
    );
    if MessageDialog::builder(&frame, &question, TITLE).with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion).build().show_modal() != ID_YES {
        return false;
    }
    let program = match install(frame, &release) {
        Ok(program) => program,
        Err(error) => {
            tell(frame, &format!("The update was not installed. {error} You can also download it from {}", release.page), MessageDialogStyle::IconWarning);
            return false;
        }
    };
    match Command::new(&program).spawn() {
        Ok(_) => true,
        Err(_) => {
            tell(frame, &format!("Orchord {} is installed. Close Orchord and start it again to use it.", release.version), MessageDialogStyle::IconInformation);
            false
        }
    }
}

/// Downloads and installs on another thread, behind a progress dialog whose Cancel stops it.
fn install(frame: Frame, release: &Release) -> Result<PathBuf, String> {
    let folder = update::program_folder().map_err(|error| format!("{error:#}."))?;
    let cancel = Arc::new(AtomicBool::new(false));
    let (sender, received) = mpsc::channel();
    let worker = {
        let (release, cancel) = (release.clone(), cancel.clone());
        std::thread::spawn(move || {
            let mut report = |bytes| {
                let _ = sender.send(bytes);
            };
            update::install(&release, &folder, &mut report, &cancel).map_err(|error| format!("{error:#}"))
        })
    };
    let progress = ProgressDialog::builder(&frame, TITLE, &format!("Downloading Orchord {}.", release.version), 100).can_abort().build();
    let (mut percent, mut shown, mut shown_at) = (0, 0, Instant::now());
    while !worker.is_finished() {
        if let Some(bytes) = received.try_iter().last() {
            // The last hundredth is kept for unpacking and putting the files in place.
            percent = if release.zip_bytes > 0 { (bytes.saturating_mul(99) / release.zip_bytes).min(99) as i32 } else { 0 };
        }
        // The bar moves at most once a second: every move is an event a screen reader has to
        // deal with, and a fast download would otherwise send it a hundred in a few seconds.
        if percent != shown && shown_at.elapsed() >= BAR_MOVES_EVERY {
            (shown, shown_at) = (percent, Instant::now());
        }
        // Updating also lets the dialog answer; a false return or the flag means Cancel was pressed.
        if !progress.update(shown, None) || progress.was_cancelled() {
            cancel.store(true, Ordering::Relaxed);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    progress.destroy();
    worker.join().unwrap_or_else(|_| Err("The update stopped unexpectedly.".to_string()))
}

fn tell(frame: Frame, text: &str, icon: MessageDialogStyle) {
    MessageDialog::builder(&frame, text, TITLE).with_style(MessageDialogStyle::OK | icon).build().show_modal();
}

/// Release notes are written with marks for a web page; read aloud as they are, a screen
/// reader would say "number number" and "star star". The marks are taken out.
fn plain_notes(notes: &str) -> String {
    let plain: String = notes.chars().filter(|character| !matches!(character, '#' | '*' | '`')).collect();
    let lines: Vec<&str> = plain.lines().map(str::trim).collect();
    let mut text = lines.join("\n");
    while text.contains("\n\n\n") {
        text = text.replace("\n\n\n", "\n\n");
    }
    let text = text.trim();
    if text.chars().count() <= NOTES_LIMIT {
        return text.to_string();
    }
    let cut: String = text.chars().take(NOTES_LIMIT).collect();
    format!("{}...\n(The release page has the rest.)", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::{NOTES_LIMIT, plain_notes};

    #[test]
    fn release_notes_lose_their_web_page_marks() {
        assert_eq!(plain_notes("## New\n\n- **Check** this `PC`.\n\n\n\nDone."), "New\n\n- Check this PC.\n\nDone.");
    }

    #[test]
    fn long_release_notes_are_cut_short() {
        let notes = plain_notes(&"word ".repeat(1000));
        assert!(notes.chars().count() < NOTES_LIMIT + 60 && notes.ends_with("(The release page has the rest.)"));
    }
}
