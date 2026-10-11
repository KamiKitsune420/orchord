//! Checks what a PC needs before it can run a Mac, and gets what is missing.
//!
//! * QEMU is installed with winget, Windows' own package manager, which downloads QEMU's
//!   installer from QEMU's site and checks it against the checksum in its catalogue.
//! * Whether the Windows Hypervisor Platform works is found out by asking QEMU itself to use
//!   it, which tests exactly what a Mac will need. Turning the feature on is done by
//!   Windows' `dism`, with the permission question Windows asks for such changes.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::settings::AppSettings;

const QEMU_PACKAGE: &str = "SoftwareFreedomConservancy.QEMU";
/// Where QEMU's installer puts it, and so where to look after installing.
pub const QEMU_INSTALL_FOLDER: &str = r"C:\Program Files\qemu";
/// Where to send someone whose Windows has no winget.
pub const QEMU_DOWNLOAD_PAGE: &str = "https://www.qemu.org/download/#windows";
/// QEMU answers the hypervisor question in a second or two; this is for a PC under heavy load.
const PROBE_LIMIT: Duration = Duration::from_secs(60);
/// winget's codes for "this is already installed" and "there is nothing newer".
const WINGET_ALREADY_INSTALLED: [i32; 2] = [0x8A15_0061_u32 as i32, 0x8A15_002B_u32 as i32];
/// dism's code for "done, and Windows must be restarted".
const DISM_RESTART_NEEDED: i32 = 3010;

fn hidden(mut command: Command) -> Command {
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// winget, if this Windows has it. It comes with Windows 11 and with current Windows 10.
pub fn winget() -> Option<PathBuf> {
    let program = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Microsoft").join("WindowsApps").join("winget.exe");
    program.exists().then_some(program)
}

/// Starts downloading and installing QEMU. It takes a few minutes, and near the end Windows
/// asks the user for permission. A QEMU that is already installed is left as it is.
pub fn start_qemu_install() -> Result<Child> {
    let winget = winget().context("This Windows has no winget")?;
    let mut command = hidden(Command::new(winget));
    command.args(["install", "--id", QEMU_PACKAGE, "--exact", "--silent", "--no-upgrade", "--disable-interactivity", "--accept-package-agreements", "--accept-source-agreements"]);
    command.stdout(Stdio::null()).stderr(Stdio::null()).spawn().context("winget could not be started")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed,
    AlreadyInstalled,
    /// With winget's code, for someone who wants to look it up.
    Failed(i32),
}

pub fn qemu_install_outcome(status: ExitStatus) -> InstallOutcome {
    match status.code() {
        Some(0) => InstallOutcome::Installed,
        Some(code) if WINGET_ALREADY_INSTALLED.contains(&code) => InstallOutcome::AlreadyInstalled,
        code => InstallOutcome::Failed(code.unwrap_or(-1)),
    }
}

/// What stops QEMU from using the Windows Hypervisor Platform on this PC, in QEMU's own
/// words, or `None` when it works. QEMU must be installed.
pub fn hypervisor_problem(settings: &AppSettings) -> Option<String> {
    accelerator_problem(settings, "whpx")
}

/// Starts a QEMU with nothing in it, paused, on the named accelerator, with its control
/// channel on its standard output. QEMU sets the accelerator up before it opens that channel:
/// one that can use the accelerator sends the channel's greeting, and one that cannot stops
/// with a message instead. Waiting for one or the other gives a sure answer on a slow or
/// busy PC, where "it has not stopped yet" would not.
fn accelerator_problem(settings: &AppSettings, accelerator: &str) -> Option<String> {
    let mut command = hidden(Command::new(settings.qemu_exe()));
    command.args(["-accel", accelerator, "-machine", "q35", "-display", "none", "-nodefaults", "-S", "-qmp", "stdio"]);
    // The channel's input is held open: QEMU would take its closing as a request to quit.
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut qemu = match command.spawn() {
        Ok(qemu) => qemu,
        Err(error) => return Some(format!("QEMU could not be started: {error}")),
    };
    let (sender, greeting) = mpsc::channel();
    if let Some(output) = qemu.stdout.take() {
        std::thread::spawn(move || {
            let mut line = String::new();
            let greeted = BufReader::new(output).read_line(&mut line).is_ok_and(|read| read > 0) && line.contains("QMP");
            let _ = sender.send(greeted);
        });
    }
    let works = greeting.recv_timeout(PROBE_LIMIT).unwrap_or(false);
    let _ = qemu.kill();
    let said = qemu.wait_with_output().map(|output| String::from_utf8_lossy(&output.stderr).into_owned()).unwrap_or_default();
    if works {
        return None;
    }
    let said = said.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" ");
    Some(if said.is_empty() { "QEMU gave no answer.".to_string() } else { said })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypervisorChange {
    /// Turned on; it works after Windows has been restarted.
    RestartNeeded,
    /// Turned on, or already on, with no restart asked for.
    Done,
    /// The user said no at Windows' permission question, or `dism` failed, with its code.
    NotDone(i32),
}

/// Turns the Windows Hypervisor Platform feature on. Windows asks the user for permission.
pub fn enable_hypervisor() -> HypervisorChange {
    // Start-Process with the RunAs verb is what brings up the permission question; a "no"
    // there makes it fail, which leaves PowerShell's own exit code, 1.
    let script = "$p = Start-Process dism.exe -ArgumentList '/online','/enable-feature','/featurename:HypervisorPlatform','/all','/norestart' -Verb RunAs -WindowStyle Hidden -Wait -PassThru; exit $p.ExitCode";
    let mut command = hidden(Command::new("powershell.exe"));
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]).stdout(Stdio::null()).stderr(Stdio::null());
    match command.status().ok().and_then(|status| status.code()) {
        Some(0) => HypervisorChange::Done,
        Some(DISM_RESTART_NEEDED) => HypervisorChange::RestartNeeded,
        code => HypervisorChange::NotDone(code.unwrap_or(-1)),
    }
}

#[cfg(test)]
mod tests {
    use super::{InstallOutcome, accelerator_problem, hypervisor_problem};
    use crate::settings::AppSettings;

    #[test]
    fn winget_codes_are_told_apart() {
        // An exit status cannot be made up, so the mapping is checked on the codes themselves.
        assert!(super::WINGET_ALREADY_INSTALLED.contains(&-1978335135));
        assert!(super::WINGET_ALREADY_INSTALLED.contains(&-1978335189));
        assert_ne!(InstallOutcome::Installed, InstallOutcome::Failed(1));
    }

    #[test]
    fn qemu_says_whether_it_can_use_an_accelerator() {
        // Needs QEMU, so it runs where QEMU is installed in its usual place.
        let settings = AppSettings::default();
        if settings.qemu_exe().is_file() {
            let problem = accelerator_problem(&settings, "no-such-accelerator");
            assert!(problem.is_some_and(|problem| problem.contains("no-such-accelerator")), "QEMU names the accelerator it was refused");
            // The emulator that needs no hypervisor always works, which shows that a working
            // accelerator is recognised. On the PC Orchord is developed on the hypervisor is
            // on as well; elsewhere either answer to that is right.
            assert_eq!(accelerator_problem(&settings, "tcg"), None);
            let _ = hypervisor_problem(&settings);
        }
    }
}
