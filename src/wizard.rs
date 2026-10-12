//! The New Virtual Machine wizard: four pages, each a plain panel of labelled controls.
//!
//! 1. Name and location.
//! 2. macOS version, where the installer comes from, and easy install.
//! 3. Memory, processor cores and disk size.
//! 4. An overview, and whether to boot the Mac when set-up completes.
//!
//! Each control is created right after its label, because on Windows a control takes its
//! accessible name from the static text created just before it, and creation order is tab order.
//! Moving between pages says the new page's name through the screen reader, since a page
//! change by itself is silent.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use orchord_core::create::{InstallerSource, NewVm};
use orchord_core::{AppSettings, MacosVersion, installer_image};
use wxdragon::prelude::*;
use wxdragon::widgets::SimpleBook;

use crate::speech;

const LICENCE_URL: &str = "https://www.apple.com/legal/sla/";
const PAGE_NAMES: [&str; 4] = ["Name and location", "macOS version and installer", "Memory, processor and disk", "Overview"];
/// Newest first: the newest is the one most people want.
const VERSIONS: [MacosVersion; 4] = [MacosVersion::Sequoia, MacosVersion::Sonoma, MacosVersion::Monterey, MacosVersion::BigSur];
const MEMORY_STEP_MB: i32 = 1024;

/// What the user asked for.
pub struct Answers {
    pub new: NewVm,
    pub boot_after: bool,
}

/// Every control whose value the wizard reads. All of them are cheap handles.
#[derive(Clone, Copy)]
struct Controls {
    dialog: Dialog,
    book: SimpleBook,
    name: TextCtrl,
    location: TextCtrl,
    version: Choice,
    source: Choice,
    file: TextCtrl,
    file_browse: Button,
    easy: CheckBox,
    agree: CheckBox,
    memory: SpinCtrl,
    cores: SpinCtrl,
    disk: SpinCtrl,
    overview: TextCtrl,
    back: Button,
    next: Button,
}

// Each page panel's built-in name is cleared: wxWidgets would otherwise present every panel
// to screen readers as an object called "panel".
/// Shows the wizard. Returns `None` if the user cancelled.
pub fn run(parent: Frame, settings: &AppSettings) -> Option<Answers> {
    let dialog = Dialog::builder(&parent, "New Virtual Machine").build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let book = SimpleBook::builder(&dialog).build();

    // Page 1: name and location.
    let page = Panel::builder(&book).build();
    page.set_name("");
    let page_sizer = BoxSizer::builder(Orientation::Vertical).build();
    let name_label = StaticText::builder(&page).with_label("Virtual machine &name:").build();
    let name = TextCtrl::builder(&page).build();
    let location_label = StaticText::builder(&page).with_label("&Location (a folder named after the virtual machine is created here):").build();
    let location = TextCtrl::builder(&page).with_value(&settings.vm_root.to_string_lossy()).build();
    let location_browse = Button::builder(&page).with_label("B&rowse for the location...").build();
    add_field(&page_sizer, &name_label, &name);
    add_field(&page_sizer, &location_label, &location);
    page_sizer.add(&location_browse, 0, SizerFlag::All, 8);
    page.set_sizer(page_sizer, true);
    book.add_page(&page, PAGE_NAMES[0], true, None);

    // Page 2: version, installer and easy install.
    let page = Panel::builder(&book).build();
    page.set_name("");
    let page_sizer = BoxSizer::builder(Orientation::Vertical).build();
    let version_label = StaticText::builder(&page).with_label("macOS &version:").build();
    let version = Choice::builder(&page).with_choices(VERSIONS.iter().map(|version| version.display_name().to_string()).collect()).with_selection(Some(0)).build();
    let source_label = StaticText::builder(&page).with_label("&Installer:").build();
    let source = Choice::builder(&page).with_choices(vec!["Download it from Apple".to_string(), "Use an installer file I already have".to_string()]).with_selection(Some(0)).build();
    let file_label = StaticText::builder(&page).with_label("Installer &file (an ISO or CDR installer, or BaseSystem.dmg):").build();
    let file = TextCtrl::builder(&page).build();
    let file_browse = Button::builder(&page).with_label("Br&owse for the installer file...").build();
    let easy = CheckBox::builder(&page).with_label("Use &easy install: Orchord installs macOS by itself").with_value(true).build();
    let easy_note = StaticText::builder(&page)
        .with_label("With easy install, Orchord prepares the new Mac's disk, starts the installer and accepts Apple's licence for you, then tells you when macOS is ready. Without it you do those steps yourself in the installer, where VoiceOver can speak for Big Sur and Sequoia, cannot for Monterey, and has not been tried for Sonoma.")
        .build();
    easy_note.wrap(520);
    let agree = CheckBox::builder(&page).with_label("I have read and &agree to the macOS software licence agreement").build();
    let licence = Button::builder(&page).with_label("Read the &licence agreement in the browser").build();
    add_field(&page_sizer, &version_label, &version);
    add_field(&page_sizer, &source_label, &source);
    add_field(&page_sizer, &file_label, &file);
    page_sizer.add(&file_browse, 0, SizerFlag::All, 8);
    page_sizer.add(&easy, 0, SizerFlag::All, 8);
    page_sizer.add(&easy_note, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 8);
    page_sizer.add(&agree, 0, SizerFlag::All, 8);
    page_sizer.add(&licence, 0, SizerFlag::All, 8);
    page.set_sizer(page_sizer, true);
    book.add_page(&page, PAGE_NAMES[1], false, None);

    // Page 3: memory, processor cores and disk size.
    let page = Panel::builder(&book).build();
    page.set_name("");
    let page_sizer = BoxSizer::builder(Orientation::Vertical).build();
    let memory_label = StaticText::builder(&page).with_label("&Memory in gigabytes:").build();
    let memory = SpinCtrl::builder(&page).with_range(2, 32).with_initial_value(VERSIONS[0].default_memory_mb() as i32 / MEMORY_STEP_MB).build();
    let cores_label = StaticText::builder(&page).with_label("&Processor cores:").build();
    let cores = SpinCtrl::builder(&page).with_range(1, 8).with_initial_value(4).build();
    let disk_label = StaticText::builder(&page).with_label("&Disk size in gigabytes (the file on Windows only grows as the Mac fills it):").build();
    let disk = SpinCtrl::builder(&page).with_range(40, 2000).with_initial_value(VERSIONS[0].default_disk_gb() as i32).build();
    add_field(&page_sizer, &memory_label, &memory);
    add_field(&page_sizer, &cores_label, &cores);
    add_field(&page_sizer, &disk_label, &disk);
    page.set_sizer(page_sizer, true);
    book.add_page(&page, PAGE_NAMES[2], false, None);

    // Page 4: overview.
    let page = Panel::builder(&book).build();
    page.set_name("");
    let page_sizer = BoxSizer::builder(Orientation::Vertical).build();
    let overview_label = StaticText::builder(&page).with_label("&Overview of the new virtual machine:").build();
    let overview = TextCtrl::builder(&page).with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly).with_size(Size::new(520, 190)).build();
    let boot_after = CheckBox::builder(&page).with_label("&Boot machine after setup completes").with_value(true).build();
    page_sizer.add(&overview_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    page_sizer.add(&overview, 1, SizerFlag::Expand | SizerFlag::All, 8);
    page_sizer.add(&boot_after, 0, SizerFlag::All, 8);
    page.set_sizer(page_sizer, true);
    book.add_page(&page, PAGE_NAMES[3], false, None);

    sizer.add(&book, 1, SizerFlag::Expand | SizerFlag::All, 4);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let back = Button::builder(&dialog).with_label("< &Back").build();
    let next = Button::builder(&dialog).with_label("&Next >").build();
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label("Cancel").build();
    next.set_default();
    buttons.add_stretch_spacer(1);
    buttons.add(&back, 0, SizerFlag::All, 8);
    buttons.add(&next, 0, SizerFlag::All, 8);
    buttons.add(&cancel, 0, SizerFlag::All, 8);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand, 0);
    dialog.set_escape_id(ID_CANCEL);
    dialog.set_sizer_and_fit(sizer, true);
    dialog.centre();

    let controls = Controls { dialog, book, name, location, version, source, file, file_browse, easy, agree, memory, cores, disk, overview, back, next };
    // The version the memory and disk defaults were last set for.
    let defaults_for = Rc::new(Cell::new(VERSIONS[0]));

    location_browse.on_click(move |_| {
        let chooser = DirDialog::builder(&dialog, "Choose where to keep the virtual machine", &location.get_value()).build();
        if chooser.show_modal() == ID_OK {
            if let Some(folder) = chooser.get_path() {
                location.set_value(&folder);
            }
        }
    });
    file_browse.on_click(move |_| {
        let chooser = FileDialog::builder(&dialog).with_message("Choose the macOS installer file").with_wildcard("macOS installer images (*.iso;*.cdr;*.dmg;*.img)|*.iso;*.cdr;*.dmg;*.img|All files (*.*)|*.*").build();
        if chooser.show_modal() == ID_OK {
            if let Some(path) = chooser.get_path() {
                file.set_value(&path);
            }
        }
    });
    licence.on_click(move |_| {
        launch_default_browser(LICENCE_URL, BrowserLaunchFlags::Default);
    });
    source.on_selection_changed(move |_| update_enabled(controls));
    easy.on_toggled(move |_| update_enabled(controls));
    back.on_click(move |_| show_page(controls, controls.book.selection().max(1) as usize - 1));
    {
        let defaults_for = defaults_for.clone();
        next.on_click(move |_| go_next(controls, &defaults_for));
    }

    update_enabled(controls);
    show_page(controls, 0);
    let answers = if dialog.show_modal() == ID_OK { Some(Answers { new: collect(controls), boot_after: boot_after.get_value() }) } else { None };
    dialog.destroy();
    answers
}

fn add_field(sizer: &BoxSizer, label: &StaticText, control: &impl WxWidget) {
    sizer.add(label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(control, 0, SizerFlag::Expand | SizerFlag::All, 8);
}

/// The installer file belongs to "use a file", and the licence box to easy install.
fn update_enabled(controls: Controls) {
    let from_file = controls.source.get_selection() == Some(1);
    controls.file.enable(from_file);
    controls.file_browse.enable(from_file);
    controls.agree.enable(controls.easy.get_value());
}

fn show_page(controls: Controls, page: usize) {
    controls.book.set_selection(page);
    controls.back.enable(page > 0);
    controls.next.set_label(if page == PAGE_NAMES.len() - 1 { "&Finish" } else { "&Next >" });
    controls.dialog.set_label(&format!("New Virtual Machine, step {} of {}: {}", page + 1, PAGE_NAMES.len(), PAGE_NAMES[page]));
    match page {
        0 => controls.name.set_focus(),
        1 => controls.version.set_focus(),
        2 => controls.memory.set_focus(),
        _ => controls.overview.set_focus(),
    }
    speech::speak(&format!("Step {} of {}: {}.", page + 1, PAGE_NAMES.len(), PAGE_NAMES[page]), false);
}

/// Next on the first three pages, Finish on the last. A page with a mistake stays put and says why.
fn go_next(controls: Controls, defaults_for: &Cell<MacosVersion>) {
    let page = controls.book.selection().max(0) as usize;
    let mut new = collect(controls);
    let problem = match page {
        0 => NewVm { source: InstallerSource::Apple, ..new.clone() }.problem(),
        1 => page_two_problem(controls, &new),
        _ => new.problem(),
    };
    if let Some(problem) = problem {
        MessageDialog::builder(&controls.dialog, &problem, "New Virtual Machine").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning).build().show_modal();
        return;
    }
    match page {
        1 => {
            // A Mac is set up differently for each version, and one set up for the wrong
            // version may not start. The file's name is the only hint of what is in it.
            if let Some(named) = named_version(&new).filter(|named| *named != new.macos) {
                let question = format!("The installer file's name says {}, but the version chosen is {}. A Mac set up for the wrong version may not start. Change the version to {}?", named.short_name(), new.macos.short_name(), named.short_name());
                if MessageDialog::builder(&controls.dialog, &question, "New Virtual Machine").with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion).build().show_modal() == ID_YES {
                    controls.version.set_selection(VERSIONS.iter().position(|version| *version == named).unwrap_or(0) as u32);
                    new = collect(controls);
                }
            }
            let silent_installer = !new.easy_install && !new.macos.installer_speaks();
            let question = format!("VoiceOver cannot speak in the {} installer. Without easy install you would need sighted help to install it. Continue without easy install?", new.macos.display_name());
            if silent_installer && MessageDialog::builder(&controls.dialog, &question, "New Virtual Machine").with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconWarning).build().show_modal() != ID_YES {
                return;
            }
            // Each version has its own sensible memory and disk size; offer them when the version changes.
            if defaults_for.replace(new.macos) != new.macos {
                controls.memory.set_value(new.macos.default_memory_mb() as i32 / MEMORY_STEP_MB);
                controls.disk.set_value(new.macos.default_disk_gb() as i32);
            }
            show_page(controls, 2);
        }
        2 => {
            controls.overview.set_value(&overview_text(&collect(controls)));
            show_page(controls, 3);
        }
        3 => controls.dialog.end_modal(ID_OK),
        _ => show_page(controls, page + 1),
    }
}

fn page_two_problem(controls: Controls, new: &NewVm) -> Option<String> {
    if let InstallerSource::File(file) = &new.source {
        if !file.is_file() {
            return Some(if file.as_os_str().is_empty() { "Choose the installer file, or change Installer to download it from Apple.".to_string() } else { format!("The installer file {} does not exist.", file.display()) });
        }
        // Better to hear now that a file is not an installer than to meet a silent black screen later.
        if let Err(error) = installer_image::inspect(file) {
            return Some(format!("{error:#}"));
        }
    }
    if new.easy_install && !controls.agree.get_value() {
        return Some("Easy install accepts Apple's licence in the installer for you, so you need to agree to it here first. Check the box that says you have read and agree to the licence, or turn easy install off.".to_string());
    }
    None
}

/// The macOS version the chosen installer file's name mentions, if it mentions one.
fn named_version(new: &NewVm) -> Option<MacosVersion> {
    match &new.source {
        InstallerSource::File(file) => MacosVersion::named_in(&file.file_name()?.to_string_lossy()),
        InstallerSource::Apple => None,
    }
}

fn collect(controls: Controls) -> NewVm {
    let macos = VERSIONS[controls.version.get_selection().unwrap_or(0) as usize % VERSIONS.len()];
    let source = if controls.source.get_selection() == Some(1) { InstallerSource::File(PathBuf::from(controls.file.get_value().trim().trim_matches('"'))) } else { InstallerSource::Apple };
    NewVm {
        name: controls.name.get_value().trim().to_string(),
        location: PathBuf::from(controls.location.get_value().trim().trim_matches('"')),
        macos,
        source,
        easy_install: controls.easy.get_value(),
        memory_mb: (controls.memory.value() * MEMORY_STEP_MB) as u32,
        cpus: controls.cores.value() as u32,
        disk_gb: controls.disk.value() as u32,
    }
}

fn overview_text(new: &NewVm) -> String {
    let installer = match &new.source {
        InstallerSource::Apple => "downloaded from Apple, about 700 megabytes".to_string(),
        InstallerSource::File(file) => match installer_image::inspect(file) {
            Ok(installer_image::Layout::Dmg) => format!("the file {}, unpacked into the virtual machine's folder", file.display()),
            // Installers of many gigabytes are not copied, so the file has to stay put.
            _ => format!("the file {}, used where it is; leave it there until macOS is installed", file.display()),
        },
    };
    let install = if new.easy_install { "Easy install: Orchord installs macOS by itself and tells you when it is ready. This takes a few hours." } else { "You install macOS yourself in the installer." };
    format!(
        "Name: {}\r\nFolder: {}\r\nSystem: {}\r\nInstaller: {}\r\n{}\r\nMemory: {} gigabytes\r\nProcessor cores: {}\r\nDisk: up to {} gigabytes\r\n",
        new.name,
        new.folder().display(),
        new.macos.display_name(),
        installer,
        install,
        new.memory_mb / MEMORY_STEP_MB as u32,
        new.cpus,
        new.disk_gb
    )
}
