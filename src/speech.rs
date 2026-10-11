//! Speaks short messages through the screen reader that is running.
//!
//! Uses Prism (<https://github.com/ethindp/prism>, MPL-2.0) through the `prismer` crate, which
//! talks to NVDA, JAWS, System Access, ZoomText and others. Only screen readers are used: when
//! none is running nothing is spoken (Prism would otherwise pick a Windows voice), and the
//! status bar still carries the text.
//!
//! Speaking is done on a thread of its own. Handing text to a screen reader is a call into
//! another program, which answers when it gets round to it. A screen reader, for its part,
//! keeps asking the window it is reading questions and waits for the answers. If the window's
//! own thread spoke, each could end up waiting for the other, and the screen reader would
//! freeze. The window's thread therefore only drops the text in a queue and carries on.

use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
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

/// Text waiting to be spoken, with whether it cuts off what is being said.
type Message = (String, bool);

static QUEUE: OnceLock<Sender<Message>> = OnceLock::new();

/// Speaks `text`, soon. With `interrupt`, whatever is being said is cut off first.
/// Returns at once; the speaking happens on the speech thread.
pub fn speak(text: &str, interrupt: bool) {
    let queue = QUEUE.get_or_init(|| {
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || speak_all(messages));
        sender
    });
    let _ = queue.send((text.to_string(), interrupt));
}

/// The speech thread: speaks what arrives, for as long as the program runs.
fn speak_all(messages: Receiver<Message>) {
    // Some screen readers are reached through COM, which each thread has to start for itself.
    // SAFETY: called once, first thing, on a thread this function owns; the null pointer is
    // the reserved argument COM asks for.
    unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(std::ptr::null(), windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32);
    }
    let mut reader = Reader::default();
    while let Ok(first) = messages.recv() {
        // If several messages have piled up, nothing before the last interrupting one would
        // be heard anyway, so those are not sent at all.
        let mut waiting: Vec<Message> = std::iter::once(first).chain(messages.try_iter()).collect();
        if let Some(last_interrupting) = waiting.iter().rposition(|(_, interrupt)| *interrupt) {
            waiting.drain(..last_interrupting);
        }
        for (text, interrupt) in waiting {
            reader.say(&text, interrupt);
        }
    }
}

#[derive(Default)]
struct Reader {
    /// Created once and kept for the life of the program; backends borrow it.
    prism: Option<&'static Prism>,
    backend: Option<Backend<'static>>,
    /// Prism itself could not start, so there is no point in trying again.
    broken: bool,
    last_look: Option<Instant>,
}

impl Reader {
    /// Hands `text` to the screen reader. Returns whether one took it.
    fn say(&mut self, text: &str, interrupt: bool) -> bool {
        let Some(backend) = self.current() else { return false };
        let spoken = backend.speak(text, interrupt).is_ok();
        if !spoken {
            // The screen reader was closed: find another next time.
            self.backend = None;
            self.last_look = None;
        }
        spoken
    }

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
        // Through the real queue and the speech thread, as the program speaks. The thread
        // reports back through a second queue only this test has.
        let (sender, messages) = std::sync::mpsc::channel();
        let (report, heard) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = super::Reader::default();
            for (text, interrupt) in messages {
                let text: String = text;
                let _ = report.send(reader.say(&text, interrupt));
            }
        });
        sender.send(("Orchord speech test.".to_string(), false)).unwrap();
        assert!(heard.recv_timeout(std::time::Duration::from_secs(20)).unwrap());
    }
}
