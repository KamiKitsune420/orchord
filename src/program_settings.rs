//! The dialog for Orchord's own settings: where QEMU is, where new virtual machines are kept,
//! and the Apple SMC key. Without it a new user would have to write the settings file by hand.
//!
//! Each control is created right after its label, because on Windows a control takes its
//! accessible name from the static text created just before it, and creation order is tab order.

use std::path::PathBuf;

use orchord_core::smc_key::{self, KEY_LENGTH};
use orchord_core::AppSettings;
use wxdragon::prelude::*;

use crate::speech;

const TITLE: &str = "Orchord Settings";

/// Shows the dialog and returns the changed settings, or `None` if the user cancelled.
pub fn edit(parent: Frame, settings: &AppSettings) -> Option<AppSettings> {
    let dialog = Dialog::builder(&parent, TITLE).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

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
    qemu.set_focus();

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
