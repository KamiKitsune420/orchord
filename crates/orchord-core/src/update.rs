//! Finds out whether a newer Orchord has been published, and installs it.
//!
//! Releases live on GitHub. The newest one is asked for with one plain request that says
//! nothing about the user or the PC. Installing downloads the release's zip, checks it against
//! the checksum GitHub publishes for it, unpacks it, and puts its files in place of the
//! running ones. Windows will not overwrite a program that is running but lets it be renamed,
//! so each old file is renamed out of the way first and removed when Orchord next starts.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const REPOSITORY: &str = "KamiKitsune420/orchord";
const PROGRAM: &str = "orchord.exe";
/// What is added to a replaced file's name until Orchord next starts. It is Orchord's own,
/// so that clearing such files away can never remove a file of the user's.
const OLD_SUFFIX: &str = ".orchord-old";
/// For trying the updater out: a version to believe this program is, in place of its own.
const PRETEND_VERSION: &str = "ORCHORD_PRETEND_VERSION";

/// A published version of Orchord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    /// What the release page says about it.
    pub notes: String,
    pub page: String,
    pub zip_url: String,
    pub zip_name: String,
    pub zip_bytes: u64,
    /// The zip's SHA-256 as GitHub reports it, when it does.
    pub zip_sha256: Option<String>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

/// The version of the running program.
pub fn current_version() -> String {
    std::env::var(PRETEND_VERSION).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

/// The newest release, if it is newer than the running program.
pub fn newer_release() -> Result<Option<Release>> {
    let release = latest_release()?;
    Ok(is_newer(&release.version, &current_version()).then_some(release))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(60)).user_agent("orchord").build()
}

pub fn latest_release() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases/latest");
    let answer = agent().get(&url).set("Accept", "application/vnd.github+json").call().context("GitHub could not be reached")?.into_string().context("GitHub's answer could not be read")?;
    parse_release(&answer)
}

fn parse_release(answer: &str) -> Result<Release> {
    let release: ApiRelease = serde_json::from_str(answer).context("GitHub's answer was not understood")?;
    let zip = release.assets.iter().find(|asset| asset.name.to_lowercase().ends_with("windows-x64.zip")).context("the newest release has no download for Windows")?;
    Ok(Release {
        version: release.tag_name.trim_start_matches('v').to_string(),
        notes: release.body.unwrap_or_default(),
        page: release.html_url,
        zip_url: zip.browser_download_url.clone(),
        zip_name: zip.name.clone(),
        zip_bytes: zip.size,
        zip_sha256: zip.digest.as_deref().and_then(|digest| digest.strip_prefix("sha256:")).map(str::to_lowercase),
    })
}

/// Whether version `candidate` comes after `current`. Versions are numbers separated by full
/// stops; a part that is not a number counts as zero, and missing parts as zero.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let parts = |version: &str| -> Vec<u64> { version.trim_start_matches('v').split('.').map(|part| part.trim().parse().unwrap_or(0)).collect() };
    let (candidate, current) = (parts(candidate), parts(current));
    let length = candidate.len().max(current.len());
    let padded = |version: &[u64]| -> Vec<u64> { (0..length).map(|index| version.get(index).copied().unwrap_or(0)).collect() };
    padded(&candidate) > padded(&current)
}

/// The folder the running program is in, which is what an update replaces.
pub fn program_folder() -> Result<PathBuf> {
    let program = std::env::current_exe().context("the running program could not be located")?;
    Ok(program.parent().context("the running program has no folder")?.to_path_buf())
}

/// Downloads `release` and installs it over the program in `folder`. `progress` is told how
/// many bytes have arrived. Returns the new program file, for starting it. Nothing in
/// `folder` is changed until the download has been checked and unpacked.
pub fn install(release: &Release, folder: &Path, progress: &mut dyn FnMut(u64), cancel: &AtomicBool) -> Result<PathBuf> {
    let work = std::env::temp_dir().join(format!("orchord-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let result = install_from(release, folder, &work, progress, cancel);
    let _ = std::fs::remove_dir_all(&work);
    result
}

fn install_from(release: &Release, folder: &Path, work: &Path, progress: &mut dyn FnMut(u64), cancel: &AtomicBool) -> Result<PathBuf> {
    let zip = work.join("update.zip");
    let found = download(&release.zip_url, &zip, progress, cancel)?;
    if let Some(wanted) = &release.zip_sha256 {
        if &found != wanted {
            bail!("The download does not match the checksum GitHub gives for it, so it was not installed.");
        }
    }
    let unpacked = work.join("unpacked");
    std::fs::create_dir_all(&unpacked)?;
    unpack(&zip, &unpacked)?;
    let new_files = folder_holding_program(&unpacked).context("The download does not hold Orchord's program file, so it was not installed.")?;
    replace_files(&new_files, folder)?;
    Ok(folder.join(PROGRAM))
}

/// Saves `url` as `target` and returns its SHA-256 in lower-case hexadecimal.
fn download(url: &str, target: &Path, progress: &mut dyn FnMut(u64), cancel: &AtomicBool) -> Result<String> {
    let mut reader = agent().get(url).call().context("the download could not be started")?.into_reader();
    let mut file = std::fs::File::create(target)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut received = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("The update was cancelled.");
        }
        let count = reader.read(&mut buffer).context("the download broke off")?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
        received += count as u64;
        progress(received);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Unpacks a zip with the `tar` that has come with Windows since 2018, which reads zips.
fn unpack(zip: &Path, into: &Path) -> Result<()> {
    let tar = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into())).join("System32").join("tar.exe");
    let mut command = Command::new(tar);
    command.arg("-xf").arg(zip).arg("-C").arg(into).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().context("Windows' tar could not be started to unpack the download")?;
    if !output.status.success() {
        bail!("The download could not be unpacked: {}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// The zip holds one folder named after the version; the program is in it.
fn folder_holding_program(unpacked: &Path) -> Option<PathBuf> {
    if unpacked.join(PROGRAM).is_file() {
        return Some(unpacked.to_path_buf());
    }
    std::fs::read_dir(unpacked).ok()?.flatten().map(|entry| entry.path()).find(|path| path.join(PROGRAM).is_file())
}

/// Copies every file under `new` to the same place under `folder`. A file already there is
/// first renamed out of the way, which works even for the running program.
fn replace_files(new: &Path, folder: &Path) -> Result<()> {
    for entry in std::fs::read_dir(new)? {
        let entry = entry?;
        let target = folder.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            replace_files(&entry.path(), &target)?;
            continue;
        }
        if target.exists() {
            let old = old_name(&target);
            let _ = std::fs::remove_file(&old);
            std::fs::rename(&target, &old).with_context(|| format!("{} could not be moved out of the way", target.display()))?;
        }
        std::fs::copy(entry.path(), &target).with_context(|| format!("{} could not be written", target.display()))?;
    }
    Ok(())
}

fn old_name(file: &Path) -> PathBuf {
    let mut name = file.as_os_str().to_os_string();
    name.push(OLD_SUFFIX);
    PathBuf::from(name)
}

/// Removes what an earlier update moved out of the way in `folder`. Called when Orchord
/// starts, by which time the program those files belonged to has ended. Only files with the
/// ending an update gives them are removed, in the folder itself and in the folders an
/// update brings with it.
pub fn remove_leftovers(folder: &Path) {
    remove_old_files(folder, false);
    for name in ["opencore", "opencore-amd", "licences"] {
        remove_old_files(&folder.join(name), true);
    }
}

fn remove_old_files(folder: &Path, and_below: bool) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            if and_below {
                remove_old_files(&path, true);
            }
        } else if path.to_string_lossy().ends_with(OLD_SUFFIX) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_newer, old_name, parse_release, remove_leftovers, replace_files};
    use std::path::PathBuf;

    #[test]
    fn versions_are_compared_as_numbers() {
        assert!(is_newer("0.1.2", "0.1.1"));
        assert!(is_newer("v0.2", "0.1.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(!is_newer("0.1.1", "0.1.1"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.1"));
    }

    #[test]
    fn a_release_is_read_from_githubs_answer() {
        let answer = r#"{"tag_name": "v0.1.2", "html_url": "https://example.org/r", "body": "New things.", "assets": [
            {"name": "notes.txt", "browser_download_url": "https://example.org/notes.txt", "size": 5},
            {"name": "Orchord-0.1.2-windows-x64.zip", "browser_download_url": "https://example.org/o.zip", "size": 7000000, "digest": "sha256:ABCDEF"}]}"#;
        let release = parse_release(answer).unwrap();
        assert_eq!((release.version.as_str(), release.zip_url.as_str(), release.zip_bytes), ("0.1.2", "https://example.org/o.zip", 7000000));
        assert_eq!(release.zip_sha256.as_deref(), Some("abcdef"));
        assert!(parse_release(r#"{"tag_name": "v0.1.2", "html_url": "x", "assets": []}"#).is_err());
        assert!(parse_release("not json").is_err());
    }

    #[test]
    fn new_files_take_the_place_of_old_ones_which_are_then_cleared_away() {
        let work: PathBuf = std::env::temp_dir().join(format!("orchord-update-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let (new, folder) = (work.join("new"), work.join("installed"));
        for (root, text) in [(&new, "new"), (&folder, "old")] {
            std::fs::create_dir_all(root.join("opencore").join("EFI")).unwrap();
            std::fs::write(root.join("orchord.exe"), text).unwrap();
            std::fs::write(root.join("opencore").join("EFI").join("config.plist"), text).unwrap();
        }
        std::fs::write(new.join("added.txt"), "new").unwrap();
        std::fs::write(folder.join("mine.txt"), "kept").unwrap();

        replace_files(&new, &folder).unwrap();
        let read = |path: PathBuf| std::fs::read_to_string(path).unwrap();
        assert_eq!(read(folder.join("orchord.exe")), "new");
        assert_eq!(read(folder.join("opencore").join("EFI").join("config.plist")), "new");
        assert_eq!(read(folder.join("added.txt")), "new");
        assert_eq!(read(folder.join("mine.txt")), "kept");
        assert_eq!(read(old_name(&folder.join("orchord.exe"))), "old");

        remove_leftovers(&folder);
        assert!(!old_name(&folder.join("orchord.exe")).exists());
        assert!(!old_name(&folder.join("opencore").join("EFI").join("config.plist")).exists());
        assert!(folder.join("mine.txt").exists());
        let _ = std::fs::remove_dir_all(work);
    }
}
