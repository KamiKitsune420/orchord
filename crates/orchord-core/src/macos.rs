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

    /// The Mac logic-board identifier Apple's recovery server is asked with; it decides
    /// which version's installer the server hands out.
    pub fn board_id(self) -> &'static str {
        match self {
            MacosVersion::BigSur => "Mac-2BD1B31983FE1663",
            MacosVersion::Monterey => "Mac-E43C1C25D4880AD6",
            MacosVersion::Sequoia => "Mac-7BA5B2D9E42DDD94",
        }
    }

    /// The name the installer gives the system it installs, as shown on its screens.
    pub fn short_name(self) -> &'static str {
        match self {
            MacosVersion::BigSur => "Big Sur",
            MacosVersion::Monterey => "Monterey",
            MacosVersion::Sequoia => "Sequoia",
        }
    }

    /// Whether VoiceOver can speak inside this version's installer (Recovery).
    /// Monterey's Recovery image cannot start VoiceOver at all.
    pub fn installer_speaks(self) -> bool {
        !matches!(self, MacosVersion::Monterey)
    }
}
