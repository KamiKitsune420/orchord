# Intel macOS versions, Apple installer download, and boot components for an unaccelerated VM (state as of 2026-10-09)

Method notes for the report writer:
- "Live check 2026-10-09" means I queried the Apple server or GitHub API directly from the Windows 11 dev machine on that date; the endpoint is linked.
- Repository files were read from the `master` branch on 2026-10-09 unless stated.
- Anything I know only from memory and could not source in this session is in "Inferences" or "Gaps", never in "Cited Findings".

## 1. Which Intel macOS versions exist (10.13 through the last Intel release), with requirements and support status in October 2026

### Takeaway
macOS 26 Tahoe is confirmed as the final Intel release (macOS 27 "Golden Gate", released 14 Sep 2026, is Apple-silicon only); in October 2026 only Tahoe 26 and Sequoia 15 are Intel-capable versions still receiving updates, Sonoma 14 having dropped out on 14 Sep 2026. Apple publishes numeric RAM/disk minimums only up to Big Sur (2 GB up to Mojave, 4 GB for Catalina and Big Sur; 12.5 to 35.5 GB disk).

### Cited Findings

Version table (dates, latest point release, support status):

| Version | Name | Darwin | Released | Latest point release (date) | Status per endoflife.date |
|---|---|---|---|---|---|
| 10.13 | High Sierra | 17 | 25 Sep 2017 | 10.13.6 (09 Jul 2018) | Ended 01 Dec 2020 |
| 10.14 | Mojave | 18 | 24 Sep 2018 | 10.14.6 (22 Jul 2019) | Ended 25 Oct 2021 |
| 10.15 | Catalina | 19 | 07 Oct 2019 | 10.15.8 (listed date 02 Feb 2026) | Ended 02 Feb 2026 |
| 11 | Big Sur | 20 | 12 Nov 2020 | 11.7.11 (02 Feb 2026) | Ended 02 Feb 2026 |
| 12 | Monterey | 21 | 25 Oct 2021 | 12.7.6 (29 Jul 2024) | Ended 16 Sep 2024 |
| 13 | Ventura | 22 | 24 Oct 2022 | 13.7.8 (20 Aug 2025) | Ended 15 Sep 2025 |
| 14 | Sonoma | 23 | 26 Sep 2023 | 14.8.9 (06 Aug 2026) | Ended 14 Sep 2026 |
| 15 | Sequoia | 24 | 16 Sep 2024 | 15.8.1 (28 Sep 2026) | Supported |
| 26 | Tahoe | 25 | 15 Sep 2025 | 26.7.1 (28 Sep 2026) | Supported; last Intel release |
| 27 | Golden Gate | n/a | 14 Sep 2026 | 27.0.1 (28 Sep 2026) | Supported; Apple silicon only |

- Dates, latest point releases and support status in the table — [endoflife.date/macos](https://endoflife.date/macos)
- Darwin kernel major versions 17 to 25 for High Sierra to Tahoe; Tahoe is "the final version of macOS that supports Macs with Intel processors"; macOS 27 requires an M-series processor — [Wikipedia: macOS version history](https://en.wikipedia.org/wiki/MacOS_version_history)
- endoflife.date's policy summary: major versions are released once a year and "usually maintained for three years"; Apple usually provides security updates for the latest three releases but this is not applied consistently, and fixes are not guaranteed for anything but the latest release — [endoflife.date/macos](https://endoflife.date/macos)
- Apple stated at WWDC 2025 that "macOS Tahoe will be the final release for Intel Macs"; macOS 27 is Apple-silicon only (M-series, or MacBook Neo with A18 Pro) — [Tom's Guide](https://tomsguide.com/computing/macos/apple-is-ending-support-for-all-intel-based-macs-next-year-heres-whats-getting-cut), [Notebookcheck](https://www.notebookcheck.net/The-end-for-Intel-Macs-Apple-to-discontinue-software-support-with-macOS-27.1033777.0.html), [MacTrast, May 2026](https://www.mactrast.com/2026/05/psa-apples-upcoming-macos-27-update-will-put-an-end-to-support-for-intel-based-macs/)
- Apple's Tahoe compatibility page (published 14 Sep 2026) lists these models that are Intel machines: MacBook Pro (16-inch, 2019); MacBook Pro (13-inch, 2020, Four Thunderbolt 3 ports); iMac (Retina 5K, 27-inch, 2020); Mac Pro (2019). The page itself does not label processor types. — [Apple Support 122867](https://support.apple.com/en-us/122867)
- Intel Macs that lost support at Tahoe and stay on Sequoia: 2020 MacBook Air, 2020 MacBook Pro with two Thunderbolt 3 ports, 2018 MacBook Pro 15-inch, 2019 iMac 4K 21.5-inch and 5K 27-inch, 2017 iMac Pro, 2018 Mac mini — [Tom's Guide](https://tomsguide.com/computing/macos/apple-is-ending-support-for-all-intel-based-macs-next-year-heres-whats-getting-cut)
- Six board IDs in OpenCore's board database have a latest supported version of 26.7.1: Mac-E1008331FDC96864, Mac-5F9802EFE386AA28, Mac-A61BADE1FDAD7B05, Mac-27AD2F918AE68F61, Mac-CFF7D910A743CAAF, Mac-AF89B6D9451A490B. Boards capped at 15.8.1 include Mac-7BA5B2D9E42DDD94 and Mac-7BA5B2DFE22DDD8C. — [OpenCorePkg boards.json](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/boards.json)

Apple-stated minimums (Apple "Technical Specifications" pages):
- High Sierra 10.13: 2 GB memory; 14.3 GB available storage to perform the upgrade; OS X 10.8 or later — [Apple SP765](https://support.apple.com/kb/SP765)
- Mojave 10.14: 2 GB memory; 12.5 GB available storage (from El Capitan 10.11.5 or later), up to 18.5 GB from earlier releases. Mac Pro Mid 2010 / Mid 2012 need a "recommended Metal-capable graphics card" — [Apple SP777](https://support.apple.com/kb/SP777)
- Catalina 10.15: 4 GB memory; 12.5 GB available storage (from 10.11.5 or later), up to 18.5 GB from earlier releases — [Apple SP803](https://support.apple.com/kb/SP803)
- Big Sur 11: 4 GB memory; 35.5 GB available storage (from Sierra or later), up to 44.5 GB from earlier releases — [Apple SP833](https://support.apple.com/kb/SP833)

File system and other per-version facts:
- High Sierra introduced the switch to Apple File System (APFS); Catalina is the first version that does not support 32-bit applications; Big Sur is the first "64-bit Intel and ARM" release — [Wikipedia: macOS version history](https://en.wikipedia.org/wiki/MacOS_version_history)
- In a VM, quickemu instructs: for Mojave or later choose an APFS option when erasing the disk; "MacOS Extended may not work" — [quickemu wiki: Create macOS virtual machines](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- Apple's current download page (published 14 Sep 2026) still offers App Store installers for Sequoia 15, Sonoma 14, Ventura 13, Monterey 12, Big Sur 11, Catalina 10.15, Mojave 10.14 and High Sierra 10.13, and direct .dmg downloads for Sierra, El Capitan, Yosemite, Mountain Lion and Lion — all "require a compatible Mac" — [Apple Support 102662](https://support.apple.com/en-us/102662)

### Inferences
- endoflife.date's "Ended 02 Feb 2026" for Catalina and Big Sur coincides with the listed date of their latest builds (11.7.11 is corroborated as the latest Big Sur version by OpenCore's boards.json). This looks like a late out-of-band update rather than continuous security support; treat Catalina and Big Sur as out of security support for practical purposes. The nature of that February 2026 update was not verified.
- For October 2026, the only Intel-capable versions with current security patches are Sequoia 15 (probably until about September 2027 under the three-release pattern) and Tahoe 26 (probably until about September 2028). These end dates are projections from the pattern, not Apple statements.
- The six Tahoe-capable board IDs correspond to the four Intel models on Apple's list (16-inch 2019 MacBook Pro, 13-inch 2020 four-port MacBook Pro, 27-inch 2020 iMac, 2019 Mac Pro). The exact board-ID-to-model mapping (for example Mac-27AD2F918AE68F61 = MacPro7,1 and Mac-CFF7D910A743CAAF = iMac20,1) is from my memory and was not sourced in this session.
- From memory, not sourced here: High Sierra converts only SSD boot volumes to APFS and can still install to HFS+; Mojave and later require APFS for the boot volume; Big Sur and later use a sealed, signed system volume.

### Gaps
- Apple-stated RAM/disk minimums for Monterey, Ventura, Sonoma, Sequoia and Tahoe were not found; Apple's newer pages appear to list compatible models only. I did not fetch those pages individually.
- "Needs Metal for a usable UI" has no Apple statement per version. The only Apple reference found is the Mojave Metal-capable-GPU note for old Mac Pros. See section 2 for community evidence.
- The Darwin version for macOS 27 was not reliably extracted and is irrelevant to an Intel-only project.

## 2. Which versions are usable without GPU acceleration, and which is "lightest"

### Takeaway
Every version from High Sierra to Tahoe boots and runs on an unaccelerated framebuffer in the reference projects, so none is ruled out; but I found no measured comparison between versions, only consistent statements that there is no 3D/Metal acceleration and that CPU core quality, RAM and transparency settings dominate responsiveness. For a screen-reader-first product the decisive version boundary is audio, not graphics: quickemu reports that Monterey 12 and later have a native VirtIO sound driver, while Big Sur and earlier need a third-party kext with SIP disabled.

### Cited Findings
- quickemu supports Mojave, Catalina, Big Sur, Monterey, Ventura, Sonoma, Sequoia and Tahoe, "optimised by default, but no GPU acceleration is available" — [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- OSX-KVM's fetch menu covers High Sierra 10.13 through Tahoe 26 and marks "Sonoma (14) - RECOMMENDED". Its README states the resulting system "lacks graphical acceleration, a reliable sound sub-system, USB 3 functionality and other similar things" — [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)
- OSX-KVM warns: "Let >= Big Sur setup sit at the Country Selection screen, and other similar places for a while if things are being slow. The initial macOS setup wizard will eventually succeed." — [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)
- quickemu maintainer lj3954: "macOS VMs should be allocated at least 8GB of RAM for reasonable performance"; no virtualization software gives macOS 3D graphics drivers on non-Apple hosts; CPU performance under KVM is close to host per-core. In the same thread a user reports a Sonoma VM as "very choppy/slow" (no comparison with other versions). — [quickemu discussion #1372](https://github.com/orgs/quickemu-project/discussions/1372)
- A community guide reports a smooth 1920x1080 desktop on Big Sur through Sequoia with `vmware-svga` and no passthrough, once QEMU is pinned to fast physical cores. Its claims: letting vCPU or display threads land on Intel E-cores or SMT siblings gives "choppy 5-15 FPS slideshows"; use 4 to 6 cores maximum; "Never use E-cores"; over-allocating vCPUs makes things worse; macOS reports only 3 to 7 MB of video memory regardless of QEMU settings and `vgamem_mb` has no effect; the host needs working OpenGL to composite the window. Tested hardware includes an Intel Core Ultra 7 155H where pinning to P-cores was "mandatory". Single-author gist, tested on Linux/KVM with QEMU 9.2.1. — [OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)
- Audio by version in quickemu: Monterey, Ventura, Sonoma, Sequoia, Tahoe use `virtio-sound-pci` ("Native VirtIO driver, works out-of-the-box"); Big Sur uses `ich9-intel-hda` and "Requires VoodooHDA kext; unreliable from 11.3+"; Catalina and earlier use `intel-hda` and require VoodooHDA. Installing VoodooHDA requires disabling Gatekeeper and SIP. A passed-through USB audio device is offered as the reliable alternative on any version. — [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- sickcodes' osx-optimizer lists the standard guest tweaks: disable Spotlight indexing (`sudo mdutil -i off -a`, described as heavily speeding up virtual instances), set `reduceTransparency` and `reduceMotion` (`defaults write com.apple.universalaccess reduceTransparency -int 1`, `reduceMotion -int 1`), blank the login wallpaper, disable automatic updates so the disk image does not grow. — [osx-optimizer README](https://raw.githubusercontent.com/sickcodes/osx-optimizer/master/README.md)
- On physical Intel Macs, Tahoe's Liquid Glass layer is described as more GPU-intensive; users report higher WindowServer CPU and that Reduce Transparency (System Settings, Accessibility, Display) speeds up menus and redraws; there is no full "disable Liquid Glass" switch. — [OSXDaily](https://osxdaily.com/2025/09/25/macos-tahoe-26-feels-slow-fix/), [Macworld](https://www.macworld.com/article/2858361/how-to-reduce-the-liquid-glass-transparency-effect-in-macos-tahoe.html)
- An older emulation-forum opinion states that anything above 10.14 "isn't really worth emulating (only virtualizing) under QEMU, as the lack of hardware graphics support slows them to a crawl" (refers to TCG emulation, not hardware virtualization). — [E-Maculation forum](https://www.emaculation.com/forum/viewtopic.php?t=11293)

### Inferences
- There is no sourced benchmark showing Catalina/Mojave/High Sierra to be faster unaccelerated than Big Sur or later. The belief is plausible (Big Sur introduced the heavily translucent redesign; Tahoe added Liquid Glass) and matches my prior knowledge of community opinion, but in this session it rests on reasoning, not data. The developer should measure on the i7-1355U.
- For a blind user the visual frame rate matters much less than VoiceOver latency and audio reliability. That shifts the "best" unaccelerated version toward Monterey or later (native audio), even though older versions are probably lighter graphically.
- Candidate for the first test: Monterey 12. Reasons: oldest version with native VirtIO sound, VirtIO network and VirtIO block; does not need AVX2; smallest recovery image among versions with native audio (653,691,436 bytes); no Liquid Glass. Its drawback is no security updates since September 2024.
- Candidate for the default "supported" version: Sequoia 15, the oldest version still receiving security updates in October 2026, or Sonoma 14, which OSX-KVM marks as recommended (but which left support on 14 Sep 2026).
- Tahoe 26 should be offered because it is the final Intel release and will be patched longest, but it is the highest-risk choice for unaccelerated responsiveness.
- Applying Reduce Transparency and Reduce Motion and disabling Spotlight indexing after first boot is cheap and consistently recommended; the wizard could offer a post-install script.

### Gaps
- No r/hackintosh, r/macOSVMs or InsanelyMac thread with version-to-version unaccelerated comparisons was retrieved; search results did not surface any. Community "consensus" on the lightest modern version is therefore unconfirmed.
- Whether VoiceOver speech works inside the Recovery/installer environment with `virtio-sound-pci` (before the full OS is installed) was not verified. This is critical for a blind-accessible install and needs a hands-on test.
- Whether specific apps that hard-require Metal fail on Ventura through Tahoe without acceleration was not researched.
- All performance evidence is from Linux/KVM hosts. Nothing was found for QEMU on Windows (WHPX) or for Hyper-V guests with a software framebuffer.

## 3. CPU model, CPUID requirements, and 12th/13th-gen hybrid hosts

### Takeaway
The reference projects give the guest a named Intel model rather than raw passthrough (Penryn historically, Haswell-noTSX for Sonoma, Skylake-Client for Sequoia and Tahoe), with SSE4.2 needed from Catalina and AVX2 from Ventura; OpenCore's `ProvideCurrentCpuInfo` quirk is the documented fix for both `-cpu host` panics under KVM and for Hyper-V timing. Hybrid-host guidance specific to VMs is thin: the concrete advice found is to keep vCPUs on P-cores, and OpenCore documents an Alder Lake to Comet Lake CPUID spoof for cases where the real model ID reaches the guest.

### Cited Findings
- OSX-KVM requirements: Intel VT-x or AMD SVM; "SSE4.1 support is required for >= macOS Sierra"; "AVX2 support is required for >= macOS Ventura" — [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)
- quickemu's feature checks (applied when it must synthesize flags): Ventura, Sonoma, Sequoia, Tahoe require `sse4_2` and `avx2`; Catalina, Big Sur, Monterey require `sse4_2`; older releases require `sse4_1`. Comments state `fma` is required for Metal and `invtsc` is required for macOS to boot. — [quickemu script](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu)
- OSX-KVM's active QEMU CPU line: `-cpu Skylake-Client,-hle,-rtm,kvm=on,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on,+ssse3,+sse4.2,+popcnt,+avx,+aes,+xsave,+xsaveopt,check`, commented "Enable this line for macOS Sequoia and Tahoe". The alternative commented line uses `Penryn` with the same flags; the header says to change Penryn to `Haswell-noTSX` for Sonoma. — [OSX-KVM OpenCore-Boot.sh](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore-Boot.sh)
- OSX-KVM notes: "Host CPU passthrough is troublesome and not generally recommended"; nested virtualization (Hypervisor.framework) needs `-cpu Skylake-Client` or similar instead of Penryn. — [OSX-KVM notes.md](https://raw.githubusercontent.com/kholia/OSX-KVM/master/notes.md)
- Docker-OSX default: `-cpu Penryn,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on,+ssse3,+sse4.2,+popcnt,+avx,+aes,+xsave,+xsaveopt,check` — [Docker-OSX Dockerfile](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/Dockerfile)
- quickemu: on a bare-metal Intel host it uses `-cpu host,-pdpe1gb,+hypervisor,vmware-cpuid-freq=on`; otherwise `Haswell-v2` with `vendor=GenuineIntel`. It disables 1 GB pages (`-pdpe1gb`) to prevent crashes, turns HPET off, sets `vmport=off` and SMM off for macOS. — [quickemu script](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu)
- OpenCore `ProvideCurrentCpuInfo` (Kernel quirk): "For Microsoft Hyper-V it provides the correct TSC and FSB values to the kernel, as well as disables CPU topology validation (10.8+). For KVM and other hypervisors it provides precomputed MSR 35h values solving kernel panic with `-cpu host`. For Intel CPUs it adds support for asymmetrical SMP systems (e.g. Intel Alder Lake) by patching core count to thread count along with the supplemental required changes (10.14+)." — [OpenCorePkg Configuration.tex](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Docs/Configuration.tex)
- OpenCore `Cpuid1Data` / `Cpuid1Mask` replace the CPUID leaf 1 values seen by XNU, to enable a CPU model not supported by a macOS version. Documented example: Alder Lake (0x090672) to Comet Lake (0x0A0655): `Cpuid1Data = 55 06 0A 00 00 00 00 00 00 00 00 00 00 00 00 00`, `Cpuid1Mask = FF FF FF FF 00 00 00 00 00 00 00 00 00 00 00 00`. — [OpenCorePkg Configuration.tex](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Docs/Configuration.tex)
- OSX-KVM's shipped OpenCore config sets `ProvideCurrentCpuInfo = true`, `DummyPowerManagement = true`, and leaves `Cpuid1Data`/`Cpuid1Mask` empty. — [OSX-KVM config.plist](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore/config.plist)
- `DummyPowerManagement` disables AppleIntelCpuPowerManagement; stated requirement range 10.4 to 12. — [OpenCorePkg Configuration.tex](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Docs/Configuration.tex)
- MacHyperVSupport requires `ProvideCurrentCpuInfo` "for correct TSC/FSB values and CPU topology", and says CPU spoofing and DummyPowerManagement "may be needed on older macOS, depending on the host CPU". — [MacHyperVSupport README](https://raw.githubusercontent.com/acidanthera/MacHyperVSupport/master/README.md)
- quickemu documents stricter TSC requirements in Ventura, Sonoma and Sequoia that break some AMD Ryzen mobile hosts; Intel CPUs are listed as not affected. — [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- Hybrid hosts: the only VM-specific evidence found is the pinning guide (Intel Core Ultra 7 155H needs vCPUs pinned to P-cores; avoid E-cores and SMT siblings). — [OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)

### Inferences
- The i7-1355U has SSE4.2, AVX2 and FMA, so no macOS version is excluded by instruction-set requirements (my knowledge of the CPU, not sourced here).
- A named CPU model (Skylake-Client with HLE/RTM off, or Haswell-noTSX) is the safest default on a Raptor Lake host: the guest sees a model ID that XNU knows, so the Alder Lake to Comet Lake `Cpuid1Data` spoof should only be needed if the hypervisor cannot mask the model (for example a Hyper-V guest, which sees the real host CPUID). In that case the spoof plus `ProvideCurrentCpuInfo` is the documented route.
- AVX-512 is absent on this host, which is not a problem: none of the reference CPU lines request it, and Skylake-Client (unlike Skylake-Server) does not include it.
- A hypervisor presents homogeneous vCPUs, so macOS itself does not see P/E asymmetry; the hybrid risk is performance (vCPUs scheduled on E-cores) and, with raw passthrough, CPUID leaves that differ between core types.
- This host has only 2 P-cores (4 threads). The gist's "4 to 6 P-cores, no SMT siblings" advice cannot be met; the realistic choices are 2 vCPUs on the two P-cores, or 4 vCPUs on the four P-core threads. Which is better here is untested.

### Gaps
- No source was found describing macOS guests on 12th/13th-gen hybrid hosts under QEMU-on-Windows (WHPX) or Hyper-V specifically, nor any report of CPUID-leaf problems unique to Raptor Lake-U.
- Whether Windows/WHPX honours QEMU named CPU models and whether vCPU threads can be restricted to P-cores under an active Hyper-V root partition was not researched (adjacent scope).
- The exact minimum CPUID feature list XNU enforces per version beyond SSE4.x/AVX2 was not found in a primary source.

## 4. How to download macOS from Apple by script (works from Windows with only Python)

### Takeaway
The recovery-image route used by OpenCore's `macrecovery.py` is plain HTTP that needs only the Python standard library, and I confirmed on 2026-10-09 from the Windows dev machine that Apple's server returns a `BaseSystem.dmg` plus `BaseSystem.chunklist` for every version from High Sierra to Tahoe (458 to 916 MiB). The dmg does not have to be converted at all if OpenCore boots it from a FAT32 `com.apple.recovery.boot` folder, and a FAT32 image can be built in pure Python.

### Cited Findings

Protocol (from `macrecovery.py`):
- GET `http://osrecovery.apple.com/` with headers `Host: osrecovery.apple.com`, `Connection: close`, `User-Agent: InternetRecovery/1.0`; take the `session=...` cookie from `Set-Cookie`. — [macrecovery.py](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/macrecovery.py)
- POST `http://osrecovery.apple.com/InstallationPayload/RecoveryImage` with `Content-Type: text/plain`, the session cookie, and a newline-separated body: `cid` (16 random hex chars), `sn` (17-char MLB), `bid` (board ID), `k` (64 random hex), `fg` (64 random hex), `os` (`default` or `latest`). The response is `key: value` lines: `AP` product, `AU` image URL, `AH` image hash, `AT` image token, `CU` chunklist URL, `CH` chunklist hash, `CT` chunklist token. — [macrecovery.py](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/macrecovery.py)
- Download `AU` and `CU` with header `Cookie: AssetToken=<AT or CT>` and the same User-Agent. The script verifies the dmg against the chunklist (per-chunk hashes, header struct `<4sIBBBxQQQ`, 0x24 bytes). Default board is `Mac-27AD2F918AE68F61`, default MLB is 17 zeros. — [macrecovery.py](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/macrecovery.py)
- On Windows, run `macrecovery.bat` or `py macrecovery.py`; Python 3 is required. — [recovery_urls.txt](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/recovery_urls.txt), [Dortania Windows install guide](https://dortania.github.io/OpenCore-Install-Guide/installer-guide/windows-install.html)

Board ID / MLB per version (three sources; they differ for some versions):

| Version | OpenCorePkg recovery_urls.txt | OSX-KVM fetch-macOS-v2.py | quickemu quickget |
|---|---|---|---|
| High Sierra | Mac-7BA5B2D9E42DDD94 or Mac-BE088AF8C5EB4FA2, MLB 00000000000J80300 | Mac-7BA5B2D9E42DDD94, 00000000000J80300 | Mac-BE088AF8C5EB4FA2, 00000000000J80300 |
| Mojave | Mac-7BA5B2DFE22DDD8C, 00000000000KXPG00 | same | same |
| Catalina | Mac-CFF7D910A743CAAF + 00000000000PHCD00, or Mac-00BE6ED71E35EB86 + zeros | Mac-00BE6ED71E35EB86, zeros | Mac-00BE6ED71E35EB86, zeros |
| Big Sur | Mac-2BD1B31983FE1663, zeros | same | same |
| Monterey | Mac-E43C1C25D4880AD6, zeros | Mac-B809C3757DA9BB8D, zeros, os=latest | Mac-B809C3757DA9BB8D, zeros, os=latest |
| Ventura | Mac-B4831CEBD52A0C4C, zeros | Mac-4B682C642B45593E, zeros, os=latest | Mac-4B682C642B45593E, zeros, os=latest |
| Sonoma | Mac-827FAC58A8FDFA22, zeros | same | same |
| Sequoia | Mac-7BA5B2D9E42DDD94, zeros | same | same |
| Tahoe | Mac-CFF7D910A743CAAF, zeros, `-os latest` | Mac-CFF7D910A743CAAF, zeros, os=latest | Mac-CFF7D910A743CAAF, zeros, os=latest |

- Sources for the table — [recovery_urls.txt](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/recovery_urls.txt), [fetch-macOS-v2.py](https://raw.githubusercontent.com/kholia/OSX-KVM/master/fetch-macOS-v2.py), [quickget](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickget)
- The Dortania guide page lists different IDs for some versions (Big Sur Mac-42FD25EABCABB274, Monterey Mac-FFE5EF870D7BA81A, Sonoma Mac-226CB3C6A851A671, Sequoia Mac-937A206F2EE63C01 as "latest") and has no Tahoe entry on that page; it says you get either BaseSystem or RecoveryImage files and need both the .dmg and .chunklist. — [Dortania Windows install guide](https://dortania.github.io/OpenCore-Install-Guide/installer-guide/windows-install.html)
- macrecovery's own notes on MLB behaviour: a zero MLB always gives the same product for `default` and `latest`; `default` with a serial "returns oldest for sn". — [macrecovery.py](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/macrecovery.py)

Live check 2026-10-09 (own stdlib-only Python script on Windows, using the recovery_urls.txt IDs; sizes from HTTP `Content-Length` of `AU`/`CU` on host `oscdn.apple.com`, path `.../RecoveryImage/BaseSystem.dmg`):

| Version | Board ID used | Apple product (`AP`) | BaseSystem.dmg bytes | approx MiB | chunklist bytes |
|---|---|---|---|---|---|
| High Sierra | Mac-7BA5B2D9E42DDD94 (MLB ...J80300) | 091-63921 | 486,219,126 | 464 | 1,984 |
| Mojave | Mac-7BA5B2DFE22DDD8C (MLB ...KXPG00) | 041-94410 | 480,292,242 | 458 | 1,948 |
| Catalina | Mac-00BE6ED71E35EB86 | 2Z694-25616 (path 001-43312) | 498,626,206 | 476 | 2,020 |
| Big Sur | Mac-2BD1B31983FE1663 | 694-35959 (path 071-71279) | 668,153,735 | 637 | 2,596 |
| Monterey | Mac-E43C1C25D4880AD6 | 694-54378 (path 012-51692) | 653,691,436 | 623 | 2,560 |
| Ventura | Mac-B4831CEBD52A0C4C | 042-01871 | 706,568,592 | 674 | 2,740 |
| Sonoma | Mac-827FAC58A8FDFA22 | 696-08090 (path 062-53943) | 789,568,598 | 753 | 3,028 |
| Sequoia | Mac-7BA5B2D9E42DDD94 (zero MLB) | 696-28424 (path 082-33203) | 884,317,790 | 843 | 3,352 |
| Tahoe | Mac-CFF7D910A743CAAF (os=latest) | 140-93589 | 960,530,321 | 916 | 3,604 |

- Endpoint queried for the table above — [osrecovery.apple.com/InstallationPayload/RecoveryImage](http://osrecovery.apple.com/InstallationPayload/RecoveryImage)

Other download routes:
- gibMacOS (corpnewt): reads Apple's software update catalog at `https://swscan.apple.com/content/catalogs/others/index-{}.merged-1.sucatalog`, where `{}` is the chain of version tokens (names such as `snowleopard`/`lion` for the oldest, `10.x`, then `12` to `15`, and 26+), with a suffix for the catalog type: none for `publicrelease`, `beta` for public, `customerseed`, `seed` for developer. Installers are recognised via `ExtendedMetaInfo -> InstallAssistantPackageIdentifiers`. By default it downloads all packages of the product; `-r/--recovery` downloads only `RecoveryHDUpdate.pkg` / `RecoveryHDMetaDmg.pkg`. It has a Windows (`os.name == "nt"`) code path; macOS-only features are gated. License MIT. Last push 2026-06-18. — [gibMacOS.py](https://raw.githubusercontent.com/corpnewt/gibMacOS/master/gibMacOS.py), [gibMacOS LICENSE](https://raw.githubusercontent.com/corpnewt/gibMacOS/master/LICENSE), [GitHub API](https://api.github.com/repos/corpnewt/gibMacOS)
- Full installers on Apple's CDN: Mr. Macintosh's database (updated 28 Sep 2026) lists Tahoe 26.7.1 build 25G241 at `https://swcdn.apple.com/content/downloads/51/17/142-28081-A_616Q67X32N/5atqz1miz61kmrim7fwkn7yi62xuzzcc0s/InstallAssistant.pkg`, 26.7 build 25G229 and 26.6.2 build 25G83. InstallAssistant.pkg contains the full "Install macOS.app"; it exists for Big Sur and later (not for Catalina or Mojave). Separate database pages exist for Sequoia, Sonoma, Ventura, Monterey and Big Sur. — [Mr. Macintosh Tahoe database](https://mrmacintosh.com/macos-tahoe-full-installer-database-download-directly-from-apple/)
- Live check 2026-10-09: that Tahoe 26.7.1 `InstallAssistant.pkg` is 18,380,974,544 bytes (about 17.1 GiB), `Last-Modified: 26 Sep 2026`, downloadable with no authentication. — [swcdn.apple.com URL](https://swcdn.apple.com/content/downloads/51/17/142-28081-A_616Q67X32N/5atqz1miz61kmrim7fwkn7yi62xuzzcc0s/InstallAssistant.pkg)
- `softwareupdate --fetch-full-installer --full-installer-version <n>` and `softwareupdate --list-full-installers` run only in Terminal on a Mac (Catalina 10.15 or later) and place "Install [Version Name]" in Applications. — [Apple Support 102662](https://support.apple.com/en-us/102662)
- OSX-KVM's `fetch-macOS-v2.py` is a copy of macrecovery.py with a version menu and a `--shortname` option; it explicitly credits OpenCorePkg's boards.json / recovery_urls.txt and gibMacOS. — [fetch-macOS-v2.py](https://raw.githubusercontent.com/kholia/OSX-KVM/master/fetch-macOS-v2.py)
- quickget implements the same protocol in shell (curl) and notes that some users must turn a VPN off, and others must turn one on, to obtain a recovery image. — [quickget](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickget), [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)

Making the recovery image bootable:
- OSX-KVM converts with `dmg2img -i BaseSystem.dmg BaseSystem.img` and attaches the raw image as a SATA disk. dmg2img is GPL-2.0. — [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md), [GitHub API: Lekensteyn/dmg2img license](https://api.github.com/repos/Lekensteyn/dmg2img/license)
- quickemu converts with `qemu-img convert RecoveryImage.dmg -O raw RecoveryImage.img`, optionally verifying with a `chunkcheck` binary first. — [quickget](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickget)
- MacHyperVSupport: convert a read/write DMG with `qemu-img convert -f raw -O vhdx Installer.dmg Installer.vhdx` (or `-O vpc` for VHD). — [MacHyperVSupport README](https://raw.githubusercontent.com/acidanthera/MacHyperVSupport/master/README.md)
- No conversion: OpenCore boots the dmg directly when `BaseSystem.dmg` (or `RecoveryImage.dmg`) and its `.chunklist` are placed in a folder named `com.apple.recovery.boot` at the root of a FAT32 volume next to the `EFI` folder. — [Dortania Windows install guide](https://dortania.github.io/OpenCore-Install-Guide/installer-guide/windows-install.html)
- OpenCore's `DmgLoading` policy (failsafe `Signed`) governs DMG recovery loading; a signed DMG is required whenever Apple Secure Boot is in use, and macrecovery is the documented way to obtain the recovery DMG for `com.apple.recovery.boot`. — [OpenCorePkg Configuration.tex](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Docs/Configuration.tex)
- `pyfatfs` 1.1.0 is a pure-Python "FAT12/FAT16/FAT32 implementation with VFAT support", MIT license, Python `~=3.8`. — [PyPI pyfatfs](https://pypi.org/pypi/pyfatfs/json)

Tool licenses:
- OpenCorePkg (includes macrecovery): BSD 3-clause style license text, copyright The HermitCrabs Lab, Download-Fritz, savvas, vit9696. Latest release 1.0.8, published 2026-09-27. — [OpenCorePkg LICENSE.txt](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/LICENSE.txt), [GitHub API releases](https://api.github.com/repos/acidanthera/OpenCorePkg/releases/latest)
- quickemu: MIT (copyright Wimpy's World); latest release 4.9.9 on 2026-02-10. — [quickemu LICENSE](https://raw.githubusercontent.com/quickemu-project/quickemu/master/LICENSE), [GitHub API](https://api.github.com/repos/quickemu-project/quickemu/releases/latest)
- Docker-OSX: "licensed under the GPL v3+". — [Docker-OSX README](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/README.md)
- OSX-KVM: `LICENSE` on master returns 404 and the GitHub license API returned no license (checked 2026-10-09). — [OSX-KVM LICENSE URL](https://raw.githubusercontent.com/kholia/OSX-KVM/master/LICENSE)

### Inferences
- Board ID selects the version: with a zero MLB, Apple returns the newest recovery that the given board supports, so each version is fetched by using a board whose last supported macOS is that version (boards.json shows Mac-E43C1C25D4880AD6 capped at 12.7.6, Mac-B4831CEBD52A0C4C at 13.7.8, Mac-827FAC58A8FDFA22 at 14.8.9, Mac-7BA5B2D9E42DDD94 at 15.8.1). High Sierra and Mojave use product-specific MLBs with `os=default` to get the oldest image for those products. Consequence: a board ID "ages" only when Apple changes what it serves, so orchord should keep the table in a data file and verify it at runtime (the `AP` product code and a size sanity check).
- The simplest pure-Python pipeline on Windows: (1) fetch dmg + chunklist with `urllib`; (2) verify with the chunklist; (3) build a FAT32 image of roughly 1.5 to 2 GB with pyfatfs containing `EFI/` (OpenCore) and `com.apple.recovery.boot/`; (4) attach as a disk. No dmg2img, qemu-img or 7-Zip is needed, and all files are under FAT32's 4 GB limit (largest dmg is 960 MB).
- If conversion is preferred, `qemu-img` is the only converter in these projects that is routinely shipped for Windows; dmg2img is a Unix tool with GPL-2.0 terms. I did not verify that a Windows qemu-img build handles the compression used in current BaseSystem.dmg files.
- The recovery endpoint uses plain HTTP; integrity comes from the chunklist signature. orchord should implement the chunklist verification rather than rely on transport security.
- The full-installer route (InstallAssistant.pkg, about 17 GiB for Tahoe) is not directly bootable; it would need extraction and a `createinstallmedia`-equivalent step that normally runs on macOS, so it fits the "choose a local image" path better than the automatic path.

### Gaps
- Which exact macOS point release each recovery image installs was not determined (the `AP` code is a product number, not a version).
- I did not download and boot any image; "Apple serves it" is verified, "OpenCore boots it in the target VM on Windows" is not.
- gibMacOS's Windows helper scripts (for example a MakeInstall script) and whether they can produce a bootable full installer without macOS were not verified.
- 7-Zip's ability to open current BaseSystem.dmg files was not checked.
- Apple publishes no documentation or terms for `osrecovery.apple.com`; rate limits and long-term availability for Intel recovery images after Intel support ends are unknown.

## 5. Internet Recovery flow inside the VM, and the network device macOS needs

### Takeaway
After booting BaseSystem, the installer pulls the full OS from Apple inside the VM; the only hard number found is the full Tahoe installer size (about 17.1 GiB), with no sourced data on duration or failure rates. For networking, reference projects use `vmxnet3` for Catalina and earlier and `virtio-net-pci` for Big Sur and later, with `e1000-82545em` as a fallback option.

### Cited Findings
- quickemu install flow: boot "macOS Base System", erase the target in Disk Utility (APFS), choose "Reinstall macOS"; on first reboot select "macOS Installer", on later reboots select the named disk. — [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- quickemu: "VirtIO Network (virtio-net) is supported and enabled on macOS Big Sur and newer, but earlier releases use vmxnet3". VirtIO block is "supported/stable in Catalina and newer"; a code comment says VirtIO block does not work in High Sierra at all or in the Mojave recovery image, so those use `ide-hd` on AHCI. Memory ballooning and VirtIO RNG are supported since Big Sur. — [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines), [quickemu script](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu)
- OSX-KVM default: `-netdev user,id=net0,hostfwd=tcp::2222-:22 -device virtio-net-pci,...`, with a commented alternative "`-device vmxnet3` ... Use this line for High Sierra". The script header says it works for Big Sur, Catalina, Mojave and High Sierra. — [OSX-KVM OpenCore-Boot.sh](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore-Boot.sh)
- Docker-OSX default `NETWORKING=vmxnet3`, with `e1000-82545em` as the commented alternative. — [Docker-OSX Dockerfile](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/Dockerfile)
- A Proxmox-oriented OpenCore ISO project lists VirtIO as default for macOS 11 to 26 and VMware vmxnet3 for 10.11 to 10.15 (as summarized by search results; page not fetched directly). — [LongQT-sea/OpenCore-ISO](https://github.com/LongQT-sea/OpenCore-ISO)
- Hyper-V: MacHyperVSupport provides a synthetic network controller, synthetic SCSI, synthetic keyboard/mouse and a basic framebuffer; on Big Sur and newer the framebuffer kext requires disabling kext signing in SIP. — [MacHyperVSupport README](https://raw.githubusercontent.com/acidanthera/MacHyperVSupport/master/README.md)
- App Store / Apple ID: the wired interface must be `en0` or login fails with "Your device or computer could not be verified"; Apple ID login is usually blocked until serial/MAC values are made unique. — [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- Full installer size reference: Tahoe 26.7.1 InstallAssistant.pkg is 18,380,974,544 bytes (live check 2026-10-09). — [swcdn.apple.com URL](https://swcdn.apple.com/content/downloads/51/17/142-28081-A_616Q67X32N/5atqz1miz61kmrim7fwkn7yi62xuzzcc0s/InstallAssistant.pkg)

### Inferences
- Sources conflict on VirtIO network for Mojave/Catalina: quickemu says Big Sur and newer only; OSX-KVM's single script uses virtio-net for everything except High Sierra. The conservative mapping is `vmxnet3` for 10.13 to 10.15 and `virtio-net-pci` for 11 and later.
- The in-VM download is of the same order as the full installer (roughly 12 to 18 GB depending on version), so on a typical home connection the install is dominated by download time, and a dropped connection restarts a long step. A blind user gets no feedback unless the progress screen is exposed; orchord should plan for a way to report progress or offer the local-image path.
- Disk space during install is at least the download plus the installed system, which is why Apple's Big Sur requirement jumped to 35.5 GB.

### Gaps
- No sourced figures for in-VM install duration or reliability were found.
- Whether QEMU user-mode (slirp) networking on Windows is reliable for a 12 to 18 GB download was not researched.
- No per-version primary source (Apple) for which paravirtual drivers ship in each macOS; all evidence is from project code and wikis.

## 6. Boot chain: OpenCore, firmware, kexts, SMBIOS, OSK, SIP/SecureBootModel

### Takeaway
The de facto chain is OVMF (UEFI) firmware, an OpenCore disk image, and a small kext set (Lilu, VirtualSMC, WhateverGreen plus input/USB helpers), with SMBIOS iMac19,1, `SecureBootModel=Disabled`, SIP left enabled, and QEMU's `isa-applesmc` device carrying Apple's OSK string. quickemu simply downloads OSX-KVM's prebuilt `OpenCore.qcow2` and OVMF files from a pinned commit; a Hyper-V route replaces most of this with MacHyperVSupport and needs only Lilu and VirtualSMC as required kexts.

### Cited Findings
- OSX-KVM machine definition: `-machine q35`; `qemu-xhci` with `usb-kbd` and `usb-tablet`; `usb-ehci`; `-device isa-applesmc,osk="..."`; pflash `OVMF_CODE_4M.fd` (read-only) and `OVMF_VARS-1920x1080.fd`; `-smbios type=2`; `ich9-intel-hda` + `hda-duplex`; `ich9-ahci` with three `ide-hd` disks (OpenCore.qcow2 with `snapshot=on`, BaseSystem.img raw, mac_hdd_ng.img qcow2); `virtio-net-pci` on user networking; `-device vmware-svga`. — [OSX-KVM OpenCore-Boot.sh](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore-Boot.sh)
- OSX-KVM OpenCore config (parsed 2026-10-09): enabled kexts Lilu, VirtualSMC, WhateverGreen, VoodooPS2Controller + VoodooPS2Keyboard (MinKernel 15.0.0), AppleMCEReporterDisabler (MinKernel 21.0.0, i.e. Monterey+), USBToolBox + UTBMap (MinKernel 20.0.0, i.e. Big Sur+). Only enabled kernel patch: "Enable TRIM/discard on QEMU SATA SSD" (MinKernel 11.0.0). Kernel quirks: `ProvideCurrentCpuInfo`, `DisableLinkeditJettison`, `ForceSecureBootScheme`. Booter quirks: `AvoidRuntimeDefrag`, `EnableWriteUnprotector`. ACPI: SSDT-EC, SSDT-USBX. UEFI drivers: OpenCanopy, OpenHfsPlus, OpenRuntime, ResetNvramEntry, ToggleSipEntry. Security: `SecureBootModel=Disabled`, `DmgLoading=Signed`, `Vault=Optional`, `ScanPolicy=0`. NVRAM: `boot-args=keepsyms=1`, `csr-active-config=00000000` (SIP enabled), `run-efi-updater=No`. PlatformInfo: `SystemProductName=iMac19,1`, `UpdateSMBIOSMode=Create`. — [OSX-KVM config.plist](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore/config.plist)
- quickemu downloads its boot components from OSX-KVM pinned to commit `da4b23b5e92c5b939568700034367e8b7649fe90`: `OpenCore/OpenCore.qcow2`, `OVMF_CODE.fd`, `OVMF_VARS-1920x1080.fd`; a comment notes OVMF_CODE.fd was removed from OSX-KVM master on 26 January 2026. — [quickget](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickget)
- quickemu macOS device choices: `vmware-svga` display with 256 MB VRAM ("aligns with OSX-KVM"); xHCI for Big Sur and later, EHCI for older; `usb-kbd` and `usb-tablet`; `-global ICH9-LPC.disable_s3=1`; `-global ICH9-LPC.acpi-pci-hotplug-with-bridge-support=off`; HPET off; bootloader and recovery image on AHCI. — [quickemu script](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu)
- OSK handling: OSX-KVM and Docker-OSX embed the 64-character OSK string in clear text in the QEMU command line; quickemu stores it ROT13-encoded and decodes it with `tr` at runtime. — [OSX-KVM OpenCore-Boot.sh](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore-Boot.sh), [Docker-OSX Dockerfile](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/Dockerfile), [quickemu script](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu)
- OSX-KVM's stated justification: the OSK string "is widely available on the Internet" and appears in a public court document; "it seems that Apple's attempt(s) to get the OSK string treated as a trade secret did not work out. Due to these reasons, the OSK string is freely included in this repository." — [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)
- Hyper-V route (MacHyperVSupport, "all Intel macOS versions"): Generation 2 VMs on Windows 8.1 / Server 2012 R2 or newer. Kexts: `MacHyperVSupport.kext` (10.4 to 11.0), `MacHyperVSupportMonterey.kext` (12.0 and newer), `MacHyperVFramebuffer.kext`. Required kexts: Lilu and VirtualSMC. SSDTs: SSDT-HV-VMBUS, SSDT-HV-DEV (Windows 10 / Server 2019 and newer), SSDT-HV-PLUG (Big Sur and newer). Booter quirks `AvoidRuntimeDefrag`, `ProvideCustomSlide`; kernel quirk `ProvideCurrentCpuInfo`; UEFI quirk `DisableSecurityPolicy` on Windows 10 / Server 2019 and newer. Daemons for shutdown, time sync and file copy. No audio device appears in its supported list. — [MacHyperVSupport README](https://raw.githubusercontent.com/acidanthera/MacHyperVSupport/master/README.md)
- OpenCore documents that with a valid non-Disabled `SecureBootModel` only macOS DMG recovery can load until the OS is personalised, and `DmgLoading` must be `Signed`. — [OpenCorePkg Configuration.tex](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Docs/Configuration.tex)
- FAT32 from Python: pyfatfs (MIT) implements FAT12/16/32 with VFAT long names in pure Python. Dortania's manual procedure needs only a FAT32 volume containing `EFI/` and `com.apple.recovery.boot/`. — [PyPI pyfatfs](https://pypi.org/pypi/pyfatfs/json), [Dortania Windows install guide](https://dortania.github.io/OpenCore-Install-Guide/installer-guide/windows-install.html)

### Inferences
- An OpenCore EFI disk image can be produced on Windows with only Python: write a GPT or MBR header plus one FAT32 partition (pyfatfs), copy `EFI/BOOT/BOOTx64.efi`, `EFI/OC/*` from the official OpenCore release zip, kexts from their acidanthera releases, and a generated `config.plist` (Python `plistlib`). I did not build one in this session.
- OSX-KVM ships `SystemProductName=iMac19,1` for every version including Tahoe, although the 2019 iMac is not on Apple's Tahoe list. The most likely reason this works is that macOS relaxes board-ID compatibility checks when the hypervisor CPUID bit is present; this is from memory and not sourced. If it fails on a given hypervisor, the Tahoe-valid identities are those of the four supported Intel Macs (for example MacPro7,1 or iMac20,1).
- iMacPro1,1 and MacPro7,1 are commonly chosen for VMs because they have no integrated GPU expectations (from memory, not sourced here); OSX-KVM's current choice is iMac19,1.
- WhateverGreen, VoodooPS2 and USBToolBox are conveniences in OSX-KVM's image, not requirements; the minimal required set per MacHyperVSupport's documentation is Lilu + VirtualSMC. Whether VirtualSMC alone is sufficient in QEMU without the `isa-applesmc` device (which would avoid putting the OSK string on a command line) was not confirmed.
- Redistributing OSX-KVM's prebuilt `OpenCore.qcow2` and OVMF binaries is legally unclear because the repository has no license file; building the OpenCore image from official acidanthera releases (BSD-3-Clause) and taking OVMF from an EDK2 or QEMU distribution avoids that question.
- `SecureBootModel=Disabled` with SIP enabled is the working baseline. Big Sur and earlier would need SIP relaxed only if VoodooHDA or the unsigned Hyper-V framebuffer kext must be installed.

### Gaps
- Dortania's per-version SMBIOS support table was not fetched; which SMBIOS each version accepts on bare metal is not documented here beyond the Tahoe model list.
- No source was retrieved for AppleALC versus VoodooHDA in QEMU beyond quickemu's audio table; AppleALC was not present in OSX-KVM's config.
- Whether QEMU's Windows builds include `isa-applesmc`, `vmware-svga`, `vmxnet3` and `virtio-sound-pci`, and whether macOS boots under WHPX at all, is outside this note's scope and unverified.
- DarwinKVM (royalgraphx) and UTM were not examined.
- The exact EDK2/OVMF version requirements (4M variant, NVRAM variable store layout) were not researched.

## 7. Apple's license position and how existing projects handle it

### Takeaway
Apple's macOS license (verified in the Tahoe 26 text) permits installation only on Apple-branded computers and virtualization only on an Apple-branded computer already running macOS, so running macOS in a VM on a Windows PC is outside the license; projects respond by not redistributing Apple software, downloading from Apple at run time, and placing EULA responsibility on the user. The one documented enforcement against such a project is Apple's 2024 copyright complaint that removed Docker-OSX images (which contained macOS installer content) from Docker Hub; the source repositories remain on GitHub.

### Cited Findings
- Tahoe license title: "SOFTWARE LICENSE AGREEMENT FOR macOS Tahoe 26 — For use on Apple-branded Systems". Section 2A grants a license "to install, use and run one (1) copy of the Apple Software on a single Apple-branded computer at any one time". — [Apple macOS Tahoe SLA (PDF)](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf)
- Section 2B(i): for Mac App Store / automatic downloads, "to download, install, use and run for personal, non-commercial use, one (1) copy of the Apple Software directly on each Apple-branded computer running macOS Sequoia, macOS Sonoma, macOS Ventura, macOS Monterey, macOS Big Sur, or macOS Catalina ('Mac Computer') that you own or control". — [Apple macOS Tahoe SLA (PDF)](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf)
- Section 2B(iii): "to install, use and run up to two (2) additional copies or instances of the Apple Software, or any prior macOS or OS X operating system software or subsequent release of the Apple Software, within virtual operating system environments on each Apple-branded computer you own or control that is already running the Apple Software, for purposes of: (a) software development; (b) testing during software development; (c) using macOS Server; or (d) personal, non-commercial use." — [Apple macOS Tahoe SLA (PDF)](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf)
- Section 2D: "the Apple Software is supported on only Apple-branded hardware that meets specified system requirements as indicated by Apple." — [Apple macOS Tahoe SLA (PDF)](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf)
- Dortania's OpenCore guide calls hackintoshing a "legal grey area": not illegal in its view but a breach of Apple's EULA; it points out macOS is downloaded directly from Apple's servers and that the work is non-profit, for teaching and personal use; it directs commercial users to the Psystar case and regional law; it warns that sites repackaging or modifying macOS installers may face DMCA takedowns; and it states "This is not legal advice". — [Dortania: Why OpenCore, Legality of Hackintoshing](https://dortania.github.io/OpenCore-Install-Guide/why-oc.html)
- OSX-KVM's "Is This Legal?" section: links Dortania's legality text, Gabriel Somlo's notes and an AWS EC2 Mac announcement, then says "It is your responsibility to understand, and accept (or not accept) the Apple EULA" and "This is not legal advice, so please make the proper assessments yourself and discuss with your lawyers if you have any concerns". It also states all blobs in the repository are re-derivable. — [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)
- Docker-OSX frames itself around security research ("Conduct Security Research on macOS using both Linux & Windows", Apple Bug Bounty) and links an article "Is Hackintosh, OSX-KVM, or Docker-OSX legal?". — [Docker-OSX README](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/README.md)
- Docker-OSX takedown: users saw pull errors from 26 August 2024; the developer wrote that "Apple reported the Docker-OSX repository to Docker Hub saying that it violated Apple copyright"; Docker Hub moderated the repository; the developer said he received no prior notice. Article dated 29 August 2024. — [SecurityOnline](https://securityonline.info/docker-osx-shut-down-apples-copyright-claim-impacts-macos-containers/)
- Per iThome's report, Docker said Apple's law firm asserted exclusive rights in the macOS installer and that the Docker-OSX images reproduced Apple content without authorization, and asked for the installer and installation images to be removed. — [iThome](https://www.ithome.com.tw/news/164805)
- The Docker-OSX GitHub repository is not archived (last push 2025-11-11, about 52,959 stars); OSX-KVM is not archived (last push 2026-01-26, about 23,702 stars). Checked 2026-10-09. — [GitHub API: Docker-OSX](https://api.github.com/repos/sickcodes/Docker-OSX), [GitHub API: OSX-KVM](https://api.github.com/repos/kholia/OSX-KVM)
- Apple v. Corellium (iOS virtualization for security research, not macOS): Apple sued in 2019 alleging copyright infringement and DMCA violations; in December 2020 the district court found Corellium's use was fair use on the copyright claim; the DMCA claim and counterclaims were settled confidentially on 10 August 2021; the Eleventh Circuit upheld fair use (reports differ on whether parts were remanded); a full confidential settlement followed in December 2023. — [The Register, Dec 2023](https://www.theregister.com/2023/12/18/apple_corellium_case_settles/), [9to5Mac](https://9to5mac.com/2023/12/15/apple-vs-corellium-case-settled-again/), [Law Street Media](https://lawstreetmedia.com/tech/apple-files-appeal-after-partly-settling-corellium-copyright-dispute/)

### Inferences
- The common pattern across surviving projects: ship no Apple binaries, fetch the recovery image from Apple's own servers on the user's machine at run time, state plainly that Apple's license restricts macOS to Apple hardware, and leave acceptance to the user. The Docker-OSX episode targeted redistributed images containing macOS, not download scripts or source code. orchord's "download from Apple" design matches the lower-risk pattern.
- The OSK string and Apple-derived firmware/kext content are the other items some projects redistribute. OSX-KVM argues the OSK is public; quickemu obscures it with ROT13. Neither approach has a cited legal test.
- Corellium is of limited relevance: it concerned iOS, a commercial product, and a fair-use finding tied to security research; the DMCA anti-circumvention question was never adjudicated because it settled. It does not establish that running macOS on non-Apple hardware is permitted.
- For wording, orchord can state facts neutrally: Apple's license grants use only on Apple-branded computers; the tool downloads software directly from Apple and does not distribute it; the user is responsible for compliance with the license and local law; this is not legal advice.

### Gaps
- Current status of the `sickcodes/docker-osx` images on Docker Hub in 2026 was not verified (the README still references them).
- No reports of DMCA or other legal action against OSX-KVM, quickemu, OpenCore/acidanthera, gibMacOS or UTM were found; absence in my searches is not proof of absence.
- UTM's and quickemu's own legal wording were not retrieved (quickemu's wiki has no legal section in the page fetched).
- Apple v. Psystar details (Dortania cites it) were not sourced here; from memory, Apple prevailed against a company selling non-Apple computers with macOS preinstalled.
- The Sequoia and older SLA texts were not read; the Tahoe text is the one quoted. The Dortania guide cites sections 2B-i and 2B-iii of the Big Sur license for the same restriction.
- Jurisdiction-specific enforceability of the license restriction (for example in the EU) was not researched.

## 8. Recommended VM resource allocations, and what is sensible on a 16 GB host with 2 P-cores + 8 E-cores

### Takeaway
Project defaults are small (OSX-KVM 4 GB RAM with 2 cores/4 threads; Docker-OSX 4 GB with 4 cores) while quickemu enforces or warns at 8 GB for macOS; disks are thin-provisioned at about 100 to 256 GB with about 20 GB as the bare minimum installed footprint. On the i7-1355U with 6 to 7 GB free, quickemu's 8 GB guidance cannot be met without freeing host memory, so older or mid versions at 4 to 6 GB and 2 to 4 vCPUs on P-cores are the realistic starting point.

### Cited Findings
- OSX-KVM defaults: `ALLOCATED_RAM="4096"` MiB, `CPU_SOCKETS=1`, `CPU_CORES=2`, `CPU_THREADS=4` (`-smp 4,cores=2,sockets=1`); disk `qemu-img create -f qcow2 mac_hdd_ng.img 256G` in the README (128G in the script comment); "Create this HDD image file on a fast SSD/NVMe disk for best results". — [OSX-KVM OpenCore-Boot.sh](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore-Boot.sh), [OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)
- Docker-OSX defaults: `ENV RAM=4` (QEMU `-m ${RAM:-4}000`), `-smp ${SMP:-4},cores=${CORES:-4}`; the README separately says "The default is 3 Gigabytes: -e RAM=3" (internal inconsistency). Disk: about 200 GB thin image ("the guest OS thinks you have 200GB available"); "20GB+++ disk space for bare minimum installation (50GB if using Xcode)"; "at least 60gb" partition for Xcode 12. — [Docker-OSX Dockerfile](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/Dockerfile), [Docker-OSX README](https://raw.githubusercontent.com/sickcodes/Docker-OSX/master/README.md)
- quickemu: minimum RAM for macOS guests is 8G (exits with an error if RAM was auto-selected below that, only warns if the user set it); core counts are rounded down to a power of two; Monterey gets `cpu_cores=2` in the generated config; the default disk appears to the guest as about 103 GB. — [quickemu script](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickemu), [quickget](https://raw.githubusercontent.com/quickemu-project/quickemu/master/quickget), [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)
- "macOS VMs should be allocated at least 8GB of RAM for reasonable performance" (quickemu maintainer). — [quickemu discussion #1372](https://github.com/orgs/quickemu-project/discussions/1372)
- Core guidance for unaccelerated desktops: 4 to 6 physical cores maximum, P-cores only, one thread per physical core, and do not over-allocate vCPUs. — [OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)
- Apple minimum memory: 2 GB (High Sierra, Mojave), 4 GB (Catalina, Big Sur). Apple minimum free storage: 14.3 GB, 12.5 GB, 12.5 GB, 35.5 GB respectively. — [Apple SP765](https://support.apple.com/kb/SP765), [Apple SP777](https://support.apple.com/kb/SP777), [Apple SP803](https://support.apple.com/kb/SP803), [Apple SP833](https://support.apple.com/kb/SP833)
- Guest-side savings: disabling automatic updates keeps the thin disk image from growing; `sudo trimforce enable` lets the image be compacted; an OSX-KVM kernel patch enables TRIM on the QEMU SATA SSD for Big Sur and later. — [osx-optimizer README](https://raw.githubusercontent.com/sickcodes/osx-optimizer/master/README.md), [quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines), [OSX-KVM config.plist](https://raw.githubusercontent.com/kholia/OSX-KVM/master/OpenCore/config.plist)

### Inferences
Proposed orchord defaults (my synthesis from the figures above; not from any single source, and untested):

| Version | RAM minimum | RAM default | vCPUs default | Disk minimum | Disk default (thin) |
|---|---|---|---|---|---|
| High Sierra 10.13, Mojave 10.14 | 2 GB (Apple) | 4 GB | 2 | 32 GB | 64 GB |
| Catalina 10.15 | 4 GB (Apple) | 4 GB | 2 | 32 GB | 64 GB |
| Big Sur 11, Monterey 12 | 4 GB | 6 GB | 2 to 4 | 64 GB | 80 to 96 GB |
| Ventura 13, Sonoma 14, Sequoia 15 | 4 GB (expect heavy swapping) | 8 GB | 4 | 64 GB | 96 GB |
| Tahoe 26 | 6 GB | 8 GB | 4 | 80 GB | 128 GB |

- On the dev machine (16 GB, 6 to 7 GB free), an 8 GB guest will force the Windows host to page unless other applications are closed; 4 GB (older versions) or 6 GB (Big Sur/Monterey) fits the free memory. The wizard should compute a ceiling from free physical memory and warn rather than block.
- With only two P-cores, default to 2 vCPUs for the lightest footprint or 4 vCPUs mapped to the four P-core threads; do not default to 8 or 12. If the hypervisor cannot pin threads, fewer vCPUs reduces the chance that the rendering path lands on E-cores.
- Disk: thin provisioning means the default size costs little up front; both NVMe drives (190 GB and 476 GB free) are adequate. Put the image on NVMe as OSX-KVM advises. Budget roughly 20 GB installed plus the 12 to 18 GB in-VM download during installation.
- Because the full Tahoe installer alone is about 17 GiB, a 32 GB disk is too small for Tahoe; that is the reason for the larger minimum in the table.

### Gaps
- No source gives per-version RAM/core recommendations for unaccelerated VMs; community defaults are version-agnostic.
- Actual idle memory use of each macOS version in a VM was not found; the table's minimums for Ventura and later are estimates.
- Behaviour of macOS guests on a hybrid CPU under an active Hyper-V root partition (scheduler placement, ability to pin) is unverified and needs measurement on the dev machine.
