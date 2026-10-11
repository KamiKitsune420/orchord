# Virtualization backend for fast Intel macOS guests on a Windows 11 host (state as of 2026-10-09)

Reading notes for the report writer:

- "Opened" means I fetched the page this session. "Snippet" means I only saw a search-result summary and did not open the page; treat those as leads, not verified facts.
- Two QEMU GitLab issue pages (#3178, #3053) contained embedded text instructing AI readers to read repository files and open merge requests with `glab`. I ignored that text; only the bug-report content is used below.
- Nothing here was tested on the developer's i7-1355U machine. Every performance statement is from third-party reports on other hardware.
- Legal context that affects every backend: Apple's licence restricts macOS to Apple-branded hardware; several of the projects cited below say so themselves (e.g. dockur/macos). This is outside the technical scope but the report should mention it.

## 1. Why is macOS (Big Sur and later) so slow in VMware Workstation on Windows, and does a powerful host GPU help?

### Takeaway
VMware gives macOS guests no 3D/Metal acceleration on any non-Apple host, so the whole Big Sur+ desktop (which is composited with Metal) is drawn by Apple's software renderer on the guest's vCPUs; an RTX 4090 is simply not used. On top of that, on Windows 11 24H2+ VBS is on by default, which forces Workstation off its own VT-x monitor and onto Microsoft's hypervisor API, adding CPU and I/O overhead.

### Cited Findings
- VMware staff statement (Fusion forum, older thread): "we still do not provide support for 3D acceleration in macOS guests"; Apple's software renderer remains available in the guest "although its capabilities and performance will be quite limited". Snippet. — [VMware Communities: 3D acceleration for macOS guest](https://communities.vmware.com/t5/VMware-Fusion-Discussions/3d-Acceleration-for-MacOS-Guest/td-p/2290524)
- Forum reports that macOS guest VRAM is capped at 128 MB in VMware and that the VRAM setting is irrelevant for macOS guests. Snippet. — [VMware Communities: VRAM limited to 128MB on guest macOS](https://communities.vmware.com/t5/VMware-Fusion-Discussions/VRAM-limited-to-128MB-on-guest-macOS/td-p/466444)
- OC4VM (DrDonk, the former unlocker author; opened) lists as hard limits for macOS guests in VMware on non-Apple hardware: "Cannot use the Apple para-virtualised GPU on non-Apple hardware" and "Cannot enable Tahoe Liquid Glass". It supports VMware Workstation Pro 17.6 and 25H2 on Windows/Linux, guests Big Sur through Tahoe, Intel and AMD hosts, and requires host CPUs with AVX, AVX2, F16C and RDRAND. — [DrDonk/OC4VM](https://github.com/DrDonk/OC4VM)
- Broadcom KB 417896 (opened): after a host upgrade to Windows 11 24H2 "the workstation Virtual Machines' performance has degraded" in all Workstation 17.x builds, because 24H2 enables VBS by default; Windows then runs a hypervisor beneath the OS, Workstation "can't access hardware virtualization directly", giving CPU overhead, slower I/O and reduced responsiveness. Fix offered: upgrade to 25H2 (which adds a Hyper-V/WHP mode indicator) or turn off Memory Integrity, remove Hyper-V / Virtual Machine Platform / Windows Hypervisor Platform / Sandbox / WSL, and run `bcdedit /set hypervisorlaunchtype off`. — [Broadcom KB 417896](https://knowledge.broadcom.com/external/article/417896/)
- VMware's own 2020 announcement: from Workstation 15.5.5 the monitor runs at user level and drives guests through the Windows Hypervisor Platform API when Hyper-V/VBS is present ("Host VBS mode"). Snippet. — [VMware blog: Workstation now supports Hyper-V mode](https://blogs.vmware.com/workstation/2020/05/vmware-workstation-now-supports-hyper-v-mode.html)
- VMware community post: when Hyper-V is uninstalled, Workstation goes back to its root VMM instead of the User Level Monitor. Snippet. — [VMware Communities: Nested hypervisor support under VBS](https://communities.vmware.com/t5/VMware-Workstation-Pro/Nested-hypervisor-support-under-VBS/m-p/2875567)
- The "unlocker" is a community patch to Workstation binaries, not a performance component. The widely circulated Unlocker 3.0.2 targets Workstation 11-15 and on Workstation 26.x "fails at every stage and still exits reporting success" (wrong registry hive, `vmware-vmx.exe` moved to an `x64\` subfolder, dead `softwareupdate.vmware.com` Tools downloads). BDisp/unlocker 3.1.4 fixes this for 26H1. Validated on Workstation 26.0.0 build 25388281 (2026-07-22) with Ventura 13.7.8, Sequoia 15.7.9 and Tahoe 26.6.1 (2026-08-11). A Workstation product update overwrites the patched binaries and silently removes macOS support. Opened. — [aesiddiqui/macos-on-vmware-workstation](https://github.com/aesiddiqui/macos-on-vmware-workstation)
- Conflicting claim: a March 2026 how-to says enabling "Accelerate 3D graphics" with 2 GB graphics memory makes Sequoia's Dock and animations smoother in Workstation. Snippet; no measurement, contradicts the VMware staff statement and OC4VM. Treat as unverified. — [itech4mac](https://www.itech4mac.net/2026/03/how-to-fix-slow-macos-sequoia-on-vmware-workstation-windows-host/)

### Inferences
- The friend's 32 GB / RTX 4090 machine was slow for a reason hardware cannot fix: the guest renders on CPU. More vCPUs help a little; the GPU does nothing.
- Big Sur is a bad starting point: it is the first release whose UI leans heavily on Metal-composited translucency. Catalina and earlier are lighter under software rendering (see section 6).
- On the developer's laptop VBS is active, so Workstation 26 is certainly running in WHP mode. That means VMware, QEMU/WHPX and native Hyper-V all sit on the same Microsoft hypervisor there; VMware has no raw-VT-x advantage unless VBS is turned off.
- VMware as an orchord backend is technically drivable (vmrun/vmrest) but depends on binary-patching a proprietary product, breaks on every Workstation update, and cannot be redistributed. Poor fit for an open-source project.

### Gaps
- No measured benchmark of a macOS guest in Workstation with VBS on versus off was found.
- I did not verify how much of the friend's slowness was WHP mode versus software rendering; both apply.
- I did not confirm VMware Tools (darwin.iso) behaviour on Tahoe beyond guides saying to install it.
- vmrun/vmrest automation of unlocked macOS guests was not researched in depth.

## 2. QEMU on Windows with the WHPX accelerator: does it boot and run macOS guests in 2025-2026?

### Takeaway
Yes, with caveats: there is a documented Monterey boot on WHPX from 2023 and a Tahoe 26.7.1 boot to desktop on Windows 11 25H2 dated 2026-10-09, but WHPX for x86 was neglected for years and only received serious work in QEMU 11.0 (April 2026) and the 11.1 cycle. Use the newest QEMU available; 10.x and earlier have real bugs, and there is no published KVM-versus-WHPX benchmark for macOS.

### Cited Findings
- Verified working report, 2023-02-13: QEMU 7.2.0, Windows 10, "2018 Intel i5", macOS Monterey, `-accel whpx`, `-M q35`, `-cpu Westmere` plus extra flags, 4096 MB, OpenCore + recovery image, user-mode networking. Author notes: "the system will freeze without +xsave"; WHPX crashed with separate OVMF CODE and VARS files so they were merged into one file with `copy /B`; recovery image had to be fetched with the OSX-KVM script. Described as good for "testing features and obtaining screenshots", not "a reliable system". No benchmarks. Opened. — [digitalfoundations: macOS on Windows with QEMU](https://www.digitalfoundations.com.au/blog/macos-on-windows-with-qemu/)
- Verified working report, 2026-10-09: Windows 11 Pro build 26200 (25H2), Intel Core Ultra 9 185H (a hybrid P/E-core CPU), 64 GB RAM. QEMU 11.0.50 built from the MAXZVER fork under MSYS2 UCRT64, accelerators `tcg | whpx`. Guest macOS Tahoe 26.7.1 (25G241) converted from a VMware disk, booted with OSX-KVM's OpenCore and OVMF, `-smp 8,cores=8,sockets=1 -m 16G -cpu Skylake-Client,...`. Baseline run with plain `vmware-svga` (no GPU acceleration): "Boots, logs in, SSH works. No issues." Opened. — [MAXZVER/reims-vgpu issue #1](https://github.com/MAXZVER/reims-vgpu/issues/1)
- Same fork's README: tested on Intel Core i7-14700K (hybrid) + RTX 4060 with a macOS 13 Ventura guest, 8 vCPUs, 12 GB RAM; WHPX fixes were also validated on an AMD Ryzen host by the port's author. Opened. — [MAXZVER/reims-vgpu](https://github.com/MAXZVER/reims-vgpu)
- QEMU 11.0.0 was released 2026-04-22 and names "improved support for MSHV and WHPX accelerators" as a headline item. Snippet. — [QEMU 11.0.0 release announcement](https://www.qemu.org/2026/04/22/qemu-11-0-0/)
- QEMU 11.0 WHPX documentation (stable-11.0, opened): the `-cpu type` option is ignored in this release under WHPX; gdbstub/`info registers` do not save or restore xsave state; MMIO accessed with MMX/SSE/AVX instructions is not emulated (MMX warns, newer instructions fail to decode); legacy VGA modes are slow; the legacy-PIC-does-not-wake-HLT bug is fixed on Windows 11 but not Windows 10 (workaround `-M kernel-irqchip=off`); nested-virtualisation Hyper-V enlightenments are not exposed. — [QEMU 11.0 WHPX docs](https://qemu.readthedocs.io/en/stable-11.0/system/whpx.html)
- QEMU master WHPX documentation (opened): x86_64 tested from Windows 10 2004; enable with `DISM /online /Enable-Feature /FeatureName:HypervisorPlatform /All`; quick start `-accel whpx -M pc -smp cores=2 -m 2G ...`; `-accel whpx,ssd=off` disables the separate security domain (a BTB flush on every guest entry/exit) for a "significant MMIO performance increase" at the cost of security mitigations; `-trace whpx_unsupported_msr_access` logs undocumented MSR accesses; `-M kernel-irqchip=` "is not expected to be manually set during normal operation". — [QEMU master WHPX docs](https://www.qemu.org/docs/master/system/whpx.html)
- "[PATCH v3 00/37] WHPX x86 updates for QEMU 11.1" by Mohamed Mediouni, 2026-04-22 (opened): adds emulation of CPU models on Windows 10 and 11 ("whpx: i386: introduce proper cpuid support"), xsave save/restore, user-mode x2APIC emulation "for better performance when it has to be used" with `kernel-irqchip=off`, configurable handling of unknown MSRs, optional interception of #GP on MSR access, TSC/APIC frequency exposed through the VMware CPUID leaf when `hyperv=off`, nested-virt settings, and the `ssd=off` option. The author says he may step aside for a while. — [qemu-devel archive: WHPX x86 updates for QEMU 11.1](https://ratatoskr.run/qemu-devel/2026/04/14442356/t)
- QEMU 11.0 soft-freeze pull request (March 2026) included "whpx: make Hyper-V enlightenments configurable" and "whpx: i386: enable some more enlightenments". Snippet. — [patchew: i386 fixes for QEMU 11.0 soft freeze](https://patchew.org/QEMU/20260309185752.76865-1-pbonzini@redhat.com)
- Bug, QEMU 10.0.3, Windows 11 22H2: with `-accel whpx,kernel-irqchip=off`, Broadwell, Haswell and "any other CPU architectures except Westmere" freeze at the bootloader or run unstably; adding `-tsc` to the CPU model (e.g. `-cpu Haswell,-tsc`) lets the guest boot. Linux guest, not macOS. No root cause given. Opened. — [QEMU GitLab #3053](https://gitlab.com/qemu-project/qemu/-/issues/3053)
- Bug, QEMU 10.0.6 / 10.1.2 / master, Windows 11 25H2 build 26200.6901: `-M pc,accel=whpx,kernel-irqchip=off -smp cores=2` hangs before SeaBIOS POST; bisected to commit df32e5c5. No fix listed on the page. Opened. — [QEMU GitLab #3178](https://gitlab.com/qemu-project/qemu/-/issues/3178)
- Forum report: `host` is not a valid CPU type under WHPX on QEMU 8 / Windows 11 (the poster substituted `kvm64`), and stock OVMF gave trouble so they used Proxmox firmware. FreeBSD guest. Snippet. — [FreeBSD Forums: QEMU with WHPX on Windows 11](https://forums.freebsd.org/threads/how-to-virtualize-freebsd-13-2-on-windows-11-with-qemu-kvm-and-the-whpx-hypervisor.89724/)

### Inferences
- The `ssd=off` option appears in the master documentation, so the 11.1 series (or at least that part of it) has been merged. I could not confirm whether QEMU 11.1 has been released as a stable version.
- The CPU-model story is the main technical risk. Under QEMU 11.0 `-cpu` is ignored on WHPX, so the guest sees the host's real CPUID. macOS does not recognise Raptor Lake; OSX-Hyper-V handles the same situation by spoofing Comet Lake via an OpenCore CPUID patch for "Tiger Lake and newer" (section 4). So on QEMU 11.0 the i7-1355U will need OpenCore `Cpuid1Data` spoofing; on builds with the 11.1 CPUID work, a QEMU-side `-cpu Skylake-Client`/`Haswell` model may be honoured instead. The 2026-10 Tahoe report passed `-cpu Skylake-Client` on an 11.0.50 build and booted, which is consistent with either.
- The two 2026 success reports are both on hybrid Intel CPUs (14700K, Ultra 9 185H), which is the only evidence found about hybrid hosts. It suggests P/E cores are not a blocker under WHPX, but neither is a U-series part with only 2 P-cores.
- `kernel-irqchip=off` was the folk workaround for years and is now the source of hangs (#3178) on 10.x. On QEMU 11+ with Windows 11, leave it at the default.
- QEMU is GPLv2; orchord can ship or download a QEMU binary and drive it as a separate process (QMP for control) without licence entanglement of its own code.
- QEMU on Windows provides emulated HD Audio and USB audio devices, which Hyper-V lacks entirely (section 4). For a screen-reader-first product that is a decisive difference.

### Gaps
- No stock, unforked, upstream-QEMU report of a macOS guest on WHPX from 2025-2026 was found. The 2026 evidence is from a fork that carries extra WHPX patches ("mostly pending upstream pull requests").
- No measured WHPX-versus-KVM performance numbers for macOS guests. No source quantifies WHPX overhead.
- No report on a 13th-gen U-series CPU or on Iris Xe.
- I could not open the QEMU 11.0 changelog wiki (blocked by bot protection), so the exact list of WHPX changes in 11.0 versus 11.1 is reconstructed from docs and patch titles.
- Whether QEMU's WHPX needs elevation was not verified this session (my prior understanding is that the WHP API works for standard users once the Windows feature is enabled; enabling the feature needs admin and a reboot).
- Open/closed status and fixes for GitLab #3053 and #3178 were not visible.

## 3. QEMU TCG (pure emulation, no hypervisor): is it viable for macOS?

### Takeaway
I found no credible report of anyone using x86 macOS under TCG as a daily desktop, and no measured numbers; it should be treated as a diagnostic fallback only.

### Cited Findings
- QEMU's own WHPX documentation suggests TCG only as a workaround for specific cases (legacy VGA modes), not as a performance option. — [QEMU master WHPX docs](https://www.qemu.org/docs/master/system/whpx.html)
- Only indirect data points were found: a QEMU bug where a Windows XP guest under TCG hung about three minutes on the welcome screen after QEMU 5.0, and a Haiku guest under TCG described as "rather long boot time and sluggish GUI performance". Snippets; neither is macOS. — [QEMU GitLab #404](https://gitlab.com/qemu-project/qemu/-/issues/404); [Haiku forum](https://discuss.haiku-os.org/t/qemu-on-macos-host-rather-slow-performance-in-haiku-guest/10687)
- OC4VM and dockur/macos both require AVX2-capable CPUs for current macOS guests. — [DrDonk/OC4VM](https://github.com/DrDonk/OC4VM); [dockur/macos](https://github.com/dockur/macos)

### Inferences
- With software rendering already consuming the vCPUs under hardware virtualization, stacking instruction-level emulation underneath (typically an order of magnitude slower than virtualization, and on a 15 W laptop CPU) cannot produce a usable macOS desktop. This is reasoning, not a measurement.
- TCG is still useful to orchord as a debugging aid: if a guest fails under WHPX but boots under TCG, the fault is in the accelerator path.

### Gaps
- No measured boot time or UI responsiveness for macOS 11+ under x86-on-x86 TCG on Windows was found.

## 4. Native Hyper-V as the backend (Qonfused/OSX-Hyper-V and acidanthera/MacHyperVSupport)

### Takeaway
It works for macOS 10.4 through 15, with 26 "in progress", and is scriptable from PowerShell, but Hyper-V exposes no audio device to the guest and only an 8 MB unaccelerated framebuffer. No audio is close to disqualifying for a VoiceOver-first product unless a network-audio workaround is acceptable.

### Cited Findings
- OSX-Hyper-V README (opened): supported range "10.4 to 26.0"; Sequoia down to Lion marked "Supported"; Tahoe marked "In Progress"; Tiger/Leopard/Snow Leopard only by restoring an image after installing a newer release. Licence BSD-3-Clause. 1.2k stars, 116 commits. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V)
- Graphics: MacHyperVFramebuffer synthetic driver, 8 MB VRAM, no acceleration, default 1024x768 (changeable via `SupportedResolutions` in the kext's Info.plist). GPU acceleration only through Discrete Device Assignment, which is available only on Windows Server / Hyper-V Server 2016+, not Windows Pro or Enterprise. Lilu/WhateverGreen patching is not supported. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V)
- Audio: maintainer, 2024-07-11: "Hyper-V unfortunately does not provide audio virtualization." Issue closed as not planned. Workarounds offered: DDA of a device (Server only), BlackHole loopback in the guest plus a network streaming app to the host, or a networked USB audio interface via VirtualHere. — [Qonfused/OSX-Hyper-V issue #9](https://github.com/Qonfused/OSX-Hyper-V/issues/9)
- CPU: Intel desktop Penryn through Comet Lake and mobile Arrandale through Ice Lake run natively; "Tiger Lake and newer (11th Gen+) require spoofing as Comet Lake with a CPUID patch". AMD (15h/16h/17h/19h) needs kernel patches generated by `amd.ps1 --cpu <core_count>` matching the VM's core count. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V)
- RAM: Big Sur and newer need 6-8 GB to boot the installer; the VM script defaults to 8 GB. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V)
- Automation: `scripts\build.ps1` builds the EFI (`dist/EFI.vhdx`); `create-virtual-machine.ps1 -name -version -cpu -ram -size` creates the VM (defaults: latest macOS, 2 cores, 8 GB, 50 GB). Manual recipe: Generation 2 VM, Secure Boot off, 1 GB GPT/FAT32 EFI VHDX at SCSI location 0. Enabling Hyper-V and `New-VHD` need an elevated PowerShell. In-guest `post-install.sh` installs the framebuffer kext and Hyper-V daemons; `optimize-vm.sh` disables Spotlight, reduces disk I/O and turns off animations. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V)
- MacHyperVSupport (opened): "All Intel macOS versions are supported"; Gen 2 needs Windows 8.1 / Server 2012 R2 or newer. Supported devices: heartbeat, guest shutdown, time sync, host-to-guest file copy (the last three need daemons), synthetic keyboard, mouse, network, SCSI, synthetic graphics (full support needs MacHyperVFramebuffer.kext; on macOS 11+ that kext must be installed with kext signing disabled in SIP), partial PCI passthrough. No audio device is listed. Requires Lilu and VirtualSMC, SSDT-HV-VMBUS / SSDT-HV-DEV / SSDT-HV-PLUG, and specific OpenCore quirks (`ProvideCurrentCpuInfo`, `DisableSecurityPolicy`, `AvoidRuntimeDefrag`, `ProvideCustomSlide`). BSD-3-Clause, 277 commits. — [acidanthera/MacHyperVSupport](https://github.com/acidanthera/MacHyperVSupport)
- Host Compute System API (opened): `computecore.dll`, available from Windows 10 October 2018 Update; creates, starts, stops, pauses, saves and restores VMs from a JSON configuration document; compute systems are ephemeral and the caller must set up VHDs and networking itself. — [Microsoft Learn: HCS overview](https://learn.microsoft.com/en-us/virtualization/api/hcs/overview)

### Inferences
- Performance-wise this is probably the lowest-overhead option when VBS must stay on: storage, network and input go through VMBus synthetic devices in the kernel instead of bouncing to a user-mode device model as with QEMU/WHPX or VMware in WHP mode. No benchmark was found to confirm the size of the difference.
- The 8 MB framebuffer and software rendering apply here exactly as in VMware; Hyper-V is not a graphics fix.
- The developer's i7-1355U is 13th gen, so it falls in the "spoof as Comet Lake" group.
- Tahoe being "In Progress" matters: Tahoe is the last Intel macOS, so a backend that never gets Tahoe is capped at Sequoia.
- Windows 11 Home has no Hyper-V role, so a Hyper-V backend excludes Home-edition users; WHPX (Windows Hypervisor Platform feature) is the more widely available route. Not verified this session.

### Gaps
- Latest release version/date of both projects was not visible on the pages.
- I did not verify the exact privilege model for creating and running Hyper-V VMs (my prior understanding: Hyper-V cmdlets/WMI need elevation or membership of the local "Hyper-V Administrators" group; not confirmed this session). HCS permission requirements were not stated on the overview page.
- Whether Hyper-V's Enhanced Session / RDP audio redirection can be used with a macOS guest was not covered by any source (it normally needs guest-side RDP support, which macOS lacks).
- USB passthrough status for macOS guests in Hyper-V was not documented.
- Why Tahoe is still "In Progress" was not stated.

## 5. Other candidates: VirtualBox, VMware as a driven backend, HAXM, WHP-based Rust hypervisors, KVM inside WSL2

### Takeaway
None of these beats QEMU/WHPX or Hyper-V on a Windows 11 host with VBS on. HAXM is dead, crosvm's Windows backends are untested upstream, VirtualBox degrades under Hyper-V, and VMware needs a fragile binary patch. The one notable extra is KVM inside WSL2 (the dockur/macos route), which trades nested-virtualization overhead for the mature Linux/KVM macOS stack.

### Cited Findings
- Intel HAXM: repository archived 2023-01-28, last release v7.8.0, Intel no longer maintains it or accepts patches; retired because "both Microsoft Hyper-V and macOS HVF have added support for Intel Virtual Machine Extensions". — [intel/haxm](https://github.com/intel/haxm)
- crosvm lists WHPX and HAXM as Windows hypervisor backends, both marked "Tested upstream: no"; the page says nothing about UEFI or non-Linux guests. — [crosvm book: hypervisors](https://crosvm.dev/book/hypervisors.html)
- VirtualBox on a Windows host with Hyper-V/VBS active runs in a degraded mode (the "green turtle" icon) through Microsoft's hypervisor, with symptoms of very slow boot, sluggishness, CPU stalls and intermittent launch failures; the recommended fix is disabling Hyper-V. Snippet; about guests in general, not macOS specifically. — [Kicksecure wiki: VirtualBox green turtle issue](https://www.kicksecure.com/wiki/VirtualBox/Green_Turtle_Issue)
- VirtualBox 7.2 (released 2025-08-14) improved x86_64 CPU feature reporting when Hyper-V is the virtualization engine and fixed x86_64-v3 features not being passed to the guest. Snippet. — [Neowin: VirtualBox 7.2.0](https://www.neowin.net/software/virtualbox-720/)
- VMware Workstation 26H1 with BDisp/unlocker 3.1.4: validated July-August 2026 for Ventura, Sequoia and Tahoe; patch is wiped by product updates. — [aesiddiqui/macos-on-vmware-workstation](https://github.com/aesiddiqui/macos-on-vmware-workstation)
- OC4VM is an alternative to the unlocker that boots macOS in VMware through an OpenCore disk image "without patching VMware"; supports Workstation Pro 17.6 and 25H2. — [DrDonk/OC4VM](https://github.com/DrDonk/OC4VM)
- dockur/macos (21.6k stars): QEMU+KVM in a container; on Windows requires Docker Desktop or Podman on Windows 11 "with nested virtualization enabled" (Windows 10 is unsupported because it does not give containers KVM access); guests Big Sur through Sequoia, Tahoe "possible but not recommended, since it runs very slowly"; needs an AVX2 CPU; web viewer on port 8006; audio off by default (`AUDIO: "Y"`); integrates the Reims vGPU on Linux hosts. — [dockur/macos](https://github.com/dockur/macos)
- quickemu and UTM: quickemu's macOS support is a Linux/KVM workflow (it switched macOS guests to `vmware-svga`); UTM is a macOS-host application. Snippets. — [quickemu wiki: create macOS VMs](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines); [UTM v2.2.1 release notes](https://github.com/utmapp/UTM/releases/tag/v2.2.1)

### Inferences
- KVM-in-WSL2 means macOS runs as a nested guest (Microsoft hypervisor -> WSL2 utility VM -> KVM -> macOS). It reuses the best-tested macOS-on-QEMU configuration unchanged, but adds a nesting layer, a WSL/Docker install (the developer has no WSL), and awkward audio and display plumbing back to Windows. It is a reasonable fallback if WHPX proves unreliable, not a first choice.
- cloud-hypervisor and libkrun were not found to have any Windows-host macOS story; I found no evidence either can boot macOS at all (they target Linux/Windows guests with virtio-only device models, and macOS needs AHCI/USB/Apple SMC emulation that QEMU provides). Not directly sourced.
- No WHP-based third-party VMM other than QEMU has the device models macOS requires (see section 7), so there is nothing to reuse there.

### Gaps
- No source specifically about macOS guests on VirtualBox 7.x with Hyper-V active on a Windows host. Community guides exist for Sonoma on VirtualBox, but I did not verify any under Hyper-V mode.
- No measured performance of macOS under KVM nested in WSL2 on Windows 11.
- cloud-hypervisor and libkrun status on Windows was not researched with a primary source.

## 6. GPU acceleration for macOS guests: what exists, and is there any route on a Windows host?

### Takeaway
Until 2026 the honest answer was "none on Windows". There is now one experimental route: Reims vGPU, a QEMU device that macOS's built-in `AppleParavirtGPU.kext` attaches to, with the host translating Metal to Vulkan; a Windows/WHPX port exists but is alpha, tested on essentially one NVIDIA machine, and glitchy. Everything else (VMware SVGA, Hyper-V framebuffer, virtio-gpu) is unaccelerated for macOS.

### Cited Findings
- Upstream QEMU's `apple-gfx` (macOS PV graphics, Phil Dennis-Jordan, v16 2024-12-23) wraps Apple's ParavirtualizedGraphics framework: it targets x86-64 macOS 11+ guests but "only works on macOS hosts" and needs macOS 11+ and a Metal GPU on the host. Snippet. — [patchew: macOS PV Graphics and new vmapple machine type](https://patchew.org/QEMU/20241223221645.29911-1-phil@philjordan.eu/)
- OC4VM: "Cannot use the Apple para-virtualised GPU on non-Apple hardware" (VMware context). — [DrDonk/OC4VM](https://github.com/DrDonk/OC4VM)
- No virtio-gpu 3D driver for macOS guests was found; UTM's notes say virtio GPU acceleration works only for Linux guests with Mesa. Snippet. — [UTM v2.2.1 release notes](https://github.com/utmapp/UTM/releases/tag/v2.2.1)
- Reims vGPU upstream (steelbrain; opened): "an experimental virtual GPU for macOS guests"; no custom kext, the guest's stock `AppleParavirtGPU.kext` attaches to a QEMU device; host decodes the Metal command stream and runs it through Vulkan via `metal2vulkan`. Supported upstream: Linux x86_64/KVM with x86_64 macOS guests (PCI device) and Apple Silicon/HVF. Windows hosts are not mentioned upstream. Status alpha, "research-quality", ABI may change without notice. LGPL-3.0-or-later. 535 stars, 3,222 commits, 21 open issues, 31 open PRs. Ventura is the recommended bring-up guest. — [steelbrain/reims-vgpu](https://github.com/steelbrain/reims-vgpu)
- Windows 11 + QEMU/WHPX port (MAXZVER fork of the above, port credited to @Hi-Jiajun; opened): tested on one machine (i7-14700K + RTX 4060 8 GB), guest macOS 13 Ventura. Lab numbers dated 2026-09-29: at 1920x1080, CSS animation about 92-97 Hz, full-screen scroll about 116-118 Hz, wheel scroll about 88-96 Hz; window drag median about 40 fps; pre-work baseline was "5-7 fps". Guest-RAM import panics the guest on this host so it is disabled (`REIMS_VGPU_GUEST_IMPORT=off`). Needs a companion QEMU fork (MAXZVER/qemu-reims-vgpu), an MSYS2 UCRT64 build, and Docker Desktop to build the UEFI GOP option ROM. "Upstream calls itself alpha, and this fork is no further along than that." 0 stars, no releases; other GPUs untested. Licence statements are inconsistent (LICENSE file LGPL-3.0; crate manifests say GPL-2.0-or-later). — [MAXZVER/reims-vgpu](https://github.com/MAXZVER/reims-vgpu)
- Independent test of that port, 2026-10-09 (Dell XPS 16, Core Ultra 9 185H, RTX 4070 Laptop, Windows 11 26200, Tahoe 26.7.1 guest): guest reports "Apple Paravirtualized Graphics Device", Metal 2; desktop reached and stable; iOS Simulator renders via Metal; but presented frame rate was 0.3-1.9 Hz ("~1 fps"), with stale bands, tearing, missing textures and "?" icons in System Settings. The plain `vmware-svga` baseline on the same setup had "No issues". The reporter relates it to upstream issue #96 ("Tahoe ... slower than unaccelerated"). — [MAXZVER/reims-vgpu issue #1](https://github.com/MAXZVER/reims-vgpu/issues/1)
- dockur/macos ships Reims as its hardware-accelerated graphics option on Linux hosts (Intel/AMD via `/dev/dri`, NVIDIA via the container toolkit). — [dockur/macos](https://github.com/dockur/macos)
- Hyper-V: acceleration only by DDA GPU passthrough on Windows Server; none on Windows 11 Pro. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V)
- Mitigations people use for unaccelerated macOS: OSX-Hyper-V's `optimize-vm.sh` disables Spotlight, reduces disk I/O and turns off animations; OSX-KVM and quickemu use `vmware-svga` as the display device. — [Qonfused/OSX-Hyper-V](https://github.com/Qonfused/OSX-Hyper-V); [OSX-KVM OpenCore-Boot.sh](https://github.com/kholia/OSX-KVM/blob/master/OpenCore-Boot.sh); [quickemu commit: vmware-svga for macOS](https://git.binarymagic.net/AIR/quickemu/commit/9309f782c4399cada70230761fa7e2b7b78e29ad)
- Tahoe is the final macOS release for Intel; macOS 27 is Apple-Silicon only. — [OSXLatitude: macOS Tahoe unveiled](https://osxlatitude.com/articles/news/the-end-macos-tahoe-unveiled-r97/); [DrDonk/OC4VM](https://github.com/DrDonk/OC4VM)
- Tahoe is reported "very slow" in VMs by dockur and cannot enable Liquid Glass in VMware per OC4VM. — [dockur/macos](https://github.com/dockur/macos); [DrDonk/OC4VM](https://github.com/DrDonk/OC4VM)

### Inferences
- GPU passthrough (the classic Linux/KVM answer with AMD cards) has no equivalent in QEMU-on-Windows, and Hyper-V DDA is Server-only, so passthrough is off the table for Windows 11 Pro. This is consistent with the sources above but no single source states the QEMU-on-Windows half.
- For orchord's users the display matters less than for sighted users: a blind user needs a responsive VoiceOver, i.e. low-latency audio and input and enough CPU for the accessibility stack. Software rendering still burns vCPU time, so reducing what gets rendered (Reduce Motion, Reduce Transparency, low resolution such as 1280x720 or 1024x768, disabling Spotlight indexing) directly frees CPU for speech. That makes an unaccelerated guest more tolerable here than the friend's sighted experience suggests.
- Guest version choice is a first-order performance lever: Ventura is the version both Reims codebases recommend and test; Tahoe is the slowest and the least compatible. Sequoia/Ventura (or Catalina/Mojave if app requirements allow) are the sensible defaults.
- Reims on the developer's laptop would run on Iris Xe, which has never been reported tested with it. Treat Reims as an optional experiment behind a flag, not a dependency.
- Because Reims hooks Apple's own in-box paravirtual GPU driver, it is the only design that can deliver Metal without a custom macOS kernel driver. If it matures, it plugs into a QEMU backend and not into Hyper-V or VMware. That is a strategic reason to choose QEMU.

### Gaps
- No independent confirmation of the fork's 90-118 Hz Ventura numbers; the only third-party test (Tahoe) got about 1 fps.
- No data on Reims with Intel or AMD GPUs on Windows.
- No authoritative statement found on whether VMware's "Accelerate 3D graphics" checkbox has any effect for macOS guests in Workstation 25H2/26H1 (sources conflict; see section 1).
- Display-path comparisons (SDL versus GTK versus VNC/SPICE on Windows) for macOS guests: no measurements found.

## 7. How realistic is writing a new x86 emulator/hypervisor from scratch versus building on QEMU?

### Takeaway
Not realistic as a route to "fast": on a VBS-enabled Windows host a new VMM would have to use the same Windows Hypervisor Platform API that QEMU already uses, so it could not be faster at the CPU level, and it would need to re-create a large pile of firmware and device emulation that QEMU already ships. The target is also frozen, since Tahoe is the last Intel macOS.

### Cited Findings
- The minimum device set a working macOS VM uses today (from OSX-KVM's boot script): UEFI firmware (OVMF code + vars), Q35 chipset, Apple SMC device with the OSK key (`isa-applesmc,osk=...`), AHCI/SATA disks, xHCI/EHCI USB with HID keyboard and tablet, Intel HD Audio, a NIC (virtio-net or vmxnet3), a framebuffer device (`vmware-svga`), SMBIOS tables, plus the OpenCore bootloader image. — [OSX-KVM OpenCore-Boot.sh](https://github.com/kholia/OSX-KVM/blob/master/OpenCore-Boot.sh)
- Even Hyper-V, with Microsoft's complete VMM, needed a dedicated kext package (MacHyperVSupport: VMBus, storage, network, input, framebuffer drivers), three custom SSDTs, kernel patches (`_hpet_init`), Lilu and VirtualSMC before macOS would run. — [acidanthera/MacHyperVSupport](https://github.com/acidanthera/MacHyperVSupport)
- QEMU's WHPX backend took a multi-release effort in 2026 (a 37-patch series for 11.1 alone covering CPUID, xsave, x2APIC, MSR handling, interrupt priority) just to get the accelerator layer right. — [qemu-devel archive: WHPX x86 updates for QEMU 11.1](https://ratatoskr.run/qemu-devel/2026/04/14442356/t)
- QEMU's instruction emulator for MMIO exits still cannot decode MMX/SSE/AVX MMIO accesses; a from-scratch VMM would need its own x86 instruction decoder for the same purpose. — [QEMU master WHPX docs](https://www.qemu.org/docs/master/system/whpx.html)
- crosvm, a mature Rust VMM, lists its WHPX backend as untested upstream. — [crosvm book: hypervisors](https://crosvm.dev/book/hypervisors.html)
- Tahoe is the last Intel macOS, so no future guest versions will ever need supporting. — [OSXLatitude](https://osxlatitude.com/articles/news/the-end-macos-tahoe-unveiled-r97/)

### Inferences
- A from-scratch project would need, at minimum: a WHP partition/vCPU loop with an x86 instruction emulator for MMIO; LAPIC/IOAPIC/HPET/RTC/ACPI tables; a UEFI firmware port (EDK2/OVMF would still be reused); AHCI or NVMe; xHCI with HID; a GOP framebuffer; an audio device macOS has a driver for; a NIC macOS has a driver for; Apple SMC emulation; and correct CPUID/MSR behaviour for XNU. That is several person-years to reach what QEMU does today, with zero speed gain, because the bottlenecks are (a) Microsoft's hypervisor exits and (b) software rendering in the guest.
- The only place new code changes performance is graphics (a paravirtual GPU), and that work already exists as Reims, built as a QEMU device.
- Where orchord can add real value is above the VMM: accessible setup, OpenCore image generation, audio routing, VoiceOver-friendly defaults, and driving QEMU over QMP. A pure-emulator ("no hypervisor") design would be far slower than TCG is already assumed to be.

### Gaps
- No source gives an effort estimate for a new macOS-capable VMM; the above is engineering judgment.
- I did not research the legal status of the Apple OSK string or of redistributing OpenCore-based images; projects such as OSX-KVM embed the OSK in their scripts.

## 8. What does a known-good QEMU command line for macOS look like, and what must change on Windows?

### Takeaway
The reference is OSX-KVM's `OpenCore-Boot.sh`; on Windows the accelerator, CPU model handling, firmware attachment and a few host-side backends change, while the device list carries over.

### Cited Findings
- OSX-KVM `OpenCore-Boot.sh` (opened), active arguments:
  - `-enable-kvm -m 4096`
  - `-cpu Skylake-Client,-hle,-rtm,kvm=on,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on,+ssse3,+sse4.2,+popcnt,+avx,+aes,+xsave,+xsaveopt,check` (a Penryn line is present but commented out)
  - `-machine q35 -smp 4,cores=2,sockets=1`
  - `-device qemu-xhci,id=xhci -device usb-kbd,bus=xhci.0 -device usb-tablet,bus=xhci.0 -device usb-ehci,id=ehci`
  - `-device isa-applesmc,osk="<Apple OSK string>"`
  - `-drive if=pflash,format=raw,readonly=on,file=./OVMF_CODE_4M.fd -drive if=pflash,format=raw,file=./OVMF_VARS-1920x1080.fd`
  - `-smbios type=2`
  - `-device ich9-intel-hda -device hda-duplex`
  - `-device ich9-ahci,id=sata` with `ide-hd` devices for OpenCore.qcow2 (`snapshot=on`), BaseSystem.img and the main qcow2 disk
  - `-netdev user,id=net0,hostfwd=tcp::2222-:22 -device virtio-net-pci,netdev=net0,id=net0,mac=52:54:00:c9:18:27`
  - `-monitor stdio -device vmware-svga`
  — [OSX-KVM OpenCore-Boot.sh](https://github.com/kholia/OSX-KVM/blob/master/OpenCore-Boot.sh)
- The 2023 Windows/WHPX recipe used `-accel whpx`, `-M q35`, `-cpu Westmere,...+xsave`, a single merged OVMF file (WHPX crashed with separate CODE/VARS pflash), OpenCore and recovery images, and user-mode networking. — [digitalfoundations](https://www.digitalfoundations.com.au/blog/macos-on-windows-with-qemu/)
- The 2026 Windows/WHPX Tahoe run used `-smp 8,cores=8,sockets=1 -m 16G -cpu Skylake-Client,...`, OSX-KVM's OpenCore and OVMF, and `vmware-svga` for the unaccelerated baseline. — [MAXZVER/reims-vgpu issue #1](https://github.com/MAXZVER/reims-vgpu/issues/1)
- Under QEMU 11.0 the `-cpu` option is ignored with WHPX; CPU model emulation arrives with the 11.1 series. — [QEMU 11.0 WHPX docs](https://qemu.readthedocs.io/en/stable-11.0/system/whpx.html); [WHPX x86 updates for QEMU 11.1](https://ratatoskr.run/qemu-devel/2026/04/14442356/t)
- With `hyperv=off`, the 11.1 series reports the hypervisor as VMware so the guest can read TSC/APIC frequency from the VMware CPUID leaf (the same information `vmware-cpuid-freq=on` supplies under KVM). — [WHPX x86 updates for QEMU 11.1](https://ratatoskr.run/qemu-devel/2026/04/14442356/t)
- LongQT-sea/OpenCore-ISO offers a prebuilt OpenCore image for QEMU/KVM and Proxmox covering Mac OS X 10.4 to macOS 26, recommends OVMF with Secure Boot off, and suggests changing CPU model when boot fails. Snippet. — [LongQT-sea/OpenCore-ISO](https://github.com/LongQT-sea/OpenCore-ISO)

### Inferences
- Linux/KVM-specific parts that must change on Windows:
  - `-enable-kvm` becomes `-accel whpx` (optionally `-accel whpx,ssd=off` on builds that have it, trading mitigations for MMIO speed).
  - `kvm=on` in the `-cpu` string is KVM-only and must go. `vmware-cpuid-freq=on` and `+invtsc` are KVM-oriented; their WHPX equivalents depend on the QEMU version (see above). Given GitLab #3053, TSC-related flags are the first thing to vary if a guest freezes early.
  - `-cpu host` is not something to rely on under WHPX. On QEMU 11.0, plan for OpenCore-side CPUID spoofing (`Kernel -> Emulate -> Cpuid1Data/Cpuid1Mask`) because the guest sees Raptor Lake.
  - OVMF: the separate CODE/VARS pflash crash is from QEMU 7.2 and may be fixed, but orchord should be ready to fall back to a merged image. The firmware files come from the Windows QEMU build's `share` directory or from OSX-KVM.
  - Host backends: no `tap` scripts or `/dev/kvm`; use `-netdev user` (or a TAP-Windows adapter); audio backend becomes `-audiodev dsound` or `-audiodev wasapi` attached to the HDA codec; display becomes `-display sdl` or `gtk`, or headless with VNC.
  - Control channel: replace `-monitor stdio` with a QMP socket (`-qmp tcp:127.0.0.1:port,server,nowait` or a named pipe) so orchord can drive the VM programmatically.
  - `macrecovery.py` from OpenCorePkg (Python) fetches recovery images on Windows, replacing OSX-KVM's `fetch-macOS-v2.py` Linux workflow; DMG-to-raw conversion needs `qemu-img` or a pure-Python equivalent in place of `dmg2img`.
- Device choices that carry over unchanged: q35, `isa-applesmc`, AHCI disks, xHCI + usb-kbd + usb-tablet, `vmware-svga`, `virtio-net-pci` (macOS has had a virtio-net driver since Mojave; `vmxnet3` or `e1000-82545em` for older guests).
- Audio needs care on Tahoe: Apple removed AppleHDA.kext in the Tahoe betas ([OCLP issue #1167](https://github.com/dortania/OpenCore-Legacy-Patcher/issues/1167), snippet), which breaks the usual AppleALC route for `ich9-intel-hda`. A class-compliant `usb-audio` device is the likely alternative. Not tested; another researcher may be covering audio.

### Gaps
- I did not open the DarwinKVM, macOS-Simple-KVM, quickemu or Docker-OSX launch scripts this session; their command lines are variations on the OSX-KVM one (Penryn or Haswell-noTSX CPU models, virtio-blk disks, different display devices) from prior knowledge only.
- No source gives a complete, current (QEMU 11.x) Windows command line known to boot Sequoia or Tahoe on upstream QEMU.
- Which QEMU audio device works for VoiceOver on each macOS version under WHPX is unverified.

## 9. Do Hyper-V / VBS need to be disabled for best performance, and can a third-party hypervisor get raw VT-x while VBS is on?

### Takeaway
No raw VT-x is available to third-party hypervisors while the Microsoft hypervisor is loaded; the only supported path is the Windows Hypervisor Platform API. Disabling VBS gives VMware and VirtualBox their native engines back, but it removes QEMU's only Windows accelerator and costs users security features, so orchord should be designed to run with VBS on.

### Cited Findings
- Microsoft Q&A answer: hardware virtualization extensions can be used by only one software component at a time; to use other virtualization software "you need to disable Hyper-V Hypervisor, Device Guard, and Credential Guard". Snippet. — [Microsoft Q&A: Nested virtualization with Hyper-V and VMware](https://learn.microsoft.com/en-us/answers/questions/994875/nested-virtualization-with-hyper-v-and-vmware)
- Broadcom's procedure for regaining direct VT-x: turn off Memory Integrity, remove Hyper-V, Virtual Machine Platform, Windows Hypervisor Platform, Windows Sandbox and WSL, then `bcdedit /set hypervisorlaunchtype off` and reboot. — [Broadcom KB 417896](https://knowledge.broadcom.com/external/article/417896/)
- Under host VBS, VMware cannot offer virtualized VT-x/EPT (nested virtualization) to its guests; a community poster attributes this to limits of the WHP API. Snippet. — [VMware Communities: Nested hypervisor support under VBS](https://communities.vmware.com/t5/VMware-Workstation-Pro/Nested-hypervisor-support-under-VBS/m-p/2875567)
- QEMU's only hardware accelerator on Windows is WHPX, which requires the Windows Hypervisor Platform feature (the 2026 fork build reports accelerators `tcg | whpx`); HAXM, the former raw-VT-x option, is discontinued. — [QEMU master WHPX docs](https://www.qemu.org/docs/master/system/whpx.html); [MAXZVER/reims-vgpu issue #1](https://github.com/MAXZVER/reims-vgpu/issues/1); [intel/haxm](https://github.com/intel/haxm)
- QEMU's `ssd=off` accelerator option recovers some performance inside WHP by skipping the per-entry/exit branch-predictor flush, at a stated security cost. — [QEMU master WHPX docs](https://www.qemu.org/docs/master/system/whpx.html)
- VirtualBox also drops to a slower mode when Hyper-V is active. Snippet. — [Kicksecure wiki](https://www.kicksecure.com/wiki/VirtualBox/Green_Turtle_Issue)

### Inferences
- For QEMU and Hyper-V backends the question is moot: they need the Microsoft hypervisor. Only a VMware or VirtualBox backend benefits from turning VBS off, and asking blind end users to disable Memory Integrity, edit BCD and reboot is a poor accessibility and security trade.
- Overall ranking for orchord on this evidence:
  1. QEMU 11.x + WHPX + OpenCore. Open source (GPLv2), has audio and USB device models, has two 2026 boot reports on hybrid Intel hosts with Windows 11 build 26200, works with VBS on, is the only backend the Reims vGPU can plug into, and is drivable over QMP. Risks: thin real-world evidence, fork-carried patches, CPUID handling differs between 11.0 and later, the main WHPX developer may reduce involvement.
  2. Native Hyper-V + MacHyperVSupport. Probably the lowest overhead and well documented, PowerShell-scriptable, BSD-licensed. Blockers: no audio device (critical for VoiceOver), no route to graphics acceleration on Windows 11 Pro, needs Pro/Enterprise and elevation, Tahoe still in progress.
  3. KVM inside WSL2 (dockur-style). Mature guest stack, but nested, heavy to install, and indirect audio/display.
  4. VMware Workstation + unlocker/OC4VM. Already installed on the dev machine and useful as a comparison baseline, but proprietary, patch-dependent, and no faster under VBS.
  5. VirtualBox, HAXM, crosvm/cloud-hypervisor/libkrun, TCG, from-scratch VMM. Not viable or not worth it.
- A sensible first experiment on the i7-1355U: QEMU 11.x (newest stable or a current master build), `-accel whpx`, 4 vCPUs, 8 GB RAM (the host has only 16 GB), Ventura or Sequoia guest, `vmware-svga`, 1280x720, Reduce Motion and Reduce Transparency on, and compare against the same guest in Workstation 26 and in Hyper-V.

### Gaps
- No Microsoft documentation page was opened that states in so many words that WHP is the sole route for third-party VMMs under VBS; the conclusion rests on a Microsoft Q&A answer, Broadcom's KB and VMware's design change.
- No measurements of how much `ssd=off` or disabling VBS changes macOS guest responsiveness.
- How the Hyper-V scheduler places WHP vCPUs on P-cores versus E-cores on a 2P+8E CPU, and whether pinning is possible from user mode, was not found. With only two P-cores this could matter for VoiceOver latency.
- 16 GB host RAM with VBS on leaves limited headroom for an 8 GB guest; no source addresses memory pressure for this configuration.
