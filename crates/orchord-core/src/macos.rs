//! The macOS versions orchord knows how to run, and what each needs from the virtual hardware.
//!
//! Every value here was established by booting that version under QEMU with the Windows
//! Hypervisor Platform accelerator; see the hands-on section of the research report.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MacosVersion {
    BigSur,
    Monterey,
    Sequoia,
}

impl MacosVersion {
    pub const ALL: [MacosVersion; 3] = [MacosVersion::BigSur, MacosVersion::Monterey, MacosVersion::Sequoia];

    pub fn display_name(self) -> &'static str {
        match self {
            MacosVersion::BigSur => "macOS Big Sur 11",
            MacosVersion::Monterey => "macOS Monterey 12",
            MacosVersion::Sequoia => "macOS Sequoia 15",
        }
    }

    /// QEMU CPU model. Sequoia needs AVX2, which Penryn lacks; Haswell-noTSX and `host`
    /// do not boot under WHPX, so Skylake-Client is the only model with AVX2 that works.
    pub fn cpu_model(self) -> &'static str {
        match self {
            MacosVersion::BigSur | MacosVersion::Monterey => "Penryn",
            MacosVersion::Sequoia => "Skylake-Client,-hle,-rtm",
        }
    }

    pub fn default_memory_mb(self) -> u32 {
        match self {
            MacosVersion::BigSur | MacosVersion::Monterey => 4096,
            MacosVersion::Sequoia => 5120,
        }
    }

    pub fn default_disk_gb(self) -> u32 {
        match self {
            MacosVersion::BigSur | MacosVersion::Monterey => 80,
            MacosVersion::Sequoia => 96,
        }
    }

    /// Whether VoiceOver can speak inside this version's installer (Recovery).
    /// Monterey's Recovery image cannot start VoiceOver at all.
    pub fn installer_speaks(self) -> bool {
        !matches!(self, MacosVersion::Monterey)
    }
}
