//! Embeds the Windows application manifest. wxWidgets needs Common Controls version 6
//! declared there; without it the program starts with a warning dialog and old-style controls.

use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Orchord")).expect("unable to embed the application manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
