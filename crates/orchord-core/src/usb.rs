//! Handing USB devices plugged into this PC over to a running Mac, and taking them back.
//!
//! QEMU does the work: it lists the PC's USB devices, and a `usb-host` device added to the
//! running Mac takes one of them away from Windows for as long as it is attached. On Windows
//! QEMU can only take a device whose driver lets it, which in practice means the UsbDk driver
//! is installed or the device uses the generic WinUSB driver; otherwise attaching fails and
//! the device simply stays with Windows.

use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use crate::supervisor::VmCommand;

/// How long the supervisor is given to answer. It serves commands many times a second.
const ANSWER_LIMIT: Duration = Duration::from_secs(5);
/// QEMU opens a host device a moment after the device is added.
const OPEN_WAIT: Duration = Duration::from_millis(2500);

/// A USB device plugged into this PC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostUsbDevice {
    pub vendor: u16,
    pub product: u16,
    /// The device's own name for itself, or its two identifiers when it gives none.
    pub name: String,
}

impl HostUsbDevice {
    /// The name the device goes by inside QEMU while it is attached to the Mac.
    fn qemu_id(&self) -> String {
        format!("orchord-usb-{:04x}-{:04x}", self.vendor, self.product)
    }
}

/// The USB devices QEMU can see on this PC.
pub fn host_devices(commands: &Sender<VmCommand>) -> Result<Vec<HostUsbDevice>> {
    Ok(parse_host_devices(&monitor(commands, "info usbhost")?))
}

/// Which of `devices` are attached to the Mac at the moment.
pub fn attached(commands: &Sender<VmCommand>, devices: &[HostUsbDevice]) -> Result<Vec<HostUsbDevice>> {
    let listing = ask(commands, "qom-list", Some(json!({ "path": "/machine/peripheral" })))?;
    let names: Vec<&str> = listing.as_array().into_iter().flatten().filter_map(|entry| entry["name"].as_str()).collect();
    Ok(devices.iter().filter(|device| names.contains(&device.qemu_id().as_str())).cloned().collect())
}

/// Takes `device` away from Windows and plugs it into the Mac.
pub fn attach(commands: &Sender<VmCommand>, device: &HostUsbDevice) -> Result<()> {
    let before = guest_device_count(commands)?;
    let arguments = json!({ "driver": "usb-host", "id": device.qemu_id(), "bus": "xhci.0", "vendorid": device.vendor, "productid": device.product });
    ask(commands, "device_add", Some(arguments))?;
    std::thread::sleep(OPEN_WAIT);
    if guest_device_count(commands)? > before {
        return Ok(());
    }
    // QEMU keeps the request and retries for ever; take it back so the device is not grabbed later by surprise.
    let _ = detach(commands, device);
    bail!("Windows did not let go of the device. QEMU can only take a USB device that uses the UsbDk or WinUSB driver.")
}

/// Unplugs `device` from the Mac, which gives it back to Windows.
pub fn detach(commands: &Sender<VmCommand>, device: &HostUsbDevice) -> Result<()> {
    ask(commands, "device_del", Some(json!({ "id": device.qemu_id() }))).map(|_| ())
}

/// How many USB devices the Mac has plugged in, counting its own virtual ones.
fn guest_device_count(commands: &Sender<VmCommand>) -> Result<usize> {
    Ok(monitor(commands, "info usb")?.lines().filter(|line| line.trim_start().starts_with("Device ")).count())
}

/// Runs a command of QEMU's human monitor and returns what it printed.
fn monitor(commands: &Sender<VmCommand>, command: &str) -> Result<String> {
    let answer = ask(commands, "human-monitor-command", Some(json!({ "command-line": command })))?;
    Ok(answer.as_str().unwrap_or_default().to_string())
}

fn ask(commands: &Sender<VmCommand>, execute: &str, arguments: Option<Value>) -> Result<Value> {
    let (reply, answer) = mpsc::channel();
    commands.send(VmCommand::Qmp { execute: execute.to_string(), arguments, reply }).map_err(|_| anyhow!("The Mac is not running."))?;
    answer.recv_timeout(ANSWER_LIMIT).map_err(|_| anyhow!("The Mac did not answer."))?.map_err(|error| anyhow!(error))
}

/// Reads QEMU's `info usbhost` listing, which gives each device two lines:
/// `  Bus 1, Addr 5, Port 3, Speed 12 Mb/s` and `    Class 00: USB device 045e:028e, Controller`.
fn parse_host_devices(listing: &str) -> Vec<HostUsbDevice> {
    let mut devices: Vec<HostUsbDevice> = Vec::new();
    for line in listing.lines() {
        let Some((_, rest)) = line.split_once("USB device ") else { continue };
        let (identifiers, name) = rest.split_once(',').unwrap_or((rest, ""));
        let Some((vendor, product)) = identifiers.trim().split_once(':') else { continue };
        let (Ok(vendor), Ok(product)) = (u16::from_str_radix(vendor, 16), u16::from_str_radix(product, 16)) else { continue };
        let name = if name.trim().is_empty() { format!("USB device {vendor:04x}:{product:04x}") } else { name.trim().to_string() };
        let device = HostUsbDevice { vendor, product, name };
        // Two identical devices cannot be told apart by QEMU's simple matching; list the kind once.
        if !devices.iter().any(|known| known.vendor == device.vendor && known.product == device.product) {
            devices.push(device);
        }
    }
    devices
}

#[cfg(test)]
mod tests {
    use super::{HostUsbDevice, parse_host_devices};

    #[test]
    fn the_host_listing_is_read_into_devices() {
        let listing = "  Bus 1, Addr 5, Port 3, Speed 12 Mb/s\n    Class 00: USB device 045e:028e, Controller\n  Bus 1, Addr 2, Port 10, Speed 12 Mb/s\n    Class e0: USB device 8087:0026\n  Bus 1, Addr 9, Port 4, Speed 12 Mb/s\n    Class 00: USB device 045e:028e, Controller\n";
        assert_eq!(
            parse_host_devices(listing),
            vec![HostUsbDevice { vendor: 0x045e, product: 0x028e, name: "Controller".to_string() }, HostUsbDevice { vendor: 0x8087, product: 0x0026, name: "USB device 8087:0026".to_string() }]
        );
        assert!(parse_host_devices("").is_empty());
    }

    #[test]
    fn a_device_has_a_stable_name_inside_qemu() {
        assert_eq!(HostUsbDevice { vendor: 0x45e, product: 0x28e, name: String::new() }.qemu_id(), "orchord-usb-045e-028e");
    }
}
