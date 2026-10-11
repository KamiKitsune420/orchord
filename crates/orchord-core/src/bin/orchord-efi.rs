//! Builds the OpenCore boot folder Orchord's Macs start from, out of official releases only.
//!
//! Usage: orchord-efi <opencore-release-folder> <kexts-folder> <output-folder>
//!
//! - `<opencore-release-folder>` is an unpacked OpenCore release (it holds `X64` and `Docs`).
//! - `<kexts-folder>` holds `Lilu.kext`, `VirtualSMC.kext`, `WhateverGreen.kext` and
//!   `VoodooPS2Controller.kext`, unpacked from their acidanthera releases.
//! - `<output-folder>` receives an `EFI` folder, replacing one that is there.
//!
//! The configuration is OpenCore's own `Docs\Sample.plist` with the changes below applied.
//! Each change is one this project found necessary, or harmless and tidy, when booting macOS
//! under QEMU with the Windows hypervisor; the reasons are in the research report.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use plist::{Dictionary, Value};

/// The kernel extensions to load, in load order: path inside `Kexts`, the program file inside
/// it (empty when it has none), and what it is for.
const KEXTS: [(&str, &str, &str); 5] = [
    ("Lilu.kext", "Contents/MacOS/Lilu", "Patching framework the others need"),
    ("VirtualSMC.kext", "Contents/MacOS/VirtualSMC", "Stands in for the Mac's system management controller"),
    ("WhateverGreen.kext", "Contents/MacOS/WhateverGreen", "Graphics fixes"),
    ("VoodooPS2Controller.kext", "Contents/MacOS/VoodooPS2Controller", "PS/2 keyboard, the only keyboard every tested macOS accepts"),
    ("VoodooPS2Controller.kext/Contents/PlugIns/VoodooPS2Keyboard.kext", "Contents/MacOS/VoodooPS2Keyboard", "PS/2 keyboard"),
];
const DRIVERS: [&str; 2] = ["OpenRuntime.efi", "OpenHfsPlus.efi"];
const ACPI_TABLE: &str = "SSDT-EC.aml";

fn main() -> Result<()> {
    let arguments: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let [release, kexts, output] = arguments.as_slice() else { bail!("usage: orchord-efi <opencore-release-folder> <kexts-folder> <output-folder>") };
    let efi = output.join("EFI");
    if efi.exists() {
        std::fs::remove_dir_all(&efi).with_context(|| format!("removing the old {}", efi.display()))?;
    }
    let source = release.join("X64").join("EFI");
    let oc = efi.join("OC");
    copy(&source.join("BOOT").join("BOOTx64.efi"), &efi.join("BOOT").join("BOOTx64.efi"))?;
    copy(&source.join("OC").join("OpenCore.efi"), &oc.join("OpenCore.efi"))?;
    for driver in DRIVERS {
        copy(&source.join("OC").join("Drivers").join(driver), &oc.join("Drivers").join(driver))?;
    }
    copy(&release.join("Docs").join("AcpiSamples").join("Binaries").join(ACPI_TABLE), &oc.join("ACPI").join(ACPI_TABLE))?;
    for kext in ["Lilu.kext", "VirtualSMC.kext", "WhateverGreen.kext", "VoodooPS2Controller.kext"] {
        copy_folder(&kexts.join(kext), &oc.join("Kexts").join(kext))?;
    }

    let sample = release.join("Docs").join("Sample.plist");
    let mut config = Value::from_file(&sample).with_context(|| format!("reading {}", sample.display()))?;
    configure(config.as_dictionary_mut().context("Sample.plist is not a dictionary")?)?;
    config.to_file_xml(oc.join("config.plist")).context("writing config.plist")?;
    println!("built {}", efi.display());
    Ok(())
}

/// Applies Orchord's changes to OpenCore's sample configuration.
fn configure(config: &mut Dictionary) -> Result<()> {
    // ACPI: one table, the embedded controller macOS expects to find.
    set(config, &["ACPI", "Add"], Value::Array(vec![file_entry(ACPI_TABLE, "Embedded controller")]))?;
    set(config, &["ACPI", "Quirks", "ResetLogoStatus"], false.into())?;

    // Booter: the firmware here is OVMF, which needs none of the workarounds real PCs do.
    for quirk in ["EnableSafeModeSlide", "FixupAppleEfiImages", "ProvideCustomSlide", "SetupVirtualMap"] {
        set(config, &["Booter", "Quirks", quirk], false.into())?;
    }
    set(config, &["Booter", "Quirks", "ResizeAppleGpuBars"], 0.into())?;

    // The sample's device properties describe a real PC's sound chip.
    set(config, &["DeviceProperties", "Add"], Value::Dictionary(Dictionary::new()))?;

    let kexts = KEXTS.iter().map(|(path, program, comment)| kext_entry(path, program, comment)).collect();
    set(config, &["Kernel", "Add"], Value::Array(kexts))?;
    // The virtual processor has no power management for macOS to drive.
    set(config, &["Kernel", "Emulate", "DummyPowerManagement"], true.into())?;
    // Without this the kernel dies at once under the Windows hypervisor.
    set(config, &["Kernel", "Quirks", "ProvideCurrentCpuInfo"], true.into())?;
    set(config, &["Kernel", "Quirks", "ForceSecureBootScheme"], true.into())?;

    // No picker: it waited for keys that simulated key presses could not reliably give it.
    set(config, &["Misc", "Boot", "ShowPicker"], false.into())?;
    set(config, &["Misc", "Boot", "Timeout"], 2.into())?;
    set(config, &["Misc", "Debug", "Target"], 0.into())?;
    set(config, &["Misc", "Security", "AllowSetDefault"], true.into())?;
    // Only Mac volumes on SATA disks are looked at, so the installer on the Mac's disk starts by itself.
    // OpenCore knows a Mac volume by its GPT partition type. Without this filter it also offers
    // its own boot disk and stops at its menu; that is why an installer ISO, which has no GPT,
    // is given one by `installer_image` instead of the filter being loosened.
    set(config, &["Misc", "Security", "ScanPolicy"], 66307.into())?;
    set(config, &["Misc", "Security", "SecureBootModel"], "Disabled".into())?;
    set(config, &["Misc", "Security", "Vault"], "Optional".into())?;
    set(config, &["Misc", "Tools"], Value::Array(Vec::new()))?;

    // Verbose start-up on screen, a crash that stays on screen, and the kernel's log on the
    // serial port, where Orchord watches for crashes.
    set(config, &["NVRAM", "Add", "7C436110-AB2A-4BBB-A880-FE41995C9F82", "boot-args"], "-v keepsyms=1 debug=0x100 serial=3".into())?;
    set(config, &["NVRAM", "Add", "7C436110-AB2A-4BBB-A880-FE41995C9F82", "prev-lang:kbd"], Value::Data(b"en-US:0".to_vec()))?;

    // Any version of Apple's file system driver may be used, however old the macOS.
    set(config, &["UEFI", "APFS", "MinDate"], (-1).into())?;
    set(config, &["UEFI", "APFS", "MinVersion"], (-1).into())?;
    set(config, &["UEFI", "Drivers"], Value::Array(DRIVERS.iter().map(|driver| driver_entry(driver)).collect()))?;
    set(config, &["UEFI", "Output", "Resolution"], "".into())?;
    Ok(())
}

/// Sets the value at `path`, which must lead through dictionaries that already exist: a
/// misspelt key should stop the build, not add a setting OpenCore will ignore.
fn set(config: &mut Dictionary, path: &[&str], value: Value) -> Result<()> {
    let (last, parents) = path.split_last().expect("a path has at least one key");
    let mut dictionary = config;
    for key in parents {
        dictionary = dictionary.get_mut(key).and_then(Value::as_dictionary_mut).with_context(|| format!("the sample configuration has no section {key} in {}", path.join("/")))?;
    }
    if !dictionary.contains_key(last) {
        bail!("the sample configuration has no key {}", path.join("/"));
    }
    dictionary.insert(last.to_string(), value);
    Ok(())
}

fn file_entry(path: &str, comment: &str) -> Value {
    let mut entry = Dictionary::new();
    entry.insert("Comment".to_string(), comment.into());
    entry.insert("Enabled".to_string(), true.into());
    entry.insert("Path".to_string(), path.into());
    Value::Dictionary(entry)
}

fn driver_entry(path: &str) -> Value {
    let mut entry = Dictionary::new();
    entry.insert("Arguments".to_string(), "".into());
    entry.insert("Comment".to_string(), "".into());
    entry.insert("Enabled".to_string(), true.into());
    entry.insert("LoadEarly".to_string(), false.into());
    entry.insert("Path".to_string(), path.into());
    Value::Dictionary(entry)
}

fn kext_entry(path: &str, program: &str, comment: &str) -> Value {
    let mut entry = Dictionary::new();
    entry.insert("Arch".to_string(), "Any".into());
    entry.insert("BundlePath".to_string(), path.into());
    entry.insert("Comment".to_string(), comment.into());
    entry.insert("Enabled".to_string(), true.into());
    entry.insert("ExecutablePath".to_string(), program.into());
    entry.insert("MaxKernel".to_string(), "".into());
    entry.insert("MinKernel".to_string(), "".into());
    entry.insert("PlistPath".to_string(), "Contents/Info.plist".into());
    Value::Dictionary(entry)
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to.parent().expect("a file has a folder"))?;
    std::fs::copy(from, to).with_context(|| format!("copying {}", from.display()))?;
    Ok(())
}

fn copy_folder(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_folder(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
