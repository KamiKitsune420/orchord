//! One virtual machine on disk: a folder with `vmconfig\vm.toml`, firmware variables and logs.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::macos::MacosVersion;

/// Where the guest's sound goes.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AudioMode {
    /// Play through the Windows default output device.
    #[default]
    Host,
    /// Record to `vmconfig\audio.wav`; used for testing without ears.
    Record,
    None,
}

/// The folder shared live between Windows and the Mac. Off unless the user turns it on.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default)]
pub struct SharedFolder {
    pub enabled: bool,
    /// Defaults to `shared` inside the VM folder when unset.
    pub path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VmConfig {
    pub name: String,
    pub macos: MacosVersion,
    pub memory_mb: u32,
    pub cpus: u32,
    /// Disk image, relative to the VM folder unless absolute.
    pub disk: PathBuf,
    #[serde(default)]
    pub audio: AudioMode,
    #[serde(default)]
    pub shared_folder: SharedFolder,
    #[serde(default = "default_mac_address")]
    pub mac_address: String,
}

fn default_mac_address() -> String {
    "52:54:00:c9:18:27".to_string()
}

impl VmConfig {
    pub fn new(name: &str, macos: MacosVersion) -> Self {
        VmConfig {
            name: name.to_string(),
            macos,
            memory_mb: macos.default_memory_mb(),
            cpus: 4,
            disk: PathBuf::from("disk").join("macos.qcow2"),
            audio: AudioMode::default(),
            shared_folder: SharedFolder::default(),
            mac_address: default_mac_address(),
        }
    }
}

/// A virtual machine folder together with its loaded configuration.
#[derive(Clone, Debug)]
pub struct Vm {
    pub dir: PathBuf,
    pub config: VmConfig,
}

impl Vm {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join("vmconfig").join("vm.toml");
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let config: VmConfig = toml::from_str(text.trim_start_matches('\u{feff}')).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Vm { dir: dir.to_path_buf(), config })
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(self.config_dir())?;
        let path = self.config_dir().join("vm.toml");
        std::fs::write(&path, toml::to_string_pretty(&self.config)?).with_context(|| format!("writing {}", path.display()))
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir.join("vmconfig")
    }

    pub fn disk_path(&self) -> PathBuf {
        if self.config.disk.is_absolute() { self.config.disk.clone() } else { self.dir.join(&self.config.disk) }
    }

    /// The firmware variable store. It must persist: the Mac records its startup disk there.
    pub fn vars_path(&self) -> PathBuf {
        self.config_dir().join("vars.fd")
    }

    pub fn serial_log_path(&self) -> PathBuf {
        self.config_dir().join("serial.log")
    }

    pub fn qemu_log_path(&self) -> PathBuf {
        self.config_dir().join("qemu.log")
    }

    pub fn audio_recording_path(&self) -> PathBuf {
        self.config_dir().join("audio.wav")
    }

    pub fn shared_folder_path(&self) -> PathBuf {
        self.config.shared_folder.path.clone().unwrap_or_else(|| self.dir.join("shared"))
    }

    pub fn validate(&self) -> Result<()> {
        if self.config.cpus == 0 || self.config.cpus > 16 {
            bail!("The number of processor cores must be between 1 and 16.");
        }
        if self.config.memory_mb < 2048 {
            bail!("A Mac needs at least 2048 MB of memory.");
        }
        if !self.disk_path().is_file() {
            bail!("The disk image {} does not exist.", self.disk_path().display());
        }
        Ok(())
    }
}

/// Every virtual machine under `root`, sorted by name. Folders without a readable
/// configuration are skipped: they are not ours to report on.
pub fn list_vms(root: &Path) -> Vec<Vm> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut vms: Vec<Vm> = entries.flatten().filter(|e| e.path().is_dir()).filter_map(|e| Vm::load(&e.path()).ok()).collect();
    vms.sort_by_key(|vm| vm.config.name.to_lowercase());
    vms
}
