//! Runs one virtual machine from the command line and prints what happens to it.
//!
//! Usage:
//!   orchord-vm <vm-folder> [--debug-qmp <port>] [--install-media <image>] [--qemu-arg <argument>]...
//!   orchord-vm new <name> <location> <big-sur|monterey|sonoma|sequoia> <apple|installer-file> [--easy]
//!   orchord-vm read <screen.ppm>      prints the text recognised on a picture of a Mac's screen
//!   orchord-vm smc-key                says whether each public source still holds the Apple SMC key
//!   orchord-vm check                  says whether this PC has what a Mac needs, changing nothing
//!   orchord-vm report <vm-folder>     writes the diagnostic report for a Mac and prints where it is
//!
//! A Mac set up for easy install whose macOS is not installed yet is run through easy install,
//! exactly as the window does it.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use orchord_core::create::{self, InstallerSource, NewVm};
use orchord_core::{AppSettings, InstallStage, MacosVersion, StopReason, Supervisor, Vm, VmEvent, easy_install};

const USAGE: &str = "usage: orchord-vm <vm-folder> [--debug-qmp <port>] [--install-media <image>] [--qemu-arg <argument>]...\n       orchord-vm new <name> <location> <big-sur|monterey|sonoma|sequoia> <apple|installer-file> [--easy]";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else { bail!(USAGE) };
    let settings = AppSettings::load()?;
    if first == "new" {
        return create_vm(&settings, args.collect());
    }
    if first == "read" {
        let picture = args.next().context("read needs a picture file")?;
        for line in orchord_core::ocr::read_ppm(&PathBuf::from(picture))?.lines {
            println!("{}", line.words.iter().map(|word| format!("{}@{:.0},{:.0}", word.text, word.x, word.y)).collect::<Vec<_>>().join("  "));
        }
        return Ok(());
    }
    if first == "check" {
        // What Orchord Settings' "Check and set up this PC" looks at, without changing anything.
        let qemu = settings.qemu_exe().is_file();
        println!("QEMU at {}: {}", settings.qemu_dir.display(), if qemu { "found" } else { "not found" });
        println!("winget: {}", if orchord_core::setup::winget().is_some() { "found" } else { "not found" });
        if qemu {
            println!("Windows Hypervisor Platform: {}", orchord_core::setup::hypervisor_problem(&settings).unwrap_or_else(|| "works".to_string()));
        }
        println!("Apple SMC key: {}", if orchord_core::smc_key::is_genuine(&settings.osk) { "entered" } else { "not entered, or not the key" });
        println!("Text recognition: {}", if orchord_core::ocr::available() { "available" } else { "no language installed" });
        return Ok(());
    }
    if first == "report" {
        // What Machine > Save Diagnostic Report writes, without opening File Explorer.
        let folder = args.next().context("report needs a virtual machine's folder")?;
        println!("{}", orchord_core::diagnostics::save(&Vm::load(&PathBuf::from(folder))?, &settings)?.display());
        return Ok(());
    }
    if first == "smc-key" {
        // Says whether each source still holds the key, without printing the key.
        for source in &orchord_core::smc_key::SOURCES {
            match orchord_core::smc_key::fetch_from(source) {
                Ok(key) => println!("{}: holds the key ({} characters, fingerprint matches)", source.project, key.len()),
                Err(error) => println!("{}: {error:#}", source.project),
            }
        }
        return Ok(());
    }
    let vm = Vm::load(&PathBuf::from(&first))?;
    let name = vm.config.name.clone();
    let easy = vm.config.install.easy && vm.config.install.stage != InstallStage::Done;
    let mut supervisor = Supervisor::new(vm.clone(), settings.clone());
    while let Some(flag) = args.next() {
        let value = args.next().with_context(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--debug-qmp" => supervisor.debug_qmp_port = Some(value.parse().context("--debug-qmp needs a port number")?),
            "--install-media" => supervisor.install_media = Some(PathBuf::from(value)),
            "--qemu-arg" => supervisor.extra_qemu_args.push(value),
            _ => bail!("unknown option {flag}"),
        }
    }

    let (event_sender, event_receiver) = mpsc::channel();
    let (_command_sender, command_receiver) = mpsc::channel();
    let started = Instant::now();
    let worker = std::thread::spawn(move || if easy { easy_install::run(vm, settings, event_sender, command_receiver) } else { supervisor.run(event_sender, command_receiver) });
    for event in event_receiver {
        let seconds = started.elapsed().as_secs();
        match event {
            VmEvent::Starting { launch } => println!("[{seconds:>5} s] {name}: starting (launch {launch})"),
            VmEvent::Running { launch } => println!("[{seconds:>5} s] {name}: running (launch {launch})"),
            VmEvent::GuestRestart { count } => println!("[{seconds:>5} s] {name}: the Mac restarted (restart {count}); starting it again"),
            VmEvent::Crashed(panic) => println!("[{seconds:>5} s] {name}: macOS crashed. {panic}"),
            VmEvent::InstallStage(stage) => println!("[{seconds:>5} s] {name}: installation stage is now {stage:?}"),
            VmEvent::InstallNote(note) => println!("[{seconds:>5} s] {name}: {note}"),
            VmEvent::Suspended => println!("[{seconds:>5} s] {name}: suspended"),
            VmEvent::Resumed => println!("[{seconds:>5} s] {name}: resumed"),
            VmEvent::Screenshot(path) => println!("[{seconds:>5} s] {name}: screen saved to {}", path.display()),
            VmEvent::Stopped(StopReason::GuestShutDown) => println!("[{seconds:>5} s] {name}: the Mac shut down"),
            VmEvent::Stopped(StopReason::StoppedByUser) => println!("[{seconds:>5} s] {name}: stopped"),
            VmEvent::Stopped(StopReason::Failed(reason)) => println!("[{seconds:>5} s] {name}: failed. {reason}"),
        }
    }
    let _ = worker.join();
    Ok(())
}

fn create_vm(settings: &AppSettings, args: Vec<String>) -> Result<()> {
    let [name, location, version, installer, rest @ ..] = args.as_slice() else { bail!(USAGE) };
    let macos = match version.as_str() {
        "big-sur" => MacosVersion::BigSur,
        "monterey" => MacosVersion::Monterey,
        "sonoma" => MacosVersion::Sonoma,
        "sequoia" => MacosVersion::Sequoia,
        _ => bail!("unknown macOS version {version}"),
    };
    let new = NewVm {
        name: name.clone(),
        location: PathBuf::from(location),
        macos,
        source: if installer == "apple" { InstallerSource::Apple } else { InstallerSource::File(PathBuf::from(installer)) },
        easy_install: rest.iter().any(|flag| flag == "--easy"),
        memory_mb: macos.default_memory_mb(),
        cpus: 4,
        disk_gb: macos.default_disk_gb(),
    };
    let mut last = String::new();
    let vm = create::create(
        &new,
        settings,
        &mut |step| {
            // A download reports every megabyte; print one line for every fifty.
            let line = match step {
                create::Step::Downloading { received, total } => format!("downloading: {} of {} MB", (received >> 20) / 50 * 50, total.map_or("?".to_string(), |total| (total >> 20).to_string())),
                other => format!("{other:?}"),
            };
            if line != last {
                println!("{line}");
                last = line;
            }
        },
        &AtomicBool::new(false),
    )?;
    println!("created {}", vm.dir.display());
    Ok(())
}
