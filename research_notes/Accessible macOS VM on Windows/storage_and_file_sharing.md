# Storage and file sharing for an Intel macOS guest on a Windows 11 host (state as of October 2026)

Scope: how an Intel macOS VM (QEMU, possibly WHPX; or Hyper-V) on Windows 11 Pro can use host storage, whether "the Mac's SSD is a plain Windows folder" is possible, and what the best achievable design is for live two-way file exchange visible in File Explorer.

Reading guide: "Cited Findings" are things a source says. "Inferences" are my reasoning or background knowledge that I could not tie to a fetched source in this session; treat them as needing a test. Nothing here was tested on real hardware.

Bottom line up front:
- The literal idea (boot volume = host directory presented as HFS+/APFS) is not achievable with any existing component. No hypervisor synthesizes HFS+ or APFS from a directory; the only directory-to-block-device synthesizer in QEMU is `vvfat` (FAT only), and its own documentation forbids the exact use case (host writing to the folder while the guest has it mounted).
- The realistic design is a hybrid: system disk is a disk image file in the VM folder; a real Windows folder in the same VM folder is exported to the guest over a network file protocol (SMB preferred) and mounted automatically inside macOS. That delivers "drop a file in from Windows, it is in the Mac" without admin rights if the app embeds its own user-mode file server.
- virtio-9p, the mechanism every Linux-host macOS-VM project uses, is NOT available in upstream QEMU on Windows hosts (verified against QEMU master build files, October 2026).

## 1. Can a macOS boot/system volume be backed by a host directory instead of a block-device image?

### Takeaway
No. macOS boots only from a block device carrying a real APFS (or, for old releases, HFS+) on-disk structure, and from Big Sur onward it boots from a cryptographically sealed, Apple-signed APFS snapshot; nothing in QEMU, Hyper-V, VMware or VirtualBox manufactures HFS+/APFS from a directory tree. QEMU's one directory-backed block driver (`vvfat`) is FAT-only, read-write is "beta", and it explicitly must not be written from the host while the guest uses it.

### Cited Findings
- QEMU's directory-as-disk driver is `vvfat` (`-hdb fat:/my_directory`), which exposes a directory as a virtual FAT disk "without having to copy them in a disk image or to export them via SAMBA or NFS". It is read-only by default; read/write (`:rw:`) is described as "beta stage"/"for testing". — [QEMU block drivers documentation](https://www.qemu.org/docs/master/system/qemu-block-drivers.html)
- The same documentation lists things you must never do with vvfat: use non-ASCII filenames, use `-snapshot` with `:rw:`, expect it to work with loadvm, or "write to the FAT directory on the host system while accessing it with the guest system". — [QEMU block drivers documentation](https://www.qemu.org/docs/master/system/qemu-block-drivers.html)
- The QEMU block-driver documentation lists no HFS+ or APFS equivalent of vvfat; the formats listed are raw, qcow2, vhdx, vmdk, vpc (VHD), plus read-only bochs, cloop, dmg, parallels. — [QEMU block drivers documentation](https://www.qemu.org/docs/master/system/qemu-block-drivers.html)
- For Mojave and later macOS guests, the Quickemu project instructs users to format the VM disk as APFS and notes that "MacOS Extended" (HFS+) may not work. — [Quickemu wiki: Create macOS virtual machines](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- From Big Sur on, the system lives on a Signed/Sealed System Volume: the installer builds a Merkle tree of hashes covering "every file on that volume individually, and its file system metadata", stores the tree and its Seal in the metadata of a snapshot, and the top-level value is verified against a value signed by Apple. — [Eclectic Light: Is Big Sur's system volume sealed?](https://eclecticlight.co/2020/11/30/is-big-surs-system-volume-sealed/)
- The Mac does not boot from the System volume itself but mounts a read-only snapshot of it (named `com.apple.os-update-<UUID>`) as the root of the file system; the writable Data volume ("Macintosh HD - Data") is hidden under `/System/Volumes` and reached through firmlinks. — [Eclectic Light: Big Sur boot volume layout](https://eclecticlight.co/2021/01/13/big-sur-boot-volume-layout/)
- A user-made snapshot of the System volume is an unsealed replica and cannot be used to boot/roll back. — [Eclectic Light: Why you can't make a snapshot of Big Sur's System volume to roll back](https://eclecticlight.co/2021/06/11/why-you-cant-make-a-snapshot-of-big-surs-system-volume-to-roll-back/)
- Snapshots still form the System volume for all macOS versions since Big Sur (2026 explainer). — [Eclectic Light: Explainer: snapshots (2026)](https://eclecticlight.co/2026/01/31/explainer-snapshots-2/)
- Even the QEMU developers' attempt to map POSIX metadata onto NTFS needed NTFS Alternate Data Streams: the Windows 9pfs patch series implements the `mapped`/`mapped-xattr` security model with NTFS ADS and therefore requires the export path to be on an NTFS partition. — [qemu-devel: hw/9pfs: Add 9pfs support for Windows (v3 thread)](https://lore.kernel.org/all/2468168.SvRIHAoRfs@silver/T/)
- Open-source APFS implementations are reverse-engineered and read-only or explicitly unsafe for writing: apfs-fuse is "a read-only FUSE driver", lists "Writing" as unsupported and warns it "may return wrong data, or it may simply crash"; linux-apfs-rw's developer says that with write support you should "expect data corruption". — [sgan81/apfs-fuse](https://github.com/sgan81/apfs-fuse); [linux-apfs/linux-apfs-rw](https://github.com/linux-apfs/linux-apfs-rw)

### Inferences
- Why the idea fails in principle, not just for lack of a tool: a block device is a contract in which the guest's filesystem driver exclusively owns every block and caches metadata in guest RAM. A synthesizer that regenerates blocks when the host folder changes would change the disk "underneath" a mounted filesystem. The guest would either not notice (stale cache) or see corruption. This is exactly why vvfat forbids host writes during guest access, and it applies equally to any hypothetical "vvhfs"/"vvapfs". Live two-way sharing therefore needs a file-level protocol (SMB/NFS/9p/WebDAV/virtiofs), where the guest asks the host per-operation, not a block device.
- An APFS synthesizer would additionally have to emit: a container with a volume group (System + Data + Preboot + Recovery + VM), copy-on-write B-trees with checkpoints, snapshots, and for Big Sur+ an Apple-signed seal over the System snapshot. The seal cannot be produced by a third party; the System volume content must be the bit-exact content Apple signed, so it cannot be "whatever is in the folder". Booting an unsealed system requires disabling authenticated-root/SIP, which is a degraded, non-default configuration (background knowledge, not sourced here).
- A synthesized HFS+ volume would only be bootable by macOS releases that still accept an HFS+ root (10.12 and earlier; 10.13 on non-SSD) - background knowledge; the only fetched evidence is Quickemu's "MacOS Extended may not work" for Mojave+. Those releases are long out of security support and of little use to end users.
- What a plain NTFS folder cannot represent natively for a macOS system volume (background knowledge, listed for the report writer; each would need an emulation layer such as ADS or sidecar files): Unix owner/group/mode bits and setuid; symlinks (NTFS symlinks need privilege or Developer Mode to create); hard links to directories and the heavy hard-link use in system frameworks; device nodes/sockets/FIFOs; resource forks and extended attributes (`com.apple.quarantine`, `com.apple.provenance`, code-signing xattrs, Finder info); BSD flags (`restricted`, `uchg`, `hidden`), ACLs; case-sensitivity semantics and Unicode normalization differences; file names containing `: \ * ? " < > |`, names ending in a dot or space, and reserved names such as `CON`, `AUX`, `NUL`; the 260-character MAX_PATH limit that many Windows tools still enforce versus deep macOS bundle paths; APFS clones, sparse files and transparent compression; firmlinks; and code signatures that are invalidated if any of the above metadata is lost. Antivirus/indexer on the host touching hundreds of thousands of system files would be a further performance and integrity hazard.
- So the honest product message is: "the Mac's system disk is one file; your documents folder is a real folder."

### Gaps
- I found no project (GitHub, mailing lists) implementing a directory-backed HFS+ or APFS block device. Absence of evidence after targeted searching, not proof of non-existence; a research prototype could exist.
- Apple's own documentation for the APFS-only boot requirement by version (10.13 SSD / 10.14 all) was not fetched; the version boundaries above are from background knowledge.

## 2. Can macOS boot from a network / shared filesystem root (NFS root, NetBoot, 9p root, virtiofs root)?

### Takeaway
Not on any current macOS. NetBoot (NFS/HTTP-served disk images) is deprecated and unsupported on T2 and Apple-silicon Macs, and it booted a disk image over the network rather than a live shared directory anyway; there is no supported 9p-root or virtiofs-root for macOS.

### Cited Findings
- NetBoot is not supported on newer Macs with the T2 security chip or Apple silicon. — [DBpedia: NetBoot](https://dbpedia.org/page/NetBoot)
- Mac admins quote Apple's support article HT202770: "Mac computers that have the Apple T2 chip don't support starting up from network volumes"; other posters note Apple has deprecated NetBoot. — [Jamf Community thread](https://community.jamf.com/t5/jamf-pro/mac-computers-that-have-the-apple-t2-chip-don-t-support-starting/td-p/154913/highlight/true)
- In the macOS-on-QEMU projects surveyed, the 9p share is something mounted after boot, not a root filesystem: Gerd Hoffmann's testing reported the "9p share just needs to be mounted". — [The Passthrough Post: Mac OS adds early support for VirtIO (June 2019)](https://passthroughpo.st/mac-os-adds-early-support-for-virtio-qemu/)

### Inferences
- A QEMU VM emulating a pre-T2 Mac model could in theory NetBoot an old macOS (10.13 and earlier), but that still delivers a block image (NBI .dmg) plus a shadow file, not a browsable folder, so it does not serve the goal. Not worth pursuing.
- Sealed-snapshot boot (Big Sur+) is tied to APFS on a local block device, which rules out NFS/9p/virtiofs root on modern releases.

### Gaps
- No Apple release note found stating the exact macOS version in which NetBoot client/NFS-root support was removed from the kernel/boot loader for Intel.

## 3. Shared-folder mechanisms that work with a macOS guest

### 3a. virtio-9p

#### Takeaway
macOS has had an Apple-written virtio-9p client since a late Mojave (10.14.x) build and it is the standard sharing method on Linux hosts (`mount_9p`), but QEMU's 9p server still does not build for Windows hosts as of QEMU master in October 2026, so it is unavailable to orchord unless orchord ships a patched QEMU.

#### Cited Findings
- QEMU developer Gerd Hoffmann found (reported June 2019) that a later macOS Mojave release added Apple virtio drivers including virtio-blk and virtio-9p; "the virtio-blk driver only works in legacy mode, and the 9p share just needs to be mounted". — [The Passthrough Post](https://passthroughpo.st/mac-os-adds-early-support-for-virtio-qemu/)
- Quickemu's macOS share-setup code prints a `sudo mount_9p` command for the guest and requires the host's public share directory to be world-writable for macOS integration to work smoothly. — [Quickemu commit "Add support for virtio-9p"](https://git.binarymagic.net/AIR/quickemu/commit/01fca96f283c984d91fe8cabad891b6a40d205f3)
- Quickemu's current macOS wiki page states host-guest file sharing uses virtio-9p and SPICE webdavd. — [Quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines); an older Quickemu commit conversely says "File sharing via SPICE webdavd is not available on macOS" — [Quickemu commit e5053ec](https://git.binarymagic.net/AIR/quickemu/commit/e5053ec566e10311ecf067f9d6a2df9563e850b3) (conflict; the wiki is newer).
- `-virtfs` is shorthand for `-fsdev ... -device virtio-9p-pci`; security models are passthrough, mapped-xattr, mapped-file, none; passthrough requires running QEMU as root. — [QEMU invocation documentation](https://www.qemu.org/docs/master/system/invocation.html)
- Windows-host 9pfs was proposed by Bin Meng / Guohuai Shi (Wind River): RFC April 2022 ('local' backend only, security_model 'none'), then v1-v5 series October 2022 to February 2023 adding `9p-util-win32.c`, the mapped model via NTFS ADS, and a final patch enabling virtfs in meson for Windows. Example from the series: `-fsdev local,path=c:\msys64,security_model=mapped,id=p9 -device virtio-9p-pci,fsdev=p9,mount_tag=p9fs`. — [RFC thread](https://lore.kernel.org/all/2766319.qo5tFDDxnU@silver/T/); [v3 thread](https://lore.kernel.org/all/2468168.SvRIHAoRfs@silver/T/); [v4 thread](https://lore.kernel.org/all/7414919.cCnjH5He9x@silver/T/)
- In March 2023 the v5 series was still receiving "Ping?" messages awaiting review. — [qemu-devel March 2023](https://mail.gnu.org/archive/html/qemu-devel/2023-03/msg01744.html)
- QEMU tracks the work in GitLab issue #974 "Enable virtio-9pfs on windows hosts"; the page shows no completion statement. — [QEMU GitLab issue 974](https://gitlab.com/qemu-project/qemu/-/issues/974)
- VERIFIED October 2026: `hw/9pfs/meson.build` on QEMU master selects host-specific sources only for `darwin`, `freebsd` and `linux`; there is no Windows/win32 branch and no `9p-util-win32.c`. — [qemu/qemu master: hw/9pfs/meson.build](https://github.com/qemu/qemu/blob/master/hw/9pfs/meson.build)

#### Inferences
- The Windows 9pfs series was never merged (stalled after v5 in 2023), and upstream Windows QEMU binaries (e.g. the Stefan Weil w64 builds, MSYS2 packages) therefore reject `-virtfs`/`-fsdev local`. I did not run a Windows QEMU binary to confirm the error message, but the build file is conclusive for upstream.
- Carrying the 16-patch v5 series forward three-plus years onto current QEMU is a significant maintenance burden and unreviewed code handling guest-controlled paths (security-sensitive). Not recommended as the primary path; possibly an experimental option later.
- Because Apple's 9p client was written for Apple's own virtualization use, its behaviour with Finder (change notifications, xattrs) under QEMU is not well documented; Linux-host users report it as functional but basic.

#### Gaps
- Exact macOS point release that introduced `AppleVirtIO` 9p (the source only says "a later Mojave release"); the task brief's guess of Monterey is not supported by the source found, which places it in Mojave (10.14.x).
- No quantitative performance or reliability reports for macOS-guest 9p were found.
- Whether any third-party Windows QEMU distribution ships the 9p patches was not determined.

### 3b. virtio-fs

#### Takeaway
Not usable. Apple's virtiofs guest driver (`mount_virtiofs`, macOS 13+) is documented only for Virtualization.framework guests, and on the host side `virtiofsd` is a Linux-only vhost-user daemon; the "Windows virtiofs" from virtio-win is a guest driver, not a host server.

#### Cited Findings
- In Apple Virtualization.framework macOS guests, shared directories are mounted with `mount_virtiofs tag directory` and are only available in macOS 13 and later. — [Code-Hex/vz wiki: Shared Directories](https://github.com/Code-Hex/vz/wiki/Shared-Directories)
- virtiofsd is a vhost-user backend, one process per virtio-fs device, that must run as root and sandboxes itself with seccomp and Linux capabilities. — [Debian manpage virtiofsd(1)](https://dyn.manpages.debian.org/bookworm/qemu-system-common/virtiofsd.1)
- The virtio-win "virtiofs" component is installed in a Windows guest (with WinFsp) and assumes "qemu and virtiofsd are set and running on the host". — [virtio-win/kvm-guest-drivers-windows issue 473](https://github.com/virtio-win/kvm-guest-drivers-windows/issues/473)
- The only Windows-host shared-filesystem effort tracked by QEMU is the 9pfs one. — [QEMU GitLab issue 974](https://gitlab.com/qemu-project/qemu/-/issues/974)

#### Inferences
- vhost-user needs Unix-domain sockets with file-descriptor passing and shared guest memory, which QEMU on Windows does not provide; so even a ported virtiofsd would have nothing to attach to. (Background knowledge.)
- Virtualization.framework is Apple-host-only, so Apple's virtiofs path is irrelevant to a Windows host. Whether the Intel macOS kernel's virtiofs kext would bind to a QEMU `vhost-user-fs-pci` device on a Linux host is unknown and moot here.

#### Gaps
- No source found showing anyone using virtiofs with an Intel macOS guest under QEMU/KVM.

### 3c. SMB (Windows built-in file sharing, or an embedded SMB server)

#### Takeaway
SMB is the most realistic channel: macOS has a first-class SMB client and Finder integration, and Windows has a first-class server. The catches are that the built-in Windows server needs administrator rights to create a share and real credentials (no guest), and that Finder's live refresh of SMB folders has been unreliable in recent macOS releases (reports through macOS 26.3).

#### Cited Findings
- Windows 11 24H2 requires SMB signing by default on all connections and disables SMB guest fallback on Pro edition. — [The Register, May 2024](https://www.theregister.com/2024/05/30/windows_11_24h2_security_tweaks/)
- Microsoft: the SMB client "no longer allows guest account access to a remote server or fallback to the guest account after invalid credentials"; SMB signing cannot be used with guest credentials. — [Microsoft Learn: SMB feature descriptions](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-feature-descriptions); [Microsoft guidance reposted on ElevenForum](https://www.elevenforum.com/t/accessing-third-party-nas-with-smb-in-windows-11-24h2-may-fail.25599)
- Signing is used whenever either side requires it (`RequireSecuritySignature` on LanmanServer or LanmanWorkstation); the server-side policy is "Microsoft network server: Digitally sign communications (always)", registry `HKLM\System\CurrentControlSet\Services\LanManServer\Parameters\RequireSecuritySignature`; 24H2 adds auditing of third-party clients that do not support signing (`Set-SmbServerConfiguration -AuditClientDoesNotSupportSigning $true`). — [Microsoft Learn: SMB signing overview](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-signing-overview)
- QEMU user-mode networking's built-in `smb=dir` option requires that "a SAMBA server must be installed on the host OS" (QEMU spawns smbd); the documentation describes it for Windows guests reaching a Unix-like host. — [QEMU invocation documentation](https://www.qemu.org/docs/master/system/invocation.html)
- Finder live-update problems: users report that after macOS 26.3 (and one on 26.2) mounted SMB shares stopped live-updating in Finder; new remote files appear only after leaving and re-entering the folder; intermittent across machines. — [TidBITS Talk: 26.3 Finder no longer live updates SMB shares](https://talk.tidbits.com/t/26-3-finder-no-longer-live-updates-smb-shares/32950)
- Ventura-era report: files added by other clients into a subfolder of an SMB share did not show until reconnect; a TrueNAS moderator suspected the Mac was not requesting a listing or not getting a change notification. — [TrueNAS community thread](https://www.truenas.com/community/goto/post?id=750716)
- Older observation that Finder column view seems to refresh network folders roughly every 20 seconds. — [Apple Support Communities thread](https://discussions.apple.com/thread/2172049?page=3)
- `.DS_Store` creation on network volumes can be disabled with `defaults write com.apple.desktopservices DSDontWriteNetworkStores true` (applies to all network drives). — [sabre/dav Finder notes](https://sabre.io/dav/clients/finder/)
- A user-mode SMB server cannot simply coexist with Windows' own server on port 445: Windows' LanmanServer binds 445 on all interfaces early in boot, and Windows SMB clients only connect to port 445. Workarounds (disabling LanmanServer, unbinding adapters in the registry, a loopback adapter, `netsh interface portproxy`) all need admin. — [zzz.buzz: forward multiple remote SMB servers on Windows](https://zzz.buzz/2020/04/26/forward-multiple-remote-smb-servers-on-windows/); [SMBLibrary on NuGet](https://www.nuget.org/packages/SMBLibrary/)
- SMBLibrary is an open-source C# SMB 1.0/2.0/2.1/3.0 server and client library that can serve a directory from a user process. — [SMBLibrary on NuGet](https://www.nuget.org/packages/SMBLibrary/)
- The BSD `mount_smbfs` URL syntax allows a non-default port: `//user@server[:port1[:port2]]/share`. — [mount_smbfs(8), FreeBSD manpage via Debian](https://manpages.debian.org/unstable/freebsd-smbfs/mount_smbfs.8.en.html)
- macOS automounting of SMB via autofs is fragile: Apple no longer allows master-map mounts directly into `/Volumes`; users report SMB automounts stopped working after upgrading to macOS Tahoe 26 and needing manual remount each restart; mount point ownership flipping to root:wheel. — [Gist: automounting on macOS](https://gist.github.com/L422Y/8697518); [Apple Support Communities: automount SMB in Tahoe 26 no longer working](https://discussions.apple.com/thread/256140465?page=2)

#### Inferences
- The 24H2 changes mostly bite Windows-as-client. For orchord the roles are reversed (Windows is the server, macOS the client). macOS's SMB client supports SMB 3 signing, so the signing requirement itself should not block it; the practical consequence is "no guest/anonymous share" - the Mac must log in with a real Windows account name and password. (Background knowledge; not tested.)
- Built-in Windows sharing costs: (1) `New-SmbShare`/`net share` require elevation (background knowledge - Microsoft cmdlet docs not fetched); (2) credentials - many Windows 11 users sign in with a Microsoft account/PIN/Windows Hello and do not know a password usable for SMB, so the app would have to create a dedicated local account (admin again) and store its password in the guest's Keychain; (3) firewall/network-profile rules for "File and Printer Sharing" if the guest reaches the host over a virtual switch rather than QEMU user-mode NAT; (4) a share is visible to the whole LAN unless scoped. This is a one-time elevated setup step, not impossible, but it conflicts with the "no admin" preference.
- QEMU's `smb=` option is not usable on a Windows host (it needs a Samba `smbd` binary and Unix-style spawning); upstream docs only state the Samba requirement, so the "not on Windows" part is inference/background.
- No-admin alternative: embed a user-mode SMB2/3 server (SMBLibrary, or impacket's Python `smbserver` with reduced protocol coverage) listening on `127.0.0.1:<high port>`. With QEMU user-mode networking the guest reaches host loopback at `10.0.2.2` (background knowledge of slirp defaults), so the guest mounts `smb://user@10.0.2.2:<port>/Shared`. Binding only to loopback avoids both the port-445 conflict and the Windows Firewall prompt, and keeps the share invisible to the LAN. Risk to test first: whether Apple's `mount_smbfs`/Finder honour a non-445 port on current macOS (documented for the BSD ancestor only) and how well SMBLibrary interoperates with Apple's client (named streams for xattrs/resource forks, change notify, durable handles).
- If a non-445 port turns out to be unsupported by the macOS client, a TAP/host-only style network is not needed: QEMU `guestfwd` (documented in the invocation page) can map a fake guest-visible address and port 445 to the app's server via a character device/command, e.g. `guestfwd=tcp:10.0.2.100:445-...`, letting the guest use the default port with no host admin rights. Needs testing.
- With SMB, AppleDouble `._` sidecar files appear on servers that lack named-stream support; NTFS via the Windows server supports streams, so metadata goes into ADS instead. An embedded server would need to implement streams or will litter `._` files in the Windows folder (background knowledge).
- Change latency: SMB2 change-notify is the only one of the candidate protocols with a real server-to-client notification, so SMB is the best chance of "appears immediately"; but the cited 2025-2026 Finder reports mean the UI may lag even when the file is already there. For blind users using VoiceOver in Finder this is a real usability issue; a guest helper that plays a sound/announces "new file from Windows" and can reveal it would hide the Finder flakiness.

#### Gaps
- No source found on macOS-client-to-Windows-11-24H2/25H2-server interoperability problems specifically; untested.
- No performance numbers for SMB over QEMU slirp (user-mode NAT is known to be slow for bulk data; magnitude unmeasured here).
- `New-SmbShare` privilege requirement and Windows 11 25H2 SMB changes (e.g. NTLM deprecation steps) were not confirmed from Microsoft docs in this session. NTLM hardening is a risk to watch: a local-account SMB login from macOS uses NTLMv2, and Microsoft's own guidance recommends Kerberos — [Microsoft Learn: SMB signing overview](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-signing-overview).

### 3d. NFS and WebDAV from a user-mode process (no admin)

#### Takeaway
Both can be served from an unprivileged process on Windows and mounted by stock macOS, but both are second-best to SMB for Finder: WebDAV is slow and chatty in Finder and needs server lock support to be writable; NFSv3 needs a root `mount` in the guest and has no change notification.

#### Cited Findings
- Finder is "among the slowest of WebDAV clients", issues many redundant requests including repeated resource-fork requests, and "if the server does not support locks, Finder will operate in read-only mode". — [sabre/dav: Finder client notes](https://sabre.io/dav/clients/finder/)
- Finder sends a high volume of PROPFIND requests per file when listing WebDAV directories; vendors recommend third-party clients instead. — [QNAP FAQ](https://www.qnap.com/la/how-to/faq/con_show.php?cid=3284)
- `nfsserve` is a Rust user-mode NFSv3 server library ("incomplete but very functional") intended as a cross-platform user-mode filesystem API; its README mounts it on macOS with `mount_nfs -o nolocks,vers=3,tcp,rsize=131072,actimeo=120,port=11111,mountport=11111 localhost:/ demo`. A maintained relative is the `nfs3_server` crate. — [nfsserve README (fork mirror)](https://github.com/helyim/nfsserve/blob/main/README.md); [docs.rs nfs3_server](https://docs.rs/nfs3_server)
- macOS NFS client notes: servers in "secure" mode need the `resvport` option; Finder's Connect to Server cannot pass mount options such as `resvport`/`nolocks`; CLI mounts are run with sudo. — [Proxmox forum](https://forum.proxmox.com/threads/mac-issues-connecting-to-nfs-zfs-on-proxmox.75846/post-337521); [InventiveHQ guide](https://inventivehq.com/knowledge-base/macos/how-to-mount-nfs-share-macos)
- Traditional Windows NFS servers (haneWIN) need administrator rights to configure. — [haneWIN NFS Server docs](https://hanewin.net/doc/nfs/nfsd.htm)

#### Inferences
- Neither NFSv3 nor WebDAV has a push notification to the client; visibility of a new host file depends on attribute-cache timeout (`actimeo`) or Finder re-listing, so "immediately" becomes "within seconds, or on folder re-entry". SMB is better on paper here.
- NFSv3 from a Windows folder has the same metadata-mapping problem as 9p (uid/gid/mode/symlinks must be synthesized or stored in ADS/sidecars), and no xattr support, so Finder will create `._` AppleDouble files in the Windows folder.
- WebDAV's advantage is trivial embedding (plain HTTP on a loopback port, many Python/Rust/Go libraries) and mounting without root (`mount_webdav` / Finder "Connect to Server" as the user). Reasonable as a fallback transport, poor as the main one.
- A root-only `mount_nfs` is acceptable if orchord installs a small LaunchDaemon in the guest during setup.

#### Gaps
- No source confirming `mount_nfs` to a high port works without `resvport`/root nuances on macOS 14-26, nor measurements of nfsserve on Windows.
- SFTP/sshfs (macFUSE) was not researched; it needs a third-party kernel/system extension in the guest and is likely worse than the above.

### 3e. VMware (HGFS) and VirtualBox shared folders

#### Takeaway
Not a path for orchord. VMware Tools for macOS exist but macOS guests on Windows VMware require an unsupported patch ("unlocker", now archived); VirtualBox has never shipped Guest Additions for macOS guests, so it has no shared folders for them.

#### Cited Findings
- The DrDonk unlocker patches `vmware-vmx` so macOS can boot on Workstation/Player and provides a copy of macOS VMware Tools (darwin.iso); Workstation Pro 16, 17, 25 and 26 recognise the ISO; the unlocker cannot add AMD CPU support or support for new macOS versions; the project is "complete and archived". — [DrDonk/unlocker](https://github.com/DrDonk/unlocker)
- "VirtualBox does not provide Guest Additions for Mac OS X at this time." — [VirtualBox manual, Known Issues (older revision)](https://git.osdyson.ru/pkg-virtualbox/virtualbox/plain/doc/manual/en_US/user_KnownIssues.xml)

#### Inferences
- VMware Tools for macOS includes an HGFS shared-folder client (shares appear under `/Volumes/VMware Shared Folders`), which is the nearest existing implementation of "host folder appears in the Mac" on a Windows host (background knowledge; the search did not confirm it for patched Workstation). It is proprietary, depends on patching VMware binaries (licence/legal exposure) and cannot be reused under QEMU.

#### Gaps
- No current confirmation that HGFS works in macOS 14-26 guests on unlocked Workstation for Windows.

### 3f. Hyper-V

#### Takeaway
No evidence of any Hyper-V guest integration (integration services, enhanced session, or Hyper-V sockets file copy) for macOS; with a Hyper-V backend, sharing would have to be plain SMB over a virtual switch.

#### Cited Findings
- None found; no source located describing macOS guest integration components for Hyper-V.

#### Inferences
- Hyper-V integration services exist for Windows, Linux and FreeBSD guests only (background knowledge). A macOS guest on Hyper-V would see only emulated/synthetic devices for which it has no drivers unless community kexts are used; file sharing would be SMB to the host over the Default Switch, which then requires the built-in Windows share (admin) or an embedded server bound to the vEthernet address (firewall prompt).

#### Gaps
- Did not research community Hyper-V kexts for macOS (e.g. MacHyperVSupport) in this session; another researcher may cover the hypervisor question.

## 4. Could the home directory or selected folders (Downloads/Documents/Desktop) live on the shared mount?

### Takeaway
Putting the whole home directory, or redirecting Downloads/Documents/Desktop, onto a network/shared mount is a known source of breakage on modern macOS (sandbox containers, TCC-protected folders, Spotlight, logout/login hangs). Keep the home on the local APFS disk and present the Windows folder as an additional, clearly named location; if the "it is in my Downloads" experience is required, deliver it with a small sync agent rather than a redirected folder.

### Cited Findings
- Network home directories: `~/Library/Containers` and its contents "are not being created with the correct permissions"; system agents fail to write there. — [Jamf Community thread](https://community.jamf.com/general-discussions-2/os-x-need-to-repair-your-library-39984/index3.html)
- After Catalina's data protections, network-share home folders reportedly broke: Documents, Desktop and Downloads appear empty with permission errors and no consent dialog. — [mjtsai.com roundup, Feb 2020](https://mjtsai.com/blog/?p=28178) (attribution from search summary; original post not opened)
- SMB network homes: some apps do not work with remote home directories; logins of more than 2 minutes; SMB connection not disconnecting at logout. — [Jamf Community: SMB network home folders](https://community.jamf.com/general-discussions-2/smb-network-home-folders-5553)
- Spotlight filename/content search is lost on SMB shares served by Windows/NAS (vendor claim). — [Acronis document](https://staticfiles.acronis.com/downloads/15d4d74d2fa342bfa48c542b8c09976c)
- autofs/`auto_master` SMB mounts: cannot target `/Volumes` directly, ownership flips to root:wheel, broke for users on macOS Tahoe 26, and Time Machine indexing can trigger mount storms. — [Gist](https://gist.github.com/L422Y/8697518); [Apple Support Communities](https://discussions.apple.com/thread/256140465?page=2); [Otago wiki: Auto-mount remote SMB volumes](https://isgb.otago.ac.nz/infosci/nigel.stanger/Wiki/wiki/Auto-mount%20remote%20SMB%20volumes/_compare/251bf88%5E...251bf88)
- Writing to SMB shares from Finder on Ventura produced "set owner/group ... Operation not permitted" errors for one user. — [OSMC forum](https://discourse.osmc.tv/t/macos-ventura-nfs-smb/94199)

### Inferences
- Mechanisms people use and why each is weak here (background knowledge): (a) Users & Groups > Advanced Options > Home directory pointing at a mounted volume - fails if the mount is not up before login, and inherits all the network-home problems above; (b) replacing `~/Downloads` with a symlink to the share - `~/Downloads`, `~/Documents`, `~/Desktop` are TCC-protected special folders that macOS and apps (Safari, sandboxed apps using security-scoped bookmarks) expect to be real local directories; the folder may be recreated, lose its localized name/icon, and network-volume access triggers a separate "files on a network volume" privacy prompt per app; (c) `/etc/synthetic.conf` creates root-level stubs/symlinks only and does not help with home subfolders; (d) autofs - cited fragility above.
- File semantics on a share differ from APFS: no FSEvents for remote changes, weaker locking, no Spotlight index, slow bundle operations, apps storing SQLite databases there can corrupt them. Fine for documents and downloads, bad for Library/app data.
- Recommended shape: mount the share at a stable user-visible path via a per-user LaunchAgent (mount with `mount_smbfs`/`open smb://...` after login, retry until the host server answers), add it to the Finder sidebar and Desktop, name it e.g. "Windows Shared". Optionally set Safari/Chrome download location to it. This avoids every cited failure mode because nothing the OS depends on lives on the share.
- To honour the developer's literal example ("drop a file into the Mac's Downloads from Windows"): have the Windows-side folder layout mirror selected Mac folders (e.g. `<VM>/drivename/Downloads`, `/Documents`, `/Desktop`) and run a tiny guest agent that two-way syncs each with the real local `~/Downloads` etc. (FSEvents on the Mac side, `ReadDirectoryChangesW` on the Windows side, transport = the shared mount itself or a private TCP channel). The Mac folders stay real local APFS directories (no TCC/sandbox/Spotlight problems), and Explorer shows a normal folder. Costs: files exist twice (image + host folder), conflict handling, and the agent needs Full Disk Access/Files-and-Folders consent once. This is the closest achievable approximation of "the Mac's disk is a folder" for user data.

### Gaps
- No authoritative Apple documentation found on redirecting individual home subfolders to network volumes on macOS 14-26; the problem list is from admin forums, mostly pre-Sonoma.
- TCC behaviour for network volumes on macOS 15/26 not verified from a primary source.

## 5. System disk image: format, bus, TRIM

### Takeaway
Use an ordinary image file for the system disk: raw (sparse on NTFS) or qcow2 under QEMU; VHDX only if a Hyper-V backend demands it. Attach it as SATA/AHCI for maximum compatibility and TRIM, or virtio-blk for speed on Catalina and newer; macOS has no SCSI support on x86.

### Cited Findings
- raw is "simple and easily exportable"; on filesystems that support holes, "for example ... NTFS on Windows", only written sectors reserve space; qcow2 is "the most versatile format" (smaller images on filesystems without holes, compression, multiple VM snapshots); vhdx, vmdk and vpc(VHD) are supported; `dmg` is a read-only format. — [QEMU block drivers documentation](https://www.qemu.org/docs/master/system/qemu-block-drivers.html)
- `-drive discard=ignore|unmap` controls whether guest discard/TRIM is passed to the image; `detect-zeroes=unmap` can turn zero writes into unmaps; "some machine types may not support discard requests". — [QEMU invocation documentation](https://www.qemu.org/docs/master/system/invocation.html)
- OSX-KVM's default disk attachment is `-device ide-hd,bus=sata.4` (AHCI); "Newer macOS (namely Mojave+) have support for some virtio drivers" and switching to `-device virtio-blk-pci` gives "some performance gain". — [kholia/OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md)
- Apple's virtio-blk driver in Mojave "only works in legacy mode". — [The Passthrough Post](https://passthroughpo.st/mac-os-adds-early-support-for-virtio-qemu/)
- Quickemu: VirtIO block is supported/stable in Catalina and newer (disabled by default for High Sierra and Mojave); virtio-net and ballooning from Big Sur (earlier releases use vmxnet3); virtio-sound from Monterey; the installer shows "Apple Inc. VirtIO Block Device" on Big Sur and newer; supported guest list runs Mojave through Tahoe. — [Quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- Quickemu at one point switched the macOS disk from `virtio-blk-pci` to AHCI to get TRIM, stating virtio-blk did not provide TRIM and that "macOS does not support SCSI disks at all on x86_64" (so no virtio-scsi); disks are attached as `ide-hd` with `discard=unmap` and `rotation_rate=1` so macOS treats them as SSD. — [Quickemu commit 881adb2](https://git.binarymagic.net/AIR/quickemu/commit/881adb289a820e0b72ee5f2a81ae3461b96e07c8)
- The current Quickemu wiki says to run `sudo trimforce enable` in the guest after first boot so the disk image can shrink when files are deleted. — [Quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- Unraid guide (High Sierra/Mojave): QEMU disks show as rotational and TRIM is not offered until the disk is presented as SSD and `sudo trimforce enable` is run; with `discard=unmap` space is released on the host image. — [Unraid forum guide](https://forums.unraid.net/topic/80691-guide-enable-trim-on-qemu-disk-in-macososx)
- OSX-KVM mentions NVMe only as passing through a physical controller, not as the emulated disk default. — [kholia/OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md)

### Inferences
- Conflict to resolve by testing: the older Quickemu commit says virtio-blk gives no TRIM and moved to AHCI; the current wiki speaks of virtio block being used where supported while still recommending `trimforce`. The safe default for orchord is AHCI (`ich9-ahci` + `ide-hd`, `discard=unmap`, `rotation_rate=1`), offering virtio-blk as a performance option for Catalina+.
- Format choice on NTFS: qcow2 gives internal snapshots, backing files (cheap "reset to clean install" and linked clones) and stays compact when copied by tools that are not sparse-aware (Explorer, zip, cloud sync). Raw sparse files are faster and simpler, but copying them with Explorer inflates them to full size and NTFS sparse flags are easily lost. Recommendation: qcow2 for the default, raw as an advanced option. (Reasoning from the QEMU doc statements; no benchmark fetched.)
- VHDX/VMDK are worth supporting only for import/export or a Hyper-V backend; QEMU can read/write both but its qcow2/raw paths are the best exercised.
- The VM folder layout that follows: `<VM>/vmconfig/` (config, OpenCore image, NVRAM/OVMF vars), `<VM>/disk/macos.qcow2` (system + home), `<VM>/drivename/` or `<VM>/Shared/` (real Windows folder exported to the guest). This preserves the developer's "each VM is a folder" idea with one substitution.

### Gaps
- No performance comparison (raw vs qcow2 vs VHDX, AHCI vs virtio-blk vs emulated NVMe) for macOS guests on Windows hosts/WHPX was found.
- Whether Apple's virtio-blk driver issues discard on recent macOS (14-26) is unverified.
- Emulated QEMU `nvme` device compatibility with macOS guests by version was not confirmed from a source.

## 6. Reading or writing a macOS disk image from Windows without booting ("browse the Mac disk")

### Takeaway
Read-only offline browsing/extraction of an APFS or HFS+ image is feasible with open-source code (apfs-fuse, libfsapfs, the Rust `apfs` crate; HFSExplorer for HFS+), but there is no trustworthy open-source APFS writer; the only read-write options are the commercial Paragon drivers. Showing the result as a drive in File Explorer needs WinFsp or Dokan, both of which install a kernel driver (admin).

### Cited Findings
- apfs-fuse (sgan81): GPL-2.0; read-only FUSE driver built for Linux; supports software-encrypted (FileVault) volumes with password or recovery key, Fusion drives, and (since 2020-07-08) mounting snapshots and sealed volumes; not supported: writing, LZFSE transparent decompression, T2 hardware-encrypted volumes, firmlinks; described by its author as experimental. Includes `apfsutil`, `apfs-dump`, `apfs-dump-quick`. No Windows/WinFsp support mentioned. — [sgan81/apfs-fuse](https://github.com/sgan81/apfs-fuse)
- The documented way to use apfs-fuse on Windows is inside WSL2 (secondary blog source). — [WiseChecker guide](https://wisechecker.com/mount-apfs-volume-read-only-windows-11-without/)
- libfsapfs (libyal): LGPLv3+, status experimental, read-only; supports APFS format version 2, ZLIB and LZVN compression, encryption; unsupported: Fusion drives, snapshots, LZFSE, T2 encryption. — [openSUSE package page for libfsapfs](https://build.openbuildservice.org/package/show/openSUSE:Factory/libfsapfs?rev=13)
- Rust `apfs` crate (0.2.4): MIT, pure Rust with no unsafe, read-only parser for raw images; no encrypted volumes, no snapshot browsing, no compressed extents, reads only the first non-empty volume in a container. — [docs.rs apfs](https://docs.rs/crate/apfs/latest)
- linux-apfs-rw: out-of-tree Linux kernel module; read-only by default, experimental opt-in write support with an explicit corruption warning; no encryption support. — [linux-apfs/linux-apfs-rw](https://github.com/linux-apfs/linux-apfs-rw)
- HFSExplorer: GPLv3+, Java, reads HFS/HFS+/HFSX volumes and most .dmg/.sparsebundle images (including encrypted), extract-only (no editing); latest release on the project site dated 9 April 2026. — [catacombae.org HFSExplorer](https://catacombae.org/hfsexplorer/); [license listing](https://gittrend.io/repo/unsound/hfsexplorer)
- Paragon APFS for Windows: commercial ($49.95 at 2018 launch), mounts read-only by default with read-write as an option. — [TechRepublic review](https://www.techrepublic.com/article/apfs-for-windows-app-review/); [Hexus press coverage](https://www.hexus.net/tech/items/software/119195-paragon-software-delivers-apfs-readwrite-access-windows/?print=1)
- WinFsp: GPLv3 with a FLOSS exception (open-source projects may link its DLL and redistribute the unmodified installer; must credit and link the WinFsp repo); proprietary use needs a commercial licence; ships signed kernel drivers via its installer. — [winfsp.dev](https://winfsp.dev/); [WinFsp commercial license](https://winfsp.dev/com/); [winfsp on GitHub](https://github.com/billziss-gh/winfsp)
- Dokan (dokany): user-mode DLL plus kernel-mode file system driver (`dokan2.sys`); library/driver/FUSE wrapper LGPL, tools/samples MIT; signed drivers each release. — [Wikipedia: Dokan Library](https://en.wikipedia.org/wiki/Dokan_Library); [dokany README](https://cdn.jsdelivr.net/gh/dokan-dev/dokany@master/README.md)

### Inferences
- Both WinFsp and Dokan are third-party kernel drivers that need an elevated install; they satisfy "no kernel driver of our own" but not "no driver install". Flag as optional.
- A no-driver alternative: an in-app, screen-reader-friendly "Mac disk browser" (list, search, extract/copy-out) built on one of the read-only parsers, opening the qcow2/raw image via `qemu-nbd`-free means (convert offsets from qcow2 with `qemu-img`/`qemu-io`, or require raw). For a GPL/LGPL-compatible open-source app, libfsapfs (LGPL) or the MIT Rust crate are the easiest licences; apfs-fuse is GPL-2.0.
- Limits to state to users: read-only; must never be opened read-write, and should not be opened at all while the VM is running (inconsistent view, and any write would corrupt); FileVault volumes need the password; the interesting content is on the Data volume, not the sealed System volume; LZFSE-compressed and cloned files may be unreadable depending on library; correctness is best-effort because APFS support is reverse-engineered.
- Offline "push a file into the Mac while it is off" is therefore not available via APFS writing. Instead place the file in the shared Windows folder; the guest sees it at next boot.
- 7-Zip can open HFS+ and some APFS/DMG images for extraction (background knowledge; not verified in this session).

### Gaps
- Current maintenance status (last commit dates) of apfs-fuse, libfsapfs and the Rust crate was not retrieved; apfs-fuse's newest dated changelog entry is 2020.
- Whether any of these parsers handle APFS as written by macOS 15/26 (newer feature flags) is unverified.
- No fetched confirmation of hfsutils/hfsprogs Windows builds, Paragon HFS+ current pricing/terms, or 7-Zip APFS support.

## 7. Existing products that give a "Mac disk is just a folder" experience

### Takeaway
None was found for a macOS guest. Every existing product keeps the macOS system on a block image and adds a file-level shared folder next to it (9p under QEMU on Linux, virtiofs under Apple Virtualization, HGFS under VMware); the "it's just a folder" feel of WSL2 or Docker bind mounts works because those guests are Linux and can use 9p/virtiofs even for most of their tree.

### Cited Findings
- QEMU-based macOS VM tooling on Linux (Quickemu) shares a host directory into macOS via virtio-9p (`mount_9p`), with the OS itself on a disk image created by the tool. — [Quickemu commit "Add support for virtio-9p"](https://git.binarymagic.net/AIR/quickemu/commit/01fca96f283c984d91fe8cabad891b6a40d205f3); [Quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- OSX-KVM's notes cover disk images (qcow2/raw) and SSH-based access but contain no shared-folder mechanism that replaces the disk. — [kholia/OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md)
- Apple Virtualization.framework macOS guests get host directories via virtiofs (`mount_virtiofs`, macOS 13+), again in addition to a disk image; the mount command must be run inside the guest. — [Code-Hex/vz wiki](https://github.com/Code-Hex/vz/wiki/Shared-Directories)
- VMware's macOS support on Windows (via unlocker) provides VMware Tools for macOS alongside a VMDK system disk. — [DrDonk/unlocker](https://github.com/DrDonk/unlocker)

### Inferences
- Docker-OSX runs QEMU/KVM inside a Linux container; its bind mounts carry the disk image file into the container, and any folder sharing with macOS is again 9p or SSH/sshfs (background knowledge; the search did not retrieve Docker-OSX documentation).
- WSL2's `\\wsl$` works the other way round: the Linux guest's ext4 lives in a VHDX and Windows reaches it through a 9p server running in the guest. The analogous trick for orchord is feasible and attractive: run macOS's built-in SMB file sharing (or an SFTP/agent endpoint) in the guest and expose the Mac's real home folder to Windows through an app-provided browser, a mapped network location, or a WinFsp/`\\host\share` path. That gives Windows users a live view of the actual Mac folders (including the real `~/Downloads`) while the VM is running, with correct macOS semantics, at the cost of only working while the VM is up and (for Explorer integration via the Windows SMB client) needing the guest reachable on port 445, e.g. via a host-only/NAT address or `hostfwd` to a loopback alias (Windows' SMB client only speaks to port 445 — [zzz.buzz](https://zzz.buzz/2020/04/26/forward-multiple-remote-smb-servers-on-windows/)). Worth prototyping as a complement to the host-folder share.

### Gaps
- Docker-OSX, UTM and Parallels documentation were not successfully retrieved; statements about them are background knowledge only. UTM's macOS-guest shared directory is believed to be virtiofs under Apple Virtualization on Apple-silicon hosts and so not applicable to Intel guests on Windows.

## 8. Recommended design and open tests (synthesis)

### Takeaway
Ship "one VM = one Windows folder" with three parts: `vmconfig/`, a disk image for the Mac's SSD, and a real Windows folder that the app serves to the guest over SMB from an embedded user-mode server on loopback (no admin, no drivers), auto-mounted in macOS by a LaunchAgent, optionally with a sync agent that mirrors it into the real `~/Downloads`/`~/Documents`/`~/Desktop`.

### Cited Findings
- Constraints that drive this: no 9p server in Windows QEMU — [qemu master hw/9pfs/meson.build](https://github.com/qemu/qemu/blob/master/hw/9pfs/meson.build); virtiofsd is Linux-only — [virtiofsd(1)](https://dyn.manpages.debian.org/bookworm/qemu-system-common/virtiofsd.1); QEMU's `smb=` needs Samba on the host — [QEMU invocation docs](https://www.qemu.org/docs/master/system/invocation.html); vvfat forbids host writes during guest use — [QEMU block drivers docs](https://www.qemu.org/docs/master/system/qemu-block-drivers.html); Windows' own SMB server owns port 445 and needs admin to reconfigure — [zzz.buzz](https://zzz.buzz/2020/04/26/forward-multiple-remote-smb-servers-on-windows/); guest/anonymous SMB is effectively gone on Windows 11 24H2 Pro — [The Register](https://www.theregister.com/2024/05/30/windows_11_24h2_security_tweaks/).
- QEMU user-mode networking offers `hostfwd` (host port to guest port) and `guestfwd` (guest-visible address:port to a host character device or spawned command), both repeatable. — [QEMU invocation documentation](https://www.qemu.org/docs/master/system/invocation.html)

### Inferences
- Option ranking for host-folder-into-guest on Windows, no admin:
  1. Embedded user-mode SMB2/3 server (e.g. SMBLibrary) on `127.0.0.1:<port>`, reached by the guest through slirp (`10.0.2.2:<port>` or a `guestfwd` alias on 445). Best Finder integration and the only candidate with change notification. Must be verified: Apple client compatibility, non-445 port, named streams.
  2. Embedded WebDAV server. Easiest to build, mounts without root, but slow in Finder and polling-only.
  3. Embedded NFSv3 server (nfsserve/nfs3_server). Good POSIX semantics, needs root mount in guest, polling-only, `._` files.
  4. Built-in Windows SMB share (one-time UAC prompt + credential setup). Most robust server, worst setup friction; reasonable as an "advanced/compatibility" mode.
  5. Patched QEMU with the unmerged Windows 9pfs series. Experimental only.
- Complementary reverse direction: enable macOS File Sharing in the guest and surface the Mac's real folders to Windows while running.
- Offline access: read-only APFS browser in-app; optional WinFsp drive (admin install).
- Tests to run before committing (none were run in this research): (a) does `mount_smbfs //user@10.0.2.2:PORT/share` work on macOS 12-26; (b) latency from file creation on Windows to appearance in Finder and to an FSEvents/kqueue watcher on the mount; (c) throughput over slirp for a multi-GB copy; (d) VoiceOver behaviour in Finder on the mounted share; (e) whether `-virtfs` is indeed rejected by the chosen Windows QEMU build; (f) AHCI vs virtio-blk TRIM behaviour and image shrink on NTFS.

### Gaps
- All design conclusions in this section are unvalidated by experiment.
- Interaction with WHPX (performance of slirp networking, virtio devices under WHPX with macOS guests) was outside what I could source; another researcher's hypervisor notes should be cross-checked.
