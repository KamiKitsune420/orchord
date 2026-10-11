//! The USB devices dialog: hands a USB device plugged into this PC to the running Mac, or
//! takes it back.
//!
//! The devices are a plain list whose lines say where each device is, with one button that
//! moves the selected device, so that a screen reader needs nothing beyond list and button.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use orchord_core::VmCommand;
use orchord_core::usb::{self, HostUsbDevice};
use wxdragon::prelude::*;

use crate::speech;

struct Entry {
    device: HostUsbDevice,
    with_mac: bool,
}

impl Entry {
    fn line(&self, mac: &str) -> String {
        let place = if self.with_mac { format!("connected to {mac}") } else { "with Windows".to_string() };
        format!("{}, {:04x}:{:04x}, {place}", self.device.name, self.device.vendor, self.device.product)
    }
}

/// Shows the dialog for the Mac behind `commands`. Returns an error message if the devices
/// could not be listed at all.
pub fn show(parent: Frame, commands: Sender<VmCommand>, mac: &str) -> Result<(), String> {
    let devices = usb::host_devices(&commands).map_err(|error| format!("{error:#}"))?;
    if devices.is_empty() {
        return Err("QEMU found no USB devices on this PC that it can offer to the Mac.".to_string());
    }
    let with_mac = usb::attached(&commands, &devices).unwrap_or_default();
    let entries: Vec<Entry> = devices.into_iter().map(|device| Entry { with_mac: with_mac.contains(&device), device }).collect();
    let mac = mac.to_string();

    let dialog = Dialog::builder(&parent, &format!("USB Devices for {mac}")).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = StaticText::builder(&dialog).with_label("&USB devices on this PC:").build();
    let list = ListBox::builder(&dialog).with_size(Size::new(520, 220)).build();
    for entry in &entries {
        list.append(&entry.line(&mac));
    }
    list.set_selection(0, true);
    let note = StaticText::builder(&dialog).with_label("A device connected to the Mac is taken away from Windows until you give it back or the Mac stops. Do not connect the keyboard or mouse you are using.").build();
    note.wrap(520);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let toggle = Button::builder(&dialog).with_label("&Connect to the Mac").build();
    let close = Button::builder(&dialog).with_id(ID_CANCEL).with_label("Close").build();
    buttons.add_stretch_spacer(1);
    buttons.add(&toggle, 0, SizerFlag::All, 8);
    buttons.add(&close, 0, SizerFlag::All, 8);
    sizer.add(&label, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Top, 8);
    sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&note, 0, SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom, 8);
    sizer.add_sizer(&buttons, 0, SizerFlag::Expand, 0);
    dialog.set_escape_id(ID_CANCEL);
    dialog.set_sizer_and_fit(sizer, true);
    dialog.centre();

    let entries = Rc::new(RefCell::new(entries));
    // The button says what it would do to the selected device.
    let label_button = {
        let entries = entries.clone();
        move || {
            let with_mac = list.get_selection().and_then(|index| entries.borrow().get(index as usize).map(|entry| entry.with_mac)).unwrap_or(false);
            toggle.set_label(if with_mac { "&Give Back to Windows" } else { "&Connect to the Mac" });
        }
    };
    label_button();
    {
        let label_button = label_button.clone();
        list.on_selection_changed(move |_| label_button());
    }
    {
        let (entries, mac) = (entries.clone(), mac.clone());
        toggle.on_click(move |_| {
            let Some(index) = list.get_selection().map(|index| index as usize) else { return };
            let (device, with_mac) = match entries.borrow().get(index) {
                Some(entry) => (entry.device.clone(), entry.with_mac),
                None => return,
            };
            let result = if with_mac { usb::detach(&commands, &device) } else { usb::attach(&commands, &device) };
            match result {
                Ok(()) => {
                    entries.borrow_mut()[index].with_mac = !with_mac;
                    let line = entries.borrow()[index].line(&mac);
                    list.set_string(index as u32, &line);
                    list.set_selection(index as u32, true);
                    label_button();
                    speech::speak(&line, true);
                }
                Err(error) => {
                    let text = format!("{} could not be {}. {error:#}", device.name, if with_mac { "given back to Windows".to_string() } else { format!("connected to {mac}") });
                    MessageDialog::builder(&dialog, &text, "USB Devices").with_style(MessageDialogStyle::OK | MessageDialogStyle::IconError).build().show_modal();
                }
            }
        });
    }
    list.set_focus();
    dialog.show_modal();
    dialog.destroy();
    Ok(())
}
