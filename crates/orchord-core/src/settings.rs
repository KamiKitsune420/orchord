//! Application-wide settings, stored in `%APPDATA%\orchord\settings.toml`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AppSettings {
    /// Folder holding `qemu-system-x86_64.exe` and `share\edk2-*.fd`.
    pub qemu_dir: PathBuf,
    /// Folder containing the `EFI` directory OpenCore boots from. The one beside the program
    /// is not written to the settings file, so the program's folder can be moved or replaced.
    #[serde(skip_serializing_if = "is_beside_program")]
    pub efi_dir: PathBuf,
    /// Folder that holds one sub-folder per virtual machine.
    pub vm_root: PathBuf,
    /// The key string QEMU's Apple SMC device must present. Not shipped with orchord.
    pub osk: String,
    /// Virtual machines the user created somewhere other than under `vm_root`.
    pub other_vms: Vec<PathBuf>,
    /// While the keyboard is captured, Right Control is sent to the Mac as Command. Windows
    /// keeps Windows+L for itself, so without this Command+L could not be typed.
    pub right_control_is_command: bool,
    /// For developers: a second QEMU control port on this PC's loopback address, which
    /// diagnostic tools can connect to while Orchord runs a Mac. Absent in normal use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_qmp_port: Option<u16>,
}

/// The boot folder travels with the program: the build puts it beside the program file.
fn beside_program() -> PathBuf {
    std::env::current_exe().ok().and_then(|program| program.parent().map(|folder| folder.join("opencore"))).unwrap_or_default()
}

fn is_beside_program(efi_dir: &PathBuf) -> bool {
    *efi_dir == beside_program()
}

impl Default for AppSettings {
    fn default() -> Self {
        let documents = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join("Documents");
        AppSettings {
            qemu_dir: PathBuf::from(r"C:\Program Files\qemu"),
            efi_dir: beside_program(),
            vm_root: documents.join("Orchord Virtual Machines"),
            osk: String::new(),
            other_vms: Vec::new(),
            right_control_is_command: true,
            debug_qmp_port: None,
        }
    }
}

impl AppSettings {
    pub fn path() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
        base.join("orchord").join("settings.toml")
    }

    /// Loads the settings file, or returns the defaults when there is none yet.
    pub fn load() -> Result<Self> {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(text.trim_start_matches('\u{feff}')).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        // A file that cannot be read was not what these settings were loaded from; replacing
        // it would throw away whatever the user has in it.
        if path.exists() && Self::load_from(&path).is_err() {
            anyhow::bail!("{} has a mistake in it, so it was left as it is.", path.display());
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, toml::to_string_pretty(self)?).with_context(|| format!("writing {}", path.display()))
    }

    pub fn qemu_img_exe(&self) -> PathBuf {
        self.qemu_dir.join("qemu-img.exe")
    }

    pub fn qemu_exe(&self) -> PathBuf {
        self.qemu_dir.join("qemu-system-x86_64.exe")
    }

    pub fn firmware_code(&self) -> PathBuf {
        self.qemu_dir.join("share").join("edk2-x86_64-code.fd")
    }

    pub fn firmware_vars_template(&self) -> PathBuf {
        self.qemu_dir.join("share").join("edk2-i386-vars.fd")
    }

    /// Everything that must be in place before any VM can start, as messages for the user.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.qemu_exe().is_file() {
            problems.push(format!("QEMU was not found at {}.", self.qemu_exe().display()));
        }
        if !self.efi_dir.join("EFI").join("OC").join("config.plist").is_file() {
            problems.push(format!("No OpenCore boot folder at {}.", self.efi_dir.display()));
        }
        if self.osk.is_empty() {
            problems.push("The Apple SMC key has not been entered.".to_string());
        }
        problems
    }
}
