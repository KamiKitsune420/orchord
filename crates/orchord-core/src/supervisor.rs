//! Runs one Mac from power-on to power-off, across any number of guest restarts.
//!
//! Under the Windows Hypervisor Platform a guest cannot restart itself: when macOS reboots,
//! QEMU reports "Unexpected VP exit code 4" and leaves the virtual CPUs paused, and resetting
//! them in place does not recover them. The only thing that works is to close QEMU and start
//! it again, which is safe because the disk and the firmware variables are both files.
//! The supervisor does exactly that, and plays a cue each time so the user knows. Switching
//! the Mac on in the first place has a different cue.
//!
//! A guest shutdown is different: QEMU exits by itself, and the supervisor reports it.
//!
//! When macOS itself crashes (a kernel panic) QEMU notices nothing: the Mac just freezes with
//! the panic on its screen. macOS also writes the panic to its serial port, which QEMU saves
//! to a file, so the supervisor watches that file to tell the user.

use std::io::{Read, Seek, SeekFrom};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::config::{InstallStage, Vm};
use crate::installer_image;
use crate::ocr::{self, ScreenText};
use crate::qemu::{LaunchOptions, build_args};
use crate::qmp::{POINTER_MAX, QmpClient};
use crate::settings::AppSettings;
use crate::sound;

/// A launch that pauses sooner than this counts towards the crash-loop guard. Real install
/// stages have ended in a restart after as little as twenty seconds, so this must stay short.
const QUICK_PAUSE: Duration = Duration::from_secs(12);
const MAX_QUICK_PAUSES: u32 = 4;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const COMMAND_WAIT: Duration = Duration::from_millis(15);
/// How long macOS is given to put its power dialog on screen before the answer is typed.
const POWER_DIALOG_WAIT: Duration = Duration::from_millis(2500);
/// How long macOS is given to put its shut-down question up, looked for every `POWER_DIALOG_WAIT`.
const POWER_QUESTION_LIMIT: Duration = Duration::from_secs(20);
/// Words common to both of macOS's shut-down questions: "Are you sure you want to shut down
/// your computer now?" and the Setup Assistant's "Do you want to shut down your computer?".
/// Both are broken over two lines, after "your" and after "down", and text is recognised a
/// line at a time, so the words looked for must all be on the first line of each.
const POWER_QUESTION: &str = "want to shut down";
/// The names under the buttons along the bottom of macOS's login window. All three together,
/// with no shut-down question on screen, mean that nobody is logged in.
const LOGIN_WINDOW: [&str; 3] = ["Shut Down", "Restart", "Sleep"];
/// How far above its name a login-window button's picture is, in the screen's own pixels.
const BUTTON_PICTURE_ABOVE_NAME: f32 = 26.0;
/// Words on the first screens of a Mac that has just been installed.
const SETUP_SCREENS: [&str; 4] = ["Country or Region", "Select Your Country", "Written and Spoken Languages", "set up your Mac with VoiceOver"];
/// A Mac disk image this large holds more than a freshly erased disk: the installer has
/// copied itself there, and the Mac can carry on without the installer image.
const INSTALLER_COPIED_BYTES: u64 = 6 << 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VmEvent {
    /// QEMU is being started; `launch` counts from 1 and rises with every guest restart.
    Starting { launch: u32 },
    /// QEMU is up and the Mac is booting or running.
    Running { launch: u32 },
    /// The Mac restarted itself, or was reset by the user, and is about to be started again.
    GuestRestart { count: u32 },
    /// macOS crashed with a kernel panic and is frozen; the text is the panic's first line.
    /// The Mac stays as it is until the user restarts it or powers it off.
    Crashed(String),
    /// Installation of macOS reached a new stage; the Mac's configuration file already says so.
    InstallStage(InstallStage),
    /// Something easy install wants the user to know, in words.
    InstallNote(String),
    /// The Mac was suspended by the user and is frozen in memory.
    Suspended,
    Resumed,
    /// A screen image asked for with `VmCommand::Screenshot` has been written.
    Screenshot(PathBuf),
    Stopped(StopReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    GuestShutDown,
    StoppedByUser,
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum VmCommand {
    /// Power the Mac off immediately, like pulling the plug.
    ForceStop,
    /// Power the Mac off and on again, like pressing a reset button.
    Reset,
    /// Ask macOS to shut down or restart properly, the way its own power dialog does.
    /// These need someone to be logged in to the Mac; otherwise nothing happens.
    AskShutDown,
    AskRestart,
    /// Freeze the Mac in memory, and thaw it again.
    Suspend,
    Resume,
    /// Press a chord of keys, given as QEMU key names.
    SendKeys(Vec<String>),
    /// One key going down or up, as a PC scancode with bit 7 set for extended keys.
    Key { scancode: u8, down: bool },
    /// Move the Mac's pointer to an absolute place on its screen; each axis runs from 0 to
    /// `POINTER_MAX`.
    PointerMove { x: u16, y: u16 },
    PointerButton { button: PointerButton, down: bool },
    /// Save the Mac's screen to this file as a PPM image.
    Screenshot(PathBuf),
    /// Run one command of QEMU's machine protocol and send its answer back, for features
    /// used too rarely to deserve a command of their own, such as USB devices.
    Qmp { execute: String, arguments: Option<serde_json::Value>, reply: Sender<std::result::Result<serde_json::Value, String>> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    /// One notch of the scroll wheel; press and release it like a button.
    WheelUp,
    WheelDown,
}

impl PointerButton {
    fn qemu_name(self) -> &'static str {
        match self {
            PointerButton::Left => "left",
            PointerButton::Right => "right",
            PointerButton::Middle => "middle",
            PointerButton::WheelUp => "wheel-up",
            PointerButton::WheelDown => "wheel-down",
        }
    }
}

pub struct Supervisor {
    vm: Vm,
    settings: AppSettings,
    /// An installer image for the first launch only, whatever the Mac's configuration says.
    /// Without it, a Mac that has nothing installed starts from the image its configuration names.
    pub install_media: Option<PathBuf>,
    pub debug_qmp_port: Option<u16>,
    /// Extra QEMU arguments for experiments; they are added last, so they win.
    pub extra_qemu_args: Vec<String>,
}

enum Outcome {
    UserStop,
    UserReset,
    Exited(ExitStatus),
    /// The virtual CPUs stopped without being asked to: the Mac restarted itself.
    Paused,
}

impl Supervisor {
    pub fn new(vm: Vm, settings: AppSettings) -> Self {
        let debug_qmp_port = settings.debug_qmp_port;
        let extra_qemu_args = settings.extra_qemu_args.clone();
        Supervisor { vm, settings, install_media: None, debug_qmp_port, extra_qemu_args }
    }

    /// Runs the Mac until it shuts down, is stopped, or fails. Blocks; call it on its own thread.
    /// The final `Stopped` event is always sent before this returns.
    pub fn run(mut self, events: Sender<VmEvent>, commands: Receiver<VmCommand>) -> StopReason {
        let reason = self.run_inner(&events, &commands).unwrap_or_else(|error| StopReason::Failed(format!("{error:#}")));
        if matches!(reason, StopReason::Failed(_)) {
            sound::play(sound::FAILED);
        }
        let _ = events.send(VmEvent::Stopped(reason.clone()));
        reason
    }

    fn run_inner(&mut self, events: &Sender<VmEvent>, commands: &Receiver<VmCommand>) -> Result<StopReason> {
        if let Some(problem) = self.settings.problems().into_iter().next() {
            return Ok(StopReason::Failed(problem));
        }
        self.vm.validate()?;
        if !self.vm.vars_path().exists() {
            std::fs::create_dir_all(self.vm.config_dir())?;
            std::fs::copy(self.settings.firmware_vars_template(), self.vm.vars_path()).context("creating the firmware variable store")?;
        }

        let mut launch = 0;
        let mut restarts = 0;
        let mut quick_pauses = 0;
        loop {
            launch += 1;
            let _ = events.send(VmEvent::Starting { launch });
            let install_media = match self.install_media.take().or_else(|| self.vm.install_media_path()) {
                Some(image) => Some(installer_image::prepare(&image, &self.vm.config_dir())?),
                None => None,
            };
            let options = LaunchOptions { qmp_port: free_port()?, debug_qmp_port: self.debug_qmp_port, install_media, extra_args: self.extra_qemu_args.clone() };
            let mut child = self.spawn(&options)?;
            let started = Instant::now();
            let mut qmp = match QmpClient::connect(options.qmp_port, Duration::from_secs(20)) {
                Ok(qmp) => qmp,
                Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let said = self.qemu_log_tail();
                    // The commonest reason on a new PC, and the one Orchord can put right itself.
                    let hint = if said.contains("ignore-unknown-msr") {
                        " This QEMU is too old for a processor that is not Intel's: Orchord needs QEMU 11.1 or newer there."
                    } else if said.to_lowercase().contains("whpx") {
                        " The Windows Hypervisor Platform may be turned off: open Orchord Settings from the File menu and press Check and set up this PC."
                    } else {
                        ""
                    };
                    return Ok(StopReason::Failed(format!("QEMU did not start. {said}{hint}")));
                }
            };
            // Later launches follow a restart, which has already played its own cue.
            if launch == 1 {
                sound::play(sound::BOOT);
            }
            let _ = events.send(VmEvent::Running { launch });

            match self.watch(&mut child, &mut qmp, events, commands)? {
                Outcome::UserStop => return Ok(StopReason::StoppedByUser),
                Outcome::UserReset => {
                    restarts += 1;
                    sound::play(sound::RESTART);
                    let _ = events.send(VmEvent::GuestRestart { count: restarts });
                }
                Outcome::Exited(status) if status.success() => return Ok(StopReason::GuestShutDown),
                Outcome::Exited(status) => return Ok(StopReason::Failed(format!("QEMU stopped unexpectedly ({status}). {}", self.qemu_log_tail()))),
                Outcome::Paused => {
                    let _ = qmp.quit();
                    let _ = child.wait();
                    quick_pauses = if started.elapsed() < QUICK_PAUSE { quick_pauses + 1 } else { 0 };
                    if quick_pauses >= MAX_QUICK_PAUSES {
                        return Ok(StopReason::Failed("The Mac keeps stopping within seconds of starting, so it was not started again.".to_string()));
                    }
                    self.note_installer_copied(events);
                    restarts += 1;
                    sound::play(sound::RESTART);
                    let _ = events.send(VmEvent::GuestRestart { count: restarts });
                }
            }
        }
    }

    /// After the installer's first stage the Mac restarts to carry on from its own disk.
    /// From then on the installer image must stay away, or the Mac would start from it again.
    fn note_installer_copied(&mut self, events: &Sender<VmEvent>) {
        let disk_bytes = std::fs::metadata(self.vm.disk_path()).map(|disk| disk.len()).unwrap_or(0);
        if self.vm.config.install.stage == InstallStage::NotStarted && disk_bytes >= INSTALLER_COPIED_BYTES && self.vm.set_install_stage(InstallStage::Copied).is_ok() {
            let _ = events.send(VmEvent::InstallStage(InstallStage::Copied));
        }
    }

    /// Watches one QEMU process until the Mac shuts down, pauses, or the user stops it.
    fn watch(&self, child: &mut Child, qmp: &mut QmpClient, events: &Sender<VmEvent>, commands: &Receiver<VmCommand>) -> Result<Outcome> {
        // A suspended Mac is also "not running", so it must not be mistaken for a restart.
        let mut suspended = false;
        // QEMU starts the serial log afresh on every launch, so the watcher does too.
        let mut serial = SerialWatch::new(self.vm.serial_log_path());
        let mut last_poll = Instant::now() - POLL_INTERVAL;
        loop {
            // Keys must reach the Mac promptly, so commands are served far more often than the state is polled.
            let received = match commands.recv_timeout(COMMAND_WAIT) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                // Nobody can send commands any more; keep supervising without spinning.
                Err(RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(COMMAND_WAIT);
                    None
                }
            };
            if let Some(command) = received {
                match command {
                    VmCommand::ForceStop | VmCommand::Reset => {
                        let _ = qmp.quit();
                        let _ = child.wait();
                        return Ok(if matches!(command, VmCommand::Reset) { Outcome::UserReset } else { Outcome::UserStop });
                    }
                    VmCommand::Suspend if !suspended && qmp.pause().is_ok() => {
                        suspended = true;
                        let _ = events.send(VmEvent::Suspended);
                    }
                    VmCommand::Resume if suspended && qmp.resume().is_ok() => {
                        suspended = false;
                        let _ = events.send(VmEvent::Resumed);
                    }
                    VmCommand::Suspend | VmCommand::Resume => {}
                    VmCommand::AskShutDown => self.ask_to_stop(qmp, events, false),
                    VmCommand::AskRestart => self.ask_to_stop(qmp, events, true),
                    VmCommand::SendKeys(keys) => {
                        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                        let _ = qmp.send_keys(&keys);
                    }
                    VmCommand::Key { scancode, down } => {
                        let _ = qmp.key_event(scancode, down);
                    }
                    VmCommand::PointerMove { x, y } => {
                        let _ = qmp.pointer_move(x, y);
                    }
                    VmCommand::PointerButton { button, down } => {
                        let _ = qmp.pointer_button(button.qemu_name(), down);
                    }
                    VmCommand::Qmp { execute, arguments, reply } => {
                        let _ = reply.send(qmp.execute(&execute, arguments).map_err(|error| format!("{error:#}")));
                    }
                    VmCommand::Screenshot(path) => {
                        if qmp.screendump(&path).is_ok() {
                            let _ = events.send(VmEvent::Screenshot(path));
                        }
                    }
                }
            }
            if last_poll.elapsed() < POLL_INTERVAL {
                continue;
            }
            last_poll = Instant::now();
            if let Some(status) = child.try_wait()? {
                return Ok(Outcome::Exited(status));
            }
            match qmp.run_state() {
                Ok(state) if !state.running && !suspended => return Ok(Outcome::Paused),
                Ok(_) => {}
                // The socket closes a moment before the process exits on a guest shutdown.
                Err(_) => return Ok(Outcome::Exited(child.wait()?)),
            }
            if let Some(panic) = serial.new_panic() {
                sound::play(sound::PANIC);
                let _ = events.send(VmEvent::Crashed(panic));
            }
        }
    }

    fn spawn(&self, options: &LaunchOptions) -> Result<Child> {
        let log = std::fs::File::create(self.vm.qemu_log_path()).context("creating the QEMU log file")?;
        let mut command = Command::new(self.settings.qemu_exe());
        command.args(build_args(&self.vm, &self.settings, options)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(log);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command.spawn().with_context(|| format!("starting {}", self.settings.qemu_exe().display()))
    }

    /// Asks macOS to shut down, or to restart, the way someone at the Mac would.
    ///
    /// What works depends on where the Mac is, so the screen is looked at first, and an answer
    /// is only typed once the screen shows the question it answers. Nothing is typed blind: a
    /// stray Return in a Mac someone is working in could do anything.
    ///
    /// * A new Mac at its first set-up screens: Command+Q makes the Setup Assistant ask "Do you
    ///   want to shut down your computer?", and Return says yes. Control+Power must not be
    ///   used there: its question appears and can be answered, but the Setup Assistant then
    ///   refuses to quit and macOS gives up with "Setup Assistant failed to quit".
    /// * Anywhere else: Control+Power brings up "Are you sure you want to shut down your
    ///   computer now?", where Return shuts down and R restarts.
    /// * If that did not bring the question, the power button, pressed through ACPI, may.
    ///
    /// If nothing gets a question, nothing more is done; the window notices that the Mac is
    /// still running and asks its user what to do.
    fn ask_to_stop(&self, qmp: &mut QmpClient, events: &Sender<VmEvent>, restart: bool) {
        // Each step is told to the user: a Mac that does not obey should at least say why.
        let tell = |text: &str| {
            self.power_diary(&format!("said: {text}"));
            let _ = events.send(VmEvent::InstallNote(text.to_string()));
        };
        self.power_diary(if restart { "asked to restart" } else { "asked to shut down" });
        let first_look = self.look(qmp);
        let shows = |phrases: &[&str]| first_look.as_ref().is_some_and(|screen| phrases.iter().any(|phrase| screen.contains(phrase)));
        // The login window, with nobody logged in. Neither keys nor the power button bring a
        // question there; the window has buttons of its own along the bottom, each a picture
        // with its name beneath it.
        if LOGIN_WINDOW.iter().all(|name| shows(&[name])) && !shows(&[POWER_QUESTION]) {
            let name = if restart { "Restart" } else { "Shut Down" };
            if let Some(screen) = &first_look {
                self.click_button_named(qmp, screen, name);
                // The picture is the button on some versions and the name on others.
                std::thread::sleep(POWER_DIALOG_WAIT * 2);
                if let Some(screen) = self.look(qmp).filter(|screen| LOGIN_WINDOW.iter().all(|name| screen.contains(name))) {
                    self.click_button_named(qmp, &screen, name);
                }
            }
            tell(if restart { "Nobody is logged in to the Mac. Orchord pressed Restart in its login window." } else { "Nobody is logged in to the Mac. Orchord pressed Shut Down in its login window." });
            return;
        }
        if !restart && shows(&SETUP_SCREENS) {
            let _ = qmp.send_keys(&["meta_l", "q"]);
            if self.screen_comes_to_show(qmp, POWER_QUESTION) {
                self.answer_question(qmp, "ret");
                tell("The Mac is at its first set-up screens. Its Setup Assistant asked whether to shut down, and Orchord answered yes.");
            } else {
                tell("The Mac is at its first set-up screens, but its Setup Assistant did not ask whether to shut down.");
            }
            return;
        }
        let answer = if restart { "r" } else { "ret" };
        let done = if restart { "macOS asked whether to shut down, and Orchord chose Restart." } else { "macOS asked whether to shut down, and Orchord answered yes." };
        let _ = qmp.send_keys(&["ctrl", "power"]);
        if self.screen_comes_to_show(qmp, POWER_QUESTION) {
            self.answer_question(qmp, answer);
            tell(done);
            return;
        }
        let _ = qmp.execute("system_powerdown", None);
        if self.screen_comes_to_show(qmp, POWER_QUESTION) {
            self.answer_question(qmp, answer);
            tell(done);
        } else {
            tell("macOS did not show its shut-down question, so nothing was answered. It may still be starting.");
        }
    }

    /// Presses `key` as the answer to the shut-down question that is on the Mac's screen. A
    /// question takes no keys while it is still sliding into place, so the key is pressed
    /// after a pause, and once more if the question is still there afterwards.
    fn answer_question(&self, qmp: &mut QmpClient, key: &str) {
        std::thread::sleep(POWER_DIALOG_WAIT);
        let _ = qmp.send_keys(&[key]);
        std::thread::sleep(POWER_DIALOG_WAIT * 2);
        if self.screen_shows(qmp, &[POWER_QUESTION]) {
            let _ = qmp.send_keys(&[key]);
        }
    }

    /// Watches the Mac's screen until it shows `phrase`, for as long as a busy Mac may need to
    /// put a question up. A single look after a fixed pause missed the question on a PC that
    /// was busy: the Setup Assistant took longer than the pause to ask.
    fn screen_comes_to_show(&self, qmp: &mut QmpClient, phrase: &str) -> bool {
        let asked = Instant::now();
        while asked.elapsed() < POWER_QUESTION_LIMIT {
            std::thread::sleep(POWER_DIALOG_WAIT);
            if self.screen_shows(qmp, &[phrase]) {
                return true;
            }
        }
        false
    }

    /// Whether the Mac's screen shows any of `phrases` right now.
    fn screen_shows(&self, qmp: &mut QmpClient, phrases: &[&str]) -> bool {
        let shown = self.look(qmp).is_some_and(|screen| phrases.iter().any(|phrase| screen.contains(phrase)));
        self.power_diary(&format!("looked for {phrases:?}: {}", if shown { "found" } else { "not found" }));
        shown
    }

    /// Reads the text on the Mac's screen as it is right now, and notes it in the diary.
    fn look(&self, qmp: &mut QmpClient) -> Option<ScreenText> {
        let picture = self.vm.config_dir().join("power-screen.ppm");
        let taken = qmp.screendump(&picture);
        let screen = ocr::read_ppm(&picture);
        let _ = std::fs::remove_file(&picture);
        let seen: String = match &screen {
            Ok(screen) => screen.text().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(240).collect(),
            Err(error) => format!("nothing: {error:#}"),
        };
        self.power_diary(&format!("the screen said: {seen}"));
        screen.ok().filter(|_| taken.is_ok())
    }

    /// Clicks a button that is a picture with `name` written beneath it: first the picture,
    /// which is where the name's middle is, a little higher up.
    fn click_button_named(&self, qmp: &mut QmpClient, screen: &ScreenText, name: &str) {
        let words: Vec<&str> = name.split_whitespace().collect();
        let Some(first) = screen.find(name) else { return };
        // The name's last word, on the same line, gives where the name ends.
        let last = screen.find_exact(words[words.len() - 1]).into_iter().find(|word| (word.y - first.y).abs() < first.height).unwrap_or(first);
        let middle = (first.x + last.x + last.width) / 2.0;
        let scale = |value: f32, size: u32| ((value / size.max(1) as f32).clamp(0.0, 1.0) * POINTER_MAX as f32) as u16;
        self.power_diary(&format!("clicked {name} at {middle:.0}, {:.0}", first.y - BUTTON_PICTURE_ABOVE_NAME));
        let _ = qmp.pointer_move(scale(middle, screen.width), scale(first.y - BUTTON_PICTURE_ABOVE_NAME, screen.height));
        std::thread::sleep(Duration::from_millis(350));
        let _ = qmp.pointer_button("left", true);
        std::thread::sleep(Duration::from_millis(130));
        let _ = qmp.pointer_button("left", false);
    }

    /// Adds a line to `vmconfig\shut-down.log`: what was looked for on the Mac's screen and
    /// what was found, each time the Mac is asked to shut down or restart. When a Mac will
    /// not obey, this says which of its screens it was at.
    fn power_diary(&self, text: &str) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(self.vm.config_dir().join("shut-down.log")) {
            let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |since| since.as_secs() % 86_400);
            let _ = writeln!(file, "[{:02}:{:02}:{:02} UTC] {text}", seconds / 3600, seconds / 60 % 60, seconds % 60);
        }
    }

    fn qemu_log_tail(&self) -> String {
        let text = std::fs::read_to_string(self.vm.qemu_log_path()).unwrap_or_default();
        let lines: Vec<&str> = text.lines().filter(|line| !line.contains("host doesn't support requested feature") && !line.trim().is_empty()).collect();
        let tail = lines.iter().rev().take(3).rev().copied().collect::<Vec<_>>().join(" ");
        if tail.is_empty() { "QEMU gave no reason.".to_string() } else { format!("QEMU said: {tail}") }
    }
}

/// Follows the Mac's serial log as QEMU writes it and reports a kernel panic, once.
struct SerialWatch {
    path: PathBuf,
    offset: u64,
    /// The end of what was read last time, in case the panic line straddles two reads.
    carry: String,
    reported: bool,
}

const PANIC_MARKER: &str = "panic(cpu ";

impl SerialWatch {
    fn new(path: PathBuf) -> Self {
        SerialWatch { path, offset: 0, carry: String::new(), reported: false }
    }

    /// The first line of a kernel panic, if one has appeared in the log since the last call.
    fn new_panic(&mut self) -> Option<String> {
        if self.reported {
            return None;
        }
        let mut file = std::fs::File::open(&self.path).ok()?;
        if file.metadata().ok()?.len() <= self.offset {
            return None;
        }
        file.seek(SeekFrom::Start(self.offset)).ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        self.offset += bytes.len() as u64;
        let text = std::mem::take(&mut self.carry) + &String::from_utf8_lossy(&bytes);
        match panic_line(&text) {
            PanicSearch::Found(line) => {
                self.reported = true;
                Some(line)
            }
            PanicSearch::Unfinished(start) => {
                self.carry = text[start..].to_string();
                None
            }
            PanicSearch::Absent => {
                // Keep just enough to recognise a marker cut in two by the end of this read.
                let keep = text.char_indices().rev().nth(PANIC_MARKER.len()).map_or(0, |(index, _)| index);
                self.carry = text[keep..].to_string();
                None
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PanicSearch {
    Absent,
    /// The panic line has begun at this byte but its end has not been written yet.
    Unfinished(usize),
    Found(String),
}

/// Looks for the line macOS writes when its kernel panics, such as
/// `panic(cpu 0 caller 0xffffff8000a1c2d3): "Kernel trap at ..., type 0=divide error"`.
fn panic_line(text: &str) -> PanicSearch {
    let Some(start) = text.find(PANIC_MARKER) else { return PanicSearch::Absent };
    let Some(length) = text[start..].find('\n') else { return PanicSearch::Unfinished(start) };
    let line: String = text[start..start + length].chars().filter(|character| !character.is_control()).take(240).collect();
    PanicSearch::Found(line.trim().to_string())
}

/// Asks Windows for a TCP port that is free right now.
fn free_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::{PanicSearch, SerialWatch, panic_line};

    #[test]
    fn a_panic_line_is_found_and_cleaned() {
        let log = "standard timeslicing quantum is 10000 us\r\npanic(cpu 0 caller 0xffffff8000a1c2d3): \"Kernel trap, type 0=divide error\"\r\nBacktrace follows\n";
        assert_eq!(panic_line(log), PanicSearch::Found("panic(cpu 0 caller 0xffffff8000a1c2d3): \"Kernel trap, type 0=divide error\"".to_string()));
    }

    #[test]
    fn an_ordinary_boot_log_has_no_panic() {
        assert_eq!(panic_line("Darwin Kernel Version 20.6.0\nzone leak detection enabled\n"), PanicSearch::Absent);
    }

    #[test]
    fn a_panic_written_in_two_pieces_is_reported_once() {
        let path = std::env::temp_dir().join(format!("orchord-serial-test-{}.log", std::process::id()));
        std::fs::write(&path, "booting\npanic(c").unwrap();
        let mut watch = SerialWatch::new(path.clone());
        assert_eq!(watch.new_panic(), None);
        std::fs::write(&path, "booting\npanic(cpu 2 caller 0x1): out of luck").unwrap();
        assert_eq!(watch.new_panic(), None);
        std::fs::write(&path, "booting\npanic(cpu 2 caller 0x1): out of luck\nmore\n").unwrap();
        assert_eq!(watch.new_panic(), Some("panic(cpu 2 caller 0x1): out of luck".to_string()));
        assert_eq!(watch.new_panic(), None);
        let _ = std::fs::remove_file(path);
    }
}
