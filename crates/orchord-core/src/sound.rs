//! Short non-speech cues. They must work when no screen reader is running and when the
//! screen reader is asleep for the console window, so they go straight to the sound card.

/// One tone: frequency in hertz, length in milliseconds.
type Tone = (u32, u32);

/// A Mac was switched on and is starting up: four quick rising notes, the last one held.
pub const BOOT: &[Tone] = &[(523, 80), (659, 80), (784, 80), (1047, 220)];
/// A Mac restarted and is being started again: two rising notes.
pub const RESTART: &[Tone] = &[(660, 110), (990, 160)];
/// macOS itself crashed (a kernel panic): the boot cue backwards, four quick falling notes.
pub const PANIC: &[Tone] = &[(1047, 80), (784, 80), (659, 80), (523, 220)];
/// A Mac could not start, or its engine stopped unexpectedly: two slow low notes.
pub const FAILED: &[Tone] = &[(330, 180), (220, 380)];
/// The keyboard now goes to the Mac: one short high note.
pub const KEYBOARD_CAPTURED: &[Tone] = &[(1320, 70)];
/// The keyboard is back with Windows: one short low note.
pub const KEYBOARD_RELEASED: &[Tone] = &[(440, 70)];

/// Plays a cue without blocking the caller.
pub fn play(cue: &'static [Tone]) {
    std::thread::spawn(move || {
        for &(frequency, duration) in cue {
            beep(frequency, duration);
        }
    });
}

#[cfg(windows)]
fn beep(frequency: u32, duration: u32) {
    // SAFETY: Beep takes two plain integers and has no other preconditions.
    unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::Beep(frequency, duration);
    }
}

#[cfg(not(windows))]
fn beep(_frequency: u32, _duration: u32) {}
