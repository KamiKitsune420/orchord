//! Downloads a macOS installer ("recovery") image from Apple.
//!
//! Apple's Macs fetch this image themselves for Internet Recovery. The exchange is small:
//! ask `osrecovery.apple.com` for a session, post the Mac's board identifier, and receive the
//! address of a `BaseSystem.dmg` and of a "chunklist" holding the SHA-256 of each piece of it.
//! The protocol is the one documented by OpenCore's `macrecovery` tool.

use std::fs::File;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};

use crate::macos::MacosVersion;

const USER_AGENT: &str = "InternetRecovery/1.0";
const SERVER: &str = "https://osrecovery.apple.com";
/// A logic-board serial number of all zeros asks for the image any Mac of that model may use.
const ANY_MAC: &str = "00000000000000000";

/// Where a download has got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub received: u64,
    /// Unknown when Apple's server does not say.
    pub total: Option<u64>,
}

/// The addresses Apple answered with.
struct ImageInfo {
    image_url: String,
    image_token: String,
    chunklist_url: String,
    chunklist_token: String,
}

/// Downloads the installer image for `macos` to `image` and checks it against Apple's chunklist.
/// `progress` is called as data arrives; setting `cancel` stops the download with an error.
pub fn download(macos: MacosVersion, image: &Path, progress: &mut dyn FnMut(Progress), cancel: &AtomicBool) -> Result<()> {
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(30)).timeout_read(Duration::from_secs(60)).user_agent(USER_AGENT).build();
    let info = image_info(&agent, macos).context("asking Apple where the installer is")?;
    let chunklist = image.with_extension("chunklist");
    fetch(&agent, &info.chunklist_url, &info.chunklist_token, &chunklist, &mut |_| {}, cancel).context("downloading the installer's checksum list")?;
    fetch(&agent, &info.image_url, &info.image_token, image, progress, cancel).context("downloading the installer")?;
    let verified = verify(image, &chunklist);
    let _ = std::fs::remove_file(&chunklist);
    verified.context("checking the downloaded installer")
}

fn image_info(agent: &ureq::Agent, macos: MacosVersion) -> Result<ImageInfo> {
    let greeting = agent.get(&format!("{SERVER}/")).call()?;
    let session = greeting
        .all("set-cookie")
        .into_iter()
        .flat_map(|cookie| cookie.split("; "))
        .find(|part| part.starts_with("session="))
        .ok_or_else(|| anyhow!("Apple's server did not start a session"))?
        .to_string();
    let body = format!("cid={}\nsn={ANY_MAC}\nbid={}\nk={}\nfg={}\nos=default", random_hex(16), macos.board_id(), random_hex(64), random_hex(64));
    let answer = agent.post(&format!("{SERVER}/InstallationPayload/RecoveryImage")).set("Cookie", &session).set("Content-Type", "text/plain").send_string(&body)?.into_string()?;
    let field = |key: &str| {
        answer.lines().find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix(": "))).map(|value| value.trim().to_string()).ok_or_else(|| anyhow!("Apple's answer has no {key} entry"))
    };
    Ok(ImageInfo { image_url: field("AU")?, image_token: field("AT")?, chunklist_url: field("CU")?, chunklist_token: field("CT")? })
}

/// Downloads one file. Apple hands out plain `http` addresses; `https` is tried first.
fn fetch(agent: &ureq::Agent, url: &str, token: &str, target: &Path, progress: &mut dyn FnMut(Progress), cancel: &AtomicBool) -> Result<()> {
    let cookie = format!("AssetToken={token}");
    let secure = url.replacen("http://", "https://", 1);
    let response = match agent.get(&secure).set("Cookie", &cookie).call() {
        Ok(response) => response,
        Err(_) if secure != url => agent.get(url).set("Cookie", &cookie).call()?,
        Err(error) => return Err(error.into()),
    };
    let total = response.header("content-length").and_then(|length| length.parse().ok());
    let mut reader = response.into_reader();
    let mut file = File::create(target).with_context(|| format!("creating {}", target.display()))?;
    let mut buffer = vec![0u8; 1 << 16];
    let mut received = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(target);
            bail!("The download was cancelled.");
        }
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        received += count as u64;
        progress(Progress { received, total });
    }
    if total.is_some_and(|total| total != received) {
        bail!("The download stopped early: {received} of {} bytes arrived.", total.unwrap_or(0));
    }
    Ok(())
}

/// Checks `image` against a chunklist: a 36-byte header, then for each chunk its length and SHA-256.
fn verify(image: &Path, chunklist: &Path) -> Result<()> {
    let list = std::fs::read(chunklist)?;
    let chunks = parse_chunklist(&list)?;
    let mut file = File::open(image)?;
    let mut buffer = Vec::new();
    for (index, (size, hash)) in chunks.iter().enumerate() {
        buffer.resize(*size as usize, 0);
        file.read_exact(&mut buffer).with_context(|| format!("the installer is shorter than its checksum list says (chunk {index})"))?;
        if Sha256::digest(&buffer).as_slice() != hash {
            bail!("The installer is damaged: chunk {index} does not match Apple's checksum.");
        }
    }
    if file.read(&mut [0u8; 1])? != 0 {
        bail!("The installer is longer than its checksum list says.");
    }
    Ok(())
}

fn parse_chunklist(list: &[u8]) -> Result<Vec<(u32, [u8; 32])>> {
    let number = |offset: usize, length: usize| -> Result<u64> {
        let bytes = list.get(offset..offset + length).ok_or_else(|| anyhow!("the checksum list is cut short"))?;
        Ok(bytes.iter().rev().fold(0u64, |value, byte| value << 8 | u64::from(*byte)))
    };
    if list.get(..4) != Some(b"CNKL") {
        bail!("the checksum list does not start with CNKL");
    }
    let (count, first) = (number(12, 8)? as usize, number(20, 8)? as usize);
    (0..count)
        .map(|index| {
            let offset = first + index * 36;
            let hash = list.get(offset + 4..offset + 36).ok_or_else(|| anyhow!("the checksum list is cut short"))?;
            Ok((number(offset, 4)? as u32, hash.try_into().expect("32 bytes were taken")))
        })
        .collect()
}

/// Random upper-case hexadecimal digits. Apple's server only needs them to look like a Mac's
/// identifiers; they protect nothing, so the standard library's hash seed is random enough.
pub(crate) fn random_hex(length: usize) -> String {
    let mut text = String::new();
    while text.len() < length {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_usize(text.len());
        text.push_str(&format!("{:016X}", hasher.finish()));
    }
    text.truncate(length);
    text
}

#[cfg(test)]
mod tests {
    use super::{parse_chunklist, random_hex, verify};
    use sha2::{Digest, Sha256};

    fn chunklist_for(chunks: &[&[u8]]) -> Vec<u8> {
        let mut list = b"CNKL".to_vec();
        list.extend_from_slice(&36u32.to_le_bytes());
        list.extend_from_slice(&[1, 1, 2, 0]);
        list.extend_from_slice(&(chunks.len() as u64).to_le_bytes());
        list.extend_from_slice(&36u64.to_le_bytes());
        list.extend_from_slice(&0u64.to_le_bytes());
        for chunk in chunks {
            list.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            list.extend_from_slice(&Sha256::digest(chunk));
        }
        list
    }

    #[test]
    fn a_chunklist_gives_each_chunk_its_size_and_hash() {
        let chunks = parse_chunklist(&chunklist_for(&[b"abc", b"de"])).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].0, 2);
        assert_eq!(chunks[0].1.as_slice(), Sha256::digest(b"abc").as_slice());
    }

    #[test]
    fn a_damaged_image_fails_the_check_and_a_whole_one_passes() {
        let folder = std::env::temp_dir().join(format!("orchord-recovery-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let (image, list) = (folder.join("image.dmg"), folder.join("image.chunklist"));
        std::fs::write(&list, chunklist_for(&[b"abc", b"de"])).unwrap();
        std::fs::write(&image, b"abcde").unwrap();
        assert!(verify(&image, &list).is_ok());
        std::fs::write(&image, b"abcdX").unwrap();
        assert!(verify(&image, &list).is_err());
        std::fs::write(&image, b"abcdef").unwrap();
        assert!(verify(&image, &list).is_err());
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn random_hex_has_the_length_asked_for() {
        let text = random_hex(64);
        assert_eq!(text.len(), 64);
        assert!(text.chars().all(|character| character.is_ascii_hexdigit() && !character.is_ascii_lowercase()));
        assert_ne!(text, random_hex(64));
    }
}
