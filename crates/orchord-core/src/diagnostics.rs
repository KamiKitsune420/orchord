//! A report on a virtual machine that will not start, for sending to someone who can help.
//!
//! It gathers what that person would otherwise have to ask for one thing at a time: the PC's
//! processor, the QEMU in use, how the Mac is started, and the ends of the logs, of which the
//! Mac's serial log matters most, because macOS writes there from its first moments, long
//! before it can draw on the screen. The Apple SMC key is never in the report.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::config::Vm;
use crate::installer_image::InstallMedia;
use crate::qemu::{LaunchOptions, build_args};
use crate::settings::AppSettings;
use crate::{host, ocr, setup, smc_key};

pub const REPORT_FILE: &str = "Orchord diagnostic report.txt";
const HIDDEN: &str = "(hidden)";

/// Writes the report into the virtual machine's folder and returns where it is.
pub fn save(vm: &Vm, settings: &AppSettings) -> Result<PathBuf> {
    let path = vm.dir.join(REPORT_FILE);
    std::fs::write(&path, report(vm, settings)).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

pub fn report(vm: &Vm, settings: &AppSettings) -> String {
    let mut text = String::new();
    let mut section = |title: &str, body: String| {
        text.push_str(&format!("== {title} ==\r\n{}\r\n\r\n", body.trim_end().replace("\r\n", "\n").replace('\n', "\r\n")));
    };

    section("Orchord", format!("Version {}", env!("CARGO_PKG_VERSION")));
    section("Windows", said_by("cmd.exe", &["/c", "ver"]));
    let threads = std::thread::available_parallelism().map(|count| count.get()).unwrap_or(0);
    section("Processor", format!("{}\nMaker: {}\nThreads: {threads}", host::cpu_name(), host::cpu_vendor()));

    let qemu = if settings.qemu_exe().is_file() { said_by(&settings.qemu_exe().to_string_lossy(), &["--version"]).lines().next().unwrap_or("").to_string() } else { "not found".to_string() };
    section("QEMU", format!("Folder: {}\n{qemu}", settings.qemu_dir.display()));
    let hypervisor = if settings.qemu_exe().is_file() { setup::hypervisor_problem(settings).unwrap_or_else(|| "works".to_string()) } else { "not checked: QEMU not found".to_string() };
    section("Hypervisor", format!("Accelerator: {}\nResult: {hypervisor}", settings.accelerator()));
    let key = if smc_key::is_genuine(&settings.osk) { "entered" } else if settings.osk.is_empty() { "not entered" } else { "entered, but not the key" };
    section("Settings", format!("Boot folder: {}\nBoot folder found: {}\nVirtual machines: {}\nApple SMC key: {key}", settings.efi_dir.display(), settings.efi_dir.join("EFI").join("OC").join("config.plist").is_file(), settings.vm_root.display()));

    section("Virtual machine", format!("Folder: {}\nDisk: {} bytes\n\n{}", vm.dir.display(), vm.disk_bytes(), read_text(&vm.config_dir().join("vm.toml"))));
    let install_media = vm.install_media_path().map(|path| InstallMedia { path, format: "raw" });
    let arguments = build_args(vm, settings, &LaunchOptions { install_media, ..Default::default() });
    section("How QEMU is started", arguments.iter().map(|argument| argument.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" "));

    section("QEMU's log, the end", ends(&read_text(&vm.qemu_log_path()), 0, 40));
    section("The Mac's serial log, start and end", ends(&read_text(&vm.serial_log_path()), 30, 150));
    section("Easy install's log, the end", ends(&read_text(&vm.config_dir().join("easy-install.log")), 0, 40));
    let picture = ["screen.ppm", "install-screen.ppm"].iter().map(|name| vm.config_dir().join(name)).find(|path| path.is_file());
    section("Text on the Mac's screen when it was last looked at", picture.and_then(|path| ocr::read_ppm(&path).ok()).map(|screen| screen.text()).unwrap_or_else(|| "no picture of the screen has been kept".to_string()));
    section("Privacy", "This report holds folder names, which may include your Windows user name. It does not hold the Apple SMC key.".to_string());

    // The key is in the way QEMU is started; this takes it out wherever it may have got to.
    if settings.osk.is_empty() { text } else { text.replace(&settings.osk, HIDDEN) }
}

/// What a program prints, or why it could not be asked.
fn said_by(program: &str, arguments: &[&str]) -> String {
    let mut command = Command::new(program);
    command.args(arguments).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    match command.output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(error) => format!("could not be asked: {error}"),
    }
}

/// A file as text, with the control codes a terminal would have obeyed taken out.
fn read_text(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).chars().filter(|character| *character == '\n' || *character == '\t' || !character.is_control()).collect(),
        Err(_) => String::new(),
    }
}

/// The first `first` and last `last` lines of `text`, saying how many were left out between.
fn ends(text: &str, first: usize, last: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return "empty, or not there".to_string();
    }
    if lines.len() <= first + last {
        return lines.join("\n");
    }
    let mut kept: Vec<String> = lines[..first].iter().map(|line| line.to_string()).collect();
    kept.push(format!("... {} lines left out ...", lines.len() - first - last));
    kept.extend(lines[lines.len() - last..].iter().map(|line| line.to_string()));
    kept.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{ends, report};
    use crate::config::{Vm, VmConfig};
    use crate::macos::MacosVersion;
    use crate::settings::AppSettings;

    #[test]
    fn long_logs_are_cut_in_the_middle() {
        let log: String = (1..=10).map(|number| format!("line {number}\n")).collect();
        assert_eq!(ends(&log, 2, 3), "line 1\nline 2\n... 5 lines left out ...\nline 8\nline 9\nline 10");
        assert_eq!(ends("one\ntwo\n", 2, 3), "one\ntwo");
        assert_eq!(ends("", 2, 3), "empty, or not there");
    }

    #[test]
    fn the_key_never_reaches_the_report() {
        let key = "NotTheRealKey(c)".repeat(4);
        let dir = std::env::temp_dir().join(format!("orchord-diagnostics-test-{}", std::process::id()));
        let vm = Vm { dir: dir.clone(), config: VmConfig::new("Report Test", MacosVersion::BigSur) };
        vm.save().unwrap();
        // A QEMU folder that is not there keeps the test from starting any program.
        let settings = AppSettings { osk: key.clone(), qemu_dir: dir.join("no-qemu"), ..AppSettings::default() };
        let text = report(&vm, &settings);
        assert!(!text.contains(&key));
        assert!(text.contains("isa-applesmc,osk=(hidden)"), "the start-up line is there with the key hidden");
        assert!(text.contains("Report Test") && text.contains("Maker: "));
        let _ = std::fs::remove_dir_all(dir);
    }
}
