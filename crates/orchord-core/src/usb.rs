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

/// The USB devices QEMU can see on this PC, under the names Windows gives them where QEMU
/// has none.
pub fn host_devices(commands: &Sender<VmCommand>) -> Result<Vec<HostUsbDevice>> {
    let mut devices = parse_host_devices(&monitor(commands, "info usbhost")?);
    for device in &mut devices {
        if device.name.starts_with("USB device ") {
            if let Some(name) = windows_name(device.vendor, device.product) {
                device.name = name;
            }
        }
    }
    Ok(devices)
}

/// Which of `devices` are attached to the Mac at the moment.
pub fn attached(commands: &Sender<VmCommand>, devices: &[HostUsbDevice]) -> Result<Vec<HostUsbDevice>> {
    Ok(devices.iter().filter(|device| is_attached(commands, device)).cloned().collect())
}

/// Takes `device` away from Windows and plugs it into the Mac.
pub fn attach(commands: &Sender<VmCommand>, device: &HostUsbDevice) -> Result<()> {
    let arguments = json!({ "driver": "usb-host", "id": device.qemu_id(), "bus": "xhci.0", "vendorid": device.vendor, "productid": device.product });
    ask(commands, "device_add", Some(arguments))?;
    std::thread::sleep(OPEN_WAIT);
    if is_attached(commands, device) {
        return Ok(());
    }
    // QEMU keeps the request and retries for ever; take it back so the device is not grabbed later by surprise.
    let _ = detach(commands, device);
    bail!("Windows did not let go of the device. QEMU can only take a USB device away from Windows when the UsbDk driver is installed, or when the device uses the generic WinUSB driver.")
}

/// Whether QEMU really has hold of `device`. Adding a host device always succeeds; QEMU
/// then opens the real device in the background, and only that shows in this property.
fn is_attached(commands: &Sender<VmCommand>, device: &HostUsbDevice) -> bool {
    let path = format!("/machine/peripheral/{}", device.qemu_id());
    ask(commands, "qom-get", Some(json!({ "path": path, "property": "attached" }))).is_ok_and(|attached| attached.as_bool() == Some(true))
}

/// The name Windows shows for a USB device, from the registry's list of USB devices.
fn windows_name(vendor: u16, product: u16) -> Option<String> {
    let key = format!(r"HKLM\SYSTEM\CurrentControlSet\Enum\USB\VID_{vendor:04X}&PID_{product:04X}");
    let mut command = std::process::Command::new("reg.exe");
    command.args(["query", &key, "/s"]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().ok()?;
    name_from_registry(&String::from_utf8_lossy(&output.stdout))
}

/// Picks the device's name out of `reg query` output: `FriendlyName` if there is one,
/// otherwise `DeviceDesc`, whose value looks like `@oem12.inf,%xusb.desc%;Xbox 360 Controller`.
fn name_from_registry(listing: &str) -> Option<String> {
    let value = |wanted: &str| {
        listing.lines().find_map(|line| {
            let mut parts = line.trim().splitn(3, "    ");
            (parts.next()? == wanted).then_some(())?;
            let text = parts.nth(1)?.trim();
            let name = text.rsplit(';').next().unwrap_or(text).trim();
            (!name.is_empty()).then(|| name.to_string())
        })
    };
    value("FriendlyName").or_else(|| value("DeviceDesc"))
}

/// Unplugs `device` from the Mac, which gives it back to Windows.
pub fn detach(commands: &Sender<VmCommand>, device: &HostUsbDevice) -> Result<()> {
    ask(commands, "device_del", Some(json!({ "id": device.qemu_id() }))).map(|_| ())
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
    use super::{HostUsbDevice, name_from_registry, parse_host_devices};

    #[test]
    fn a_device_name_is_read_from_the_registry_listing() {
        let listing = "HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Enum\\USB\\VID_045E&PID_028E\\6&E086E1&0&4\r\n    DeviceDesc    REG_SZ    @oem12.inf,%xusb22.devicedesc%;Xbox 360 Controller for Windows\r\n    Service    REG_SZ    xusb22\r\n";
        assert_eq!(name_from_registry(listing), Some("Xbox 360 Controller for Windows".to_string()));
        let friendly = "    DeviceDesc    REG_SZ    USB Composite Device\r\n    FriendlyName    REG_SZ    Integrated Camera\r\n";
        assert_eq!(name_from_registry(friendly), Some("Integrated Camera".to_string()));
        assert_eq!(name_from_registry("ERROR: The system was unable to find the specified registry key or value."), None);
    }

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
