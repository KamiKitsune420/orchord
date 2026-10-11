//! The dialog for Orchord's own settings: where QEMU is, where new virtual machines are kept,
//! and the Apple SMC key. Without it a new user would have to write the settings file by hand.
//! Its first button checks the PC and offers to get whatever is missing: QEMU, the Windows
//! Hypervisor Platform and the key.
//!
//! Each control is created right after its label, because on Windows a control takes its
//! accessible name from the static text created just before it, and creation order is tab order.

use std::path::PathBuf;

use orchord_core::smc_key::{self, KEY_LENGTH};
use orchord_core::{AppSettings, ocr, setup};
use wxdragon::prelude::*;

use crate::speech;

const TITLE: &str = "Orchord Settings";

/// Shows the dialog and returns the changed settings, or `None` if the user cancelled.
pub fn edit(parent: Frame, settings: &AppSettings) -> Option<AppSettings> {
    let dialog = Dialog::builder(&parent, TITLE).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    // First in the dialog, because it is all a new user needs: it finds what is missing and gets it.
    let set_up = Button::builder(&dialog).with_label("&Check and set up this PC...").build();
    sizer.add(&set_up, 0, SizerFlag::All, 8);

    let qemu_label = StaticText::builder(&dialog).with_label("&QEMU folder (the one that holds qemu-system-x86_64.exe):").build();
    let qemu = TextCtrl::builder(&dialog).with_value(&settings.qemu_dir.to_string_lossy()).with_size(Size::new(520, -1)).build();
    let qemu_browse = Button::builder(&dialog).with_label("&Browse for the QEMU folder...").build();
    sizer.add(&qemu_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&qemu, 0, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&qemu_browse, 0, SizerFlag::All, 8);

    let root_label = StaticText::builder(&dialog).with_label("Folder where &virtual machines are kept:").build();
    let root = TextCtrl::builder(&dialog).with_value(&settings.vm_root.to_string_lossy()).build();
    let root_browse = Button::builder(&dialog).with_label("B&rowse for the virtual machines folder...").build();
    sizer.add(&root_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&root, 0, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&root_browse, 0, SizerFlag::All, 8);

    let key_label = StaticText::builder(&dialog).with_label("Apple SMC &key (64 characters; Orchord does not come with it):").build();
    let key = TextCtrl::builder(&dialog).with_value(&settings.osk).build();
    let key_fetch = Button::builder(&dialog).with_label("&Get the key from the internet...").build();
    sizer.add(&key_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&key, 0, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&key_fetch, 0, SizerFlag::All, 8);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let ok = Button::builder(&dialog).with_label("OK").build();
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label("Cancel").build();
    ok.set_default();
    buttons.add_stretch_spacer(1);
    buttons.add(&ok, 0, SizerFlag::All, 8);
    buttons.add(&cancel, 0, SizerFlag::All, 8);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand, 0);

    qemu_browse.on_click(move |_| browse(dialog, "Choose the folder QEMU is installed in", qemu));
    root_browse.on_click(move |_| browse(dialog, "Choose the folder to keep virtual machines in", root));
    key_fetch.on_click(move |_| fetch_key(dialog, key));
    {
        let settings = settings.clone();
        set_up.on_click(move |_| set_up_pc(dialog, qemu, key, &settings));
    }
    // OK only closes the dialog when everything in it can be used; otherwise it says what is wrong.
    let collect = {
        let settings = settings.clone();
        move || AppSettings { qemu_dir: path_in(qemu), vm_root: path_in(root), osk: key.get_value().trim().to_string(), ..settings.clone() }
    };
    {
        let collect = collect.clone();
        ok.on_click(move |_| match problem(&collect()) {
            Some(problem) => {
                MessageDialog::builder(&dialog, &problem, TITLE).with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning).build().show_modal();
            }
            // A key of the right length with one letter wrong would only show itself much
            // later, as a Mac that never starts and says nothing.
            None if !smc_key::is_genuine(&collect().osk) && !confirm(dialog, "What is entered is 64 characters long, but it is not the Apple SMC key: at least one character differs. macOS will not start with a wrong key. Save it anyway?") => {}
            None => dialog.end_modal(ID_OK),
        });
    }

    dialog.set_escape_id(ID_CANCEL);
    dialog.set_sizer_and_fit(sizer, true);
    dialog.centre();
    // Someone with nothing set up yet lands on the button that sets things up.
    if settings.problems().is_empty() { qemu.set_focus() } else { set_up.set_focus() }

    let result = if dialog.show_modal() == ID_OK { Some(collect()) } else { None };
    dialog.destroy();
    result
}

fn browse(dialog: Dialog, message: &str, field: TextCtrl) {
    let chooser = DirDialog::builder(&dialog, message, &field.get_value()).build();
    if chooser.show_modal() == ID_OK {
        if let Some(folder) = chooser.get_path() {
            field.set_value(&folder);
        }
    }
}

/// Goes through what a PC needs before it can run a Mac, offers to get each thing that is
/// missing, and ends with a summary. Nothing is installed or changed without a Yes.
fn set_up_pc(dialog: Dialog, qemu_field: TextCtrl, key_field: TextCtrl, settings: &AppSettings) {
    let mut report = Vec::new();
    let mut hypervisor_works = false;
    let mut current = AppSettings { qemu_dir: path_in(qemu_field), ..settings.clone() };

    if !current.qemu_exe().is_file() {
        if setup::winget().is_none() {
            report.push(format!("QEMU: not installed, and this Windows has no winget to install it with. Download QEMU for Windows from {} and install it.", setup::QEMU_DOWNLOAD_PAGE));
        } else if confirm(dialog, &format!("QEMU, the program that runs the Macs, was not found in {}. Download and install it now? It is about 200 megabytes and comes from QEMU's own site, through Windows' package manager. Near the end Windows asks for permission to install it.", current.qemu_dir.display())) {
            match install_qemu(dialog) {
                Ok(outcome) => {
                    let installed = AppSettings { qemu_dir: PathBuf::from(setup::QEMU_INSTALL_FOLDER), ..current.clone() };
                    if installed.qemu_exe().is_file() {
                        qemu_field.set_value(setup::QEMU_INSTALL_FOLDER);
                        current = installed;
                    } else {
                        report.push(match outcome {
                            setup::InstallOutcome::AlreadyInstalled => "QEMU: Windows says it is already installed, but not where it usually is. Choose its folder with the Browse button.".to_string(),
                            setup::InstallOutcome::Failed(code) => format!("QEMU: it was not installed (winget's code {code}). That is also what happens when Windows' permission question is answered with No."),
                            setup::InstallOutcome::Installed => "QEMU: installed, but not where it usually is. Choose its folder with the Browse button.".to_string(),
                        });
                    }
                }
                Err(error) => report.push(format!("QEMU: it was not installed. {error}")),
            }
        } else {
            report.push("QEMU: not installed.".to_string());
        }
    }
    if current.qemu_exe().is_file() {
        report.push(format!("QEMU: found in {}.", current.qemu_dir.display()));
        speech::speak("Checking the Windows Hypervisor Platform.", true);
        let cursor = BusyCursor::new(None);
        let problem = setup::hypervisor_problem(&current);
        drop(cursor);
        hypervisor_works = problem.is_none();
        report.push(match problem {
            None => "Windows Hypervisor Platform: working.".to_string(),
            Some(problem) if confirm(dialog, &format!("QEMU cannot use the Windows Hypervisor Platform, which a Mac needs. It said: {problem} This usually means the Windows feature is turned off. Turn it on now? Windows asks for permission, and Windows has to be restarted afterwards.")) => match setup::enable_hypervisor() {
                setup::HypervisorChange::RestartNeeded => "Windows Hypervisor Platform: turned on. Restart Windows, then it works.".to_string(),
                setup::HypervisorChange::Done => "Windows Hypervisor Platform: Windows says it is on. If Macs still do not start, restart Windows and make sure virtualisation is turned on in the PC's firmware settings.".to_string(),
                setup::HypervisorChange::NotDone(code) => format!("Windows Hypervisor Platform: not turned on (code {code}). You can turn it on yourself: Turn Windows features on or off, check Windows Hypervisor Platform, restart."),
            },
            Some(problem) => format!("Windows Hypervisor Platform: not working. QEMU said: {problem}"),
        });
    }

    if !smc_key::is_genuine(key_field.get_value().trim()) {
        fetch_key(dialog, key_field);
    }
    report.push(if smc_key::is_genuine(key_field.get_value().trim()) { "Apple SMC key: entered.".to_string() } else { "Apple SMC key: not entered. Use the button Get the key from the internet, or type it in.".to_string() });
    report.push(if ocr::available() {
        "Text recognition: available.".to_string()
    } else {
        "Text recognition: Windows has no text recognition language installed. Easy install needs one: in Windows Settings, under Time and language, Language and region, add the Optical character recognition feature to your language.".to_string()
    });

    let ready = hypervisor_works && smc_key::is_genuine(key_field.get_value().trim());
    let ending = if ready { "This PC is ready. Press OK in Orchord Settings to save." } else { "This PC is not ready yet; see the lines above." };
    MessageDialog::builder(&dialog, &format!("{}\n\n{ending}", report.join("\n")), TITLE).with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation).build().show_modal();
}

/// Runs the QEMU installation behind a progress dialog whose Cancel stops it.
fn install_qemu(dialog: Dialog) -> Result<setup::InstallOutcome, String> {
    let mut winget = setup::start_qemu_install().map_err(|error| format!("{error:#}."))?;
    let progress = ProgressDialog::builder(&dialog, "Installing QEMU", "Downloading and installing QEMU. This takes a few minutes. Windows will ask for permission near the end.", 100).can_abort().build();
    let outcome = loop {
        if let Some(status) = winget.try_wait().map_err(|error| error.to_string())? {
            break setup::qemu_install_outcome(status);
        }
        // Pulsing also lets the window answer; a false return or the flag means Cancel was pressed.
        if !progress.pulse(None) || progress.was_cancelled() {
            let _ = winget.kill();
            let _ = winget.wait();
            progress.destroy();
            return Err("The installation was cancelled.".to_string());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    progress.destroy();
    Ok(outcome)
}

/// Downloads a public start-up script of another project and takes the key from it. The user
/// is told what will happen and where the key comes from, and decides; nothing is fetched
/// before that.
fn fetch_key(dialog: Dialog, field: TextCtrl) {
    let sources = smc_key::SOURCES.iter().map(|source| source.project).collect::<Vec<_>>().join(" or ");
    let question = format!("Orchord will download a public file of the {sources} project from GitHub and take the Apple SMC key from it. Nothing about you or this PC is sent. The key is Apple's text and is not part of Orchord; using it, like accepting Apple's licence for macOS, is your own decision. Get the key now?");
    if !confirm(dialog, &question) {
        return;
    }
    speech::speak("Getting the key.", true);
    let cursor = BusyCursor::new(None);
    let result = smc_key::fetch();
    drop(cursor);
    match result {
        Ok((key, source)) => {
            field.set_value(&key);
            field.set_focus();
            speech::speak(&format!("The key was found in the {} project's file and has been entered. Press OK to save it.", source.project), true);
        }
        Err(error) => {
            let message = format!("{error:#} Check the internet connection and try again, or get the key another way: Orchord's README has a section called Getting the Apple SMC key.");
            MessageDialog::builder(&dialog, &message, TITLE).with_style(MessageDialogStyle::OK | MessageDialogStyle::IconWarning).build().show_modal();
        }
    }
}

fn confirm(dialog: Dialog, question: &str) -> bool {
    MessageDialog::builder(&dialog, question, TITLE).with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconQuestion).build().show_modal() == ID_YES
}

fn path_in(field: TextCtrl) -> PathBuf {
    PathBuf::from(field.get_value().trim().trim_matches('"'))
}

/// The first thing wrong with what was entered, as a message for the user.
fn problem(settings: &AppSettings) -> Option<String> {
    if !settings.qemu_exe().is_file() {
        return Some(format!("There is no qemu-system-x86_64.exe in {}. Choose the folder QEMU was installed in; the usual one is C:\\Program Files\\qemu.", settings.qemu_dir.display()));
    }
    if !settings.vm_root.is_absolute() {
        return Some("The folder for virtual machines must be a full path, such as D:\\Virtual Machines.".to_string());
    }
    let length = settings.osk.chars().count();
    if length != KEY_LENGTH {
        return Some(if length == 0 { "Enter the Apple SMC key. A Mac cannot start without it.".to_string() } else { format!("The Apple SMC key is {KEY_LENGTH} characters long; what is entered has {length}.") });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::problem;
    use orchord_core::AppSettings;
    use std::path::PathBuf;

    #[test]
    fn settings_that_cannot_work_are_named() {
        let missing = AppSettings { qemu_dir: PathBuf::from(r"C:\nowhere-orchord"), ..AppSettings::default() };
        assert!(problem(&missing).unwrap().contains("qemu-system-x86_64.exe"));
        // The later checks are only reached with a real QEMU folder, so they run where QEMU
        // is installed in its usual place.
        let installed = AppSettings::default();
        if installed.qemu_exe().is_file() {
            let short = AppSettings { osk: "abc".to_string(), ..installed.clone() };
            assert!(problem(&short).unwrap().contains("has 3"));
            let relative = AppSettings { vm_root: PathBuf::from("Macs"), osk: "k".repeat(64), ..installed.clone() };
            assert!(problem(&relative).unwrap().contains("full path"));
            assert_eq!(problem(&AppSettings { osk: "k".repeat(64), ..installed }), None);
        }
    }
}
