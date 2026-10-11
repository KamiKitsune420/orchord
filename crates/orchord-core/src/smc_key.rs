//! Getting the Apple SMC key without carrying it.
//!
//! macOS will not start unless QEMU's Apple SMC device presents a 64-character key that is
//! the same on every Intel Mac. The key is Apple's text and is not part of Orchord. Other
//! projects that run macOS under QEMU publish it in their start-up scripts, so Orchord can,
//! when the user asks, download one of those scripts and take the key from it.
//!
//! Orchord does carry the key's SHA-256 fingerprint. A fingerprint cannot be turned back
//! into the key, but it tells the key from anything else: the key is found wherever and
//! however a script happens to write it, and a file that has been changed, or a key that
//! was mistyped by hand, is recognised as wrong.

use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

pub const KEY_LENGTH: usize = 64;
const FINGERPRINT: &str = "5c0caad0974db5850bfcff8f1ce4839216cac63eea42e33f0b332bbf26f9c443";
/// A start-up script is a few tens of kilobytes; anything much longer is not one.
const DOWNLOAD_LIMIT: u64 = 4 << 20;

/// A public file that holds the key.
#[derive(Debug)]
pub struct Source {
    pub project: &'static str,
    pub url: &'static str,
}

/// Tried in this order; the first that still holds the key is used.
pub const SOURCES: [Source; 2] = [
    Source { project: "OSX-KVM", url: "https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore-Boot.sh" },
    Source { project: "quickemu", url: "https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu" },
];

/// Whether `key` is the Apple SMC key.
pub fn is_genuine(key: &str) -> bool {
    fingerprint(key.as_bytes()) == FINGERPRINT
}

/// Downloads the sources in turn and returns the key with the source it came from.
pub fn fetch() -> Result<(String, &'static Source)> {
    let mut failures = Vec::new();
    for source in &SOURCES {
        match fetch_from(source) {
            Ok(key) => return Ok((key, source)),
            Err(error) => failures.push(format!("{}: {error:#}", source.project)),
        }
    }
    bail!("The key could not be fetched. {}.", failures.join("; "))
}

/// Downloads one source and takes the key from it.
pub fn fetch_from(source: &Source) -> Result<String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent("orchord").build();
    let mut file = Vec::new();
    agent.get(source.url).call().context("its file could not be downloaded")?.into_reader().take(DOWNLOAD_LIMIT).read_to_end(&mut file).context("its file could not be read")?;
    find(&file, FINGERPRINT).context("its file no longer holds the key")
}

/// The run of `KEY_LENGTH` characters in `file` whose fingerprint is `wanted`. Some scripts
/// store the key with every letter moved thirteen places along the alphabet (ROT13), so each
/// run is tried both ways.
fn find(file: &[u8], wanted: &str) -> Option<String> {
    file.windows(KEY_LENGTH).filter(|run| run.iter().all(u8::is_ascii_graphic)).find_map(|run| {
        let turned: Vec<u8> = run.iter().map(|byte| rot13(*byte)).collect();
        [run.to_vec(), turned].into_iter().find(|candidate| fingerprint(candidate) == wanted).and_then(|key| String::from_utf8(key).ok())
    })
}

fn rot13(byte: u8) -> u8 {
    match byte {
        b'a'..=b'z' => (byte - b'a' + 13) % 26 + b'a',
        b'A'..=b'Z' => (byte - b'A' + 13) % 26 + b'A',
        other => other,
    }
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{KEY_LENGTH, find, fingerprint, is_genuine, rot13};

    /// A stand-in of the right length; the real key appears nowhere in this project.
    fn made_up_key() -> String {
        "NotTheRealKey(c)".repeat(4)
    }

    #[test]
    fn a_key_is_found_wherever_a_script_writes_it() {
        let key = made_up_key();
        assert_eq!(key.len(), KEY_LENGTH);
        let wanted = fingerprint(key.as_bytes());
        let script = format!("#!/bin/sh\nqemu -device isa-applesmc,osk=\"{key}\" -m 4096\n");
        assert_eq!(find(script.as_bytes(), &wanted), Some(key.clone()));
        assert_eq!(find(b"#!/bin/sh\nqemu -m 4096\n", &wanted), None);
        let one_letter_off = script.replace("NotTheRealKey(c)\"", "NotTheRealKey(d)\"");
        assert_eq!(find(one_letter_off.as_bytes(), &wanted), None);
    }

    #[test]
    fn a_key_stored_with_its_letters_turned_is_found_too() {
        let key = made_up_key();
        let turned: String = key.bytes().map(|byte| rot13(byte) as char).collect();
        assert_ne!(turned, key);
        let script = format!("OSK=$(echo \"{turned}\" | tr 'A-Za-z' 'N-ZA-Mn-za-m')\n");
        assert_eq!(find(script.as_bytes(), &fingerprint(key.as_bytes())), Some(key));
    }

    #[test]
    fn a_made_up_key_is_not_taken_for_the_real_one() {
        assert!(!is_genuine(&made_up_key()));
        assert!(!is_genuine(""));
    }
}
