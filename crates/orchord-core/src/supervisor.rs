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
use crate::qemu::{LaunchOptions, build_args};
use crate::qmp::QmpClient;
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
        Supervisor { vm, settings, install_media: None, debug_qmp_port, extra_qemu_args: Vec::new() }
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
                    return Ok(StopReason::Failed(format!("QEMU did not start. {}", self.qemu_log_tail())));
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
                    VmCommand::AskShutDown | VmCommand::AskRestart => {
                        // Control+Power opens macOS's "Are you sure you want to shut down" dialog.
                        // There, Return chooses Shut Down and R chooses Restart.
                        let _ = qmp.send_keys(&["ctrl", "power"]);
                        std::thread::sleep(POWER_DIALOG_WAIT);
                        let _ = qmp.send_keys(&[if matches!(command, VmCommand::AskRestart) { "r" } else { "ret" }]);
                    }
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

    /// The last meaningful lines QEMU wrote, for error messages.
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
