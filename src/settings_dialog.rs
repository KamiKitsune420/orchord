//! The settings dialog for one virtual machine.
//!
//! Each control is created right after its label, because on Windows a control takes its
//! accessible name from the static text created just before it, and creation order is tab order.

use orchord_core::{AudioMode, VmConfig};
use wxdragon::prelude::*;

const MEMORY_STEP_MB: i32 = 1024;

/// Shows the dialog and returns the changed configuration, or `None` if the user cancelled.
pub fn edit(parent: Frame, config: &VmConfig, running: bool) -> Option<VmConfig> {
    let title = format!("Settings for {}", config.name);
    let dialog = Dialog::builder(&parent, &title).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    if running {
        let note = StaticText::builder(&dialog).with_label("This virtual machine is running. Changes take effect the next time it starts.").build();
        sizer.add(&note, 0, SizerFlag::All, 8);
    }

    let memory_label = StaticText::builder(&dialog).with_label("&Memory in gigabytes:").build();
    let memory = SpinCtrl::builder(&dialog).with_range(2, 32).with_initial_value(config.memory_mb as i32 / MEMORY_STEP_MB).build();
    sizer.add(&memory_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&memory, 0, SizerFlag::Expand | SizerFlag::All, 8);

    let cores_label = StaticText::builder(&dialog).with_label("&Processor cores:").build();
    let cores = SpinCtrl::builder(&dialog).with_range(1, 8).with_initial_value(config.cpus as i32).build();
    sizer.add(&cores_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&cores, 0, SizerFlag::Expand | SizerFlag::All, 8);

    let sound_label = StaticText::builder(&dialog).with_label("&Sound from the Mac:").build();
    let sound_choices = vec!["Play through this PC".to_string(), "Off".to_string()];
    let sound_selection = if config.audio == AudioMode::None { 1 } else { 0 };
    let sound = Choice::builder(&dialog).with_choices(sound_choices).with_selection(Some(sound_selection)).build();
    sizer.add(&sound_label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&sound, 0, SizerFlag::Expand | SizerFlag::All, 8);

    let share = CheckBox::builder(&dialog).with_label("Share a &folder between Windows and this Mac (file server)").with_value(config.shared_folder.enabled).build();
    sizer.add(&share, 0, SizerFlag::All, 8);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let ok = Button::builder(&dialog).with_id(ID_OK).with_label("OK").build();
    let cancel = Button::builder(&dialog).with_id(ID_CANCEL).with_label("Cancel").build();
    ok.set_default();
    buttons.add_stretch_spacer(1);
    buttons.add(&ok, 0, SizerFlag::All, 8);
    buttons.add(&cancel, 0, SizerFlag::All, 8);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand, 0);

    dialog.set_affirmative_id(ID_OK);
    dialog.set_escape_id(ID_CANCEL);
    dialog.set_sizer_and_fit(sizer, true);
    dialog.centre();
    memory.set_focus();

    let result = if dialog.show_modal() == ID_OK {
        let mut changed = config.clone();
        changed.memory_mb = (memory.value() * MEMORY_STEP_MB) as u32;
        changed.cpus = cores.value() as u32;
        // A VM set up to record its sound for testing keeps doing so unless sound is turned off.
        changed.audio = match (sound.get_selection(), config.audio) {
            (Some(1), _) => AudioMode::None,
            (_, AudioMode::Record) => AudioMode::Record,
            _ => AudioMode::Host,
        };
        changed.shared_folder.enabled = share.get_value();
        Some(changed)
    } else {
        None
    };
    dialog.destroy();
    result
}
