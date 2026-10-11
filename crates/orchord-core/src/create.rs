//! Creates a new virtual machine: its folder, an empty disk, and the macOS installer image.
//!
//! The folder is named after the Mac and holds `disk\` (the Mac's drive) and `vmconfig\`
//! (settings, firmware variables, logs and, until macOS is installed, the installer image).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result, bail};

use crate::config::{Install, InstallStage, Vm, VmConfig};
use crate::macos::MacosVersion;
use crate::recovery;
use crate::settings::AppSettings;

/// Where the macOS installer comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallerSource {
    /// Download the installer for the chosen version from Apple.
    Apple,
    /// A `BaseSystem.dmg` the user already has, or the same image already converted (`.img`).
    File(PathBuf),
}

/// Everything the wizard collects.
#[derive(Clone, Debug)]
pub struct NewVm {
    pub name: String,
    /// The folder the Mac's own folder is created in.
    pub location: PathBuf,
    pub macos: MacosVersion,
    pub source: InstallerSource,
    pub easy_install: bool,
    pub memory_mb: u32,
    pub cpus: u32,
    pub disk_gb: u32,
}

/// What creation is doing, for the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    CreatingDisk,
    Downloading { received: u64, total: Option<u64> },
    CheckingDownload,
    PreparingInstaller,
}

impl NewVm {
    pub fn folder(&self) -> PathBuf {
        self.location.join(self.name.trim())
    }

    /// The first thing wrong with what the user entered, as a message for them.
    pub fn problem(&self) -> Option<String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Some("The virtual machine needs a name.".to_string());
        }
        if name.chars().any(|character| r#"\/:*?"<>|"#.contains(character) || character.is_control()) || name.ends_with('.') {
            return Some(r#"The name cannot contain any of these characters: \ / : * ? " < > | and cannot end with a full stop."#.to_string());
        }
        if !self.location.is_absolute() {
            return Some("The location must be a full path, such as D:\\Virtual Machines.".to_string());
        }
        if self.folder().exists() {
            return Some(format!("There is already a folder named {} in that location.", name));
        }
        if let InstallerSource::File(file) = &self.source {
            if !file.is_file() {
                return Some(format!("The installer file {} does not exist.", file.display()));
            }
        }
        None
    }
}

/// Creates the virtual machine. Blocks for as long as the download takes; call it on its own
/// thread. If anything fails, or `cancel` is set, the half-made folder is removed again.
pub fn create(new: &NewVm, settings: &AppSettings, progress: &mut dyn FnMut(Step), cancel: &AtomicBool) -> Result<Vm> {
    if let Some(problem) = new.problem() {
        bail!(problem);
    }
    let folder = new.folder();
    let result = create_in(&folder, new, settings, progress, cancel);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&folder);
    }
    result
}

fn create_in(folder: &Path, new: &NewVm, settings: &AppSettings, progress: &mut dyn FnMut(Step), cancel: &AtomicBool) -> Result<Vm> {
    let mut config = VmConfig::new(new.name.trim(), new.macos);
    config.memory_mb = new.memory_mb;
    config.cpus = new.cpus;
    config.mac_address = random_mac_address();
    let mut vm = Vm { dir: folder.to_path_buf(), config };
    std::fs::create_dir_all(vm.config_dir()).with_context(|| format!("creating {}", folder.display()))?;
    let disk = vm.disk_path();
    std::fs::create_dir_all(disk.parent().expect("the disk is inside the VM folder"))?;

    progress(Step::CreatingDisk);
    qemu_img(settings, &["create", "-f", "qcow2"], &[&disk], Some(&format!("{}G", new.disk_gb))).context("creating the Mac's disk")?;

    let media = match &new.source {
        InstallerSource::File(file) if !has_extension(file, "dmg") => file.clone(),
        source => {
            let dmg = match source {
                InstallerSource::File(file) => file.clone(),
                InstallerSource::Apple => {
                    let dmg = vm.config_dir().join("BaseSystem.dmg");
                    let mut last = (0, None);
                    recovery::download(
                        new.macos,
                        &dmg,
                        &mut |download| {
                            // Report each whole megabyte once, and the end of the download as "checking".
                            let megabytes = download.received >> 20;
                            if megabytes != last.0 || download.total != last.1 {
                                last = (megabytes, download.total);
                                progress(if download.total == Some(download.received) { Step::CheckingDownload } else { Step::Downloading { received: download.received, total: download.total } });
                            }
                        },
                        cancel,
                    )?;
                    dmg
                }
            };
            progress(Step::PreparingInstaller);
            let image = vm.config_dir().join("BaseSystem.img");
            qemu_img(settings, &["convert", "-f", "dmg", "-O", "raw"], &[&dmg, &image], None).context("converting the installer image")?;
            if new.source == InstallerSource::Apple {
                let _ = std::fs::remove_file(&dmg);
            }
            PathBuf::from("vmconfig").join("BaseSystem.img")
        }
    };
    vm.config.install = Install { stage: InstallStage::NotStarted, easy: new.easy_install, media: Some(media) };
    vm.save()?;
    Ok(vm)
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension().is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// Runs `qemu-img` with `options`, then `paths`, then an optional last argument.
fn qemu_img(settings: &AppSettings, options: &[&str], paths: &[&Path], last: Option<&str>) -> Result<()> {
    let mut command = Command::new(settings.qemu_img_exe());
    command.args(options).args(paths).args(last).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().with_context(|| format!("starting {}", settings.qemu_img_exe().display()))?;
    if !output.status.success() {
        bail!("qemu-img failed: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// A network card address in QEMU's own range, different for every Mac.
fn random_mac_address() -> String {
    let digits = recovery::random_hex(6).to_lowercase();
    format!("52:54:00:{}:{}:{}", &digits[0..2], &digits[2..4], &digits[4..6])
}

#[cfg(test)]
mod tests {
    use super::{InstallerSource, NewVm, random_mac_address};
    use crate::macos::MacosVersion;
    use std::path::PathBuf;

    fn new_vm(name: &str, location: &str) -> NewVm {
        NewVm { name: name.to_string(), location: PathBuf::from(location), macos: MacosVersion::BigSur, source: InstallerSource::Apple, easy_install: true, memory_mb: 4096, cpus: 4, disk_gb: 80 }
    }

    #[test]
    fn names_that_windows_cannot_use_for_a_folder_are_refused() {
        assert!(new_vm("", r"C:\nowhere-orchord").problem().is_some());
        assert!(new_vm("Big: Sur", r"C:\nowhere-orchord").problem().is_some());
        assert!(new_vm("Big Sur.", r"C:\nowhere-orchord").problem().is_some());
        assert!(new_vm("Big Sur", "relative").problem().is_some());
        assert_eq!(new_vm("Big Sur", r"C:\nowhere-orchord").problem(), None);
    }

    #[test]
    fn the_folder_is_named_after_the_mac() {
        assert_eq!(new_vm("  Work Mac ", r"D:\VMs").folder(), PathBuf::from(r"D:\VMs\Work Mac"));
    }

    #[test]
    fn a_network_address_is_in_qemus_range() {
        let address = random_mac_address();
        assert!(address.starts_with("52:54:00:"));
        assert_eq!(address.len(), 17);
    }
}
