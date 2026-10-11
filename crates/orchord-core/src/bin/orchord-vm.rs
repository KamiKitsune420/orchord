//! Runs one virtual machine from the command line and prints what happens to it.
//!
//! Usage: orchord-vm <vm-folder> [--debug-qmp <port>] [--install-media <image>] [--qemu-arg <argument>]...

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use orchord_core::{AppSettings, StopReason, Supervisor, Vm, VmEvent};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(vm_dir) = args.next() else { bail!("usage: orchord-vm <vm-folder> [--debug-qmp <port>] [--install-media <image>] [--qemu-arg <argument>]...") };
    let settings = AppSettings::load()?;
    let vm = Vm::load(&PathBuf::from(&vm_dir))?;
    let name = vm.config.name.clone();
    let mut supervisor = Supervisor::new(vm, settings);
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
    let worker = std::thread::spawn(move || supervisor.run(event_sender, command_receiver));
    for event in event_receiver {
        let seconds = started.elapsed().as_secs();
        match event {
            VmEvent::Starting { launch } => println!("[{seconds:>5} s] {name}: starting (launch {launch})"),
            VmEvent::Running { launch } => println!("[{seconds:>5} s] {name}: running (launch {launch})"),
            VmEvent::GuestRestart { count } => println!("[{seconds:>5} s] {name}: the Mac restarted (restart {count}); starting it again"),
            VmEvent::Crashed(panic) => println!("[{seconds:>5} s] {name}: macOS crashed. {panic}"),
            VmEvent::Suspended =>println!("[{seconds:>5} s] {name}: suspended"),
            VmEvent::Resumed => println!("[{seconds:>5} s] {name}: resumed"),
            VmEvent::Screenshot(path) => println!("[{seconds:>5} s] {name}: screen saved to {}", path.display()),
            VmEvent::Stopped(StopReason::GuestShutDown) => println!("[{seconds:>5} s] {name}: the Mac shut down"),
            VmEvent::Stopped(StopReason::StoppedByUser) => println!("[{seconds:>5} s] {name}: stopped"),
            VmEvent::Stopped(StopReason::Failed(reason)) => println!("[{seconds:>5} s] {name}: failed. {reason}"),
        }
    }
    worker.join().expect("the supervisor thread panicked");
    Ok(())
}
