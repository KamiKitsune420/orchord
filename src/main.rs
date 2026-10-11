//! Orchord: an accessible manager for macOS virtual machines on Windows.
#![windows_subsystem = "windows"]

mod busy;
mod keyboard_capture;

mod main_window;
mod program_settings;
mod settings_dialog;
mod speech;
mod updater;
mod usb_dialog;
mod wizard;

fn main() {
    let _ = wxdragon::main(|_| main_window::show());
}
