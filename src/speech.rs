//! Speaks short messages through the screen reader that is running.
//!
//! Uses Prism (<https://github.com/ethindp/prism>, MPL-2.0) through the `prismer` crate, which
//! talks to NVDA, JAWS, System Access, ZoomText and others. Only screen readers are used: when
//! none is running nothing is spoken (Prism would otherwise pick a Windows voice), and the
//! status bar still carries the text.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use prismer::{Backend, BackendId, Error, Features, Prism};

/// Prism's list also has plain voices (SAPI, OneCore) and UI Automation; not those.
const SCREEN_READERS: [BackendId; 9] = [
    BackendId::NVDA,
    BackendId::JAWS,
    BackendId::SYSTEM_ACCESS,
    BackendId::ZOOM_TEXT,
    BackendId::ZDSR,
    BackendId::BOY_PC_READER,
    BackendId::PC_TALKER,
    BackendId::SENSE_READER,
    BackendId::WINDOW_EYES,
];

/// While no screen reader is found, look again no more often than this.
const LOOK_AGAIN_AFTER: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Reader {
    /// Created once and kept for the life of the program; backends borrow it.
    prism: Option<&'static Prism>,
    backend: Option<Backend<'static>>,
    /// Prism itself could not start, so there is no point in trying again.
    broken: bool,
    last_look: Option<Instant>,
}

thread_local! {
    // Everything that speaks runs on the window's thread.
    static READER: RefCell<Reader> = RefCell::new(Reader::default());
}

/// Speaks `text`. With `interrupt`, whatever is being said is cut off first.
/// Returns whether a screen reader took the text.
pub fn speak(text: &str, interrupt: bool) -> bool {
    READER.with(|reader| {
        let Ok(mut reader) = reader.try_borrow_mut() else { return false };
        let Some(backend) = reader.current() else { return false };
        let spoken = backend.speak(text, interrupt).is_ok();
        if !spoken {
            // The screen reader was closed: find another next time.
            reader.backend = None;
            reader.last_look = None;
        }
        spoken
    })
}

impl Reader {
    /// The screen reader to speak to, looking for one if there is none yet.
    fn current(&mut self) -> Option<&Backend<'static>> {
        if self.backend.is_none() && !self.broken && self.last_look.is_none_or(|last| last.elapsed() >= LOOK_AGAIN_AFTER) {
            self.last_look = Some(Instant::now());
            self.backend = self.find();
        }
        self.backend.as_ref()
    }

    fn find(&mut self) -> Option<Backend<'static>> {
        let prism = match self.prism {
            Some(prism) => prism,
            None => match Prism::new() {
                Ok(prism) => *self.prism.insert(Box::leak(Box::new(prism))),
                Err(_) => {
                    self.broken = true;
                    return None;
                }
            },
        };
        let needed = Features(Features::IS_SUPPORTED_AT_RUNTIME.0 | Features::SPEAK.0);
        SCREEN_READERS.into_iter().find_map(|id| {
            // An installed screen reader that is not running fails here or has no speech.
            let backend = prism.acquire(id).ok()?;
            match backend.initialize() {
                Ok(()) | Err(Error::AlreadyInitialized) => {}
                Err(_) => return None,
            }
            backend.supports(needed).then_some(backend)
        })
    }
}

#[cfg(test)]
mod tests {
    /// Needs a running screen reader, and is heard: run it with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn the_running_screen_reader_takes_a_message() {
        assert!(super::speak("Orchord speech test.", false));
    }
}
