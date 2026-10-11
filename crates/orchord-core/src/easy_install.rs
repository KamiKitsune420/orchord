//! Easy install: Orchord works the macOS installer itself.
//!
//! The driver sits between the window and the supervisor. It passes commands and events
//! through, and in between it takes its own pictures of the Mac's screen, reads them with
//! text recognition, and answers what it sees with clicks and typing:
//!
//! 1. In the installer (Recovery) it opens Terminal and erases the Mac's empty disk.
//! 2. It starts "Reinstall macOS", accepts the licence the user agreed to in the wizard,
//!    and picks the disk.
//! 3. It waits through the restarts, which the supervisor handles, saying how long is left.
//! 4. At the first screen of the installed system it turns VoiceOver on and steps aside.
//!
//! Every rule looks at what is on the screen now, not at what should have happened, so a slow
//! Mac only makes it wait. If it cannot make sense of the screen for too long it says so and
//! leaves the Mac running for the user to finish by hand.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::config::{InstallStage, Vm};
use crate::keys;
use crate::ocr::{self, ScreenText, Word};
use crate::qmp::POINTER_MAX;
use crate::settings::AppSettings;
use crate::supervisor::{PointerButton, StopReason, Supervisor, VmCommand, VmEvent};

/// How long after one look or action the next look is taken.
const LOOK_EVERY: Duration = Duration::from_secs(4);
/// While macOS installs itself there is nothing to do but watch.
const WATCH_EVERY: Duration = Duration::from_secs(15);
/// How long the installer may take to show its first window before easy install gives up.
const RECOVERY_START_LIMIT: Duration = Duration::from_secs(25 * 60);
const ERASE_LIMIT: Duration = Duration::from_secs(180);
/// The same answer to the same screen this many times in a row means it is not working.
const REPEAT_LIMIT: u32 = 10;
const PROGRESS_EVERY: Duration = Duration::from_secs(15 * 60);
const PICTURE_LIMIT: Duration = Duration::from_secs(30);

/// Finds the one disk with no partition table (the new Mac's empty disk) and erases it.
/// A disk that is already formatted leaves nothing to find; `$((6*7))` then prints 42, which
/// the typed command itself never shows, so the two outcomes can be told apart on screen.
const ERASE_COMMAND: &str = r#"D=$(diskutil list physical | awk '/^\/dev/{d=$1} $1=="0:"&&$2~/^\*/{print d}' | head -1); [ -n "$D" ] && diskutil eraseDisk APFS "Macintosh HD" GPT $D || echo orchord $((6*7))"#;

/// Runs the Mac like `Supervisor::run`, working the installer as it goes.
/// Blocks; call it on its own thread. The final `Stopped` event is always sent.
pub fn run(vm: Vm, settings: AppSettings, events: Sender<VmEvent>, commands: Receiver<VmCommand>) -> StopReason {
    let (inner_events_sender, inner_events) = mpsc::channel();
    let (inner_commands, inner_commands_receiver) = mpsc::channel();
    let supervisor = Supervisor::new(vm.clone(), settings);
    let worker = std::thread::spawn(move || supervisor.run(inner_events_sender, inner_commands_receiver));
    let mut driver = Driver::new(vm, inner_commands.clone(), events.clone());
    driver.note("Easy install is working. It prepares the Mac's disk, starts the installer and tells you when macOS is ready. This takes a few hours.");
    loop {
        for command in commands.try_iter() {
            let _ = inner_commands.send(command);
        }
        match inner_events.recv_timeout(Duration::from_millis(100)) {
            Ok(VmEvent::Screenshot(path)) if path == driver.picture => driver.look(),
            Ok(VmEvent::Stopped(reason)) => {
                let _ = events.send(VmEvent::Stopped(reason.clone()));
                let _ = worker.join();
                return reason;
            }
            Ok(event) => {
                driver.notice(&event);
                let _ = events.send(event);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let reason = worker.join().unwrap_or_else(|_| StopReason::Failed("The supervisor stopped unexpectedly.".to_string()));
                let _ = events.send(VmEvent::Stopped(reason.clone()));
                return reason;
            }
        }
        driver.ask_for_picture();
    }
}

struct Driver {
    vm: Vm,
    /// Where the driver's own pictures of the screen are saved; the window uses another file.
    picture: PathBuf,
    commands: Sender<VmCommand>,
    events: Sender<VmEvent>,
    running: bool,
    launch_started: Instant,
    next_look: Instant,
    /// When a picture was asked for, while it has not arrived yet.
    asked: Option<Instant>,
    terminal_seen: Option<Instant>,
    erase_typed: Option<Instant>,
    erased: bool,
    last_rule: Option<Rule>,
    repeats: u32,
    last_progress: Option<Instant>,
    /// Easy install has stopped trying; the Mac is the user's from here.
    stood_down: bool,
}

/// The answers the driver can give to a screen. Used to notice when one keeps being repeated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    OpenUtilitiesMenu,
    ChooseTerminal,
    TypeErase,
    QuitTerminal,
    ChooseReinstall,
    Agree,
    ChooseDisk,
    Continue,
}

impl Driver {
    fn new(vm: Vm, commands: Sender<VmCommand>, events: Sender<VmEvent>) -> Self {
        let picture = vm.config_dir().join("install-screen.ppm");
        let now = Instant::now();
        Driver { vm, picture, commands, events, running: false, launch_started: now, next_look: now, asked: None, terminal_seen: None, erase_typed: None, erased: false, last_rule: None, repeats: 0, last_progress: None, stood_down: false }
    }

    fn note(&self, text: &str) {
        let _ = self.events.send(VmEvent::InstallNote(text.to_string()));
    }

    /// Keeps track of what the supervisor reports.
    fn notice(&mut self, event: &VmEvent) {
        match event {
            VmEvent::Running { .. } => {
                self.running = true;
                self.asked = None;
                self.launch_started = Instant::now();
                self.next_look = Instant::now() + LOOK_EVERY;
                // A new start of the installer begins with no Terminal open.
                self.terminal_seen = None;
                self.erase_typed = None;
                self.last_rule = None;
                self.repeats = 0;
            }
            VmEvent::GuestRestart { .. } => self.running = false,
            VmEvent::InstallStage(stage) => {
                self.vm.config.install.stage = *stage;
                if *stage == InstallStage::Copied {
                    self.note("The first part of the installation is finished. The Mac restarts a few times now; macOS is installing itself.");
                }
            }
            VmEvent::Suspended => self.running = false,
            VmEvent::Resumed => self.running = true,
            _ => {}
        }
    }

    fn ask_for_picture(&mut self) {
        // A picture that fails to be taken is never announced, so an old request is forgotten.
        let waiting = self.asked.is_some_and(|asked| asked.elapsed() < PICTURE_LIMIT);
        if self.running && !self.stood_down && !waiting && self.vm.config.install.stage != InstallStage::Done && Instant::now() >= self.next_look {
            self.asked = Some(Instant::now());
            let _ = self.commands.send(VmCommand::Screenshot(self.picture.clone()));
        }
    }

    fn look(&mut self) {
        self.asked = None;
        self.next_look = Instant::now() + LOOK_EVERY;
        let Ok(screen) = ocr::read_ppm(&self.picture) else { return };
        match self.vm.config.install.stage {
            InstallStage::NotStarted => self.work_the_installer(&screen),
            InstallStage::Copied => self.watch_macos_install(&screen),
            InstallStage::Done => {}
        }
    }

    /// The installer (Recovery): erase the disk, then walk through "Reinstall macOS".
    fn work_the_installer(&mut self, screen: &ScreenText) {
        if self.say_progress(screen) {
            return;
        }
        let terminal_open = screen.lines.iter().flat_map(|line| &line.words).any(|word| word.text.to_lowercase().contains("bash"));
        let main_window = screen.contains("Reinstall macOS") && screen.contains("Disk Utility");
        if !self.erased {
            if let Some(typed) = self.erase_typed {
                let mistyped = screen.contains("syntax error") || screen.contains("command not found");
                if mistyped && typed.elapsed() > LOOK_EVERY {
                    // A key went missing on the way. Typing again starts by clearing the window.
                    self.erase_typed = None;
                } else if screen.contains("Finished erase") || screen.contains("orchord 42") {
                    self.erased = true;
                    self.last_rule = None;
                    self.note("The Mac's disk is ready. Starting the macOS installer.");
                    self.press(&["meta_l", "q"]);
                } else if typed.elapsed() > ERASE_LIMIT {
                    self.stand_down("it could not prepare the Mac's disk");
                }
            } else if terminal_open {
                // Terminal shows its window a moment before it takes keys; the first look only notes it.
                let ready = self.terminal_seen.get_or_insert_with(Instant::now).elapsed() >= LOOK_EVERY;
                if !ready || self.repeated(Rule::TypeErase) {
                    return;
                }
                // Control+C drops anything half typed and Control+L wipes the window, so that
                // an earlier attempt's error message is not read as this attempt's.
                self.press(&["ctrl", "c"]);
                self.press(&["ctrl", "l"]);
                std::thread::sleep(Duration::from_secs(1));
                self.type_text(ERASE_COMMAND);
                self.press(&["ret"]);
                self.erase_typed = Some(Instant::now());
            } else if let Some(item) = screen.find_exact("Terminal").into_iter().find(|word| word.y > 30.0) {
                self.answer(Rule::ChooseTerminal, screen, item);
            } else if let Some(menu) = screen.find_exact("Utilities").into_iter().find(|word| word.y < 40.0).filter(|_| main_window) {
                self.answer(Rule::OpenUtilitiesMenu, screen, menu);
            } else if self.launch_started.elapsed() > RECOVERY_START_LIMIT {
                self.stand_down("the installer did not show its first window");
            }
            return;
        }

        if terminal_open {
            if !self.repeated(Rule::QuitTerminal) {
                self.press(&["meta_l", "q"]);
            }
        } else if let Some(disk) = self.disk_to_choose(screen) {
            if !self.repeated(Rule::ChooseDisk) {
                self.click(screen, disk);
                std::thread::sleep(Duration::from_millis(1500));
                // The button under the disks reads Continue or Install, depending on the version.
                if let Some(button) = screen.find_exact("Continue").into_iter().chain(screen.find_exact("Install")).last() {
                    self.click(screen, button);
                }
            }
        } else if let Some(button) = agree_button(screen) {
            self.answer(Rule::Agree, screen, button);
        } else if main_window {
            if !self.repeated(Rule::ChooseReinstall) {
                if let Some(choice) = screen.find("Reinstall macOS") {
                    self.click(screen, choice);
                    std::thread::sleep(Duration::from_millis(1000));
                }
                if let Some(button) = screen.find_exact("Continue").into_iter().last() {
                    self.click(screen, button);
                }
            }
        } else if let Some(button) = screen.find_exact("Continue").into_iter().last() {
            // Sentences such as "click Continue" sit above the button, so the lowest one is the button.
            self.answer(Rule::Continue, screen, button);
        }
    }

    /// The disk to click on the "select the disk" page, if that page is showing.
    fn disk_to_choose<'a>(&self, screen: &'a ScreenText) -> Option<&'a Word> {
        if !(screen.contains("Select the disk") || screen.contains("where you want to install")) {
            return None;
        }
        // The disk's own caption is a line of its own; sentences naming the disk are not.
        let caption = screen.lines.iter().find(|line| line.text.trim().eq_ignore_ascii_case("Macintosh HD")).and_then(|line| line.words.first());
        caption.or_else(|| screen.find("Macintosh HD"))
    }

    /// After the installer has moved to the Mac's own disk: wait for the installed system.
    fn watch_macos_install(&mut self, screen: &ScreenText) {
        self.next_look = Instant::now() + WATCH_EVERY;
        if screen.contains("Country or Region") || screen.contains("Select Your Country") {
            if self.vm.set_install_stage(InstallStage::Done).is_ok() {
                let _ = self.events.send(VmEvent::InstallStage(InstallStage::Done));
            }
            self.vm.config.install.stage = InstallStage::Done;
            // The installer image is only needed again if the user installs from scratch.
            if let Some(media) = self.vm.config.install.media.as_ref().filter(|media| media.is_relative()) {
                let _ = std::fs::remove_file(self.vm.dir.join(media));
            }
            let _ = std::fs::remove_file(&self.picture);
            // Command+F5 turns VoiceOver on, so the Mac itself says where it is.
            self.press(&["meta_l", "f5"]);
            self.note("macOS is installed. The Mac is at its first set-up screen, Select Your Country or Region, and VoiceOver has been turned on. Press Control+G to send the keyboard to the Mac and finish setting it up.");
        } else {
            self.say_progress(screen);
        }
    }

    /// Passes on the installer's own "about 20 minutes remaining", now and then.
    /// Returns whether the screen shows an installation in progress.
    fn say_progress(&mut self, screen: &ScreenText) -> bool {
        let Some(line) = screen.lines.iter().find(|line| line.text.to_lowercase().contains("remaining")) else { return false };
        if self.last_progress.is_none_or(|last| last.elapsed() >= PROGRESS_EVERY) {
            self.last_progress = Some(Instant::now());
            self.note(&format!("Installing. The Mac says: {}.", line.text.trim().trim_end_matches('.')));
        }
        self.next_look = Instant::now() + WATCH_EVERY;
        true
    }

    /// Clicks `target` as the answer of `rule`, unless that answer keeps not working.
    fn answer(&mut self, rule: Rule, screen: &ScreenText, target: &Word) {
        if !self.repeated(rule) {
            self.click(screen, target);
        }
    }

    /// Counts how often the same rule fired in a row. True means easy install has given up.
    fn repeated(&mut self, rule: Rule) -> bool {
        self.repeats = if self.last_rule == Some(rule) { self.repeats + 1 } else { 0 };
        self.last_rule = Some(rule);
        if self.repeats >= REPEAT_LIMIT {
            self.stand_down(match rule {
                Rule::OpenUtilitiesMenu | Rule::ChooseTerminal | Rule::TypeErase | Rule::QuitTerminal => "it could not use Terminal in the installer",
                Rule::ChooseReinstall | Rule::Continue => "the installer did not move on",
                Rule::Agree => "the licence page did not move on",
                Rule::ChooseDisk => "the installer did not accept the Mac's disk",
            });
        }
        self.stood_down
    }

    fn stand_down(&mut self, why: &str) {
        self.stood_down = true;
        self.note(&format!("Easy install has stopped because {why}. The Mac is still running: you can finish the installation yourself, or power it off from the Machine menu and start it again to let easy install retry."));
    }

    fn click(&self, screen: &ScreenText, target: &Word) {
        let (x, y) = target.centre();
        let scale = |value: f32, size: u32| ((value / size.max(1) as f32).clamp(0.0, 1.0) * POINTER_MAX as f32) as u16;
        let _ = self.commands.send(VmCommand::PointerMove { x: scale(x, screen.width), y: scale(y, screen.height) });
        std::thread::sleep(Duration::from_millis(350));
        let _ = self.commands.send(VmCommand::PointerButton { button: PointerButton::Left, down: true });
        std::thread::sleep(Duration::from_millis(130));
        let _ = self.commands.send(VmCommand::PointerButton { button: PointerButton::Left, down: false });
    }

    fn press(&self, keys: &[&str]) {
        keys::press(&self.commands, keys);
    }

    fn type_text(&self, text: &str) {
        keys::type_text(&self.commands, text);
    }
}

/// The licence's Agree button: the one on the confirmation sheet when that is showing,
/// otherwise the one at the bottom of the licence page.
fn agree_button(screen: &ScreenText) -> Option<&Word> {
    let buttons = screen.find_exact("Agree");
    match screen.find("Read License") {
        // The sheet's buttons sit side by side, so its Agree is the one level with Read License.
        Some(sheet) => buttons.into_iter().min_by(|a, b| (a.y - sheet.y).abs().total_cmp(&(b.y - sheet.y).abs())),
        None => buttons.into_iter().last(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ERASE_COMMAND, agree_button};
    use crate::keys::keys_for;
    use crate::ocr::{Line, ScreenText, Word};

    fn word(text: &str, x: f32, y: f32) -> Word {
        Word { text: text.to_string(), x, y, width: 40.0, height: 12.0 }
    }

    fn screen(lines: Vec<Vec<Word>>) -> ScreenText {
        let lines = lines.into_iter().map(|words| Line { text: words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" "), words }).collect();
        ScreenText { width: 1280, height: 800, lines }
    }

    #[test]
    fn every_character_of_the_erase_command_can_be_typed() {
        let missing: Vec<char> = ERASE_COMMAND.chars().filter(|character| keys_for(*character).is_none()).collect();
        assert!(missing.is_empty(), "no keys for {missing:?}");
        assert_eq!(keys_for('$'), Some(vec!["shift", "4"]));
        assert_eq!(keys_for('d'), Some(vec!["d"]));
    }

    #[test]
    fn the_licence_page_button_is_the_lowest_agree() {
        let page = screen(vec![vec![word("Agree", 100.0, 200.0), word("to", 150.0, 200.0)], vec![word("Disagree", 900.0, 700.0), word("Agree", 1000.0, 700.0)]]);
        assert_eq!(agree_button(&page).map(|word| word.y), Some(700.0));
    }

    #[test]
    fn with_the_confirmation_sheet_showing_its_own_agree_is_chosen() {
        let sheet = screen(vec![
            vec![word("Read", 500.0, 150.0), word("License", 550.0, 150.0), word("Disagree", 650.0, 150.0), word("Agree", 760.0, 151.0)],
            vec![word("Disagree", 900.0, 700.0), word("Agree", 1000.0, 700.0)],
        ]);
        assert_eq!(agree_button(&sheet).map(|word| word.y), Some(151.0));
    }
}
