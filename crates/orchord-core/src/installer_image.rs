//! Works out what kind of file a macOS installer image is, from what is in it, and makes it
//! something a Mac can start from.
//!
//! Installer files come in three shapes, and the file's extension does not say which:
//!
//! * An Apple disk image (`BaseSystem.dmg` from Apple's servers). QEMU cannot start a Mac from
//!   one; `qemu-img` unpacks it into a plain disk image when the Mac is created.
//! * A plain disk image with a GPT partition table (the unpacked `.img`). Used as it is.
//! * A Mac volume with no GPT round it. That is what `hdiutil` writes when a full installer
//!   made with `createinstallmedia` is saved as `.iso` or `.cdr`: an old Apple partition map
//!   and one volume. The firmware cannot read that map, and OpenCore only starts volumes whose
//!   GPT partition type says they are a Mac's; left alone, such a file gives a black screen.
//!
//! The third kind is given a GPT without copying its many gigabytes: a small VMDK description
//! tells QEMU to present a new partition table, then the volume straight out of the installer
//! file, then the table's spare copy, as one disk. To the Mac it looks like the USB stick
//! `createinstallmedia` would have written.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const SECTOR: u64 = 512;
/// An Apple partition map this long is not a partition map.
const MAP_ENTRY_LIMIT: u64 = 64;

/// The new disk's first sectors: protective MBR, GPT header, 32 sectors of entries, padding.
const HEAD_SECTORS: u64 = 64;
/// The new disk's last sectors: the spare copy of the entries, then of the header.
const TAIL_SECTORS: u64 = 33;
const GPT_ENTRIES: u32 = 128;
const GPT_ENTRY_BYTES: u32 = 128;
const GPT_HEADER_BYTES: u32 = 92;
/// Identifiers of the made-up disk and its one partition. They are the same on every start,
/// so that what the firmware remembers about the installer disk stays true.
const DISK_ID: &[u8; 16] = b"OrchordInstaller";
const VOLUME_ID: &[u8; 16] = b"OrchordMacVolume";
const HEAD_FILE: &str = "installer-start.bin";
const TAIL_FILE: &str = "installer-end.bin";
const DESCRIPTION_FILE: &str = "installer.vmdk";
/// QEMU reads at most this many characters of a file name in a VMDK description.
const DESCRIPTION_NAME_LIMIT: usize = 511;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeKind {
    Hfs,
    Apfs,
}

impl VolumeKind {
    /// The GPT partition type, as its sixteen bytes are stored on disk.
    fn partition_type(self) -> [u8; 16] {
        let first = match self {
            VolumeKind::Hfs => [0x00, 0x53, 0x46, 0x48],
            VolumeKind::Apfs => [0xEF, 0x57, 0x34, 0x7C],
        };
        let mut kind = [0, 0, 0, 0, 0x00, 0x00, 0xAA, 0x11, 0xAA, 0x11, 0x00, 0x30, 0x65, 0x43, 0xEC, 0xAC];
        kind[..4].copy_from_slice(&first);
        kind
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// An Apple disk image, to be unpacked with `qemu-img` before use.
    Dmg,
    /// A disk image the Mac can start from as it is.
    Disk,
    /// A Mac volume with no GPT round it: where it starts in the file and how long it is, in bytes.
    Volume { offset: u64, size: u64, kind: VolumeKind },
}

/// What QEMU is given as the installer disk: a file, and the QEMU driver that reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallMedia {
    pub path: PathBuf,
    pub format: &'static str,
}

/// Reads the few places in `path` that tell the shapes apart. A file that is none of them is
/// refused with a message for the user: a Mac given such a file shows a black screen for ever.
pub fn inspect(path: &Path) -> Result<Layout> {
    let mut file = File::open(path).with_context(|| format!("The installer file {} cannot be opened", path.display()))?;
    let layout = layout_of(&mut file, path)?;
    // A Mac disc that holds only files, such as a disc of drivers, has the same shape as an
    // installer. What sets an installer apart is the mark saying which folder starts a Mac.
    if let Layout::Volume { offset, kind: VolumeKind::Hfs, .. } = layout {
        let header = read_at(&mut file, offset + 1024, 512)?;
        let modern = header.starts_with(b"H+") || header.starts_with(b"HX");
        if !modern || (big_endian(&header, 80) == 0 && big_endian(&header, 84) == 0) {
            bail!("The file {} holds a Mac disc, but not one a Mac can start from, so it is not a macOS installer.", path.display());
        }
    }
    Ok(layout)
}

fn layout_of(file: &mut File, path: &Path) -> Result<Layout> {
    let length = file.metadata()?.len();
    // Apple disk images end with a block that starts "koly".
    if length >= SECTOR && read_at(file, length - SECTOR, 4)? == b"koly" {
        return Ok(Layout::Dmg);
    }
    let start = read_at(file, 0, 0x8800.min(length) as usize)?;
    let has = |at: usize, signature: &[u8]| start.get(at..at + signature.len()) == Some(signature);
    if has(512, b"EFI PART") {
        return Ok(Layout::Disk);
    }
    // The map's entries are one block each, from block 1. Images made from hard-disk style media
    // use 512-byte blocks and images of optical discs 2048; the first block says which.
    let declared = start.get(2..4).filter(|_| has(0, b"ER")).map_or(0, |size| u16::from_be_bytes([size[0], size[1]]) as u64);
    for block in [declared, 512, 2048] {
        if block != 0 && has(block as usize, b"PM") {
            return mac_volume(file, block, length).with_context(|| format!("The installer file {} has no Mac volume Orchord can start from", path.display()));
        }
    }
    // A Mac volume with nothing round it at all.
    let whole = length / SECTOR * SECTOR;
    if has(1024, b"H+") || has(1024, b"HX") {
        return Ok(Layout::Volume { offset: 0, size: whole, kind: VolumeKind::Hfs });
    }
    if has(32, b"NXSB") {
        return Ok(Layout::Volume { offset: 0, size: whole, kind: VolumeKind::Apfs });
    }
    // A PC-style partition table; the firmware reads those.
    if has(510, &[0x55, 0xAA]) {
        return Ok(Layout::Disk);
    }
    if has(0x8001, b"CD001") {
        bail!("The file {} is a data disc image, not a macOS installer a Mac can start from. Installers that work are the .iso or .cdr made on a Mac with Apple's createinstallmedia, or BaseSystem.dmg.", path.display());
    }
    bail!("The file {} is not a macOS installer image Orchord recognises.", path.display());
}

/// The first Mac volume listed in the Apple partition map whose blocks are `block` bytes long.
fn mac_volume(file: &mut File, block: u64, length: u64) -> Result<Layout> {
    let entries = (big_endian(&read_at(file, block, 16)?, 4) as u64).min(MAP_ENTRY_LIMIT);
    for index in 1..=entries {
        let entry = read_at(file, block * index, 80)?;
        if entry.len() < 80 || &entry[..2] != b"PM" {
            break;
        }
        let kind = match String::from_utf8_lossy(&entry[48..80]).trim_end_matches('\0') {
            "Apple_HFS" | "Apple_HFSX" => VolumeKind::Hfs,
            "Apple_APFS" => VolumeKind::Apfs,
            _ => continue,
        };
        let offset = big_endian(&entry, 8) as u64 * block;
        // QEMU counts in whole sectors, and an image cut short must not promise more than it has.
        let size = (big_endian(&entry, 12) as u64 * block).min(length.saturating_sub(offset)) / SECTOR * SECTOR;
        if size == 0 || offset % SECTOR != 0 {
            bail!("its Mac volume lies beyond the end of the file; the file may be incomplete");
        }
        return Ok(Layout::Volume { offset, size, kind });
    }
    bail!("its partition map lists no Mac volume");
}

/// Turns the installer file `image` into something QEMU can attach for a Mac to start from.
/// The few small files a bare Mac volume needs are written into `folder`.
pub fn prepare(image: &Path, folder: &Path) -> Result<InstallMedia> {
    let (offset, size, kind) = match inspect(image)? {
        Layout::Dmg => bail!("The installer file {} is an Apple disk image that has not been unpacked. Create the Mac again in the wizard, which unpacks it.", image.display()),
        Layout::Disk => return Ok(InstallMedia { path: image.to_path_buf(), format: "raw" }),
        Layout::Volume { offset, size, kind } => (offset, size, kind),
    };
    // QEMU reads the description as plain bytes and Windows then reads those bytes in the
    // PC's own character set, so only plain characters are sure to name the same file.
    let name = std::path::absolute(image)?.to_string_lossy().into_owned();
    if !name.is_ascii() || name.contains('"') || name.len() > DESCRIPTION_NAME_LIMIT {
        bail!("The installer file {name} has a name QEMU cannot use for this kind of installer. Move or rename it so that its folder and name use only English letters, digits, spaces and common punctuation, and are not extremely long.");
    }
    let sectors = size / SECTOR;
    let (head, tail) = partition_table(sectors, kind);
    std::fs::create_dir_all(folder)?;
    std::fs::write(folder.join(HEAD_FILE), head).with_context(|| format!("writing {HEAD_FILE}"))?;
    std::fs::write(folder.join(TAIL_FILE), tail).with_context(|| format!("writing {TAIL_FILE}"))?;
    // No empty line may come before a line that names a piece: QEMU reads through the empty
    // line into the next one, then reads that one again, and the disk gets the piece twice.
    let description = format!(
        "# Disk DescriptorFile\nversion=1\nCID=fffffffe\nparentCID=ffffffff\ncreateType=\"twoGbMaxExtentFlat\"\n# Extent description\nRW {HEAD_SECTORS} FLAT \"{HEAD_FILE}\" 0\nRW {sectors} FLAT \"{name}\" {}\nRW {TAIL_SECTORS} FLAT \"{TAIL_FILE}\" 0\n# The Disk Data Base\nddb.adapterType = \"ide\"\n",
        offset / SECTOR
    );
    let path = folder.join(DESCRIPTION_FILE);
    std::fs::write(&path, description).with_context(|| format!("writing {DESCRIPTION_FILE}"))?;
    Ok(InstallMedia { path, format: "vmdk" })
}

/// The first and last sectors of a GPT disk whose one partition is a Mac volume of
/// `volume_sectors` sectors, starting right after the first sectors.
fn partition_table(volume_sectors: u64, kind: VolumeKind) -> (Vec<u8>, Vec<u8>) {
    let total = HEAD_SECTORS + volume_sectors + TAIL_SECTORS;
    let mut entries = vec![0u8; (GPT_ENTRIES * GPT_ENTRY_BYTES) as usize];
    entries[..16].copy_from_slice(&kind.partition_type());
    entries[16..32].copy_from_slice(VOLUME_ID);
    entries[32..40].copy_from_slice(&HEAD_SECTORS.to_le_bytes());
    entries[40..48].copy_from_slice(&(HEAD_SECTORS + volume_sectors - 1).to_le_bytes());
    for (index, unit) in "Install macOS".encode_utf16().enumerate() {
        entries[56 + index * 2..58 + index * 2].copy_from_slice(&unit.to_le_bytes());
    }
    let entry_sectors = entries.len() as u64 / SECTOR;
    // A header names its own sector, its twin's, and where its copy of the entries starts.
    let header = |own: u64, twin: u64, entries_at: u64| {
        let mut header = vec![0u8; SECTOR as usize];
        header[..8].copy_from_slice(b"EFI PART");
        header[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        header[12..16].copy_from_slice(&GPT_HEADER_BYTES.to_le_bytes());
        header[24..32].copy_from_slice(&own.to_le_bytes());
        header[32..40].copy_from_slice(&twin.to_le_bytes());
        header[40..48].copy_from_slice(&(2 + entry_sectors).to_le_bytes());
        header[48..56].copy_from_slice(&(total - TAIL_SECTORS - 1).to_le_bytes());
        header[56..72].copy_from_slice(DISK_ID);
        header[72..80].copy_from_slice(&entries_at.to_le_bytes());
        header[80..84].copy_from_slice(&GPT_ENTRIES.to_le_bytes());
        header[84..88].copy_from_slice(&GPT_ENTRY_BYTES.to_le_bytes());
        header[88..92].copy_from_slice(&crc32(&entries).to_le_bytes());
        // The header's own checksum is worked out with its field still zero.
        let checksum = crc32(&header[..GPT_HEADER_BYTES as usize]);
        header[16..20].copy_from_slice(&checksum.to_le_bytes());
        header
    };

    let mut head = vec![0u8; (HEAD_SECTORS * SECTOR) as usize];
    // The protective MBR: one partition of type EE covering the disk, for software that predates GPT.
    head[446..454].copy_from_slice(&[0x00, 0x00, 0x02, 0x00, 0xEE, 0xFF, 0xFF, 0xFF]);
    head[454..458].copy_from_slice(&1u32.to_le_bytes());
    head[458..462].copy_from_slice(&u32::try_from(total - 1).unwrap_or(u32::MAX).to_le_bytes());
    head[510..512].copy_from_slice(&[0x55, 0xAA]);
    head[512..1024].copy_from_slice(&header(1, total - 1, 2));
    head[1024..1024 + entries.len()].copy_from_slice(&entries);

    let mut tail = entries.clone();
    tail.extend(header(total - 1, 1, total - TAIL_SECTORS));
    (head, tail)
}

/// The checksum GPT uses (CRC-32 as in zip files).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn big_endian(bytes: &[u8], at: usize) -> u32 {
    bytes.get(at..at + 4).map_or(0, |four| u32::from_be_bytes([four[0], four[1], four[2], four[3]]))
}

/// Up to `count` bytes from `position`; fewer if the file ends first.
fn read_at(file: &mut File, position: u64, count: usize) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(position))?;
    let mut bytes = Vec::with_capacity(count);
    file.take(count as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("orchord-image-test-{}-{name}", std::process::id()))
    }

    /// A file of `length` zero bytes with `pieces` (position, bytes) written into it.
    fn image(name: &str, length: usize, pieces: &[(usize, &[u8])]) -> PathBuf {
        let mut bytes = vec![0u8; length];
        for (position, piece) in pieces {
            bytes[*position..*position + piece.len()].copy_from_slice(piece);
        }
        let path = scratch(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn inspect_image(name: &str, length: usize, pieces: &[(usize, &[u8])]) -> Result<Layout> {
        let path = image(name, length, pieces);
        let layout = inspect(&path);
        let _ = std::fs::remove_file(path);
        layout
    }

    /// One entry of an Apple partition map: its type, first block and number of blocks.
    fn map_entry(kind: &str, first: u32, blocks: u32) -> Vec<u8> {
        let mut entry = vec![0u8; 80];
        entry[..2].copy_from_slice(b"PM");
        entry[4..8].copy_from_slice(&3u32.to_be_bytes());
        entry[8..12].copy_from_slice(&first.to_be_bytes());
        entry[12..16].copy_from_slice(&blocks.to_be_bytes());
        entry[48..48 + kind.len()].copy_from_slice(kind.as_bytes());
        entry
    }

    /// The start of an HFS+ volume header, and the mark that names its start-up folder.
    const HFS: &[u8] = b"H+";
    const BLESSED: &[u8] = &[0, 0, 4, 55];

    fn little_endian(bytes: &[u8], at: usize) -> u64 {
        u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
    }

    #[test]
    fn an_iso_made_by_hdiutil_gives_up_its_mac_volume() {
        let (map, free, volume) = (map_entry("Apple_partition_map", 1, 63), map_entry("Apple_Free", 64, 16), map_entry("Apple_HFS", 80, 40));
        let header = 80 * 512 + 1024;
        let pieces: [(usize, &[u8]); 6] = [(0, b"ER\x02\x00"), (512, &map), (1024, &free), (1536, &volume), (header, HFS), (header + 80, BLESSED)];
        assert_eq!(inspect_image("apm", 120 * 512, &pieces).unwrap(), Layout::Volume { offset: 80 * 512, size: 40 * 512, kind: VolumeKind::Hfs });
        // The same disc without the mark that names a start-up folder is a disc of files.
        let files_only = inspect_image("apm-files", 120 * 512, &pieces[..5]).unwrap_err();
        assert!(format!("{files_only:#}").contains("not one a Mac can start from"));
    }

    #[test]
    fn a_volume_running_past_the_end_of_the_file_is_cut_to_whole_sectors() {
        let (map, volume) = (map_entry("Apple_partition_map", 1, 63), map_entry("Apple_APFS", 4, 1000));
        let layout = inspect_image("short", 4 * 2048 + 1000, &[(0, b"ER\x08\x00"), (2048, &map), (4096, &volume)]).unwrap();
        assert_eq!(layout, Layout::Volume { offset: 4 * 2048, size: 512, kind: VolumeKind::Apfs });
    }

    #[test]
    fn an_apple_disk_image_is_known_by_its_last_block() {
        assert_eq!(inspect_image("dmg", 4096, &[(4096 - 512, b"koly")]).unwrap(), Layout::Dmg);
    }

    #[test]
    fn a_disk_with_a_gpt_is_used_as_it_is_and_a_bare_volume_is_not() {
        assert_eq!(inspect_image("gpt", 4096, &[(512, b"EFI PART")]).unwrap(), Layout::Disk);
        assert_eq!(inspect_image("hfs", 4096, &[(1024, HFS), (1104, BLESSED)]).unwrap(), Layout::Volume { offset: 0, size: 4096, kind: VolumeKind::Hfs });
    }

    #[test]
    fn files_no_mac_can_start_from_are_refused() {
        let data_disc = inspect_image("iso9660", 0x8800, &[(0x8001, b"CD001")]).unwrap_err();
        assert!(format!("{data_disc:#}").contains("data disc image"));
        assert!(inspect_image("blank", 4096, &[]).is_err());
        let map = map_entry("Apple_partition_map", 1, 63);
        assert!(inspect_image("empty-map", 4096, &[(0, b"ER\x02\x00"), (512, &map)]).is_err());
        assert!(inspect(&scratch("no-such-image.iso")).is_err());
    }

    #[test]
    fn the_checksum_is_the_standard_one() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn the_partition_table_has_one_mac_partition_and_two_headers_that_check_out() {
        let (head, tail) = partition_table(1000, VolumeKind::Hfs);
        assert_eq!((head.len(), tail.len()), (64 * 512, 33 * 512));
        let total = 64 + 1000 + 33;
        let (first, last) = (&head[512..1024], &tail[32 * 512..]);
        for (header, own, twin, entries_at) in [(first, 1, total - 1, 2), (last, total - 1, 1, total - 33)] {
            assert_eq!(&header[..8], b"EFI PART");
            assert_eq!((little_endian(header, 24), little_endian(header, 32), little_endian(header, 72)), (own, twin, entries_at));
            let mut unsigned = header[..92].to_vec();
            unsigned[16..20].fill(0);
            assert_eq!(crc32(&unsigned).to_le_bytes(), header[16..20]);
            assert_eq!(crc32(&head[1024..1024 + 128 * 128]).to_le_bytes(), header[88..92]);
        }
        assert_eq!(head[1024..1024 + 128 * 128], tail[..128 * 128]);
        // The partition is the volume: sectors 64 to 1063, inside what the header calls usable.
        assert_eq!((little_endian(&head, 1024 + 32), little_endian(&head, 1024 + 40)), (64, 1063));
        assert!(little_endian(first, 40) <= 64 && little_endian(first, 48) >= 1063);
        assert_eq!(&head[1024..1028], &[0x00, 0x53, 0x46, 0x48]);
    }

    #[test]
    fn a_bare_volume_becomes_a_description_of_a_gpt_disk() {
        let folder = scratch("prepare");
        let source = image("bare.iso", 8192, &[(1024, HFS), (1104, BLESSED)]);
        let media = prepare(&source, &folder).unwrap();
        assert_eq!((media.path.clone(), media.format), (folder.join("installer.vmdk"), "vmdk"));
        let description = std::fs::read_to_string(&media.path).unwrap();
        let pieces = format!("\nRW 64 FLAT \"installer-start.bin\" 0\nRW 16 FLAT \"{}\" 0\nRW 33 FLAT \"installer-end.bin\" 0\n", source.display());
        assert!(description.contains(&pieces), "{description}");
        assert!(!description.contains("\n\n"), "an empty line makes QEMU read the next piece twice");
        assert_eq!(std::fs::metadata(folder.join("installer-start.bin")).unwrap().len(), 64 * 512);

        // A disk that already has a GPT is attached as it is, and nothing is written for it.
        let disk = image("gpt.img", 4096, &[(512, b"EFI PART")]);
        assert_eq!(prepare(&disk, &folder).unwrap(), InstallMedia { path: disk.clone(), format: "raw" });
        for path in [source, disk] {
            let _ = std::fs::remove_file(path);
        }
        let _ = std::fs::remove_dir_all(folder);
    }
}
