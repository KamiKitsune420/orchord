//! Core of orchord: everything about virtual machines that does not involve a window.
//!
//! The user interface crate depends on this one, never the other way round, so the
//! launcher and supervisor can be exercised from tests and from the `orchord-vm`
//! command-line runner.

pub mod config;
pub mod create;
pub mod easy_install;
pub mod file_server;
pub mod host;
pub mod keys;
pub mod macos;
pub mod ocr;
pub mod qemu;
pub mod qmp;
pub mod recovery;
pub mod settings;
pub mod sound;
pub mod supervisor;
pub mod usb;

pub use config::{AudioMode, Install, InstallStage, SharedFolder, Vm, VmConfig};
pub use macos::MacosVersion;
pub use settings::AppSettings;
pub use qmp::POINTER_MAX;
pub use supervisor::{PointerButton, StopReason, Supervisor, VmCommand, VmEvent};
