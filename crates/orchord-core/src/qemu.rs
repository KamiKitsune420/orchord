//! Builds the QEMU command line for a Mac guest on a Windows host.
//!
//! The choices below are not arbitrary; each one fixes a failure seen on real hardware:
//!
//! * One core per socket. Under WHPX the guest sees contradictory core counts, and macOS
//!   panics with a divide error in `cpu_thread_alloc` for any other layout.
//! * No USB keyboard. USB input is dead on Catalina and on installed Sequoia, so key events
//!   go to the PS/2 keyboard (driven in the guest by VoodooPS2Controller) on every version.
//! * VirtIO sound. It is the only sound device that is clean on Big Sur and later.
//! * A fixed device order. The firmware stores the startup disk as a PCI path, so adding a
//!   PCI device ahead of the disk controller makes a VM unbootable.

use std::ffi::OsString;
use std::path::Path;

use crate::config::{AudioMode, Vm};
use crate::installer_image::InstallMedia;
use crate::settings::AppSettings;

const CPU_FLAGS: &str = "vendor=GenuineIntel,+invtsc,+ssse3,+sse4.2,+popcnt,+avx,+aes,+xsave,+xsaveopt";

#[derive(Clone, Debug, Default)]
pub struct LaunchOptions {
    /// TCP port on 127.0.0.1 for the supervisor's QMP connection.
    pub qmp_port: u16,
    /// A second QMP port for diagnostic tools; QMP accepts one client per socket.
    pub debug_qmp_port: Option<u16>,
    /// A macOS installer image to attach as install media, for the first boot of an installation.
    pub install_media: Option<InstallMedia>,
    /// Extra QEMU arguments, added last so that they win. For experiments from the command-line runner.
    pub extra_args: Vec<String>,
}

/// A file name inside a QEMU option. QEMU splits options at commas; a doubled comma is a real one.
fn file(path: &Path) -> String {
    path.to_string_lossy().replace(',', ",,")
}

fn forward_slashes(path: &Path) -> String {
    file(path).replace('\\', "/")
}

pub fn build_args(vm: &Vm, settings: &AppSettings, options: &LaunchOptions) -> Vec<OsString> {
    let config = &vm.config;
    let mut args: Vec<String> = Vec::new();
    let mut arg = |name: &str, value: String| {
        args.push(name.to_string());
        args.push(value);
    };

    arg("-accel", settings.accelerator());
    arg("-machine", "q35".into());
    arg("-m", config.memory_mb.to_string());
    arg("-cpu", format!("{},{}", crate::host::guest_cpu(config.macos.cpu_model(), &crate::host::cpu_vendor()), CPU_FLAGS));
    arg("-smp", format!("{0},sockets={0},cores=1,threads=1", config.cpus));

    arg("-device", "qemu-xhci,id=xhci".into());
    arg("-device", "usb-ehci,id=ehci,addr=0x10".into());
    arg("-device", format!("isa-applesmc,osk={}", settings.osk));
    arg("-drive", format!("if=pflash,format=raw,readonly=on,file={}", file(&settings.firmware_code())));
    arg("-drive", format!("if=pflash,format=raw,file={}", file(&vm.vars_path())));
    arg("-smbios", "type=2".into());

    arg("-device", "ich9-ahci,id=sata".into());
    // The OpenCore folder is presented as a FAT disk by QEMU's vvfat driver, read-only in effect.
    arg("-drive", format!("id=OpenCoreBoot,if=none,snapshot=on,format=raw,file.driver=vvfat,file.dir={}", forward_slashes(&settings.efi_dir)));
    arg("-device", "ide-hd,bus=sata.2,drive=OpenCoreBoot".into());
    if let Some(media) = &options.install_media {
        arg("-drive", format!("id=InstallMedia,if=none,format={},snapshot=on,file={}", media.format, file(&media.path)));
        arg("-device", "ide-hd,bus=sata.3,drive=InstallMedia".into());
    }
    arg("-drive", format!("id=MacHDD,if=none,format=qcow2,discard=unmap,file={}", file(&vm.disk_path())));
    arg("-device", "ide-hd,bus=sata.4,drive=MacHDD,rotation_rate=1".into());

    arg("-netdev", "user,id=net0".into());
    arg("-device", format!("vmxnet3,netdev=net0,id=net0,mac={}", config.mac_address));
    arg("-device", "vmware-svga".into());

    match config.audio {
        AudioMode::Host => {
            arg("-audiodev", "dsound,id=snd0".into());
            arg("-device", "virtio-sound-pci,audiodev=snd0".into());
        }
        AudioMode::Record => {
            arg("-audiodev", format!("wav,id=snd0,path={}", file(&vm.audio_recording_path())));
            arg("-device", "virtio-sound-pci,audiodev=snd0".into());
        }
        AudioMode::None => {}
    }

    // The pointer stays on USB: it works where USB works and is ignored where it does not.
    arg("-device", "usb-tablet,bus=xhci.0".into());

    arg("-display", "none".into());
    arg("-qmp", format!("tcp:127.0.0.1:{},server=on,wait=off", options.qmp_port));
    if let Some(port) = options.debug_qmp_port {
        arg("-qmp", format!("tcp:127.0.0.1:{port},server=on,wait=off"));
    }
    // Everything after "file:" is the name, commas included, so this one is not doubled.
    arg("-serial", format!("file:{}", vm.serial_log_path().display()));

    args.extend(options.extra_args.iter().cloned());
    args.into_iter().map(OsString::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VmConfig;
    use crate::macos::MacosVersion;
    use std::path::PathBuf;

    fn sample(macos: MacosVersion) -> (Vm, AppSettings) {
        let vm = Vm { dir: PathBuf::from(r"C:\vms\Test"), config: VmConfig::new("Test", macos) };
        let settings = AppSettings { efi_dir: PathBuf::from(r"C:\efi"), osk: "KEY".into(), ..AppSettings::default() };
        (vm, settings)
    }

    fn joined(args: &[OsString]) -> String {
        args.iter().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn every_core_is_its_own_socket() {
        let (mut vm, settings) = sample(MacosVersion::BigSur);
        vm.config.cpus = 4;
        let line = joined(&build_args(&vm, &settings, &LaunchOptions { qmp_port: 5000, ..Default::default() }));
        assert!(line.contains("-smp 4,sockets=4,cores=1,threads=1"), "{line}");
    }

    #[test]
    fn sequoia_gets_a_cpu_model_with_avx2() {
        let (vm, settings) = sample(MacosVersion::Sequoia);
        let line = joined(&build_args(&vm, &settings, &LaunchOptions::default()));
        // What follows the model depends on who made the processor the test runs on.
        assert!(line.contains("-cpu Skylake-Client,-hle,-rtm,") && line.contains(",vendor=GenuineIntel,"), "{line}");
    }

    #[test]
    fn there_is_no_usb_keyboard() {
        let (vm, settings) = sample(MacosVersion::Monterey);
        let line = joined(&build_args(&vm, &settings, &LaunchOptions::default()));
        assert!(!line.contains("usb-kbd"), "{line}");
    }

    #[test]
    fn the_disk_controller_comes_before_optional_devices() {
        let (vm, settings) = sample(MacosVersion::BigSur);
        let line = joined(&build_args(&vm, &settings, &LaunchOptions::default()));
        let ahci = line.find("ich9-ahci").unwrap();
        for later in ["vmxnet3", "vmware-svga", "virtio-sound-pci"] {
            assert!(line.find(later).unwrap() > ahci, "{later} must come after the disk controller");
        }
    }

    #[test]
    fn install_media_is_attached_only_when_asked() {
        let (vm, settings) = sample(MacosVersion::BigSur);
        let without = joined(&build_args(&vm, &settings, &LaunchOptions::default()));
        assert!(!without.contains("InstallMedia"));
        let options = LaunchOptions { install_media: Some(InstallMedia { path: PathBuf::from(r"C:\base.img"), format: "raw" }), ..Default::default() };
        assert!(joined(&build_args(&vm, &settings, &options)).contains("ide-hd,bus=sata.3,drive=InstallMedia"));
    }

    #[test]
    fn a_comma_in_a_file_name_is_doubled_for_qemu() {
        let (vm, settings) = sample(MacosVersion::BigSur);
        let media = InstallMedia { path: PathBuf::from(r"D:\Macs\Home, work\vmconfig\installer.vmdk"), format: "vmdk" };
        let line = joined(&build_args(&vm, &settings, &LaunchOptions { install_media: Some(media), ..Default::default() }));
        assert!(line.contains(r"format=vmdk,snapshot=on,file=D:\Macs\Home,, work\vmconfig\installer.vmdk"), "{line}");
    }
}
