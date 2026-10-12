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
    Sonoma,
    Sequoia,
}

impl MacosVersion {
    pub const ALL: [MacosVersion; 4] = [MacosVersion::BigSur, MacosVersion::Monterey, MacosVersion::Sonoma, MacosVersion::Sequoia];

    pub fn display_name(self) -> &'static str {
        match self {
            MacosVersion::BigSur => "macOS Big Sur 11",
            MacosVersion::Monterey => "macOS Monterey 12",
            MacosVersion::Sonoma => "macOS Sonoma 14",
            MacosVersion::Sequoia => "macOS Sequoia 15",
        }
    }

    /// QEMU CPU model. Sonoma and Sequoia need AVX2, which Penryn lacks; Haswell-noTSX and `host`
    /// do not boot under WHPX, so Skylake-Client is the only model with AVX2 that works.
    pub fn cpu_model(self) -> &'static str {
        match self {
            MacosVersion::BigSur | MacosVersion::Monterey => "Penryn",
            MacosVersion::Sonoma | MacosVersion::Sequoia => "Skylake-Client,-hle,-rtm",
        }
    }

    pub fn default_memory_mb(self) -> u32 {
        match self {
            MacosVersion::BigSur | MacosVersion::Monterey => 4096,
            MacosVersion::Sonoma | MacosVersion::Sequoia => 5120,
        }
    }

    pub fn default_disk_gb(self) -> u32 {
        match self {
            MacosVersion::BigSur | MacosVersion::Monterey => 80,
            MacosVersion::Sonoma | MacosVersion::Sequoia => 96,
        }
    }

    /// The Mac logic-board identifier Apple's recovery server is asked with; it decides
    /// which version's installer the server hands out.
    pub fn board_id(self) -> &'static str {
        match self {
            MacosVersion::BigSur => "Mac-2BD1B31983FE1663",
            MacosVersion::Monterey => "Mac-E43C1C25D4880AD6",
            MacosVersion::Sonoma => "Mac-827FAC58A8FDFA22",
            MacosVersion::Sequoia => "Mac-7BA5B2D9E42DDD94",
        }
    }

    /// The name the installer gives the system it installs, as shown on its screens.
    pub fn short_name(self) -> &'static str {
        match self {
            MacosVersion::BigSur => "Big Sur",
            MacosVersion::Monterey => "Monterey",
            MacosVersion::Sonoma => "Sonoma",
            MacosVersion::Sequoia => "Sequoia",
        }
    }

    /// The version a file name such as `MacOSBigSur.iso` or `Install macOS Sequoia.cdr` mentions.
    pub fn named_in(file_name: &str) -> Option<MacosVersion> {
        let letters: String = file_name.chars().filter(|character| character.is_alphanumeric()).flat_map(char::to_lowercase).collect();
        MacosVersion::ALL.into_iter().find(|version| letters.contains(&version.short_name().replace(' ', "").to_lowercase()))
    }

    /// Whether VoiceOver can speak inside this version's installer (Recovery).
    /// Monterey's Recovery image cannot start VoiceOver at all.
    pub fn installer_speaks(self) -> bool {
        !matches!(self, MacosVersion::Monterey)
    }
}

#[cfg(test)]
mod tests {
    use super::MacosVersion;

    #[test]
    fn a_file_name_gives_away_its_version() {
        assert_eq!(MacosVersion::named_in("MacOSBigSur.iso"), Some(MacosVersion::BigSur));
        assert_eq!(MacosVersion::named_in("Install macOS Sequoia 15.1.cdr"), Some(MacosVersion::Sequoia));
        assert_eq!(MacosVersion::named_in("installer.iso"), None);
    }
}
