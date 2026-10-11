//! Embeds the Windows application manifest and puts the speech library beside the program.
//!
//! wxWidgets needs Common Controls version 6 declared in the manifest; without it the program
//! starts with a warning dialog and old-style controls. `prism.dll` must sit in the same folder
//! as `orchord.exe`, or Windows refuses to start the program.

use std::path::PathBuf;

use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Orchord")).expect("unable to embed the application manifest");
        copy_speech_library();
    }
    println!("cargo:rerun-if-changed=build.rs");
}

fn copy_speech_library() {
    let source = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR")).join("third_party").join("prism").join("bin").join("prism.dll");
    println!("cargo:rerun-if-changed={}", source.display());
    // OUT_DIR is target\<profile>\build\orchord-<hash>\out; the program lands three levels up.
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let profile_dir = out_dir.ancestors().nth(3).expect("OUT_DIR is inside the target folder");
    std::fs::copy(&source, profile_dir.join("prism.dll")).expect("unable to copy prism.dll beside the program");
}
