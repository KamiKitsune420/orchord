//! The main window. It has two faces: the list of virtual machines, and, while one is
//! running, that Mac's console. The console shows the Mac's screen and nothing else, so that
//! a screen reader's text recognition reads the Mac and not Orchord; a status log can be
//! swapped in for the screen. The mouse always works over that screen, captured keyboard or
//! not, so that a screen reader can route the mouse to recognised text and click it.
//!
//! Every command is a menu item, so nothing depends on a shortcut or on the mouse. The list is
//! a native list box and the log a native text field, which screen readers read unaided.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use orchord_core::config::list_vms;
use orchord_core::create::{self, Step};
use orchord_core::file_server::{self, FileServer};
use orchord_core::{AppSettings, Appearance, InstallStage, POINTER_MAX, PointerButton, StopReason, Supervisor, Vm, VmCommand, VmEvent, diagnostics, easy_install, keys, ocr, sound, update};
use wxdragon::prelude::*;
use wxdragon::widgets::GenericStaticBitmap;

use crate::{busy, keyboard_capture, program_settings, settings_dialog, speech, updater, usb_dialog, wizard};

const ID_NEW_VM: Id = ID_HIGHEST + 1;
const ID_START: Id = ID_HIGHEST + 2;
const ID_SETTINGS: Id = ID_HIGHEST + 3;
const ID_OPEN_FOLDER: Id = ID_HIGHEST + 5;
const ID_REFRESH: Id = ID_HIGHEST + 6;
const ID_CAPTURE: Id = ID_HIGHEST + 7;
const ID_POWER_RESTART: Id = ID_HIGHEST + 10;
const ID_POWER_SHUT_DOWN: Id = ID_HIGHEST + 11;
const ID_POWER_SUSPEND: Id = ID_HIGHEST + 12;
const ID_FORCE_RESTART: Id = ID_HIGHEST + 13;
const ID_FORCE_OFF: Id = ID_HIGHEST + 14;
const ID_TOGGLE_LOG: Id = ID_HIGHEST + 15;
const ID_OPEN_SHARED: Id = ID_HIGHEST + 16;
const ID_CONNECT_SHARED: Id = ID_HIGHEST + 17;
const ID_USB: Id = ID_HIGHEST + 18;
const ID_RIGHT_CONTROL: Id = ID_HIGHEST + 19;
const ID_PROGRAM_SETTINGS: Id = ID_HIGHEST + 20;
const ID_CHECK_UPDATES: Id = ID_HIGHEST + 21;
const ID_FULL_SCREEN: Id = ID_HIGHEST + 22;
const ID_DELETE: Id = ID_HIGHEST + 23;
const ID_DIAGNOSTICS: Id = ID_HIGHEST + 24;
/// Added to every message about Orchord not being set up, so the user knows where to fix it.
const SETTINGS_HINT: &str = "Open Orchord Settings in the File menu, or press Control+Comma.";

const EMPTY_LIST_TEXT: &str = "No virtual machines created yet. To create one, go to File > New Virtual Machine, or press Control plus N.";
const TICK_MS: i32 = 250;
/// The screen picture is refreshed every this many ticks.
const SCREEN_EVERY_TICKS: u32 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunState {
    Stopped,
    Starting,
    Running,
    Restarting,
    Suspended,
}

impl RunState {
    fn label(self) -> &'static str {
        match self {
            RunState::Stopped => "stopped",
            RunState::Starting => "starting",
            RunState::Running => "running",
            RunState::Restarting => "restarting",
            RunState::Suspended => "suspended",
        }
    }
}

/// One line of the list: a virtual machine and, while it runs, the channels to its supervisor.
struct Row {
    vm: Vm,
    state: RunState,
    events: Option<Receiver<VmEvent>>,
    commands: Option<Sender<VmCommand>>,
    /// Serves the shared folder while the Mac runs, if the folder is turned on.
    file_server: Option<FileServer>,
}

impl Row {
    fn new(vm: Vm) -> Self {
        Row { vm, state: RunState::Stopped, events: None, commands: None, file_server: None }
    }

    fn is_active(&self) -> bool {
        self.state != RunState::Stopped
    }

    fn list_text(&self) -> String {
        let install = match self.vm.config.install.stage {
            InstallStage::NotStarted => ", macOS not installed yet",
            InstallStage::Copied => ", macOS installation not finished",
            InstallStage::Done => "",
        };
        format!("{}, {}, {}{install}", self.vm.config.name, self.vm.config.macos.display_name(), self.state.label())
    }

    /// Whether Orchord itself is working this Mac's installer, so the user's mouse must keep out.
    fn easy_install_active(&self) -> bool {
        self.vm.config.install.easy && self.vm.config.install.stage != InstallStage::Done
    }
}

/// A virtual machine being created on a background thread, and the dialog showing how far it is.
struct Creation {
    name: String,
    boot_after: bool,
    messages: Receiver<CreationMessage>,
    cancel: Arc<AtomicBool>,
    dialog: ProgressDialog,
    shown: Option<Shown>,
}

/// What the creation dialog last showed, and when.
struct Shown {
    step: std::mem::Discriminant<Step>,
    percent: i32,
    bar_at: Instant,
    text_at: Instant,
}

/// The creation dialog's bar moves at most this often, and its words change at most this
/// often while one step lasts. Every change is an event the screen reader has to deal with;
/// a download that reported each megabyte sent it dozens a second, and it froze.
const PROGRESS_BAR_EVERY: Duration = Duration::from_secs(1);
const PROGRESS_TEXT_EVERY: Duration = Duration::from_secs(10);

impl Creation {
    fn show(&mut self, step: &Step) {
        let (percent, text) = creation_progress(step);
        let (kind, now) = (std::mem::discriminant(step), Instant::now());
        match &mut self.shown {
            Some(shown) if shown.step == kind => {
                if now.duration_since(shown.text_at) >= PROGRESS_TEXT_EVERY {
                    self.dialog.update(percent, Some(&text));
                    (shown.percent, shown.bar_at, shown.text_at) = (percent, now, now);
                } else if percent != shown.percent && now.duration_since(shown.bar_at) >= PROGRESS_BAR_EVERY {
                    self.dialog.update(percent, None);
                    (shown.percent, shown.bar_at) = (percent, now);
                } else {
                    self.dialog.update(shown.percent, None);
                }
            }
            _ => {
                self.dialog.update(percent, Some(&text));
                self.shown = Some(Shown { step: kind, percent, bar_at: now, text_at: now });
            }
        }
    }
}

enum CreationMessage {
    Step(Step),
    Finished(Result<Vm, String>),
}

struct State {
    settings: AppSettings,
    rows: Vec<Row>,
    /// The row whose console is showing, if any.
    console: Option<usize>,
    ticks: u32,
    last_frame: Vec<u8>,
    /// Width and height of the Mac's screen in its own pixels, once a picture has arrived.
    frame_size: Option<(u32, u32)>,
    /// Where the Mac's pointer was last sent, so that a mouse at rest sends nothing.
    last_pointer: Cell<Option<(u16, u16)>>,
    /// The mouse buttons currently held down in the Mac.
    held_buttons: Cell<[bool; 3]>,
    creation: Option<Creation>,
    /// Where word of a newer version arrives, if Orchord asked when it started.
    update_news: Option<Receiver<update::Release>>,
    /// Lines for the status log from work done on other threads, and the end they are sent from.
    notes: Receiver<String>,
    note_sender: Sender<String>,
}

type Shared = Rc<RefCell<State>>;

/// The widgets every command needs. All of them are cheap handles.
#[derive(Clone, Copy)]
struct Ui {
    frame: Frame,
    panel: Panel,
    list_label: StaticText,
    list: ListBox,
    /// Holds the picture of the Mac's screen. A picture cannot take the focus, so this does,
    /// which is what makes the screen the object a screen reader recognises text in.
    screen_panel: Panel,
    screen: GenericStaticBitmap,
    log_label: StaticText,
    log: TextCtrl,
}

pub fn show() {
    let settings = AppSettings::load().unwrap_or_default();
    // Light or dark has to be chosen before the first window exists.
    wxdragon::app::set_appearance(match settings.appearance {
        Appearance::System => wxdragon::appearance::Appearance::System,
        Appearance::Light => wxdragon::appearance::Appearance::Light,
        Appearance::Dark => wxdragon::appearance::Appearance::Dark,
    });
    // An update leaves the files it replaced behind, because the program they belonged to was running.
    if let Ok(folder) = update::program_folder() {
        update::remove_leftovers(&folder);
    }
    let update_news = settings.check_for_updates.then(updater::check_in_background);
    let rows = list_vms(&settings.vm_root, &settings.other_vms).into_iter().map(Row::new).collect();
    let (note_sender, notes) = mpsc::channel();
    let state: Shared = Rc::new(RefCell::new(State { settings, rows, console: None, ticks: 0, last_frame: Vec::new(), frame_size: None, last_pointer: Cell::new(None), held_buttons: Cell::new([false; 3]), creation: None, update_news, notes, note_sender }));

    let frame = Frame::builder().with_title("Orchord").with_size(Size::new(900, 700)).build();
    let panel = Panel::builder(&frame).build();
    // wxWidgets would otherwise present the panel to screen readers as an object called "panel".
    panel.set_name("");
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    // Each label is created immediately before its control so that it becomes the control's name.
    let list_label = StaticText::builder(&panel).with_label("&Virtual machines:").build();
    let list = ListBox::builder(&panel).build();
    // The picture control cannot be created empty, so it starts with a small black square.
    let blank = Bitmap::from_rgba(&[0, 0, 0, 255].repeat(16 * 16), 16, 16);
    let screen_panel = Panel::builder(&panel).build();
    screen_panel.set_background_color(Colour::new(0, 0, 0, 255));
    // Without a name of its own, the screen is announced with the list's label.
    screen_panel.set_label("Mac screen");
    screen_panel.set_accessibility_label("Mac screen");
    // The generic picture control, not the native one: the native one never resizes its picture,
    // it only centres it, and crops it when the window is smaller than the Mac's screen.
    let screen = GenericStaticBitmap::builder(&screen_panel).with_bitmap(blank).with_scale_mode(Some(ScaleMode::AspectFit)).build();
    screen.set_background_color(Colour::new(0, 0, 0, 255));
    let screen_sizer = BoxSizer::builder(Orientation::Vertical).build();
    screen_sizer.add(&screen, 1, SizerFlag::Expand, 0);
    screen_panel.set_sizer(screen_sizer, true);
    let log_label = StaticText::builder(&panel).with_label("Mac &status:").build();
    let log = TextCtrl::builder(&panel).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).build();
    sizer.add(&list_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 8);
    // No border: every pixel given to the screen makes its text easier to recognise.
    sizer.add(&screen_panel, 1, SizerFlag::Expand, 0);
    sizer.add(&log_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&log, 1, SizerFlag::Expand | SizerFlag::All, 8);
    panel.set_sizer(sizer, true);
    let ui = Ui { frame, panel, list_label, list, screen_panel, screen, log_label, log };

    bind_pointer(ui, &state);
    frame.set_menu_bar(build_menu_bar());
    let right_control = state.borrow().settings.right_control_is_command;
    keyboard_capture::set_right_control_is_command(right_control);
    if let Some(item) = frame.get_menu_bar().and_then(|bar| bar.find_item(ID_RIGHT_CONTROL)) {
        item.check(right_control);
    }
    frame.create_status_bar(1, 0, ID_ANY as Id, "status");
    fill_list(list, &state.borrow());
    show_list(ui);
    report_setup_problems(frame, &state.borrow());

    {
        let state = state.clone();
        frame.on_menu_selected(move |event| match event.get_id() {
            ID_NEW_VM => new_vm(ui, &state),
            ID_START => start_selected(ui, &state),
            ID_SETTINGS => edit_selected(ui, &state),
            ID_OPEN_FOLDER => open_selected_folder(ui, &state),
            ID_REFRESH => reload(ui, &state),
            ID_PROGRAM_SETTINGS => program_settings(ui, &state),
            ID_CHECK_UPDATES => check_for_updates(ui, &state),
            ID_FULL_SCREEN => toggle_full_screen(ui),
            ID_DELETE => delete_selected(ui, &state),
            ID_DIAGNOSTICS => save_diagnostics(ui, &state),
            ID_CAPTURE => capture_keyboard(ui, &state),
            ID_TOGGLE_LOG => toggle_log(ui, &state),
            ID_OPEN_SHARED => open_shared_folder(ui, &state),
            ID_CONNECT_SHARED => connect_shared_folder(ui, &state),
            ID_USB => usb_devices(ui, &state),
            ID_RIGHT_CONTROL => toggle_right_control(ui, &state),
            ID_POWER_RESTART => power_restart(ui, &state),
            ID_POWER_SHUT_DOWN => power_shut_down(ui, &state),
            ID_POWER_SUSPEND => power_suspend(ui, &state),
            ID_FORCE_RESTART => force_restart(ui, &state),
            ID_FORCE_OFF => force_off(ui, &state),
            ID_EXIT => frame.close(false),
            _ => {}
        });
    }

    // Supervisors run on their own threads; this timer carries their events to the window.
    let timer = Timer::new(&frame);
    {
        let state = state.clone();
        timer.on_tick(move |_| tick(ui, &state));
    }
    timer.start(TICK_MS, false);
    // The timer must live as long as the window; dropping it would stop it.
    Box::leak(Box::new(timer));

    {
        let state = state.clone();
        frame.on_close(move |event| {
            let running = state.borrow().rows.iter().filter(|row| row.is_active()).count();
            if running == 0 {
                return;
            }
            let question = if running == 1 {
                "One virtual machine is still running. Closing Orchord will power it off without shutting macOS down. Close anyway?".to_string()
            } else {
                format!("{running} virtual machines are still running. Closing Orchord will power them off without shutting macOS down. Close anyway?")
            };
            if confirm(frame, &question, "Orchord") {
                keyboard_capture::release();
                for row in state.borrow().rows.iter() {
                    if let Some(commands) = &row.commands {
                        let _ = commands.send(VmCommand::ForceStop);
                    }
                }
            } else {
                if let WindowEventData::General(inner) = &event {
                    inner.veto();
                }
                event.skip(false);
            }
        });
    }

    frame.show(true);
    frame.centre();
    list.set_focus();
}

fn build_menu_bar() -> MenuBar {
    let file = Menu::builder()
        .append_item(ID_NEW_VM, "&New Virtual Machine...\tCtrl+N", "Create a new virtual machine")
        .append_item(ID_REFRESH, "&Refresh List\tF5", "Read the list of virtual machines again")
        .append_item(ID_PROGRAM_SETTINGS, "&Orchord Settings...\tCtrl+,", "Where QEMU is installed, where virtual machines are kept, and the Apple SMC key")
        .append_item(ID_CHECK_UPDATES, "Check for &Updates...", "See whether a newer version of Orchord has been published, and install it")
        .append_separator()
        .append_item(ID_EXIT, "E&xit\tAlt+F4", "Close Orchord")
        .build();
    let machine = Menu::builder()
        .append_item(ID_START, "&Start\tEnter", "Start the selected virtual machine")
        .append_item(ID_CAPTURE, "Send &Keyboard to the Mac\tCtrl+G", "Send every key to the Mac. Press and release Control and Alt together to get the keyboard back.")
        .append_check_item(ID_RIGHT_CONTROL, "Right Control Works as Co&mmand", "While the keyboard is captured, Right Control and the Menu key act as the Mac's Command key, so that Command+L can be typed")
        .append_item(ID_USB, "&USB Devices...	Ctrl+U", "Connect a USB device plugged into this PC to the Mac, or give one back to Windows")
        .append_item(ID_TOGGLE_LOG, "Switch Between Mac Screen and Status &Log\tCtrl+L", "Show the status log in place of the Mac's screen, or the screen again")
        .append_item(ID_FULL_SCREEN, "Full Scr&een\tF11", "Let the window fill the whole screen, without its title, menus and status line, or bring them back")
        .append_separator()
        .append_item(ID_SETTINGS, "Se&ttings...\tCtrl+P", "Change the virtual machine's settings")
        .append_item(ID_OPEN_FOLDER, "Open &Folder", "Open the virtual machine's folder in File Explorer")
        .append_item(ID_OPEN_SHARED, "Open S&hared Folder", "Open the folder shared with the Mac in File Explorer")
        .append_item(ID_CONNECT_SHARED, "&Connect Shared Folder in the Mac...", "Make the Mac connect the shared folder now and every time you log in to it")
        .append_item(ID_DIAGNOSTICS, "Save Dia&gnostic Report", "Write a report about a virtual machine that will not start, to send to someone who can help")
        .append_separator()
        .append_item(ID_FORCE_RESTART, "Force &Restart", "Power the Mac off and on again without shutting macOS down")
        .append_item(ID_FORCE_OFF, "Force Power &Off", "Power the Mac off immediately without shutting macOS down")
        .append_separator()
        .append_item(ID_DELETE, "&Delete...\tDel", "Delete the selected virtual machine and its disk from this PC")
        .build();
    let power = Menu::builder()
        .append_item(ID_POWER_RESTART, "&Restart", "Ask macOS to restart")
        .append_item(ID_POWER_SHUT_DOWN, "&Shut Down", "Ask macOS to shut down")
        .append_item(ID_POWER_SUSPEND, "S&uspend", "Freeze the Mac where it is, or wake it again")
        .build();
    MenuBar::builder().append(file, "&File").append(machine, "&Machine").append(power, "&Power").build()
}

/// Shows a message in the status bar and speaks it through the screen reader. It is the one
/// place for short, non-urgent feedback.
fn say(frame: Frame, text: &str) {
    frame.set_status_text(text, 0);
    speech::speak(text, false);
}

/// Adds a line to the console's status log and repeats it in the status bar. The log is only
/// on show when the user swaps it in, so the status bar is where a message is first met.
fn log_line(ui: Ui, text: &str) {
    ui.log.append_text(&format!("{text}\n"));
    say(ui.frame, text);
}

fn confirm(frame: Frame, question: &str, caption: &str) -> bool {
    MessageDialog::builder(&frame, question, caption).with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconWarning).build().show_modal() == ID_YES
}

fn show_error(frame: Frame, text: &str) {
    MessageDialog::builder(&frame, text, "Orchord").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconError).build().show_modal();
}

/// Shows the list of virtual machines and hides the console.
fn show_list(ui: Ui) {
    ui.screen_panel.hide();
    ui.log_label.hide();
    ui.log.hide();
    ui.list_label.show(true);
    ui.list.show(true);
    ui.panel.layout();
    ui.frame.set_title("Orchord");
    ui.list.set_focus();
}

/// Hides the list and shows the console of the Mac in `row`, with its screen filling the window.
fn show_console(ui: Ui, row: &Row) {
    ui.list_label.hide();
    ui.list.hide();
    ui.log.clear();
    // The bigger the picture, the better text recognition reads it.
    ui.frame.maximize(true);
    set_console_title(ui, row);
    show_screen(ui);
}

/// Console only: shows the Mac's screen and puts the status log away.
fn show_screen(ui: Ui) {
    ui.log_label.hide();
    ui.log.hide();
    ui.screen_panel.show(true);
    ui.panel.layout();
    ui.screen_panel.set_focus();
}

/// Console only: shows the status log in place of the Mac's screen.
fn show_log(ui: Ui) {
    ui.screen_panel.hide();
    ui.log_label.show(true);
    ui.log.show(true);
    ui.panel.layout();
    ui.log.set_focus();
}

fn toggle_log(ui: Ui, state: &Shared) {
    if state.borrow().console.is_none() {
        say(ui.frame, "The status log belongs to a running virtual machine. None is running.");
    } else if ui.log.is_shown() {
        show_screen(ui);
        say(ui.frame, "Showing the Mac's screen.");
    } else {
        show_log(ui);
    }
}

fn set_console_title(ui: Ui, row: &Row) {
    let captured = if keyboard_capture::is_captured() { ", keyboard captured" } else { "" };
    ui.frame.set_title(&format!("Orchord, {}, {}{captured}", row.vm.config.name, row.vm.config.macos.display_name()));
}

fn fill_list(list: ListBox, state: &State) {
    let selection = list.get_selection().unwrap_or(0);
    list.clear();
    if state.rows.is_empty() {
        list.append(EMPTY_LIST_TEXT);
    } else {
        for row in &state.rows {
            list.append(&row.list_text());
        }
    }
    list.set_selection(selection.min(list.get_count().saturating_sub(1)), true);
}

fn report_setup_problems(frame: Frame, state: &State) {
    match state.settings.problems().first() {
        Some(problem) => say(frame, &format!("Orchord is not fully set up. {problem} {SETTINGS_HINT}")),
        None if state.rows.len() == 1 => say(frame, "1 virtual machine."),
        None => say(frame, &format!("{} virtual machines.", state.rows.len())),
    }
}

/// The row a command applies to: the Mac whose console is showing, otherwise the selected
/// list line. `None` when the list holds only the placeholder.
fn target_index(ui: Ui, state: &State) -> Option<usize> {
    let index = state.console.or_else(|| ui.list.get_selection().map(|index| index as usize).filter(|&index| index < state.rows.len()));
    if index.is_none() {
        say(ui.frame, "No virtual machine selected.");
    }
    index
}

/// The command channel of the running Mac a power command applies to, with its name.
fn running_target(ui: Ui, state: &State) -> Option<(usize, Sender<VmCommand>, String)> {
    let index = target_index(ui, state)?;
    let row = &state.rows[index];
    match &row.commands {
        Some(commands) => Some((index, commands.clone(), row.vm.config.name.clone())),
        None => {
            say(ui.frame, &format!("{} is not running.", row.vm.config.name));
            None
        }
    }
}

fn start_selected(ui: Ui, state: &Shared) {
    let mut state = state.borrow_mut();
    let Some(index) = target_index(ui, &state) else { return };
    let settings = state.settings.clone();
    let notes = state.note_sender.clone();
    let row = &mut state.rows[index];
    if row.is_active() {
        say(ui.frame, &format!("{} is already {}.", row.vm.config.name, row.state.label()));
        return;
    }
    let (event_sender, event_receiver) = mpsc::channel();
    let (command_sender, command_receiver) = mpsc::channel();
    let vm = row.vm.clone();
    if row.easy_install_active() {
        std::thread::spawn(move || easy_install::run(vm, settings, event_sender, command_receiver));
    } else {
        std::thread::spawn(move || Supervisor::new(vm, settings).run(event_sender, command_receiver));
    }
    row.events = Some(event_receiver);
    row.commands = Some(command_sender);
    row.state = RunState::Starting;
    ui.list.set_string(index as u32, &row.list_text());
    show_console(ui, row);
    log_line(ui, &format!("{} is starting. Press Control+G to send the keyboard to the Mac, Alt+P for the power menu, and Control+L to switch between the Mac's screen and this status log.", row.vm.config.name));
    start_file_server(ui, row);
    // Listing the PC's running programs can take a second, so it is not done on the window's thread.
    std::thread::spawn(move || {
        for cleaner in orchord_core::host::memory_cleaners_running() {
            let _ = notes.send(format!("Warning: {cleaner} is running on this PC. Memory cleaners take a running Mac's memory away and can stall it for a long time. Close {cleaner} while the Mac runs."));
        }
    });
    match (row.vm.config.install.stage, row.vm.config.install.easy) {
        (InstallStage::Done, _) | (_, true) => {}
        (InstallStage::NotStarted, false) => log_line(ui, "This Mac has no macOS yet, so it is starting from the installer. When it has started, press Control+G and then Windows+F5, which is Command+F5 on the Mac, to turn VoiceOver on."),
        (InstallStage::Copied, false) => log_line(ui, "macOS is still installing on this Mac. It restarts by itself a few times."),
    }
    state.console = Some(index);
    state.last_frame.clear();
    state.frame_size = None;
    state.last_pointer.set(None);
    state.held_buttons.set([false; 3]);
}

fn capture_keyboard(ui: Ui, state: &Shared) {
    let state = state.borrow();
    let Some((index, commands, name)) = running_target(ui, &state) else { return };
    if state.console != Some(index) {
        say(ui.frame, &format!("{name} is not on screen."));
        return;
    }
    if keyboard_capture::capture(ui.frame.get_handle(), commands) {
        sound::play(sound::KEYBOARD_CAPTURED);
        set_console_title(ui, &state.rows[index]);
        log_line(ui, "Keyboard captured. Every key now goes to the Mac. Press and release Control and Alt together to get it back.");
    } else {
        show_error(ui.frame, "Windows did not allow Orchord to capture the keyboard.");
    }
}

fn release_keyboard(ui: Ui, state: &State) {
    if !keyboard_capture::is_captured() {
        return;
    }
    keyboard_capture::release();
    sound::play(sound::KEYBOARD_RELEASED);
    if let Some(index) = state.console {
        set_console_title(ui, &state.rows[index]);
    }
    log_line(ui, "Keyboard released.");
}

fn power_restart(ui: Ui, state: &Shared) {
    let Some((_, commands, name)) = running_target(ui, &state.borrow()) else { return };
    let _ = commands.send(VmCommand::AskRestart);
    log_line(ui, &format!("Asked {name} to restart. If nothing happens, nobody is logged in to the Mac; use Force Restart in the Machine menu."));
}

fn power_shut_down(ui: Ui, state: &Shared) {
    let Some((_, commands, name)) = running_target(ui, &state.borrow()) else { return };
    let _ = commands.send(VmCommand::AskShutDown);
    log_line(ui, &format!("Asked {name} to shut down. If nothing happens, nobody is logged in to the Mac; use Force Power Off in the Machine menu."));
}

fn force_restart(ui: Ui, state: &Shared) {
    let Some((_, commands, name)) = running_target(ui, &state.borrow()) else { return };
    let question = format!("Force {name} to restart now? This is like pressing a reset button: macOS is not shut down first, and unsaved work in the Mac will be lost.");
    if confirm(ui.frame, &question, "Force Restart") {
        let _ = commands.send(VmCommand::Reset);
    }
}

fn force_off(ui: Ui, state: &Shared) {
    let Some((_, commands, name)) = running_target(ui, &state.borrow()) else { return };
    let question = format!("Power off {name} now? macOS is not shut down first, and unsaved work in the Mac will be lost.");
    if confirm(ui.frame, &question, "Force Power Off") {
        let _ = commands.send(VmCommand::ForceStop);
    }
}

fn power_suspend(ui: Ui, state: &Shared) {
    let state = state.borrow();
    let Some((index, commands, _)) = running_target(ui, &state) else { return };
    let command = if state.rows[index].state == RunState::Suspended { VmCommand::Resume } else { VmCommand::Suspend };
    let _ = commands.send(command);
}

fn edit_selected(ui: Ui, state: &Shared) {
    // The dialog runs its own event loop and the timer ticks inside it, so the state must
    // not stay borrowed while the dialog is open.
    let (index, current, active) = {
        let state = state.borrow();
        let Some(index) = target_index(ui, &state) else { return };
        (index, state.rows[index].vm.config.clone(), state.rows[index].is_active())
    };
    let Some(config) = settings_dialog::edit(ui.frame, &current, active) else { return };
    let mut state = state.borrow_mut();
    let row = &mut state.rows[index];
    row.vm.config = config;
    match row.vm.save() {
        Ok(()) if row.is_active() => say(ui.frame, &format!("Settings saved. They take effect the next time {} starts.", row.vm.config.name)),
        Ok(()) => say(ui.frame, "Settings saved."),
        Err(error) => show_error(ui.frame, &format!("The settings could not be saved. {error:#}")),
    }
    ui.list.set_string(index as u32, &row.list_text());
}

/// Starts serving the Mac's shared folder, if the user has turned it on in the settings.
fn start_file_server(ui: Ui, row: &mut Row) {
    if !row.vm.config.shared_folder.enabled || row.vm.config.install.stage != InstallStage::Done {
        return;
    }
    let folder = row.vm.shared_folder_path();
    // The port is kept from one run to the next because the Mac remembers the address. If
    // another program has taken it since, any free port will do and the Mac must connect again.
    let remembered = row.vm.config.shared_folder.port;
    // A new folder first tries a short range of ports that Windows does not hand out by itself,
    // so that the address chosen now is likely to be free every time.
    let first_choices = remembered.into_iter().chain(8580..8600);
    let server = first_choices.map(|port| FileServer::start(&folder, port)).find(|server| server.is_ok()).unwrap_or_else(|| FileServer::start(&folder, 0));
    match server {
        Ok(server) => {
            if remembered != Some(server.port()) {
                row.vm.config.shared_folder.port = Some(server.port());
                let _ = row.vm.save();
                log_line(ui, "The shared folder has a new address. Choose Connect Shared Folder in the Mac from the Machine menu once the Mac is at its desktop.");
            }
            log_line(ui, &format!("The shared folder {} is on. In the Mac it is the {} folder in your home folder, once connected.", folder.display(), file_server::MAC_FOLDER_NAME));
            row.file_server = Some(server);
        }
        Err(error) => log_line(ui, &format!("The shared folder could not be turned on. {error:#}")),
    }
}

fn toggle_right_control(ui: Ui, state: &Shared) {
    let mut state = state.borrow_mut();
    let enabled = !state.settings.right_control_is_command;
    state.settings.right_control_is_command = enabled;
    keyboard_capture::set_right_control_is_command(enabled);
    if let Some(item) = ui.frame.get_menu_bar().and_then(|bar| bar.find_item(ID_RIGHT_CONTROL)) {
        item.check(enabled);
    }
    if let Err(error) = state.settings.save() {
        say(ui.frame, &format!("The settings file could not be saved. {error:#}"));
    } else if enabled {
        say(ui.frame, "Right Control now works as Command while the keyboard is captured.");
    } else {
        say(ui.frame, "Right Control is Control again. The Menu key still works as Command.");
    }
}

fn usb_devices(ui: Ui, state: &Shared) {
    // The dialog runs its own event loop, so the state is let go of before it opens.
    let Some((_, commands, name)) = running_target(ui, &state.borrow()) else { return };
    if let Err(problem) = usb_dialog::show(ui.frame, commands, &name) {
        show_error(ui.frame, &problem);
    }
}

fn open_shared_folder(ui: Ui, state: &Shared) {
    let state = state.borrow();
    let Some(index) = target_index(ui, &state) else { return };
    let vm = &state.rows[index].vm;
    if !vm.config.shared_folder.enabled {
        say(ui.frame, &format!("{} has no shared folder. Turn it on in Settings, Control+P.", vm.config.name));
        return;
    }
    let folder = vm.shared_folder_path();
    let _ = std::fs::create_dir_all(&folder);
    if let Err(error) = std::process::Command::new("explorer.exe").arg(&folder).spawn() {
        show_error(ui.frame, &format!("File Explorer could not be opened. {error}"));
    }
}

/// Types one line into the Mac's Terminal that makes the Mac connect the shared folder now
/// and at every login. Typing is the only way in: nothing of Orchord's runs inside the Mac.
fn connect_shared_folder(ui: Ui, state: &Shared) {
    let (commands, port, name, picture) = {
        let state = state.borrow();
        let Some((index, commands, name)) = running_target(ui, &state) else { return };
        let Some(server) = &state.rows[index].file_server else {
            say(ui.frame, &format!("{name} has no shared folder running. Turn it on in Settings, Control+P, then start the Mac again."));
            return;
        };
        (commands, server.port(), name, state.rows[index].vm.config_dir().join("screen.ppm"))
    };
    let question = format!(
        "Orchord will now type into {name}: it opens Terminal through Spotlight and runs one command, which makes the Mac connect the shared folder now and every time you log in.\n\nBefore you answer, make sure you are logged in to the Mac and it is at its desktop, and do not type while Orchord does. It takes about half a minute.\n\nTo do it by hand instead, press Command+K in the Mac's Finder and connect to {} as a guest.\n\nType into the Mac now?",
        file_server::mac_url(port)
    );
    if !confirm(ui.frame, &question, "Connect Shared Folder") {
        return;
    }
    if keyboard_capture::is_captured() {
        release_keyboard(ui, &state.borrow());
    }
    log_line(ui, "Typing into the Mac. Please wait about half a minute.");
    std::thread::spawn(move || {
        // Spotlight, then Terminal, then the command. The pauses give a slow Mac time to keep up.
        keys::press(&commands, &["meta_l", "spc"]);
        std::thread::sleep(Duration::from_secs(2));
        keys::type_text(&commands, "Terminal");
        std::thread::sleep(Duration::from_secs(2));
        keys::press(&commands, &["ret"]);
        // Terminal drops keys typed before it is ready, and how long it takes to start varies a
        // great deal. Its Shell menu showing in the menu bar is the sign that it is in front.
        let terminal_in_front = || ocr::read_ppm(&picture).is_ok_and(|screen| screen.find_exact("Shell").iter().any(|word| word.y < 40.0));
        let waiting_since = std::time::Instant::now();
        while !terminal_in_front() && waiting_since.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_secs(2));
        }
        std::thread::sleep(Duration::from_secs(4));
        keys::type_text(&commands, &file_server::setup_command(port));
        keys::press(&commands, &["ret"]);
    });
}

fn open_selected_folder(ui: Ui, state: &Shared) {
    let state = state.borrow();
    let Some(index) = target_index(ui, &state) else { return };
    if let Err(error) = std::process::Command::new("explorer.exe").arg(&state.rows[index].vm.dir).spawn() {
        show_error(ui.frame, &format!("File Explorer could not be opened. {error}"));
    }
}

/// File > Orchord Settings: where QEMU is, where virtual machines are kept, and the Apple SMC key.
fn program_settings(ui: Ui, state: &Shared) {
    // The dialog runs its own event loop, in which the timer ticks; nothing may stay borrowed.
    let current = state.borrow().settings.clone();
    let Some(changed) = program_settings::edit(ui.frame, &current) else { return };
    if let Err(error) = changed.save() {
        show_error(ui.frame, &format!("The settings could not be saved. {error:#}"));
        return;
    }
    let mut state = state.borrow_mut();
    let moved = changed.vm_root != state.settings.vm_root;
    let restyled = changed.appearance != state.settings.appearance;
    state.settings = changed;
    if moved && state.rows.iter().any(Row::is_active) {
        say(ui.frame, "Settings saved. The list will show the new folder's virtual machines once the running one has stopped and you press F5.");
        return;
    }
    if moved {
        state.rows = list_vms(&state.settings.vm_root, &state.settings.other_vms).into_iter().map(Row::new).collect();
        fill_list(ui.list, &state);
    }
    match state.settings.problems().first() {
        Some(problem) => say(ui.frame, &format!("Settings saved, but Orchord is still not fully set up. {problem}")),
        None if restyled => say(ui.frame, "Settings saved. The new appearance is used when Orchord next starts."),
        None => say(ui.frame, "Settings saved."),
    }
}

/// File > Check for Updates.
fn check_for_updates(ui: Ui, state: &Shared) {
    let running = state.borrow().rows.iter().any(Row::is_active);
    // The dialogs run their own event loops, in which the timer ticks; nothing stays borrowed.
    if updater::check_now(ui.frame, running) {
        ui.frame.close(true);
    }
}

/// Machine > Full Screen (F11). While the keyboard is sent to the Mac, F11 goes to the Mac.
fn toggle_full_screen(ui: Ui) {
    let full = !ui.frame.is_full_screen();
    ui.frame.show_full_screen(full);
    // The status line is hidden in full screen, so this is only heard, not seen.
    say(ui.frame, if full { "Full screen. Press F11 to leave it." } else { "Full screen off." });
}

/// Machine > Delete (the Delete key). Asks first, with No as the answer Enter gives.
fn delete_selected(ui: Ui, state: &Shared) {
    let (name, dir, size) = {
        let state = state.borrow();
        let Some(index) = target_index(ui, &state) else { return };
        let row = &state.rows[index];
        if row.is_active() {
            say(ui.frame, &format!("{} is running. Shut it down or power it off before deleting it.", row.vm.config.name));
            return;
        }
        (row.vm.config.name.clone(), row.vm.dir.clone(), size_text(row.vm.disk_bytes()))
    };
    let question = format!("Delete the virtual machine {name}? Its disk, {size}, and its settings will be removed from this PC for good. This cannot be undone.");
    // wxWidgets' "No is the default button" flag, which the wx bindings do not name. With it,
    // Delete followed by Enter keeps the Mac.
    const NO_IS_DEFAULT: i64 = 0x80;
    let style = MessageDialogStyle::YesNo | MessageDialogStyle::from_bits_retain(NO_IS_DEFAULT) | MessageDialogStyle::IconWarning;
    if MessageDialog::builder(&ui.frame, &question, "Delete Virtual Machine").with_style(style).build().show_modal() != ID_YES {
        return;
    }
    let mut state = state.borrow_mut();
    // The list may have changed while the question was up; the Mac is found again by its folder.
    let Some(index) = state.rows.iter().position(|row| row.vm.dir == dir && !row.is_active()) else { return };
    match state.rows[index].vm.delete() {
        Ok(left) => {
            state.rows.remove(index);
            if let Some(place) = state.settings.other_vms.iter().position(|other| *other == dir) {
                state.settings.other_vms.remove(place);
                let _ = state.settings.save();
            }
            fill_list(ui.list, &state);
            ui.list.set_focus();
            match left {
                0 => say(ui.frame, &format!("{name} was deleted.")),
                1 => say(ui.frame, &format!("{name} was deleted. One other item in its folder, such as the shared folder, is not Orchord's and was left in {}.", dir.display())),
                left => say(ui.frame, &format!("{name} was deleted. {left} other items in its folder are not Orchord's and were left in {}.", dir.display())),
            }
        }
        Err(error) => show_error(ui.frame, &format!("{name} could not be deleted completely. {error:#}")),
    }
}

/// A file size the way people say it.
fn size_text(bytes: u64) -> String {
    const GIGABYTE: u64 = 1 << 30;
    const MEGABYTE: u64 = 1 << 20;
    match bytes {
        bytes if bytes >= GIGABYTE => format!("{:.1} gigabytes", bytes as f64 / GIGABYTE as f64),
        bytes if bytes <= MEGABYTE => "less than a megabyte".to_string(),
        bytes => format!("{} megabytes", bytes.div_ceil(MEGABYTE)),
    }
}

/// Machine > Save Diagnostic Report: everything someone helping from afar would ask for.
fn save_diagnostics(ui: Ui, state: &Shared) {
    let (vm, settings) = {
        let state = state.borrow();
        let Some(index) = target_index(ui, &state) else { return };
        (state.rows[index].vm.clone(), state.settings.clone())
    };
    match busy::wait(&ui.frame, "Writing the diagnostic report.", move || diagnostics::save(&vm, &settings)) {
        Ok(path) => {
            // File Explorer opens with the report selected, ready to be sent or copied.
            let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", path.display())).spawn();
            say(ui.frame, &format!("The diagnostic report was saved as {} and is selected in File Explorer. It does not hold the Apple SMC key.", path.display()));
        }
        Err(error) => show_error(ui.frame, &format!("The diagnostic report could not be written. {error:#}")),
    }
}

fn reload(ui: Ui, state: &Shared) {
    let mut state = state.borrow_mut();
    if state.rows.iter().any(Row::is_active) {
        say(ui.frame, "The list cannot be refreshed while a virtual machine is running.");
        return;
    }
    state.settings = AppSettings::load().unwrap_or_default();
    state.rows = list_vms(&state.settings.vm_root, &state.settings.other_vms).into_iter().map(Row::new).collect();
    fill_list(ui.list, &state);
    report_setup_problems(ui.frame, &state);
}

/// Runs four times a second: applies what the supervisors reported, honours the keyboard
/// release gesture, and asks for a fresh picture of the Mac's screen.
fn tick(ui: Ui, state: &Shared) {
    let mut notices = Vec::new();
    {
        // A dialog shown from inside a tick keeps the timer running; that inner tick must wait.
        let Ok(mut state) = state.try_borrow_mut() else { return };
        if keyboard_capture::take_release_request() {
            release_keyboard(ui, &state);
        }
        pump_events(ui, &mut state, &mut notices);
        for note in state.notes.try_iter().collect::<Vec<_>>() {
            log_line(ui, &note);
        }
        // The answer to the question asked of GitHub when Orchord started, if there is news.
        if let Some(release) = state.update_news.as_ref().and_then(|news| news.try_recv().ok()) {
            state.update_news = None;
            say(ui.frame, &format!("Orchord {} is available. Choose Check for Updates in the File menu to install it.", release.version));
        }
        state.ticks = state.ticks.wrapping_add(1);
        if state.ticks % SCREEN_EVERY_TICKS == 0 {
            if let Some(row) = state.console.map(|index| &state.rows[index]) {
                if let (RunState::Running, Some(commands)) = (row.state, &row.commands) {
                    let _ = commands.send(VmCommand::Screenshot(row.vm.config_dir().join("screen.ppm")));
                }
            }
        }
    }
    // Shown after the borrow ends: a modal dialog runs its own event loop, and the timer may tick inside it.
    for notice in notices {
        show_error(ui.frame, &notice);
    }
    pump_creation(ui, state);
}

/// File > New Virtual Machine: asks the wizard's questions, then builds the Mac in the background.
fn new_vm(ui: Ui, state: &Shared) {
    if state.borrow().creation.is_some() {
        say(ui.frame, "A virtual machine is already being created.");
        return;
    }
    let settings = state.borrow().settings.clone();
    if let Some(problem) = settings.problems().first() {
        show_error(ui.frame, &format!("Orchord is not fully set up, so it cannot create a virtual machine yet. {problem} {SETTINGS_HINT}"));
        return;
    }
    let Some(answers) = wizard::run(ui.frame, &settings) else { return };
    let (sender, messages) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let name = answers.new.name.clone();
    {
        let (new, cancel) = (answers.new.clone(), cancel.clone());
        std::thread::spawn(move || {
            let steps = sender.clone();
            let result = create::create(&new, &settings, &mut |step| drop(steps.send(CreationMessage::Step(step))), &cancel);
            let _ = sender.send(CreationMessage::Finished(result.map_err(|error| format!("{error:#}"))));
        });
    }
    let dialog = ProgressDialog::builder(&ui.frame, &format!("Creating {name}"), "Creating the Mac's disk.", 100).can_abort().build();
    state.borrow_mut().creation = Some(Creation { name, boot_after: answers.boot_after, messages, cancel, dialog, shown: None });
}

/// Carries the creation thread's news to its progress dialog, and finishes up when it is done.
fn pump_creation(ui: Ui, state: &Shared) {
    // The creation is taken out of the state while its dialog is touched: updating a progress
    // dialog lets other events run, and those must find the state free.
    let Some(mut creation) = state.try_borrow_mut().ok().and_then(|mut state| state.creation.take()) else { return };
    let mut finished = None;
    // Only the newest of the steps that have arrived is worth showing.
    let mut newest = None;
    for message in creation.messages.try_iter() {
        match message {
            CreationMessage::Step(step) => newest = Some(step),
            CreationMessage::Finished(result) => finished = Some(result),
        }
    }
    match newest {
        Some(step) => creation.show(&step),
        // The dialog only learns that Cancel was pressed when it is updated, so it is updated
        // on every tick; with nothing changed, that shows and reports nothing new.
        None => {
            let percent = creation.shown.as_ref().map_or(0, |shown| shown.percent);
            creation.dialog.update(percent, None);
        }
    }
    if creation.dialog.was_cancelled() && !creation.cancel.swap(true, Ordering::Relaxed) {
        creation.dialog.update(0, Some("Cancelling."));
    }
    let Some(result) = finished else {
        state.borrow_mut().creation = Some(creation);
        return;
    };
    creation.dialog.destroy();
    match result {
        Ok(vm) => {
            let boot = creation.boot_after;
            let index = add_vm(ui, state, vm);
            say(ui.frame, &format!("{} was created.", creation.name));
            if let Some(index) = index.filter(|_| boot) {
                ui.list.set_selection(index as u32, true);
                start_selected(ui, state);
            }
        }
        Err(_) if creation.cancel.load(Ordering::Relaxed) => say(ui.frame, &format!("Creating {} was cancelled. Nothing was kept.", creation.name)),
        Err(error) => show_error(ui.frame, &format!("{} could not be created. {error}", creation.name)),
    }
}

/// A new Mac joins the list. One outside the usual folder is remembered in the settings file.
/// Returns its place in the list.
fn add_vm(ui: Ui, state: &Shared, vm: Vm) -> Option<usize> {
    let mut state = state.borrow_mut();
    if vm.dir.parent() != Some(state.settings.vm_root.as_path()) && !state.settings.other_vms.contains(&vm.dir) {
        state.settings.other_vms.push(vm.dir.clone());
        if let Err(error) = state.settings.save() {
            say(ui.frame, &format!("The settings file could not be saved. {error:#}"));
        }
    }
    let folder = vm.dir.clone();
    state.rows.push(Row::new(vm));
    state.rows.sort_by_key(|row| row.vm.config.name.to_lowercase());
    fill_list(ui.list, &state);
    state.rows.iter().position(|row| row.vm.dir == folder)
}

/// How far creation is, as a percentage for the progress bar and a sentence for the user.
fn creation_progress(step: &Step) -> (i32, String) {
    const MEGABYTE: u64 = 1 << 20;
    match step {
        Step::CreatingDisk => (1, "Creating the Mac's disk.".to_string()),
        Step::Downloading { received, total: Some(total) } if *total > 0 => {
            // The download is nearly all of the wait, so it gets nearly all of the bar.
            let percent = 2 + (received * 90 / total) as i32;
            (percent.min(92), format!("Downloading the macOS installer from Apple: {} of {} megabytes.", received / MEGABYTE, total / MEGABYTE))
        }
        Step::Downloading { received, .. } => (2, format!("Downloading the macOS installer from Apple: {} megabytes so far.", received / MEGABYTE)),
        Step::CheckingDownload => (93, "Checking the downloaded installer.".to_string()),
        Step::PreparingInstaller => (95, "Preparing the installer for the Mac.".to_string()),
    }
}

fn pump_events(ui: Ui, state: &mut State, notices: &mut Vec<String>) {
    let console = state.console;
    let mut console_closed = false;
    let mut new_frame: Option<PathBuf> = None;
    for (index, row) in state.rows.iter_mut().enumerate() {
        let Some(events) = &row.events else { continue };
        let pending: Vec<VmEvent> = events.try_iter().collect();
        if pending.is_empty() {
            continue;
        }
        let name = row.vm.config.name.clone();
        let before = row.state;
        for event in pending {
            match event {
                VmEvent::Starting { .. } => {
                    if row.state == RunState::Stopped {
                        row.state = RunState::Starting;
                    }
                }
                VmEvent::Running { launch } => {
                    row.state = RunState::Running;
                    log_line(ui, &if launch == 1 { format!("{name} is running.") } else { format!("{name} is running again.") });
                }
                VmEvent::GuestRestart { .. } => {
                    row.state = RunState::Restarting;
                    log_line(ui, &format!("{name} restarted and is starting again."));
                }
                VmEvent::Crashed(panic) => {
                    log_line(ui, &format!("{name} has crashed. macOS reported: {panic}"));
                    // Easy install deals with a crash itself, and says so; a dialog would only be in the way.
                    if row.easy_install_active() {
                        continue;
                    }
                    notices.push(format!("{name} has crashed. macOS stopped with a kernel panic and is frozen. Choose Force Restart or Force Power Off from the Machine menu.\n\nmacOS reported: {panic}"));
                }
                VmEvent::InstallStage(stage) => {
                    row.vm.config.install.stage = stage;
                    if stage == InstallStage::Done {
                        sound::play(sound::READY);
                    }
                    ui.list.set_string(index as u32, &row.list_text());
                }
                VmEvent::InstallNote(note) => log_line(ui, &note),
                VmEvent::Suspended => {
                    row.state = RunState::Suspended;
                    set_suspend_label(ui, true);
                    log_line(ui, &format!("{name} is suspended. Choose Resume from the Power menu to wake it."));
                }
                VmEvent::Resumed => {
                    row.state = RunState::Running;
                    set_suspend_label(ui, false);
                    log_line(ui, &format!("{name} is running again."));
                }
                VmEvent::Screenshot(path) => {
                    if console == Some(index) {
                        new_frame = Some(path);
                    }
                }
                VmEvent::Stopped(reason) => {
                    row.state = RunState::Stopped;
                    row.events = None;
                    row.commands = None;
                    row.file_server = None;
                    console_closed |= console == Some(index);
                    match reason {
                        StopReason::GuestShutDown => say(ui.frame, &format!("{name} shut down.")),
                        StopReason::StoppedByUser => say(ui.frame, &format!("{name} was powered off.")),
                        StopReason::Failed(reason) => {
                            say(ui.frame, &format!("{name} stopped because of a problem."));
                            notices.push(format!("{name} could not keep running. {reason}"));
                        }
                    }
                    break;
                }
            }
        }
        if row.state != before {
            ui.list.set_string(index as u32, &row.list_text());
        }
    }
    if console_closed {
        // Keep the closing message: `show_list` must not be followed by anything that overwrites it.
        if keyboard_capture::is_captured() {
            keyboard_capture::release();
            sound::play(sound::KEYBOARD_RELEASED);
        }
        state.console = None;
        set_suspend_label(ui, false);
        show_list(ui);
    } else if let Some(path) = new_frame {
        show_frame(ui, state, &path);
    }
}

fn set_suspend_label(ui: Ui, suspended: bool) {
    if let Some(item) = ui.frame.get_menu_bar().and_then(|bar| bar.find_item(ID_POWER_SUSPEND)) {
        item.set_label(if suspended { "Res&ume" } else { "S&uspend" });
    }
}

/// Puts a freshly saved picture of the Mac's screen into the window, unless nothing changed.
fn show_frame(ui: Ui, state: &mut State, path: &Path) {
    let Ok(data) = std::fs::read(path) else { return };
    if data == state.last_frame {
        return;
    }
    if let Some((width, height, rgba)) = ppm_to_rgba(&data) {
        if let Some(bitmap) = Bitmap::from_rgba(&rgba, width, height) {
            ui.screen.set_bitmap(&bitmap);
            // Setting a picture shrinks the control to the picture's own size; this stretches it back.
            ui.screen_panel.layout();
            state.frame_size = Some((width, height));
        }
    }
    state.last_frame = data;
}

#[derive(Clone, Copy)]
enum PointerAction {
    Move,
    Press(PointerButton),
    Release(PointerButton),
    Wheel,
    /// The mouse left the Mac's screen.
    Leave,
}

/// Sends the mouse to the Mac whenever it is over the Mac's screen. The Mac's pointer jumps to
/// the spot of its screen that lies under the Windows pointer, and clicks and the wheel follow.
/// The keyboard need not be captured: a screen reader's own mouse commands only work while it is not.
fn bind_pointer(ui: Ui, state: &Shared) {
    let handler = |action: PointerAction| {
        let state = state.clone();
        move |event: WindowEventData| pointer_event(ui, &state, &event, action)
    };
    ui.screen.on_mouse_motion(handler(PointerAction::Move));
    ui.screen.on_mouse_left_down(handler(PointerAction::Press(PointerButton::Left)));
    ui.screen.on_mouse_left_up(handler(PointerAction::Release(PointerButton::Left)));
    ui.screen.on_mouse_right_down(handler(PointerAction::Press(PointerButton::Right)));
    ui.screen.on_mouse_right_up(handler(PointerAction::Release(PointerButton::Right)));
    ui.screen.on_mouse_middle_down(handler(PointerAction::Press(PointerButton::Middle)));
    ui.screen.on_mouse_middle_up(handler(PointerAction::Release(PointerButton::Middle)));
    ui.screen.on_mouse_leave(handler(PointerAction::Leave));
    // Windows sends the wheel to whichever of the two is under the pointer or has the focus.
    ui.screen.on_mouse_wheel(handler(PointerAction::Wheel));
    ui.screen_panel.on_mouse_wheel(handler(PointerAction::Wheel));
}

fn pointer_event(ui: Ui, state: &Shared, event: &WindowEventData, action: PointerAction) {
    // A dialog opened from a menu command may still hold the state; the mouse can wait.
    let Ok(state) = state.try_borrow() else { return };
    let Some(row) = state.console.map(|index| &state.rows[index]) else { return };
    if row.easy_install_active() {
        return;
    }
    let (RunState::Running, Some(commands), Some(picture)) = (row.state, &row.commands, state.frame_size) else { return };
    let send_button = |button: PointerButton, down: bool| {
        let _ = commands.send(VmCommand::PointerButton { button, down });
        if let Some(slot) = held_slot(button) {
            let mut held = state.held_buttons.get();
            held[slot] = down;
            state.held_buttons.set(held);
        }
    };
    if let PointerAction::Leave = action {
        // A button let go outside the picture is never reported here, so let go of them all now.
        for button in [PointerButton::Left, PointerButton::Right, PointerButton::Middle] {
            if held_slot(button).is_some_and(|slot| state.held_buttons.get()[slot]) {
                send_button(button, false);
            }
        }
        return;
    }
    let (position, rotation) = match event {
        WindowEventData::MouseButton(event) => (event.get_position(), event.event.get_wheel_rotation()),
        WindowEventData::MouseMotion(event) => (event.get_position(), 0),
        _ => (None, 0),
    };
    let Some(position) = position else { return };
    let size = ui.screen.get_client_size();
    let place = pointer_position((size.width, size.height), picture, (position.x, position.y));
    if state.last_pointer.replace(Some(place)) != Some(place) {
        let _ = commands.send(VmCommand::PointerMove { x: place.0, y: place.1 });
    }
    match action {
        PointerAction::Press(button) => send_button(button, true),
        PointerAction::Release(button) => send_button(button, false),
        PointerAction::Wheel if rotation != 0 => {
            let button = if rotation > 0 { PointerButton::WheelUp } else { PointerButton::WheelDown };
            send_button(button, true);
            send_button(button, false);
        }
        PointerAction::Wheel | PointerAction::Move | PointerAction::Leave => {}
    }
}

/// Where a real button's state is kept in `State::held_buttons`. The wheel is never held.
fn held_slot(button: PointerButton) -> Option<usize> {
    match button {
        PointerButton::Left => Some(0),
        PointerButton::Right => Some(1),
        PointerButton::Middle => Some(2),
        PointerButton::WheelUp | PointerButton::WheelDown => None,
    }
}

/// Converts a place in the picture control into a place on the Mac's screen, each axis running
/// from 0 to `POINTER_MAX`. The picture is drawn as large as fits and centred, which can leave
/// bars beside or above it; a place in a bar counts as the nearest edge of the Mac's screen.
fn pointer_position(control: (i32, i32), picture: (u32, u32), point: (i32, i32)) -> (u16, u16) {
    let (control_width, control_height) = (control.0.max(1) as f64, control.1.max(1) as f64);
    let (picture_width, picture_height) = (picture.0.max(1) as f64, picture.1.max(1) as f64);
    let scale = (control_width / picture_width).min(control_height / picture_height);
    let axis = |point: i32, control: f64, drawn: f64| {
        // Half a pixel is added so that a pixel's centre, not its edge, is what gets converted.
        let fraction = ((point as f64 + 0.5 - (control - drawn) / 2.0) / drawn).clamp(0.0, 1.0);
        (fraction * POINTER_MAX as f64).round() as u16
    };
    (axis(point.0, control_width, picture_width * scale), axis(point.1, control_height, picture_height * scale))
}

/// Decodes a binary PPM (the format QEMU's `screendump` writes) into RGBA pixels.
fn ppm_to_rgba(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    // Header: "P6", width, height and the maximum value, separated by whitespace, then one whitespace byte.
    let mut position = 0;
    let mut fields = Vec::new();
    while fields.len() < 4 {
        while data.get(position)?.is_ascii_whitespace() {
            position += 1;
        }
        let start = position;
        while !data.get(position)?.is_ascii_whitespace() {
            position += 1;
        }
        fields.push(std::str::from_utf8(&data[start..position]).ok()?);
    }
    position += 1;
    if fields[0] != "P6" || fields[3] != "255" {
        return None;
    }
    let (width, height): (u32, u32) = (fields[1].parse().ok()?, fields[2].parse().ok()?);
    let pixels = data.get(position..position + (width as usize * height as usize * 3))?;
    let mut rgba = Vec::with_capacity(pixels.len() / 3 * 4);
    for pixel in pixels.chunks_exact(3) {
        rgba.extend_from_slice(pixel);
        rgba.push(255);
    }
    Some((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::{pointer_position, ppm_to_rgba};
    use orchord_core::POINTER_MAX;

    #[test]
    fn the_middle_of_the_picture_is_the_middle_of_the_mac_screen() {
        // A 1280 by 800 screen in a 1920 by 1058 control is drawn 1693 wide with bars at the sides.
        let (x, y) = pointer_position((1920, 1058), (1280, 800), (960, 529));
        assert!((x as i32 - POINTER_MAX as i32 / 2).abs() < 40, "x was {x}");
        assert!((y as i32 - POINTER_MAX as i32 / 2).abs() < 40, "y was {y}");
    }

    #[test]
    fn the_bars_beside_the_picture_count_as_its_edges() {
        assert_eq!(pointer_position((1920, 1058), (1280, 800), (10, 0)).0, 0);
        assert_eq!(pointer_position((1920, 1058), (1280, 800), (1915, 500)).0, POINTER_MAX);
    }

    #[test]
    fn a_spot_in_the_picture_lands_on_the_same_spot_of_the_mac_screen() {
        // The picture starts 113.6 control pixels in; Mac pixel 320 of 1280 is a quarter across.
        let (x, _) = pointer_position((1920, 1058), (1280, 800), (537, 100));
        assert!((x as f64 / POINTER_MAX as f64 - 0.25).abs() < 0.001, "x was {x}");
    }

    #[test]
    fn a_two_pixel_ppm_decodes_to_rgba() {
        let mut data = b"P6\n2 1\n255\n".to_vec();
        data.extend_from_slice(&[10, 20, 30, 40, 50, 60]);
        assert_eq!(ppm_to_rgba(&data), Some((2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255])));
    }

    #[test]
    fn a_truncated_ppm_is_rejected() {
        assert_eq!(ppm_to_rgba(b"P6\n2 1\n255\n\x01\x02"), None);
    }
}
