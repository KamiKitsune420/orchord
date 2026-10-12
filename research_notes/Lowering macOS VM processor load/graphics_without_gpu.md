# Graphics without a GPU: how an Intel macOS guest draws its screen, what it costs, and every route to something cheaper or accelerated (state of play, October 2026)

Notes on method. Research date: 12 October 2026. Target: Intel macOS guest, QEMU 11.1 on Windows with the Windows Hypervisor Platform accelerator (WHPX), `vmware-svga`, `-display none`, frames via QMP `screendump`. Every item under "Cited Findings" carries a link. Anything I know only from background knowledge and could not confirm from a source this session is placed under "Inferences" or "Gaps" and labelled as unverified. No tables are used, so the file reads cleanly with a screen reader.

The single most important finding, stated up front: an open-source re-implementation of the host side of Apple's paravirtualised graphics now exists (reims-vgpu, by steelbrain). It drives the stock `AppleParavirtGPU.kext` already inside macOS 11 and later, translates the guest's Metal command stream to Vulkan on the host, and has experimental Windows 11 plus WHPX work in progress (upstream issue 17 closed 31 August 2026 with "initial windows support has been merged"; a community Windows fork reports about 90 to 115 presented frames per second in a macOS 13 guest on an RTX 4060). It is the only realistic route to real acceleration on this user's host, but as of October 2026 it is unstable on Windows, tested there on macOS 13 only, and on macOS 26 Tahoe it is reported slower than no acceleration.

## Key Question 1: What is the rendering path when macOS has no supported graphics hardware, and what specifically is slow?

### Takeaway
With only a plain framebuffer, macOS has no Metal device at all, so WindowServer and every application composites on the processor; the framebuffer driver does nothing except expose pixels. I found no primary Apple source naming the exact software path per version, and no published per-version WindowServer CPU measurements, but OpenCore Legacy Patcher's non-Metal work shows clearly which components became Metal-dependent in which release.

### Cited Findings
- In a virtual machine with no GPU, Metal reports no device: GitHub's hosted macOS runners (virtual machines) return nil from `MTLCreateSystemDefaultDevice`, which the reporter attributes to the guest's GPU configuration — [actions/runner-images issue 1779](https://github.com/actions/virtual-environments/issues/1779)
- OpenCore Legacy Patcher's non-Metal acceleration for Big Sur and later is built on the macOS 10.14.6 (Mojave) window-server library: Stage Manager is listed as "Unsupported on non-Metal (incompatible with 10.14.6 SkyLight)". This is direct evidence that Mojave 10.14.6 is the last release whose SkyLight (the WindowServer core) still carries a non-Metal compositing path that the community can reuse — [OCLP issue 108, non-Metal tracker](https://github.com/dortania/OpenCore-Legacy-Patcher/issues/108)
- Components that require Metal, by release, from the same tracker: "Monterey VectorKit requires Metal" (Maps, Find My, Places broken from Monterey; fix is "Downgrade to Big Sur"); Stage Manager (Ventura) unsupported; "Show desktop" and "Cycle through windows" shortcuts broken in Sonoma and later ("Downgrade to Ventura"); Sequoia: Metal-rendered wallpapers do not work ("Requires Metal API"), Weather background broken ("WeatherUI requires Metal API"), Photos gallery frozen, window tiling does not work; DRM video in Safari, TV and Music is "Unfixable" because "Hardware decoding unavailable on non-Metal GPU drivers" — [OCLP issue 108](https://github.com/dortania/OpenCore-Legacy-Patcher/issues/108)
- Ventura was a distinct step: OCLP's Ventura notes call non-Metal support the project's greatest challenge since Big Sur and say "Apple has made significant changes to the graphics stack in order to facilitate fancy effects" (Stage Manager named) — [OCLP Ventura notes](https://dortania.github.io/OpenCore-Legacy-Patcher/VENTURA-DROP.html)
- Non-Metal support had to be re-done for Monterey as well ("Non-Metal Acceleration (Resolved in 0.2.5 and newer)") — [OCLP Monterey notes](https://dortania.github.io/OpenCore-Legacy-Patcher/MONTEREY-DROP.html)
- Mojave (10.14) is where Apple tied full acceleration to Metal-capable GPUs; at the time forum users only speculated that "WindowServer is Metal only", and dosdude1 restored non-Metal acceleration for 10.14.4 in March 2019 by a patch script. This is forum-level evidence, not an Apple statement — [MacRumors Mojave on Unsupported Macs thread](https://forums.macrumors.com/threads/macos-10-14-mojave-on-unsupported-macs-thread.2121473/page-499); background in [InsanelyMac: Mojave on non-Metal graphics](https://www.insanelymac.com/forum/topic/334561-mojave-on-non-metal-graphics/)
- Real-world non-Metal symptoms that match the above: on a non-Metal MacBook Pro, Maps shows a grey screen on Monterey, Ventura and Sonoma but works on Big Sur; another user had to disable WebGL in Safari to stop Google Maps crashing the tab — [MacRumors Sonoma on Unsupported Macs thread, page 308](https://forums.macrumors.com/threads/macos-14-sonoma-on-unsupported-macs-thread.2391630/page-308)
- In an unaccelerated QEMU guest, System Information reports only "3–7 MB of Video Memory regardless of QEMU settings", i.e. macOS is treating the device as a dumb framebuffer — [OrelSokolov gist, created 29 May 2026](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)
- Order of magnitude of software-rendered speed: a Windows/WHPX tester with an i7-14700K measured 5 to 7 presented frames per second "before" on a full-screen Safari CSS animation, Safari scroll and window drag at 1920x1080 in a macOS 13 guest, against about 93 after the fork's GPU work. The page does not say whether "before" means no acceleration or the earlier slow Windows port, so treat it as an upper-bound hint only — [MAXZVER/reims-vgpu README, results dated 29 September 2026](https://github.com/MAXZVER/reims-vgpu)
- A Linux/KVM guide for unaccelerated macOS (Big Sur to Sequoia, `vmware-svga`, 1920x1080) describes "choppy 5–15 FPS slideshows" when QEMU's threads are badly scheduled, and a smooth desktop once the VM is pinned to 4 to 6 fast physical cores with one thread per core; it also says giving the guest 16 virtual processors "does not make macOS faster". No frame timings are given; anecdotal — [OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)

### Inferences
- The step sequence for Metal dependence is: Mojave (WindowServer prefers Metal; 10.14.6 SkyLight still has the older path), Big Sur (OCLP has to transplant Mojave-era libraries), Monterey (Maps/VectorKit Metal-only), Ventura (graphics stack reworked for Stage Manager), Sonoma (more window-management features break), Sequoia (Metal-only wallpapers, Weather, Photos, tiling). In a VM with no Metal device these same components must either fall back to processor rendering or fail outright.
- Unverified background knowledge, not confirmed by any source fetched this session: on Intel macOS without a GPU driver, OpenGL falls back to Apple's built-in software renderer (shown as "Apple Software Renderer", implemented in the OpenGL framework's `GLRendererFloat` bundle), and Core Animation/WindowServer composite with processor code paths; I found no evidence of a kext called `com.apple.driver.AppleSoftwareRenderer` and no evidence that Intel macOS ships a software Metal device comparable to Mesa's llvmpipe. The report writer should present this as likely rather than established.
- The things that hurt most in software are those that need per-pixel blending over large areas every frame: translucency and blur (menu bar, sidebars, Dock, Notification Centre), animated wallpapers and screen savers (Sonoma and later), window animations, video, and web content (Safari and Chromium/Electron fall back to software rasterisation and software WebGL, or lose WebGL). Turning on Reduce Transparency and Reduce Motion, a static plain-colour wallpaper, and no screen saver should remove much of this. This is reasoning from the list of Metal-dependent features above, not a measured result.
- The cost of software compositing scales with the number of pixels, so resolution is the one lever guaranteed to matter (see Key Question 3).

### Gaps
- No primary source (Apple documentation, WWDC session, or reverse-engineering write-up) was found that names the exact WindowServer compositing backend per macOS version when no GPU is present.
- No published measurements of WindowServer CPU percentage in unaccelerated virtual machines by macOS version were found in OSX-KVM, Proxmox, VMware, VirtualBox or UTM communities, despite several differently worded searches. This would have to be measured directly (for example `top -l 5 -stats pid,command,cpu` for WindowServer on each guest at the same resolution).
- No source was found quantifying the software cost of Safari/WebKit or Electron specifically in a macOS guest.
- The deepwiki pages on OCLP non-Metal troubleshooting appeared in results but are machine-generated summaries, so I did not rely on them.

## Key Question 2: Which macOS version is lightest without acceleration?

### Takeaway
There is no benchmark; the evidence is indirect. It points to "older is lighter", with identifiable steps at Monterey, Ventura, Sonoma, Sequoia and a large one at Tahoe 26, and with Big Sur as the earliest release that carries the paravirtual GPU driver (which matters if acceleration via reims-vgpu is ever wanted).

### Cited Findings
- Metal-only features accumulate release by release: Monterey (Maps), Ventura (Stage Manager and a reworked graphics stack), Sonoma (window-management shortcuts), Sequoia (Metal wallpapers, WeatherUI, Photos, tiling) — [OCLP issue 108](https://github.com/dortania/OpenCore-Legacy-Patcher/issues/108), [OCLP Ventura notes](https://dortania.github.io/OpenCore-Legacy-Patcher/VENTURA-DROP.html)
- Tahoe 26 is reported as very heavy even with acceleration: "the performance in tahoe is crazy bad its worse then unaccelerated tahoe" (reims-vgpu on Linux, RTX 3060, 22 September 2026), and a Windows/WHPX tester saw 0.3 to 1.9 presented frames per second in Tahoe 26.7.1 with QEMU using about four host cores (9 October 2026) — [reims-vgpu issue 96](https://github.com/steelbrain/reims-vgpu/issues/96), [MAXZVER/reims-vgpu issue 1](https://github.com/MAXZVER/reims-vgpu/issues/1)
- A Proxmox tutorial's download script labels Big Sur 11.7 "RECOMMENDED" without giving a reason — [Proxmox forum tutorial](https://forum.proxmox.com/threads/gpu-passthrough-on-proxmox-ve-macos-monterey-part-04x04.140579/)
- Ventura has a known unaccelerated-VM hazard: choosing a predefined resolution in Displays can leave a black screen with only a small visible portion; recovery is deleting the windowserver preference files — [OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md)
- The one guide that covers Big Sur through Sequoia unaccelerated reports no version-specific differences, only host scheduling effects — [OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)
- The paravirtual GPU guest driver is present from Big Sur onwards ("natively present from Big Sur onwards"; "guest drivers which ship with macOS versions 11 and up") — [AMD-OSX forum post, 14 September 2026](https://forum.amd-osx.com/threads/reims-vgpu-graphics-paravirtualization-for-macos.6616/), [QEMU apple-gfx patch series](https://patchew.org/QEMU/20240717110326.45230-1-phil@philjordan.eu)

### Inferences
- Likely ordering from lightest to heaviest with no GPU: High Sierra and Mojave (smallest effects budget; 10.13 predates the Metal WindowServer), then Catalina, then Big Sur (new translucent design but still the simplest of the 11-and-later line), Monterey, Ventura, Sonoma (animated wallpapers and screen savers, desktop widgets), Sequoia, and Tahoe 26 far behind because of its glass-style interface. This ordering is my synthesis from the Metal-dependence timeline; it is not measured.
- For a blind VoiceOver user the practical sweet spot is probably Big Sur 11 or Monterey 12 if the lowest processor cost with a still-usable modern system is the goal, and Ventura 13 if reims-vgpu acceleration is a medium-term aim (Ventura is the only guest tested on the Windows fork). Sonoma (shipped by orchord 0.1.4 per the repository's commit log) sits in the heavier half.
- Tahoe 26 should be avoided for this use: it is the heaviest unaccelerated and currently the worst case even when accelerated.

### Gaps
- No side-by-side comparison of idle or interactive WindowServer CPU across High Sierra, Mojave, Catalina, Big Sur, Monterey, Ventura, Sonoma, Sequoia in an unaccelerated VM exists in any source I could find. A like-for-like test on the user's own host is the only way to settle it.
- No VirtualBox, VMware Workstation or UTM-on-Intel reports comparing versions were found.
- Security-update status of older releases (relevant to choosing an old version) was outside this scope and was not researched.

## Key Question 3: Which emulated display device is best for macOS under QEMU, and how do resolution and video memory affect cost?

### Takeaway
All practical choices are framebuffer-only inside macOS, so none changes rendering cost; they differ only in which resolutions are offered and in how QEMU tracks changes. Resolution is the lever that matters, and video memory size does not.

### Cited Findings
- macOS has shipped its own driver for QEMU's standard VGA and Cirrus cards since a Mojave update: `AppleVirtualGraphics.kext` with personalities `AppleBochVGAFB` (PCI match `0x11111234`, i.e. vendor 1234 device 1111) and `AppleCirrusGD5446FB` (`0x00001013`). With it the Displays panel offered only three modes, 1920x1080 was not among them, and changing OVMF's mode list had "No effect". `AppleVirtIO.kext` has no display personality (so no virtio-gpu support on Intel) — [Gerd Hoffmann, "macos guest support for qemu", 17 June 2019](https://www.kraxel.org/blog/2019/06/macos-qemu-guest/)
- OSX-KVM uses `vmware-svga` ("Switching to vmware-svga device seems to have 'fixed' the following problem!") and quickemu changed its macOS default to `vmware-svga` to follow it — [OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md), [quickemu commit](https://git.binarymagic.net/AIR/quickemu/commit/9309f782c4399cada70230761fa7e2b7b78e29ad)
- Video memory size is irrelevant: `vgamem_mb` "does not affect the macOS driver or framebuffer performance", and macOS shows 3 to 7 MB regardless — [OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)
- VMware's own guest graphics driver is `VMwareGfx.kext`, delivered by VMware Tools (darwin.iso), not by macOS; OSX-KVM-era advice says SVGA "acceleration" under QEMU would require installing that kext with `-vga vmware`, and a commenter doubted it is a correct way to get acceleration; a VirtualBox user found that re-pointing it at another virtual GPU did not work — [Unraid forum thread](https://forums.unraid.net/topic/99434-qemu-args/), [VirtualBox forum](https://forums.virtualbox.org/viewtopic.php?t=93158)
- Setting resolution, OVMF plus OpenCore: set `UEFI -> Output -> Resolution` in OpenCore's config.plist (default `Max`), and make OVMF's own preferred resolution match ("default is 1024x768") through the firmware menu: Device Manager, OVMF Platform Configuration, "Change Preferred Resolution for Next Boot". OSX-KVM ships a variables file named `OVMF_VARS-1024x768.fd` — [OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md), [Fedora developer guide on OVMF resolution](https://developer.fedoraproject.org/tools/virtualization/setting-viewport-resolution-using-ovmf-bios.html)
- OVMF's list of selectable resolutions is filtered by video RAM size, and the stored preference "might be unset, or even invalid (hence ignored)" — [EDK2 OvmfPkg PlatformDxe strings](https://qemu.googlesource.com/edk2/+/d98a39d4ceeb7204b33fc330fdfee8cdb5ddbb43/OvmfPkg/PlatformDxe/Platform.uni)
- The QEMU `xres`/`yres` device properties "appear to be ignored by OVMF"; changing the setting inside the OVMF configuration is what works — [openSUSE issue 111992](https://progress.opensuse.org/issues/111992)
- Some OVMF builds fall back to 800x600 on cold boot unless the preference is saved in the variables store — [Proxmox forum, OVMF screen resolution bug](https://forum.proxmox.com/goto/post?id=326265)
- With `vmware-svga` on recent macOS, resolution can be changed inside macOS from Settings, Displays, except that on Ventura this can break the display (see Key Question 2) — [OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md)
- QEMU's `apple-gfx-pci` device "ships a UEFI bootrom that provides basic framebuffer mode", and reims-vgpu likewise needs its own GOP option ROM for firmware display output; these are the only devices for which macOS loads an accelerated driver — [QEMU apple-gfx patch series v16](https://patchew.org/QEMU/20241223221645.29911-1-phil@philjordan.eu/), [reims-vgpu README](https://github.com/steelbrain/reims-vgpu/blob/master/README.md)

### Inferences
- Under OVMF and OpenCore, macOS draws into the firmware's GOP framebuffer whichever QEMU card is used unless a macOS driver binds to it. With `vmware-svga` and no VMware Tools, nothing in macOS binds, so it is a plain linear framebuffer: no 2D acceleration, no hardware cursor, no mode-switching beyond what the firmware set up. With `-device VGA` (std), Apple's `AppleBochVGAFB` binds and offers a short fixed mode list. Neither does any drawing work, so neither is cheaper for WindowServer. (The claim that stock macOS lacks any VMware SVGA driver is inferred from the VMwareGfx sources above plus background knowledge; I did not find a source that says it in so many words.)
- `bochs-display`, `ramfb`, `qxl` and `virtio-vga` behave the same way from macOS's side: firmware framebuffer only, if the firmware has a GOP driver for them. `bochs-display` and std VGA share the 1234:1111 identity, so Apple's driver should also match `bochs-display`; unverified.
- Host-side difference: QEMU's VGA-family devices track which guest memory pages were written, so a consumer that honours dirty regions (VNC, D-Bus display) does little when the screen is static. The `vmware-svga` device relies on guest-issued update commands, which an undriven macOS guest never sends, so QEMU has to treat the whole surface as changed on each refresh. On a headless setup that polls with `screendump` this difference is moot, but it matters if frame retrieval is changed to a dirty-rectangle method. This is from general knowledge of QEMU's display code and should be checked against `hw/display/vmware_vga.c` before relying on it.
- Lowering resolution is the most reliable saving: 1024x768 has 38 percent of the pixels of 1920x1080, and 800x600 has 23 percent; software compositing and frame retrieval both scale roughly with pixel count. A blind user loses nothing. One caution: VoiceOver and many applications assume at least 1024x768 for layout, and some macOS dialogs do not fit at 800x600, so 1024x768 is the sensible floor.
- Practical recipe for a low resolution: set OpenCore `Resolution` to `1024x768`, boot once into the OVMF menu and set the preferred resolution to the same, keep the resulting variables file, and avoid using the Displays panel on Ventura.

### Gaps
- I did not find a source that tests `bochs-display`, `ramfb`, `qxl` or `virtio-vga` with a current Intel macOS guest and reports what the guest sees.
- I did not confirm from QEMU source this session how `vmware-svga` dirty tracking behaves with a guest that has no SVGA driver.
- Whether `AppleVirtualGraphics.kext` still ships in macOS 13 to 26 on Intel was not confirmed; the only source is from 2019 (Mojave).
- No source measured CPU cost against resolution for an unaccelerated macOS guest.

## Key Question 4: Third-party guest display drivers (VMsvga2, VMQemuVGA and similar)

### Takeaway
They are old, target long-gone macOS versions, and at best add resolution switching rather than rendering acceleration on anything modern; none is a route to lower processor load on macOS 11 or later.

### Cited Findings
- VMQemuVGA is a display driver for OS X guests under VirtualBox that enables changing resolution from System Preferences; the author says the binary should work from Snow Leopard through Yosemite 10.10, and "I haven't really gotten around to proving it on QEMU". Original post 22 April 2018; source at github.com/ivanagui2/VMQemuVGA; a fork (startergo/VMQemuVGA) was used in October 2024 with UTM's QXL device by adding PCI match `0x01001b36` to get 512 MB of reported video memory — [InsanelyMac: VMQemuVGA display driver](https://www.insanelymac.com/forum/topic/333901-vmqemuvga-display-driver-for-os-x-under-virtualbox/)
- VMsvga2 ("VMware SVGA II display driver for OS X", version 1.2.5) exists as an InsanelyMac project thread — [InsanelyMac: VMware SVGA II display driver for OS X, v1.2.5](https://www.insanelymac.com/forum/topic/188962-vmware-svga-ii-display-driver-for-os-x-v125/)
- VMware's `VMwareGfx.kext` is specific to VMware's virtual hardware and can fail to load on Mojave and later without approving VMware's team identifier — [Mojave guest on Fusion 11 gist](https://gist.github.com/bradleysepos/8afbf990fffbd69dc8f4aedd164cdaec), [VirtualBox forum](https://forums.virtualbox.org/viewtopic.php?t=93158)

### Inferences
- Unverified background knowledge: VMsvga2 (by Zenith432) offered 2D acceleration and, on Leopard/Snow Leopard-era systems, OpenGL acceleration through VMware's SVGA3D, and has not been maintained for modern macOS; QEMU's `vmware-svga` implements no 3D in any case, so even a working SVGA guest driver could only give a hardware cursor and explicit update rectangles.
- Loading any third-party kext on macOS 11 or later needs relaxed System Integrity Protection and kext approval, and on 13 and later the kernel collection rebuild; the benefit here would be nil for rendering cost.

### Gaps
- I did not fetch the VMsvga2 thread contents, so supported versions and last update date are not sourced.
- No evidence was found of anyone running VMsvga2 or VMQemuVGA on macOS 11 or later under QEMU.

## Key Question 5: VMware's macOS guest graphics (Workstation with the unlocker; Fusion)

### Takeaway
Only VMware Fusion on a Mac host offers macOS guest 3D, and it is Metal-only via Apple's host framework; I found no evidence of any 3D or Metal acceleration for macOS guests in Workstation on Windows or Linux.

### Cited Findings
- Fusion: 3D acceleration for macOS guests requires macOS 11 Big Sur or later on both host and guest, is enabled by adding an Apple GPU device to the .vmx file, and is verified by "Metal: Supported" in System Report; "OpenGL-based applications are not supported" and "only a subset of Metal is currently available inside of the virtual machine" — [VMware KB 81657](https://kb.vmware.com/s/article/81657) (the article has since moved to Broadcom's site; the redirect target returned 404 on 12 October 2026, so this is from the search-indexed copy)
- Before Fusion 12, a VMware employee stated "we still do not provide support for 3D acceleration in macOS guests" — [VMware Communities thread](https://communities.vmware.com/thread/630585)
- Running macOS in Workstation on Windows relies on an unofficial unlocker patch and is officially unsupported; none of the guides claim Metal or 3D — [SYSNETTECH guide to macOS Sonoma on Workstation 17](https://www.sysnettechsolutions.com/en/install-macos-vmware/)
- The reason it cannot exist off-Mac with Apple's code: ParavirtualizedGraphics.framework "is a black box implementing most of the logic" of the device and exists only on macOS hosts — [QEMU apple-gfx patch series v16](https://patchew.org/QEMU/20241223221645.29911-1-phil@philjordan.eu/)

### Inferences
- Fusion's macOS guest acceleration is the same Apple paravirtual GPU (same guest kext, Apple's host framework), which is why it appeared exactly at Big Sur and why Workstation never gained it. The .vmx key is, from memory, `appleGPU0.present = "TRUE"`; unverified.
- In Workstation, a macOS guest with VMware Tools gets `VMwareGfx.kext`: resolution switching and a 2D framebuffer, no Metal. So moving this user to Workstation would not reduce rendering cost.

### Gaps
- No current official Broadcom statement about macOS guest graphics in Workstation 17 or later was found.
- VirtualBox's manual on macOS guest graphics was not retrieved; I found nothing suggesting it accelerates macOS guests.

## Key Question 6: Apple's paravirtualised graphics, QEMU's apple-gfx devices, and open-source host re-implementations

### Takeaway
Apple's own host side is macOS-only, so stock QEMU's `apple-gfx-pci` is no use on Windows. However, reims-vgpu is a working open-source replacement for that host side (Metal on Mac hosts, Vulkan on Linux), and Windows 11 with WHPX is being actively brought up by several contributors. On Windows it is experimental: macOS 13 works with real speed-ups on one tester's machine, guest kernel panics need workarounds, and Tahoe 26 is unusably slow.

### Cited Findings
Apple and stock QEMU:
- QEMU 10.0 (April 2025) added `apple-gfx-pci` (x86-64 guests) and `apple-gfx-mmio` (arm64 `vmapple` machine), providing accelerated graphics for macOS guests on macOS hosts through ParavirtualizedGraphics.framework — [Phoronix, QEMU 10.0 released](https://www.phoronix.com/news/QEMU-10.0-Released)
- Host needs macOS 11 or later and a Metal-capable GPU; `apple-gfx-pci` "mainly targets x86-64 macOS guests from version 11 on"; an x86-64 guest on an arm64 host shows corrupted rendering; the guest framebuffer is copied to system memory for QEMU's display — [apple-gfx patch series v16, 23 December 2024](https://patchew.org/QEMU/20241223221645.29911-1-phil@philjordan.eu/)
- A later Kconfig change by Paolo Bonzini builds the device only when guest and host architecture match, because Apple's framework is not cross-architecture; so stock `apple-gfx-pci` needs an Intel Mac host — [QEMU hw/display/Kconfig history](https://mirror.iscas.ac.cn/riscv-toolchains/git/riscv-collab/riscv-gnu-toolchain/qemu.git/log/hw/display/Kconfig?showmsg=1)

reims-vgpu, upstream (steelbrain):
- "reims-vgpu is a virtual GPU for macOS VMs"; it replaces Apple's framework and runs the guest's GPU command stream on Metal (Mac hosts) or Vulkan (Linux, through a component called metal2vulkan); "There's nothing to install in the guest" because it uses the stock `AppleParavirtGPU.kext`. Status "Beta ... has largely stabilized", with behaviour still depending on host and GPU driver. Licence "GPL-2.0-or-later OR LGPL-3.0-or-later". 547 stars, 3,248 commits, 21 open issues, 34 open pull requests, no tagged releases — [reims-vgpu repository](https://github.com/steelbrain/reims-vgpu), [README](https://github.com/steelbrain/reims-vgpu/blob/master/README.md)
- Supported paths: arm64 guests on Apple silicon (HVF, `vmapple`, `apple-gfx-mmio`); x86-64 guests on Linux x86-64 with KVM and a Vulkan driver, machine `q35`, device `apple-gfx-pci` (PCI ID 106b:eeee) plus its UEFI option ROM. Tested: "arm64 macOS 13, 14, 15 and 26 on an M3 Max, and x86-64 macOS 26 on Linux with an RTX 5080 laptop GPU" — [README](https://github.com/steelbrain/reims-vgpu/blob/master/README.md)
- It needs a patched QEMU (`steelbrain/qemu-reims-vgpu`), not stock QEMU; the library is linked statically into the device. Known limits: no HDR, x86-64 guests need a Metal error-mode setting or "the guest's icon service crash-loops", closing the window powers the VM off — [README](https://github.com/steelbrain/reims-vgpu/blob/master/README.md)
- The project website still describes an older state ("research-quality software", Ventura the only tested guest, both paths "Works · alpha"), which conflicts with the README's "Beta" and wider test list; the README is the newer statement — [reims-vgpu.com](https://reims-vgpu.com/)
- A packaged Linux build exists: qemus/qemu-macos, "QEMU build for running macOS guests with hardware-accelerated graphics", requiring the Reims PCI device, the Reims GOP option ROM, memfd-backed shared guest RAM, KVM, and a Vulkan 1.2 or newer host driver; updated about two weeks before 12 October 2026 — [Docker Hub qemux/qemu-macos](https://hub.docker.com/r/qemux/qemu-macos), [qemus/qemu-macos](https://github.com/qemus/qemu-macos)
- Community report, 14 September 2026: "It's still in the alpha phase; it works from macOS Big Sur up to Tahoe"; tested Sonoma on Linux with an RTX 2080 Ti; "The translation engine in this QEMU adaptation is still slow, but it shows great promise" — [AMD-OSX forum](https://forum.amd-osx.com/threads/reims-vgpu-graphics-paravirtualization-for-macos.6616/)

reims-vgpu on Windows:
- Issue 17 "Support Windows" (opened 31 July 2026) was closed on 31 August 2026 with the maintainer's statement "Initial windows support has been merged. I think it should compile and run (albeit with glitches)". A tester on Windows 11 25H2 reported it works but "the acceleration is slower like 5 FPS" — [reims-vgpu issue 17](https://github.com/steelbrain/reims-vgpu/issues/17)
- Issue 71 (opened 2 September 2026, still open): on Windows 11 with QEMU and WHPX, RTX 5060, macOS 13.6 guest, the guest kernel panics 70 to 160 seconds into boot with an assertion at `AppleParavirtPageTable.cpp:210`; the same class of panic occurs on Linux (issue 70). Setting `REIMS_VGPU_GUEST_IMPORT=off` removed the corruption; a later bisect found the guest survives with `REIMS_VGPU_GUEST_IMPORT_SURFACE_WRITES=off` plus pull request 106 (380 seconds of continuous use). The reporter also described a separate Hyper-V/WHPX shadow-page-table problem causing a 15 to 20 minute boot stall, later worked around with QEMU/WHPX fixes — [reims-vgpu issue 71](https://github.com/steelbrain/reims-vgpu/issues/71)
- A Windows-focused fork collects the pending Windows work: "A macOS guest with a paravirtual GPU on a Windows 11 PC", labelled "Experimental ... Expect glitches, and don't use it for anything you can't lose". Lab host: i7-14700K, 96 GB, RTX 4060, Windows 11 Pro, WHPX. Only macOS 13 (13.7.8) tested. Results dated 29 September 2026, median presented frames per second at 1920x1080: CSS animation about 92 to 97, full-screen CSS scroll about 116 to 118, wheel scroll about 88 to 96; window drag about 40 is "the weak spot"; earlier figure for the same tests was 5 to 7. All numbers use `REIMS_VGPU_GUEST_IMPORT=off`. Build is with MSYS2 UCRT64 against a companion QEMU fork (`MAXZVER/qemu-reims-vgpu`) because the upstream QEMU submodule branch "does not compile on Windows"; the guest is booted with OSX-KVM's OpenCore and OVMF, with QMP on 127.0.0.1:4444. The steps "have not been re-run end-to-end from a fresh clone". One Windows-specific bug fixed: a dirty-bitmap error from `long` being 32 bits on Windows. CPU usage is not reported — [MAXZVER/reims-vgpu](https://github.com/MAXZVER/reims-vgpu)
- Tahoe on Windows, 9 October 2026: Windows 11 Pro 26200, WHPX, RTX 4070 Laptop, QEMU fork build 11.0.50, macOS 26.7.1 guest reaches the desktop and stays up, Xcode's iOS Simulator renders through Metal, but presentation is 0.3 to 1.9 frames per second, GPU use 1 to 2 percent, QEMU using about four host cores, missing textures and "?" icons — [MAXZVER/reims-vgpu issue 1](https://github.com/MAXZVER/reims-vgpu/issues/1)
- Alternative Windows route that already works for one user: WSL2 with nested KVM. Windows 11 build 26100, AMD Ryzen 7 7735HS with Radeon 680M, self-built QEMU 8.2.2 with reims-vgpu 0.1.0, Vulkan through Mesa's Dozen driver (Vulkan on Direct3D 12), macOS 13.7.8 guest shows "Metal 2 Supported"; display was taken over QEMU VNC; the Dozen path does a per-frame CPU copy because it lacks `VK_EXT_external_memory_host`. The same reporter found native WHPX blocked on that AMD machine by "missing ignore_msrs / AMD-SVM MSI issues / cpu vendor masking dropped by Hyper-V" (13 September 2026) — [reims-vgpu issue 86](https://github.com/steelbrain/reims-vgpu/issues/86)
- WSL2 with an NVIDIA GPU is worse: Mesa Dozen crashes in `vkCreateDevice` when the host window is enabled — [reims-vgpu issue 32](https://github.com/steelbrain/reims-vgpu/issues/32), tracked with other blockers (present stalls on NVIDIA, AMD RADV support, Intel Iris Xe out-of-device-memory errors) in [AQEMU issue 42, opened 8 August 2026](https://github.com/chronic8000/aqemu/issues/42)

### Inferences
- Feasibility for this user's setup, honestly stated. What is in favour: no guest changes beyond booting with the Reims option ROM; macOS 11 and later already contain the driver; the project is moving fast (thousands of commits, multiple Windows contributors within two months); QMP still works, and frames are written back into guest-visible memory, so a headless viewer can keep fetching frames. What is against: it requires replacing stock QEMU 11.1 with a patched fork built from source under MSYS2; it requires a host GPU with a good Vulkan driver (only NVIDIA RTX 4060, 4070 and 5060 are reported on native Windows; Intel integrated graphics has open bugs upstream); guest kernel panics on Windows are still open and need environment-variable workarounds; only macOS 13 is validated on Windows; AMD processors under WHPX have separate blockers.
- It is not established that acceleration lowers total host processor use. Reported numbers are frame rates, not CPU. The translation layer and the framebuffer write-back cost host CPU (four cores busy in the Tahoe report). For a blind user who needs responsiveness and low load rather than frame rate, the benefit must be measured, not assumed. The plausible gain is that WindowServer and applications inside the guest stop burning virtual processors on compositing.
- Recommendation implied by the evidence: treat reims-vgpu as something to prototype on a branch with a macOS 13 guest and re-evaluate every few weeks, not as something to ship to users in October 2026.
- Because the guest kext is Apple's and the protocol was never published, an Apple update could change the guest driver at any time; the project says it treats observed behaviour as the contract.

### Gaps
- Apple's developer documentation pages for ParavirtualizedGraphics were not retrieved this session; the host requirements above come from the QEMU patch series and release coverage.
- Whether upstream reims-vgpu's master currently builds and runs on Windows without the MAXZVER fork was not verified; the README does not mention Windows at all.
- No CPU-use or VoiceOver-responsiveness measurements exist for reims-vgpu on any host.
- I did not read the project's `docs/qemu.md`, so exact QEMU command lines for Windows are not captured here.
- Legal position of re-implementing Apple's host interface, and of running macOS on non-Apple hardware generally, was not researched.

## Key Question 7: Other acceleration routes and their status

### Takeaway
Nothing else gives an Intel macOS guest a GPU on a Windows client host. Each alternative either lacks a macOS guest driver, is Windows-guest or Linux-guest only, or needs a Linux or Mac host.

### Cited Findings
- virtio-gpu with Venus or virgl: the current work is host-side for Mac hosts (a UTM developer's December 2025 RFC brings Venus to macOS hosts for Linux guests); no macOS guest driver for virtio-gpu was found, and Apple's `AppleVirtIO.kext` has no display personality — [QEMU RFC: Venus support for macOS, 3 December 2025](https://patchew.org/QEMU/20251203040754.94487-1-j@getutm.app), [kraxel.org](https://www.kraxel.org/blog/2019/06/macos-qemu-guest/)
- Metal-on-Vulkan translation exists as part of reims-vgpu (the metal2vulkan component, consuming Apple-compiled Metal shader libraries); on Mac hosts the Vulkan path runs on MoltenVK — [reims-vgpu README](https://github.com/steelbrain/reims-vgpu/blob/master/README.md)
- Hyper-V GPU paravirtualisation: a Microsoft Q&A answer (not formal documentation) says the current implementation supports DirectX 11.1, OpenGL and basic DirectCompute in Windows 11 and Windows Server guests and does not forward DirectX 12 or Vulkan — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/5605686/is-it-possible-to-add-support-for-directx-12-in-hy)
- Discrete Device Assignment is a Windows Server Hyper-V feature for generation 2 virtual machines — [Microsoft Learn: deploying graphics devices using DDA](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/deploy/deploying-graphics-devices-using-dda), [Windows OS Hub](https://woshub.com/passthrough-gpu-to-hyperv-vm/)
- GPU passthrough to a macOS guest works on a Linux host with IOMMU and a supported AMD card (tested: RX 6600 recommended, RX 570, GT 710 up to Big Sur) — [OSX-KVM notes.md](https://github.com/kholia/OSX-KVM/blob/master/notes.md)
- A Linux virtual machine inside Windows (WSL2) can host KVM and run the accelerated macOS guest, using Direct3D 12 underneath through Mesa Dozen — [reims-vgpu issue 86](https://github.com/steelbrain/reims-vgpu/issues/86)

### Inferences
Verdict for each route, for QEMU with WHPX on a Windows client edition:
- reims-vgpu native on Windows with WHPX: possible, experimental, the only real candidate (see Key Question 6).
- reims-vgpu inside WSL2 with nested KVM: possible today for at least one AMD integrated-graphics user; adds a nested virtualisation layer and a per-frame copy, and is unreliable on NVIDIA. Needs the user to run a Linux environment, which complicates an accessible one-click product.
- Stock QEMU `apple-gfx-pci`: no. Needs a Mac host (Intel Mac for an Intel guest).
- virtio-gpu, virgl, Venus: no. No macOS guest driver exists; writing one would mean a full Metal driver for macOS, which Apple does not permit third parties to supply in any documented way.
- VMware SVGA 3D: no. QEMU's device has no 3D and macOS has no usable driver.
- Hyper-V GPU paravirtualisation: no. It needs Microsoft's guest-side driver model (Windows guests, or Linux through the dxgkrnl driver used by WSL); there is no macOS guest component, and macOS is not a supported Hyper-V guest. The only indirect use is the WSL2 route above.
- Discrete Device Assignment: no on client Windows (Server only), and it is a Hyper-V feature, not available to QEMU with WHPX.
- PCI passthrough from QEMU on Windows: no. QEMU's passthrough depends on Linux's VFIO; WHPX exposes processor virtualisation only. (General knowledge; no source found this session that states it explicitly.)
- Intel GVT-g and SR-IOV graphics: no. Both are Linux-host features, and macOS has no guest driver for Intel virtual functions. (General knowledge; not sourced this session.)
- A Linux host (dual boot or a second machine) with GPU passthrough: works and is mature, but needs a second GPU that macOS supports and is outside the Windows-host constraint.
- A real Mac, or a second machine running macOS natively, accessed remotely: works, outside the constraint. For a VoiceOver user, remote audio latency would be the main concern.

### Gaps
- No authoritative Microsoft statement was retrieved on which guest systems GPU paravirtualisation supports; the Q&A answer is informal.
- No source was found on any QEMU-on-Windows passthrough effort; absence of results is weak evidence that none exists.
- GVT-g and SR-IOV statements are from background knowledge only.

## Key Question 8: Cheaper frame retrieval for a headless viewer

### Takeaway
`screendump` is the most expensive way to fetch frames: every call forces a display update and converts and writes the entire surface with no change tracking. QEMU on Windows has a purpose-built alternative, the D-Bus display with a shared-memory mapping and per-rectangle update notifications.

### Cited Findings
- `qmp_screendump` is a coroutine that calls `qemu_console_co_wait_update(con)` (asking the display device to refresh and waiting for it), then takes a reference to the surface image (no snapshot copy; the source warns "The image content could potentially be updated as the coroutine yields"), and `ppm_save` writes a `P6` header and then, for every row, converts the row to 24-bit RGB with `qemu_pixman_linebuf_fill` and writes it with `qio_channel_write_all`. There is no damage or region tracking; the whole height is converted and written every time. `png_save` follows the same row-by-row pattern through libpng — [QEMU ui/ui-qmp-cmds.c, master](https://gitlab.com/qemu-project/qemu/-/raw/master/ui/ui-qmp-cmds.c)
- The update wait is implemented by scheduling the device's `gfx_update` handler and parking the caller until it finishes — [QEMU ui/console.c, master](https://gitlab.com/qemu-project/qemu/-/raw/master/ui/console.c)
- PPM is the default and is uncompressed; PNG output has been available since QEMU 7.1; a QEMU 7.2 bug swapped red and blue in PNG output — [QEMU commit adding PNG to screendump](https://qemu.googlesource.com/qemu/+/9a0a119a382867dc9a5c2ae9348003bf79d84af2), [QEMU issue 1622](https://gitlab.com/qemu-project/qemu/-/issues/1622)
- D-Bus display (`-display dbus`): a client registers a listener; QEMU calls `Scanout` on resize and `Update(x, y, width, height)` for each changed rectangle. On Windows there are two optional zero-copy interfaces: `org.qemu.Display1.Listener.Win32.Map` with `ScanoutMap(handle, offset, width, height, stride, pixman_format)` ("Resize and update the display content with a shared file mapping object") and `UpdateMap(x, y, width, height)`; and `org.qemu.Display1.Listener.Win32.D3d11` with `ScanoutTexture2d` and `UpdateTexture2d` for a shared Direct3D 11 texture — [QEMU D-Bus display documentation](https://www.qemu.org/docs/master/interop/dbus-display.html)
- Windows support for the D-Bus display was added in June 2023: sockets are passed between processes with `WSADuplicateSocket()` because D-Bus cannot pass file descriptors on Windows, and "When the display surface has an associated HANDLE, we can duplicate it to the client process" (the shared mapping). These changes are in the 8.1/8.2 release line — [QEMU commit "ui/dbus: win32 support"](https://qemu.googlesource.com/qemu/+/6cc5a6159a0067fefbe0b7912c187018aa4b460a), [dbus-display1.xml history](https://grok.dragonflybsd.org/history/qemu/ui/dbus-display1.xml)
- QEMU's VNC server is a workable headless transport for an accelerated macOS guest (used with noVNC in the WSL2 report) — [reims-vgpu issue 86](https://github.com/steelbrain/reims-vgpu/issues/86)

### Inferences
- Per-call cost of `screendump` at the current setup: at 1920x1080 it converts 2.07 million pixels and writes about 6.2 MB of PPM to disk; at 1024x768 about 2.3 MB. The consumer then has to read and decode that file. At 10 fetches per second that is 62 MB per second of file traffic at 1080p, plus a forced device refresh each time. Fetching less often reduces host cost linearly; fetching only on demand (for example only when a sighted helper opens the viewer) reduces it to nothing. For a blind user the picture is needed rarely, so "do not fetch unless a viewer window is visible" is the largest and simplest saving.
- With `-display none` and no polling, QEMU does not run the periodic display refresh at all, which is the cheapest possible host state. Any always-attached display consumer (VNC client, D-Bus listener) re-enables periodic refresh, typically at about 30 times per second while changes occur, backing off when idle. So an alternative transport is only cheaper than `screendump` when frames are wanted continuously; if frames are wanted occasionally, a rare `screendump` is already near-optimal. (General knowledge of QEMU's display refresh logic; not confirmed from source this session.)
- If a live view is wanted, the ranking by host cost should be: D-Bus `Win32.Map` (shared memory, only dirty rectangles signalled, no encoding), then VNC with raw or hextile encoding and its own dirty tracking, then SPICE, then repeated `screendump`. With `vmware-svga` undriven by a guest driver, dirty tracking may degrade to whole-frame updates (see Key Question 3), which would favour switching to `-device VGA` if a dirty-rectangle transport is adopted.
- Frame retrieval affects host processor time only. It does nothing for the guest's own WindowServer cost, which is the larger item.

### Gaps
- No measurement of `screendump` cost versus VNC or D-Bus on Windows was found.
- I did not confirm that the specific Windows QEMU 11.1 build in use was compiled with VNC, D-Bus display or SPICE; `qemu-system-x86_64 -display help` and `-vnc help` will show it.
- The Windows transport for a D-Bus client (how the private peer-to-peer socket is established on Windows in practice) is not described in QEMU's documentation page; the source would need reading.
- ivshmem as a framebuffer channel has no macOS guest driver that I know of and no source was found; it is not a realistic option without writing a guest kext.

## Key Question 9: Does running with the display "off", at minimal resolution, or with Screen Curtain reduce WindowServer work?

### Takeaway
No source shows that Screen Curtain stops compositing; the available (anecdotal) evidence is that it only covers the output. Lower resolution certainly reduces work; a way to make WindowServer skip rendering altogether was not found.

### Cited Findings
- Apple documents Screen Curtain purely as a privacy feature toggled with VO-Shift-F11 (or a three-finger triple-tap on a trackpad), turned off when VoiceOver is turned off; nothing is said about rendering or power — [Apple VoiceOver guide](https://support.apple.com/en-by/guide/voiceover-guide/vo2726/web)
- AppleVis users (June 2024) state that the screen stays on under Screen Curtain and only a blank window covers it, so it saves no power on LCD panels; any saving is attributed to OLED pixels being black — [AppleVis comment 171673](https://applevis.com/comment/171673); a low-vision user reports residual brightness under Screen Curtain — [AppleVis comment 178356](https://applevis.com/comment/178356)
- General guidance attributes WindowServer load to transparency and motion effects, window count and scaled or multiple displays, and recommends Reduce Transparency as a reversible test with before-and-after measurement — [Cindori: WindowServer high CPU](https://cindori.com/how-to/windowserver-high-cpu-mac-fixes), [OSXDaily, 22 April 2026](https://osxdaily.com/2026/04/22/why-windowserver-uses-high-cpu-memory-macos-fix/)

### Inferences
- If Screen Curtain is an opaque black overlay, then in a software-rendered guest it could still help: a compositor that tracks opaque regions does not need to draw what is fully hidden behind an opaque full-screen layer. Whether WindowServer's software path applies that optimisation, and whether applications still render their own window contents off-screen (they almost certainly do), is unknown. It costs nothing to test: measure WindowServer CPU with Screen Curtain on and off during the same VoiceOver navigation task.
- macOS has no supported "headless, do not composite" mode for a logged-in graphical session; VoiceOver itself depends on the window server and the accessibility tree, not on pixels, but applications still draw. Display sleep stops the refresh but VoiceOver use wakes the display. So the realistic levers are: lowest practical resolution (1024x768), Reduce Transparency, Reduce Motion, a solid-colour static wallpaper, no screen saver, no desktop widgets, Dock without magnification or animation, fewer open windows, and avoiding animated web content.
- Display sleep in the guest with VoiceOver still running is worth one experiment: if WindowServer stops compositing while the virtual display is asleep and VoiceOver keeps speaking, that would be the closest thing to a headless mode. I found no source either way.

### Gaps
- No Apple or reverse-engineering source describes what Screen Curtain does inside WindowServer.
- No measurement exists of WindowServer CPU with Screen Curtain on versus off, on real or virtual hardware.
- No documented WindowServer default or boot argument that disables compositing for a headless session was found.
