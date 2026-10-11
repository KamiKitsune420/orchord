//! Orchord: an accessible manager for macOS virtual machines on Windows.
#![windows_subsystem = "windows"]

mod keyboard_capture;

mod main_window;
mod settings_dialog;
mod speech;

fn main() {
    let _ = wxdragon::main(|_| main_window::show());
}
