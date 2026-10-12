//! Embeds the Windows application manifest and puts the speech library and the Macs' boot
//! folder beside the program.
//!
//! wxWidgets needs Common Controls version 6 declared in the manifest; without it the program
//! starts with a warning dialog and old-style controls. `prism.dll` must sit in the same folder
//! as `orchord.exe`, or Windows refuses to start the program. The `opencore` folder is where
//! the program looks for the boot folder unless the settings file names another; on a PC whose
//! processor is not Intel's it looks in `opencore-amd`.

use std::path::{Path, PathBuf};

use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Orchord")).expect("unable to embed the application manifest");
        copy_beside_program();
    }
    println!("cargo:rerun-if-changed=build.rs");
}

fn copy_beside_program() {
    let third_party = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")).join("third_party");
    // OUT_DIR is target\<profile>\build\orchord-<hash>\out; the program lands three levels up.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let profile_dir = out_dir.ancestors().nth(3).expect("OUT_DIR is inside the target folder");

    let speech = third_party.join("prism").join("bin").join("prism.dll");
    println!("cargo:rerun-if-changed={}", speech.display());
    std::fs::copy(&speech, profile_dir.join("prism.dll")).expect("unable to copy prism.dll beside the program");

    let boot_folder = third_party.join("opencore").join("EFI");
    println!("cargo:rerun-if-changed={}", boot_folder.join("OC").join("config.plist").display());
    copy_folder(&boot_folder, &profile_dir.join("opencore").join("EFI")).expect("unable to copy the OpenCore boot folder beside the program");

    // The boot folder for processors that are not Intel's: the same files, another configuration.
    let other_config = third_party.join("opencore").join("config-amd.plist");
    println!("cargo:rerun-if-changed={}", other_config.display());
    let other_folder = profile_dir.join("opencore-amd").join("EFI");
    copy_folder(&boot_folder, &other_folder).expect("unable to copy the second OpenCore boot folder beside the program");
    std::fs::copy(&other_config, other_folder.join("OC").join("config.plist")).expect("unable to copy the configuration for other processors");
}

fn copy_folder(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
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
