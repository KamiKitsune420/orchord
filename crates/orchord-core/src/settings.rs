//! Application-wide settings, stored in `%APPDATA%\orchord\settings.toml`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct AppSettings {
    /// Folder holding `qemu-system-x86_64.exe` and `share\edk2-*.fd`.
    pub qemu_dir: PathBuf,
    /// Folder containing the `EFI` directory OpenCore boots from.
    pub efi_dir: PathBuf,
    /// Folder that holds one sub-folder per virtual machine.
    pub vm_root: PathBuf,
    /// The key string QEMU's Apple SMC device must present. Not shipped with orchord.
    pub osk: String,
    /// Virtual machines the user created somewhere other than under `vm_root`.
    pub other_vms: Vec<PathBuf>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let documents = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join("Documents");
        AppSettings {
            qemu_dir: PathBuf::from(r"C:\Program Files\qemu"),
            efi_dir: PathBuf::new(),
            vm_root: documents.join("Orchord Virtual Machines"),
            osk: String::new(),
            other_vms: Vec::new(),
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
            problems.push("The Apple SMC key is not set in the settings file.".to_string());
        }
        problems
    }
}
