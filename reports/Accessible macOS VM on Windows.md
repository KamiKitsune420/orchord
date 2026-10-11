# Build orchord on QEMU, OpenCore and wxDragon

Orchord should be a thin, accessible manager that drives an existing engine, not a new emulator. The best-supported plan is QEMU 11 or newer with the Windows Hypervisor Platform accelerator (WHPX), the OpenCore boot loader, a Rust front end on wxDragon (wxWidgets 3.3), and orchord's own console window that sends keys over QEMU's control protocol and speaks state changes through NVDA. No backend on Windows gives macOS a working graphics card, which is why your friend's Big Sur crawled on an RTX 4090; for a VoiceOver user the goal is responsive speech, not frame rate, and that is more achievable. Your literal "the Mac's SSD is a Windows folder" idea is not possible, but "one VM is one folder, with a disk image plus a real Windows folder that appears inside the Mac" is. Two links in the chain are unproven and decide everything else: whether macOS boots under WHPX on your i7-1355U (the notes contain both success reports and an unresolved failure report), and whether guest audio works inside the macOS installer so VoiceOver can talk before installation. No published account was found of a blind person installing Intel macOS in a VM on Windows without sighted help. The research itself tested nothing on your laptop, but both gate experiments have since been run on it, and both passed for Sequoia; see "Hands-on results on your laptop" below, which overrides the research wherever the two disagree.

## Hands-on results on your laptop (9 October 2026)

These were run after the research, on the i7-1355U laptop, with stock QEMU 11.1.0 from winget, the Windows hypervisor accelerator, and the OpenCore 1.0.6 EFI folder from OSX-KVM. They settle the two gate questions.

What works:

- Monterey 12 Recovery boots to its graphical screen under `-accel whpx`. About 4 minutes with 1 virtual core, under 2 minutes with 4.
- Sequoia 15 Recovery boots to its graphical screen too, in about 5 to 6 minutes with 4 virtual cores and 5 GB of memory.
- VoiceOver speaks in Sequoia Recovery. With `-device virtio-sound-pci` and Command+F5 sent over QMP, VoiceOver started, stayed running, and produced about 15 seconds of audio. Windows speech recognition transcribed the recording as "Recovery ... Restore from Time Machine icon image has keyboard focus. You are currently on an image. To begin interacting with the contents of this image press Control Option Shift Down Arrow". The recording is at `lab\vm-test\voiceover-sequoia-recovery.wav`.
- Keyboard and mouse injection over QMP, screenshots over QMP, and the kernel log over the serial port all work headless with `-display none`.

What it took, which orchord must do for every VM:

- Present each virtual core as its own socket: `-smp 4,sockets=4,cores=1,threads=1`. Any layout with more than one core or thread per socket panics the Mac kernel with a divide error in `cpu_thread_alloc`. The kernel's debug output shows why: under WHPX the guest sees "1 core per package; 4 logical cpus per package", two CPUID answers that contradict each other.
- Use the `Skylake-Client,-hle,-rtm` CPU model for Sequoia (it has AVX2). `Penryn` also boots Monterey. `Haswell-noTSX` dies instantly with "WHPX: Unexpected VP exit code 4", the same error as the published failure report, and `host` never leaves the firmware.
- Keep OpenCore's `ProvideCurrentCpuInfo` quirk on. With it off the kernel dies immediately with the same exit code 4.
- The recovery image converts on Windows with `qemu-img convert -f dmg -O raw`; no extra tool is needed.
- A plain Windows folder holding the EFI files can be attached as the boot disk through QEMU's `vvfat` driver, which makes the OpenCore configuration editable without building a disk image.

What does not work:

- VoiceOver in Monterey 12 Recovery. Command+F5 arrives and VoiceOver launches, but it exits within two seconds. Run by hand in the Recovery Terminal it prints "Someone has already checked in my running label. Exiting." This happens with both the VirtIO and the USB sound card, so it is not a sound hardware problem. Monterey's Recovery contains VoiceOver, the Fred voice and Apple's USB audio driver, but no driver for an HDA sound chip.
- So for a blind user, Monterey would be a no-speech installation. Sequoia is the version to build on.

Older versions, tested the same day:

- Big Sur 11 Recovery boots to its graphical screen in about 6 minutes (Penryn CPU model, 4 single-core sockets). After Command+F5, the virtual USB sound card (`-device usb-audio`) produced 6 seconds of loud audio with a speech-like shape, so VoiceOver starts and sound works with no extra driver. Windows speech recognition could not make out the words, so clarity is unconfirmed; the recording is at `lab\vm-test\voiceover-bigsur-recovery.wav`.
- Monterey 12: Apple serves the same recovery build (kernel 21.6.0, August 2022) under both documented board identifiers, and VoiceOver exits in it every time. Confirmed silent installer.
- Catalina 10.15 Recovery boots to its graphical screen in about 5 minutes, but keyboard and pointer input never reach it, with the keyboard on either the XHCI or the EHCI virtual controller. VoiceOver could therefore not be started or tested. Catalina is not usable until that input problem is solved.
- Lion 10.7 was not tested. It would need a different OpenCore configuration, because the Mac model the current one presents (iMac19,1) is far too new for Lion.

Follow-up on Big Sur and Monterey sound, 10 October 2026:

- Big Sur 11.6 Recovery includes Apple's VirtIO driver, and `-device virtio-sound-pci` works there. A 72-second recording of VoiceOver speech had no waveform discontinuities, the same score as the clean Sequoia recording. The published claim that VirtIO sound needs macOS 12 or later is wrong for 11.6. Use VirtIO sound for Big Sur, not USB audio.
- The virtual USB sound card is unreliable on Big Sur: speech starts cleanly, then chunks go missing. Changing the sample rate, the device buffer size or the controller did not fix it. On a UHCI controller the Mac did not activate the card at all.
- Monterey 12 Recovery (the only build Apple serves, 12.5-era, kernel 21.6.0): VoiceOver cannot be made to run. Two faults were found. First, the root volume is mounted read-only, so the `voiceOverOnOffKey` preference cannot be written; `mount -uw /` fixes that. Second, VoiceOver then runs for ten seconds and exits when its check-in for the `com.apple.VoiceOver.running` name fails ("Socket is already connected"). Removing the launchd job with `launchctl bootout` and running the VoiceOver binary directly also exits ("could not toggle keep alive"). Monterey's Recovery does load AppleVirtIO, so sound after installation is expected to work.
- Consequence: Monterey needs an installation that orchord drives itself, with VoiceOver starting after installation.

Full Big Sur installation, started 9 October 2026 at 11:05 pm:

- Stage one (download and prepare from Recovery) took 3 hours 15 minutes on a connection averaging about 1 MB per second, and grew the disk image to 23 GB. The steps were: `diskutil eraseDisk APFS "Macintosh HD" GPT` on the blank disk from the Recovery Terminal, then Reinstall macOS Big Sur, Continue, Agree, select the disk.
- A guest restart does not work under WHPX. When macOS reboots, QEMU reports "WHPX: Unexpected VP exit code 4" and pauses the VM. QMP `system_reset` followed by `cont` does not recover it: the virtual CPUs keep their old state and fault again. Closing QEMU and starting it again does work, because the disk image and the firmware variable file are both on disk.
- The installation completed on 10 October at about 5:55 am. After stage one there were three more boots (about 28 minutes, 3 minutes and 1 minute), each ending in a guest restart that needed a relaunch. The fifth boot reached the Setup Assistant ("Select Your Country or Region") about 100 seconds after launch.
- VoiceOver works on the installed system with VirtIO sound. Command+F5 at the Setup Assistant produced clear speech; Windows speech recognition transcribed the opening announcement, and an 83-second recording had no dropouts (`lab\vm-test\voiceover-bigsur-installed.wav`).
- Measuring audio without ears: count exact-silence gaps of 2 to 40 ms that have loud samples on both sides. That count matched a listener's report (one glitched word in a 72-second clip gave a count of one; the bad USB clip gave ten). A fixed "sudden jump" threshold gives false alarms on loud recordings.
- Orchord must therefore watch for the paused state after every guest restart or shutdown and relaunch QEMU itself. For the later install stages it should relaunch without the recovery image attached, so the only Mac volume OpenCore finds is the one being installed.

Full Monterey installation, 10 October 2026, 6:05 am to about 9:35 am:

- The installer was driven entirely from outside with QMP mouse clicks and typed Terminal commands, because VoiceOver cannot run in Monterey's Recovery. The steps were the same as for Big Sur. Stage one took 2 hours 30 minutes, stage two 33 minutes, then three short boots; five launches in all.
- VoiceOver works on the installed Monterey with VirtIO sound. Command+F5 at the Setup Assistant gave clear speech, transcribed by Windows speech recognition, with no dropouts in an 85-second recording (`lab\vm-test\voiceover-monterey-installed.wav`).
- So Monterey is supportable if orchord performs the installation itself and hands over at the Setup Assistant, where VoiceOver works.
- A guest shutdown works normally on both versions: QEMU exits by itself within about ten seconds. Only restarts need the relaunch.
- In the Recovery Terminal the blank disk was `disk1` on Big Sur and `disk2` on Monterey. Orchord must find the blank disk by size, not assume a number.

Catalina, Lion and Mountain Lion, 10 October 2026:

- Reading installer images offline: a portable 7-Zip (`lab\bin\7zip`) lists the files inside a recovery `.dmg` directly on Windows. That shows which drivers, voices and apps a version's installer carries without booting it.
- Catalina 10.15.7 Recovery has no AppleVirtIO driver, so VirtIO sound cannot work there. It has VoiceOver, ten voices, AppleHDA, AppleUSBAudio and the usual USB controller drivers. Big Sur, Monterey and Sequoia all have AppleVirtIO with its sound driver.
- Catalina keyboard: USB input does not arrive on any tested controller (qemu-xhci, nec-usb-xhci, EHCI, with or without MSI). A PS/2 keyboard does work: add acidanthera's VoodooPS2Controller kext (the OSX-KVM configuration already names it but does not ship it) and launch without the USB keyboard so QMP key events go to PS/2. With that, VoiceOver starts in Catalina's Recovery and the menu bar is reachable with Control+F2.
- Catalina sound: still not working. With `ich9-intel-hda` plus `hda-output` and the VoodooHDA kext (CloverHackyColor release 368, BSD-2-Clause) injected by OpenCore, no audio device appears in Recovery, even with IOAudioFamily force-loaded through `Kernel > Force`. VoiceOver runs silently.
- Lion 10.7 and Mountain Lion 10.8: Apple still serves both recovery images (about 440 MB each, Apple Partition Map layout). With the Mac model changed to MacPro5,1 both restart at the moment OpenCore hands over, with no output. Not booting yet; OpenCore's debug build is the next diagnostic step.
- Sequoia: its VM (`lab\vm-sequoia`, 96 GB disk, Skylake-Client, 5 GB) is prepared and its disk formatted. The installation has not been started; it is waiting at the licence screen for the user's decision.

Design decision from the developer, 10 October 2026: the creation wizard gets a "Use easy install" check box, in the manner of VMware's Easy Install. When it is checked, orchord performs the installation itself: it formats the blank disk, starts the installer, accepts the steps the user has agreed to in the wizard, relaunches QEMU after each guest restart, announces progress, and hands over at the Setup Assistant, where VoiceOver works on every tested version. The two overnight installations were done exactly this way by hand, so every step is known to be drivable from outside. For Monterey, whose installer cannot speak, easy install is the only practical route; the licence must be presented and accepted in the wizard, not clicked silently.

Design decision from the developer, 10 October 2026, on file exchange: the "Mac drive as a Windows folder" feature is dropped, because Windows cannot read or write APFS without a commercial driver. It is replaced by copying from the host to the VM. Each VM gets a transfer folder on Windows that orchord presents to the Mac as an extra disk at start, in the manner of Dolphin's folder-synced SD card. QEMU's `vvfat` driver already does the Windows-to-Mac direction, and it is how every test Mac here was booted. Changes are picked up at the Mac's next start, not live. Copying back from the Mac to Windows is optional and untested. The memory lesson from the Sequoia install also belongs here: a memory-cleaning utility on the host (Mem Reduct) evicted the VM's RAM to the page file and stalled the install for about an hour, so orchord should warn when one is running.

Full Sequoia installation, 10 October 2026, 10:36 am to 4:46 pm:

- Sequoia 15 installs and runs with the Skylake-Client CPU model, four single-core sockets and 5 GB of memory. It took six hours here, but about an hour of that was lost to a host memory cleaner evicting the VM's memory; the download stage alone was about four hours for roughly 14.5 GB, then 40 minutes for stage two and three short boots.
- On the installed Sequoia (kernel 24.6.0) the USB keyboard and tablet do not deliver input, although they do in Sequoia's Recovery (kernel 24.4.0) and on installed Big Sur and Monterey. The fix is the same as for Catalina: load VoodooPS2Controller and launch without the USB keyboard, so QMP key events go to the PS/2 keyboard. Orchord should use the PS/2 keyboard for every version; it worked wherever it was tried.
- VoiceOver works on the installed Sequoia with VirtIO sound: a 45-second recording with no dropouts, transcribed by Windows speech recognition (`lab\vm-test\voiceover-sequoia-installed.wav`). Its first words were "Setup Assistant is not responding", so Sequoia is noticeably slower to settle than the older versions on this hardware.
- On the final boot the OpenCore picker waited for a key press instead of starting by itself. Orchord should set `ShowPicker` to false once installation is complete.
- Command+Q then Return at the Setup Assistant shuts the Mac down cleanly from the keyboard.

Live file exchange, tested 10 October 2026 on the installed Big Sur 11.7.11 (account kitsune):

- A WebDAV server on the host, bound to 127.0.0.1 on port 8580 and serving one folder with anonymous access, is reachable from the guest at `http://10.0.2.2:8580/` through QEMU's user-mode network. `mount_webdav -S http://10.0.2.2:8580/ ~/win` mounted it with no prompt.
- A file added on Windows while the Mac was running was listed and readable on the Mac within three seconds, with no remount. A 20 MB file read back with a matching MD5 in about 1.5 seconds. A file written on the Mac appeared in the Windows folder.
- No administrator rights, no Windows file sharing and no firewall change were needed, because the server listens on loopback only and QEMU forwards to it. This is the mechanism to build: orchord runs its own small WebDAV server per VM and mounts it in the guest at login. The test used the Python `wsgidav` package only as a stand-in; orchord needs a Rust equivalent.
- Not yet tested: behaviour in Finder and with VoiceOver, large files, automatic mounting at login, and the other macOS versions.

First-run notes from setting up that Mac:

- After the Setup Assistant, macOS opens a "Keyboard Setup Assistant" window because it cannot identify the virtual USB keyboard. Orchord should pre-set the keyboard type or dismiss it.
- The ACPI power button (QMP `system_powerdown`) is ignored both at the Setup Assistant and at the desktop. A clean shutdown needs a guest-side command such as `sudo shutdown -h now`, or the Apple menu.
- Setup choices used: region Trinidad and Tobago to match the host, Atlantic Standard Time, no Apple ID, location services off, analytics off, Siri off.

Two more things orchord must do, learned from these runs:

- Set OpenCore's `ScanPolicy` to 66307 so only Mac volumes on SATA are listed. The installer then boots by itself; with the default policy the picker waits for a key press, and simulated key presses to it proved unreliable.
- Keep the virtual hardware layout fixed for the life of a VM. Adding a PCI device moved the disk controller to a new slot, and the firmware then ignored its saved boot entry and dropped to a firmware shell.

Still untested: a complete installation, VoiceOver responsiveness after install, live audio through Windows (only recording to a file was tested), the keyboard capture design with NVDA, OCR, the shared folder, and Ventura, Sonoma and Tahoe.

Changes this makes to the recommendations below: test and default to Sequoia 15, not Monterey; use the socket layout above in place of cores and threads; and treat 4 virtual cores as the working figure.

### Console screen, mouse and crash detection (10 October 2026)

- **NVDA's text recognition can read the Mac directly.** The console now shows only the Mac's screen, in a focusable object named "Mac screen", with the status log swapped in by Control+L. Windows' text recognition, the engine NVDA uses, read the Big Sur login window and the Sequoia set-up screen from that area.
- **The native wx picture control does not scale.** It centres the picture and crops it in a small window. Orchord uses the generic picture control, which scales to fit, and must lay the panel out again after every new picture because setting a picture shrinks the control.
- **The mouse works through QEMU's USB tablet on Big Sur and Sequoia.** The Mac's pointer goes to the spot under the Windows pointer. A click found by text recognition and sent through the window restarted and shut down Big Sur from its login window and selected a country in Sequoia's set-up list. Monterey was not tested.
- **At the Big Sur login window the click target is the icon, not its caption.** Clicking the words "Shut Down" did nothing; clicking the icon above them worked.
- **A kernel panic reaches the serial log.** A deliberately bad processor layout produced `panic(cpu 0 caller ...): Kernel trap ... type 0=divide error` in the serial log 11 seconds after start, and the supervisor reported it. QEMU itself stays running, frozen at the panic.
- **QEMU holds the serial log open for writing alone.** Other programs can read it but not append to it.

### Wizard, easy install, speech and the shared folder (11 October 2026)

- **Speech works through Prism.** The `prismer` crate (Prism 0.18.3, MPL-2.0) hands text to NVDA. Building Prism from source needs Microsoft's ATL headers, which the Build Tools do not install, so the official prebuilt `prism.dll` is used.
- **Apple's installer download works in Rust.** A port of OpenCore's `macrecovery` exchange fetched Monterey's 623 MB image over HTTPS, checked it against Apple's chunklist, and `qemu-img` converted it.
- **Easy install's first stage runs unattended on Big Sur and Monterey.** From a blank disk it opened Terminal in the installer, erased the disk, started Reinstall macOS, accepted the licence, chose the disk, and reported the installer's own "About 2 hours and 19 minutes remaining". Monterey passed on its first attempt. The later stages (restarts and the first set-up screen) use the supervisor's proven restart handling but had not been run end to end when this was written.
- **Windows' text recognition misses pale text on grey buttons.** It could not read "Agree" in the installer's dark windows. Reading each picture a second time, in greys with the bright end stretched, finds those words.
- **QEMU's `send-key` loses key releases when text is typed quickly.** It queues every press with a delay; the queue overflows and keys stay down in the Mac. Explicit press and release events with their own timing are reliable.
- **The pointer must be moved away after a click.** Left on a button, it splits the next page's button name in two for the text recogniser.
- **The installer's buttons appear minutes before they work.** Giving up on a step must be by time (20 minutes here), not by a count of tries.
- **Sequoia's installer is much slower.** It took between 6 and more than 25 minutes to show its first window, depending on what else the PC was doing, and its disk erase takes minutes, not seconds.
- **Starting the installer can crash.** One start in about ten died with `AppleAHCIDiskQueueManager::setPowerState ... timed out`; easy install now restarts a Mac that panics while installing.
- **MSAA is what to check, not UI Automation.** Through UI Automation every wx control shows as a nameless pane. Through MSAA, which is how NVDA reads wx programs, the wizard's fields, boxes and buttons have the right roles and names.
- **The shared-folder server is `dav-server` on Windows' loopback address.** It answers `OPTIONS` with DAV class 2, which macOS needs before it will mount a server for writing.
- **The shared folder works on the installed Big Sur.** One typed line, `curl -s 10.0.2.2:<port>/orchord-setup.sh | sh`, installs a launch agent that mounts the folder at `~/Windows` at every login. A file made on Windows was read on the Mac, and a file made on the Mac appeared on Windows, with no `._` companion files left behind.
- **Typing into the Mac's Terminal must wait for Terminal.** With a fixed pause the first letters of the command were lost. Orchord now waits until text recognition sees Terminal's Shell menu.
- **Easy install's first stage also passes on Sequoia**, after two fixes: its disk erase needs minutes, and Continue pressed too soon after choosing Reinstall macOS opened Time Machine.
- **A whole easy install ran from the wizard to the first set-up screen.** The wizard created a Big Sur Mac, downloaded the installer from Apple, and easy install finished 3 hours 22 minutes later at "Select Your Country or Region" with VoiceOver on (its caption panel was on screen and the Mac was producing sound). It started from Orchord's own boot folder. One step needed a hand: a second press of Continue landed just as the button became Cancel, and the installer asked "Are you sure you want to cancel installation?". The download carried on behind the question but the Mac could not restart until it was answered. Easy install now answers that question itself and waits longer after choosing the disk; that fix has not been through a second full run.
- **USB hand-over does not work on this PC without another driver.** QEMU lists the PC's five USB devices, and adding a `usb-host` device succeeds, but its `attached` property stays false and Windows keeps the device. Neither UsbDk nor WinUSB is in place here. Installing UsbDk is an administrator-level driver installation and was left as the user's decision.
- **Suspend and Resume work from the window.** QEMU reported "paused" and then "running".
- **Orchord's own boot folder works.** Built from the official OpenCore 1.0.8 release, OpenCore's `Sample.plist` plus about thirty settings, and four acidanthera kernel extensions, it booted the installed Big Sur, Monterey (67 seconds to the first set-up screen) and Sequoia (89 seconds). The OSX-KVM configuration in use until now named several files that were never present; OpenCore had been skipping them.

### Installing from an ISO the user already has (11 October 2026)

Tested with `MacOSBigSur.iso`, a 13.2 GB full installer of Big Sur 11.2 that was already on the laptop.

- **An installer ISO is not a CD image.** It is what `hdiutil` writes from a `createinstallmedia` disk: an old Apple partition map and one HFS+ volume. Nothing in it is ISO 9660.
- **Attached as it is, it gives a black screen.** The firmware cannot read an Apple partition map, so OpenCore never sees the volume.
- **Attaching only the volume is not enough either.** OpenCore's scan policy tells Mac volumes apart by their GPT partition type, and a bare volume has none.
- **Loosening the scan policy is the wrong fix.** With the file-system filter off, OpenCore found the installer but also offered its own boot disk, and stopped at its menu waiting for a key.
- **What works is giving the volume a GPT without copying it.** Orchord writes a 32 KB start (protective MBR, GPT header, entries) and a 17 KB end (the spare copies), and a VMDK description that tells QEMU to present start, the volume straight out of the ISO, and end as one disk. The Mac sees what a `createinstallmedia` USB stick looks like. Existing Macs and the boot folder are untouched.
- **QEMU's VMDK reader has a trap.** An empty line before the lines that name the pieces makes it add the first piece twice. The firmware coped, by finding the partition table again inside the partition; macOS did not, and waited for ever for its root device.
- **A full installer is much quicker than Apple's small one.** Its first window appeared 110 seconds after starting, and its first stage, copying to the Mac's disk, took about three minutes, because nothing is downloaded.
- **Easy install needed two changes.** The first window says "Install macOS Big Sur", not "Reinstall macOS Big Sur". And the page that asks for a disk also shows the installer's own volume, named "Install macOS Big Sur" and locked; its name was being taken for the Install button.
- **The wizard checks the file before anything is created.** It looks at what the file holds, not at its extension: an Apple disk image is unpacked as before, a disk with a GPT is used as it is, a Mac volume without one gets the treatment above. A data disc, a Mac disc with no start-up folder (VMware's Tools disc was the test) and a file that is no disc at all are each refused with their own message.
- **The wizard also reads the version in the file's name.** Choosing Sequoia for `MacOSBigSur.iso` brings a question offering to change the version to Big Sur.
- **The whole installation ran from the ISO to the first set-up screen in about 65 minutes**, against 3 hours 22 minutes for the same system downloaded from Apple. It ended at "Select Your Country or Region" with VoiceOver's "Press the escape key to hear..." prompt on screen.
- **OpenCore's menu appeared once, uninvited.** After the fourth restart OpenCore showed its text menu, with "Macintosh HD" marked, and waited for a key; macOS had most likely asked for a one-time start from something that was gone by then, and OpenCore falls back to its menu when a start fails. The next start went straight into macOS. Easy install now presses Return when it reads "OpenCore Boot Menu". That was proved by switching the menu on in a copy of the boot folder: Return started macOS, and the Mac reached the set-up screen. Someone installing by hand would have to press Return there themselves; nothing speaks at that menu.
- **Limits.** The ISO is used where it is, so it must stay there until macOS is installed. Its folder and name must be plain English letters, digits and punctuation, because of how QEMU on Windows reads the description file. Only the Big Sur ISO was tested; Monterey and Sequoia ISOs have the same shape but were not tried.

### First release (11 October 2026)

- **A new user could not set Orchord up without editing a file.** There is now an Orchord Settings dialog (File menu, Control+Comma) for the QEMU folder, the folder for virtual machines and the Apple SMC key, and every "not fully set up" message says where it is. It refuses a folder with no QEMU in it and a key that is not 64 characters long.
- **The boot folder beside the program is no longer written into the settings file,** so an unzipped copy of Orchord can be moved or replaced by a newer one.
- **The program needs the Microsoft Visual C++ Redistributable.** `orchord.exe` loads `VCRUNTIME140.dll`, `VCRUNTIME140_1.dll` and `MSVCP140.dll`. Most PCs have them; the README says how to get them.
- **The release zip was tested as a stranger would meet it.** Unzipped into an empty folder and run with an empty settings folder, it reported what was missing, took the settings through the dialog and saved them. Run with the real settings, it started the installed Monterey and reached its set-up screen in 55 seconds.
- **Shut Down from the Power menu does nothing at macOS's first set-up screen.** The Mac was still running 75 seconds later; Force Power Off stopped it.
- **VoodooPS2Controller's licence was misdescribed.** Its repository's licence file is the Apple Public Source License 2.0, not GPL-2.0; the VoodooInput it carries is GPL-2.0. The licence texts of every bundled part are now in the repository and in the zip.
- **Before the repository was made public its whole history was searched.** The Apple SMC key, the owner's email address and access tokens appear nowhere in it; commits carry GitHub's no-reply address.
- **Orchord can fetch the Apple SMC key when asked.** A button in Orchord Settings downloads OSX-KVM's `OpenCore-Boot.sh` (or the `quickemu` script) from GitHub and takes the key from it. Orchord carries the key's SHA-256 fingerprint, not the key, and tries every run of 64 characters in the file against it, plain and with letters turned by thirteen places; that matters because quickemu stores the key in ROT13. Both sources were tried and both gave the key already in use on the test PC. The same fingerprint lets the dialog ask before saving 64 characters that are not the key. This lowers the project's exposure compared with shipping Apple's text, but it is not a legal guarantee: the program still obtains and uses the key.
- **One button sets a new PC up.** "Check and set up this PC" in Orchord Settings looks for QEMU, the Windows Hypervisor Platform, the key and a text recognition language, and offers to get what is missing; nothing is changed without a Yes.
- **QEMU is installed through winget,** with `--no-upgrade`, so a QEMU that is already there is never replaced. winget answers an existing installation with code 0x8A150061, which Orchord reads as "already installed". On the test PC that is the only path that could be run: installing from nothing, with Windows' permission question, has not been tried.
- **Whether the hypervisor works is asked of QEMU itself.** An empty, paused QEMU is started with the WHPX accelerator and its control channel on standard output. QEMU sets the accelerator up before it opens that channel, so the channel's greeting means "works" and a QEMU that stops without one says why. A first version simply waited three seconds for QEMU to stop, and on a busy PC that was too short: it reported "works" for an accelerator that does not exist. Both answers are now checked by tests, with a made-up accelerator name and with QEMU's own emulator. Turning the feature on (`dism`, behind Windows' permission question) could not be tested, because it is already on here.
- **The Microsoft runtime is built into the program.** With the `crt-static` target feature, which wxDragon's build script follows for wxWidgets, `orchord.exe` no longer loads `VCRUNTIME140.dll` or `MSVCP140.dll`, so it starts on a PC without the Visual C++ Redistributable. A missing runtime could not have been fixed from inside the program, which would not have started.

### AMD processors, a frozen screen reader, and version 0.1.2 (11 October 2026)

- **On an AMD PC a Mac stops on a screen of start-up text.** A friend of the owner tried Big Sur and Sequoia on one; both ended at the last lines of Apple's boot loader (the "Err(0xE)" and "wake-failure" lines that are also there on a start that works), with nothing from macOS itself after them. So macOS dies in its first moments, before it can draw.
- **The likeliest cause was found in QEMU's source, not by experiment.** No AMD PC was at hand. In QEMU 11.1's `target/i386/whpx/whpx-all.c`, a request for a processor register (an MSR) that Windows' hypervisor does not handle is answered with a fault unless the accelerator option `ignore-unknown-msr` is on, and that option is off unless given. macOS asks for several registers only Intel processors have. On an Intel PC the hypervisor handles them; on an AMD PC it does not, QEMU answers with a fault, and macOS, which has no means of surviving a fault that early, stops. Linux's `ignore_msrs` exists for exactly this, and macOS guides for KVM require it.
- **From 0.1.2, on a processor that is not Intel's, Macs are started with `-accel whpx,ignore-unknown-msr=on`.** On the Intel test PC, where the option changes nothing that happens, QEMU accepted it and Monterey reached its set-up screen with it on.
- **On such a processor the Mac is also not promised `pcid`, `invpcid` or `tsc-deadline`.** The processor model Sequoia needs promises all three, many AMD processors have none of them, and macOS uses what it is promised. macOS runs without them; the older model used for Big Sur and Monterey never had them.
- **None of this has been tried on an AMD PC.** The same source shows other differences that may matter: QEMU passes the processor's own answer for CPUID leaf 4 through to the Mac, and an AMD processor answers that leaf with zeros. macOS appears to cope, by falling back to one core per package, which is the layout Orchord uses anyway, but that is reading, not testing.
- **A diagnostic report makes remote testing possible.** Machine, Save Diagnostic Report writes the processor, the QEMU version, the accelerator, the way QEMU is started and the ends of the logs to one file, with the Apple SMC key taken out. The Mac's serial log is in it, and macOS writes to that from its first moments. Two lines that can be added to the settings file, `accelerator` and `extra_qemu_args`, let someone try other settings without a new build.
- **NVDA froze while Orchord downloaded or created something. Three causes were found and removed.** (1) The download reported every megabyte and each report changed the words in the progress dialog, so a screen reader got dozens of events a second; the words now change once in ten seconds and the bar moves at most once a second (measured: four changes in thirty seconds). (2) Several jobs ran on the window's own thread (asking GitHub, fetching the key, probing the hypervisor, writing the report, listing running programs), and a window that is busy does not answer a screen reader, which then waits; they now run on other threads behind a small progress dialog. (3) Text was handed to the screen reader from the window's thread, so the window and the screen reader could each be waiting for the other; speech now has a thread of its own.
- **A progress dialog only learns of Cancel when it is updated.** Throttling the updates broke Cancel until the dialog was again updated on every tick with an unchanged value, which shows and reports nothing.
- **Delete removes only what Orchord made.** The Delete key asks first, with No as the default button, then removes the Mac's disk and `vmconfig` folder. A file placed in a test Mac's shared folder was left, and Orchord said so.
- **Full screen and dark mode come from wxWidgets 3.3.** F11 toggles full screen (the window measured 1920 by 1200 at 0,0 and went back to its old size). The appearance can follow Windows or be forced light or dark; it has to be chosen before the first window exists, so it changes at the next start.
- **The updater was run for real.** A copy of the program made to believe it was version 0.1.0 said at start-up that 0.1.1 was available, showed the release notes, downloaded the zip, checked it against GitHub's SHA-256, replaced its own files and restarted as exactly the published 0.1.1. Windows will not overwrite a running program but lets it be renamed; replaced files get the ending `.orchord-old` and are removed at the next start. A file that was not Orchord's in the same folder was untouched.
- **Tests take the keyboard.** A test copy of Orchord comes to the front when it starts, and a key the owner pressed at that moment started a throwaway test Mac. Test Macs and test settings are separate from the owner's, so nothing was harmed, but it is a reason to keep tests short while the owner is at the keyboard.

## Recommendations at a glance

- Engine: QEMU 11.x or newer with `-accel whpx`, started headless and controlled over QMP. Do not write an emulator or hypervisor from scratch.
- Boot chain: OVMF firmware, an OpenCore image that orchord builds itself from official releases, and a small kext set (Lilu, VirtualSMC).
- First macOS to test: Monterey 12, then Sequoia 15. Monterey answers "does it boot and fit in memory"; Sequoia answers "does the audio that users have confirmed also work in the installer".
- Default macOS to offer users once things work: Sequoia 15. Offer Tahoe 26 with a "slowest" warning. Mark Big Sur and older as experimental for blind installation.
- First VM allocation on your laptop: 6 GB RAM, 4 virtual CPUs (compare against 2), 80 GB thin disk image on the drive with 476 GB free. For Sequoia use 8 GB with other programs closed.
- Language: Rust with wxDragon, ranked first with moderate confidence. C++ with wxWidgets directly is a close second. Do a one-day build spike before deciding.
- UI: native Win32-backed wx controls only, label created immediately before each control, a hand-built wizard on a dialog plus simplebook instead of wxWizard, and spoken status through UI Automation notification events.
- VM folder: `vmconfig/`, `disk/` holding one disk image file, and `drivename/` as a real Windows folder shared into the Mac, optionally mirrored into the Mac's real Downloads, Documents and Desktop by a small guest agent.
- Console: orchord's own window in a separate process, QEMU run with `-display none`, keys injected over QMP, NVDA put to sleep for that process by a bundled add-on. Control+G captures the keyboard; Control+Alt pressed and released on their own releases it, with the release key configurable and both events always announced.
- Safety nets: a "Start VoiceOver in guest" command, a "Read guest screen" OCR command, a screen-change watcher that announces progress, and a guest-audio self-test.
- Fallbacks if WHPX fails on your CPU: VMware Workstation 26 (already installed, has a sound device, but proprietary and patch-dependent) or KVM inside WSL2 (open, but nested and heavier). Hyper-V is ruled out for unassisted blind installs because it has no audio device.

## The laptop can try every Intel macOS; start with Monterey, then Sequoia

### Which versions exist and can be fetched

Tahoe 26 is the final Intel release; macOS 27 is Apple silicon only. In October 2026 only Sequoia 15 and Tahoe 26 still receive updates ([endoflife.date](https://endoflife.date/macos)). That means the target is frozen: no future Intel guest version will ever need support.

A live check on 2026-10-09 from your Windows machine confirmed that Apple's recovery server returns a BaseSystem image and chunklist for every version from High Sierra to Tahoe, using the board identifiers in OpenCore's `recovery_urls.txt` ([recovery_urls.txt](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/recovery_urls.txt), [Apple recovery endpoint](http://osrecovery.apple.com/InstallationPayload/RecoveryImage)). "Apple serves it" is verified. "It boots in a VM on your laptop" is not.

Per version, with recovery image size, support status and the audio path that decides blind usability:

- High Sierra 10.13: 464 MiB recovery image. Out of support since December 2020. Needs the VoodooHDA kext for sound. Experimental.
- Mojave 10.14: 458 MiB. Out of support since October 2021. Needs VoodooHDA. Experimental.
- Catalina 10.15: 476 MiB. Out of support. Needs VoodooHDA. Experimental.
- Big Sur 11: 637 MiB. Out of support. VoodooHDA injected by OpenCore is reported unreliable from 11.3. The worst case for a blind install.
- Monterey 12: 623 MiB. Out of support since September 2024. Oldest version claimed to have a native VirtIO sound driver; not independently confirmed on 12.
- Ventura 13: 674 MiB. Out of support since September 2025. First version that requires AVX2. Native VirtIO sound claimed, not independently confirmed on 13.
- Sonoma 14: 753 MiB. Left support on 14 September 2026. Native VirtIO sound confirmed by a user.
- Sequoia 15: 843 MiB. Supported (15.8.1, 28 September 2026). Native VirtIO sound confirmed by the person who reported it.
- Tahoe 26: 916 MiB. Supported; last Intel release. Native VirtIO sound confirmed by one user. Reported as the slowest in VMs.

The instruction-set requirements do not exclude any version on your CPU: SSE4.1 is needed from Sierra and AVX2 from Ventura ([OSX-KVM README](https://raw.githubusercontent.com/kholia/OSX-KVM/master/README.md)). That your i7-1355U has AVX2 is the researchers' knowledge of the chip, not a sourced statement, but it is not in doubt.

### Reconciling the audio findings with the CPU findings

The notes point at different "first" versions depending on which problem they looked at. Here is how they fit together.

Audio says: use macOS 12 or later. The quickemu project selects `virtio-sound-pci` for Monterey through Tahoe and describes it as a native driver that works with no kext, while Big Sur and older need VoodooHDA with System Integrity Protection disabled ([quickemu wiki](https://github.com/quickemu-project/quickemu/wiki/03-Create-macOS-virtual-machines)). A sound device that needs no kext is the only kind that can possibly speak inside the installer. But the user tests behind that claim cover only macOS 14, 15 and 26, after installation, on Linux or Mac hosts ([quickemu issue 1642](https://github.com/quickemu-project/quickemu/issues/1642)). Monterey and Ventura are inferred, not tested. Audio inside Recovery or the installer is undocumented for every device and every version.

CPU says: under QEMU 11.0 the `-cpu` option is ignored with WHPX, so the guest sees your real Raptor Lake processor, which macOS does not recognize ([QEMU 11.0 WHPX docs](https://qemu.readthedocs.io/en/stable-11.0/system/whpx.html)). The documented fix is OpenCore's `Cpuid1Data` and `Cpuid1Mask` settings, which make the kernel see a Comet Lake model, together with the `ProvideCurrentCpuInfo` quirk ([OpenCore Configuration](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Docs/Configuration.tex)). Proper CPU model emulation for WHPX arrives with the QEMU 11.1 patch series ([WHPX updates for 11.1](https://ratatoskr.run/qemu-devel/2026/04/14442356/t)). One note assumed a named CPU model such as Skylake-Client is the safe default; that holds under KVM on Linux, and on Windows only on QEMU builds that include the 11.1 work.

Putting those together:

- Monterey 12 is the most forgiving first boot test. It is the oldest version with claimed native sound, it does not need AVX2 (so it survives either an old named CPU model or the real CPU with a spoofed model ID), it has the smallest recovery image among the native-audio versions, it has no Liquid Glass, and its suggested 6 GB fits your free memory. The 2023 WHPX success report was also Monterey. Its weakness is that native sound on 12 is unconfirmed and it gets no security updates.
- Sequoia 15 is the right second test and the right default for users. Native sound is user-confirmed, it still gets security updates, and it avoids Tahoe's heavier interface. It needs AVX2 to be visible to the guest and wants 8 GB.
- If Monterey boots but is silent in Recovery, that result is ambiguous (driver missing in 12, or missing in every Recovery image). Booting the Sequoia recovery image the same way removes the ambiguity. Recovery images are under 1 GB each, so testing both is cheap.
- Ventura 13 is the version the experimental GPU project recommends, and it is also the version in the one WHPX failure report. Test it third.
- Tahoe 26 should be tested because it will be patched longest, but last, and with expectations set low for speed.

### RAM, CPU and disk for the first VM

Community defaults are small (OSX-KVM uses 4 GB and 4 threads) while the quickemu maintainer says macOS VMs need at least 8 GB for reasonable performance ([quickemu discussion 1372](https://github.com/orgs/quickemu-project/discussions/1372)). No source gives per-version numbers for unaccelerated VMs, so the figures below are the researchers' synthesis and are untested.

- Monterey 12 first test: 6 GB RAM, 4 virtual CPUs, 80 GB thin qcow2 image.
- Sequoia 15: 8 GB RAM with other programs closed; fall back to 6 GB and expect swapping if Windows starts paging. 4 virtual CPUs. 96 GB thin image.
- Tahoe 26: 8 GB RAM, 4 virtual CPUs, 128 GB thin image. A 32 GB disk is too small because the full installer alone is about 17 GiB.
- High Sierra to Catalina, if you try them: 4 GB RAM, 2 virtual CPUs, 64 GB thin image.

On CPU cores: your chip has only two performance cores (four threads). The one detailed guide on unaccelerated macOS desktops says to keep virtual CPUs on performance cores, never on efficiency cores, and not to over-allocate ([OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)). That guide is a single author on Linux. On your laptop the realistic choices are 2 virtual CPUs or 4; never default to 8 or 12. Start with 4 and compare with 2. How the Windows hypervisor places WHPX virtual CPUs on performance versus efficiency cores, and whether orchord can pin them, was not found; restricting the QEMU process to the performance-core threads is an untested idea worth trying if speech stutters.

On memory: with 6 to 7 GB free, an 8 GB guest forces Windows to page unless you close things. The wizard should compute a ceiling from free physical memory and warn, not block.

On disk: thin images cost little up front. Put the image on NVMe; the drive with 476 GB free is the obvious home. Budget roughly 20 GB for an installed system plus a 12 to 18 GB download that happens inside the VM during installation.

## VMware was slow because macOS draws every frame on the CPU

### Why Big Sur crawled on 32 GB and an RTX 4090

VMware gives macOS guests no 3D or Metal acceleration on any non-Apple host. The whole Big Sur desktop, which is composited with Metal, is drawn by Apple's software renderer on the guest's virtual CPUs. The RTX 4090 is simply not used. The OC4VM project lists "cannot use the Apple para-virtualised GPU on non-Apple hardware" as a hard limit ([OC4VM](https://github.com/DrDonk/OC4VM)), and a VMware staff statement (seen only as a search snippet) says 3D acceleration is not provided for macOS guests ([VMware Communities](https://communities.vmware.com/t5/VMware-Fusion-Discussions/3d-Acceleration-for-MacOS-Guest/td-p/2290524)).

A second cause is likely. Windows 11 24H2 and later turn on virtualization-based security by default, which forces Workstation off its own hypervisor and onto Microsoft's, adding CPU and disk overhead ([Broadcom KB 417896](https://knowledge.broadcom.com/external/article/417896/)). How much of your friend's slowness came from each cause was not measured. One how-to claims VMware's "Accelerate 3D graphics" checkbox helps Sequoia; it has no measurements and contradicts the other sources, so treat it as unverified.

Big Sur is also a poor starting point because it is the first release with the heavily translucent redesign. That older versions are lighter is plausible reasoning; no benchmark was found.

### What would actually be fast

Be clear about what "fast" can mean. No route on Windows 11 Pro gives macOS a dependable GPU. Every practical backend uses a plain framebuffer and software rendering. For a blind user that matters less than it did for your friend: what you need is low-latency audio and input and enough CPU left over for speech. Reducing what gets drawn (Reduce Motion, Reduce Transparency, low resolution, Spotlight indexing off) directly frees CPU for VoiceOver ([osx-optimizer](https://raw.githubusercontent.com/sickcodes/osx-optimizer/master/README.md)). There are no measurements or even anecdotes of VoiceOver responsiveness in a QEMU macOS guest on Windows, so this is an expectation, not a result.

Backend ranking from the evidence:

- First: QEMU 11.x with WHPX and OpenCore. Open source, has sound and USB device models, works with virtualization-based security left on, is controlled over QMP, and is the only engine the experimental GPU device plugs into. Risks: thin real-world evidence, fixes carried in a fork, CPU handling that differs between 11.0 and later builds, and the main WHPX developer saying he may step back.
- Second on paper, but disqualified for orchord: native Hyper-V with MacHyperVSupport. Probably the lowest overhead, but Hyper-V exposes no audio device to the guest; the maintainer closed the request as not planned ([OSX-Hyper-V issue 9](https://github.com/Qonfused/OSX-Hyper-V/issues/9)). Tahoe support is still "in progress". No audio means no VoiceOver during install.
- Fallback A: VMware Workstation with the unlocker or OC4VM. It has an emulated sound card and is already on your machine, so it is a useful comparison baseline. It depends on patching a proprietary product, a Workstation update silently removes macOS support, and it is no faster with virtualization-based security on ([macos-on-vmware-workstation](https://github.com/aesiddiqui/macos-on-vmware-workstation)).
- Fallback B: KVM inside WSL2, the route the dockur/macos container uses. It reuses the best-tested macOS-on-QEMU setup, but macOS becomes a nested guest, it needs WSL and a container runtime installed, and getting audio back to Windows was not researched ([dockur/macos](https://github.com/dockur/macos)).
- Not viable: VirtualBox (degrades under Hyper-V, no macOS guest tools), Intel HAXM (archived in 2023), crosvm's Windows backends (untested upstream), and pure emulation without a hypervisor.

Turning virtualization-based security off would give VMware its native engine back, but it removes QEMU's only Windows accelerator and asks blind users to edit boot configuration and reboot. Design orchord to run with it on.

### The WHPX conflict: both reports, and the test that settles it

The notes disagree on whether macOS boots under WHPX at all. Both sides are real reports.

Success reports:

- February 2023: Monterey on QEMU 7.2.0, Windows 10, a 2018 Intel i5, with `-cpu Westmere` plus extra flags. The author says the system freezes without the xsave flag and that the firmware files had to be merged into one. Described as good for testing, not a reliable system ([digitalfoundations](https://www.digitalfoundations.com.au/blog/macos-on-windows-with-qemu/)).
- 9 October 2026: Tahoe 26.7.1 on Windows 11 Pro build 26200, Intel Core Ultra 9 185H (a hybrid chip), QEMU 11.0.50 built from the MAXZVER fork, 8 virtual CPUs and 16 GB. With the plain display device: "Boots, logs in, SSH works. No issues." ([MAXZVER issue 1](https://github.com/MAXZVER/reims-vgpu/issues/1)).
- The same fork's README: Ventura on an Intel i7-14700K (hybrid) with 8 virtual CPUs and 12 GB ([MAXZVER/reims-vgpu](https://github.com/MAXZVER/reims-vgpu)).

Failure report:

- macOS 13.7.8 under `-accel whpx` on Windows 11 24H2 with an AMD Ryzen 7 9700X, using a QEMU 11.0.50 development build plus four upstream WHPX patches, dies in the earliest kernel boot with "WHPX: Unexpected VP exit code 4". The same guest boots under KVM. There is no maintainer response and no workaround ([QEMU issue 4385](https://gitlab.com/qemu-project/qemu/-/work_items/4385)).

How to read this. The reports differ in host CPU vendor (Intel versus AMD), QEMU build (a fork carrying extra WHPX patches "mostly pending upstream" versus a development build with four patches), Windows build (25H2 versus 24H2) and guest version. Your laptop resembles the success cases on two counts: Intel hybrid CPU and build 26200. But the 2026 successes come from one small fork, no success was found on stock upstream QEMU from 2025 or 2026, and none of the machines was a U-series chip with only two performance cores. Neither side should be treated as settled.

The test that settles it is the first hardware experiment below: boot a Monterey recovery image on your i7-1355U under the newest upstream QEMU with `-accel whpx`. If it fails, repeat with the fork build, then run the same VM under pure emulation as a control. If it boots under emulation but not WHPX, the fault is in the accelerator. Everything about audio and keyboard depends on this answer, so do it first.

Known WHPX hazards to carry into that test:

- Leave the interrupt-controller setting at its default on QEMU 11 with Windows 11. The old `kernel-irqchip=off` workaround now causes hangs on 10.x builds ([QEMU issue 3178](https://gitlab.com/qemu-project/qemu/-/issues/3178)).
- Timestamp-counter flags are the first thing to vary if a guest freezes early ([QEMU issue 3053](https://gitlab.com/qemu-project/qemu/-/issues/3053)).
- Be ready to merge the firmware code and variables into one file if separate files crash, as in the 2023 report.
- Newer builds offer `-accel whpx,ssd=off`, which trades a security mitigation for faster device access ([QEMU master WHPX docs](https://www.qemu.org/docs/master/system/whpx.html)).

Whether QEMU 11.1 has shipped as a stable release was not confirmed.

### Writing a new emulator does not make sense

With Microsoft's hypervisor loaded, a new engine would have to use the same Windows Hypervisor Platform interface QEMU already uses, so it could not be faster at the CPU level. It would also have to recreate what macOS needs before it boots: UEFI firmware, a Q35 chipset, the Apple SMC device, AHCI disks, USB with keyboard and tablet, a sound device, a network card and a framebuffer ([OSX-KVM boot script](https://github.com/kholia/OSX-KVM/blob/master/OpenCore-Boot.sh)). QEMU's own WHPX layer needed a 37-patch series in 2026 just to get CPU identification, state saving and interrupts right. The researchers' judgment, not a sourced estimate, is several person-years to reach what QEMU does today, for zero speed gain. A pure emulator with no hypervisor would be far slower still.

The only place new code changes performance is graphics, and that work exists: Reims vGPU, a QEMU device that macOS's built-in paravirtual GPU driver attaches to, translating Metal to Vulkan on the host ([steelbrain/reims-vgpu](https://github.com/steelbrain/reims-vgpu)). Its Windows port is alpha. The fork reports 90 to 118 Hz on one NVIDIA desktop with Ventura; the only independent test got about 1 frame per second on Tahoe with visual glitches. It has never been reported on Intel Iris Xe. Treat it as an optional experiment behind a flag, and as a strategic reason to choose QEMU, not as a dependency.

Orchord's value is above the engine: accessible setup, building the OpenCore image, audio routing, VoiceOver-friendly defaults, and telling the user what is happening.

## Rust with wxDragon ranks first; C++ is a close second

Only two compiled routes to wx are credible in 2026: C++ with wxWidgets directly, and Rust with wxDragon. The other Rust bindings are abandoned alphas.

The front-end language has no effect on guest speed. QEMU and the Windows hypervisor do the virtualization; orchord only starts the process and exchanges small messages. This is reasoning from the architecture, not a benchmark.

Why Rust ranks first:

- Two 2026 screen-reader-first applications are built on wxDragon by developers who test with NVDA: the Paperback document reader and the hn-blind Hacker News client ([Paperback](https://github.com/trypsynth/paperback), [hn-blind](https://github.com/garo-pro/hn-blind)).
- wxDragon is actively released (eight releases in about five months), bundles wxWidgets 3.3.3, and has had setters for accessible name, role, description, value and state since version 0.9.18 ([wxDragon releases](https://github.com/allendang/wxdragon/releases)).
- A ready crate, live-region, delivers prioritized announcements verified with NVDA ([live-region](https://github.com/trypsynth/live-region)).
- Most of orchord's risk is outside the UI: process supervision, QMP, resumable multi-gigabyte downloads, building disk images, Win32 calls. Rust has a QMP crate (`qapi`) and the official `windows` crate. No C++ QMP library was found ([qapi](https://docs.rs/qapi/latest/index.html)).

What counts against Rust:

- wxDragon is pre-1.0, effectively single-maintainer, with recent breaking changes and a fix for reading freed memory in its latest release.
- `wxWizard` and `wxListbook` appear not to be wrapped. That does not hurt, because the wizard should be hand-built anyway.
- Newer wx accessibility functions reach Rust only after the binding adopts them. A request to expose `SetAccessibleName` and `EnableFocusFromKeyboard` was open with no maintainer reply ([wxDragon issue 229](https://github.com/AllenDang/wxDragon/issues/229)).
- The Rust toolchain is a superset of the C++ one: you need the Visual Studio C++ build tools and CMake either way, plus rustup, Ninja and possibly LLVM. Sources conflict on whether the first build uses prebuilt libraries (under 3 minutes) or compiles wxWidgets (long).
- rustc's default error format is full of visual alignment that is noisy with a screen reader. Running cargo with `--message-format=short` gives one line per error; that tip is background knowledge, not sourced.

What favors C++: the complete wx API including full `wxAccessible` subclassing, new accessibility functions the day they merge, a choice between the stable 3.2.11 line and 3.3.3, prebuilt MSVC binaries, and documented static single-file builds ([wxMSW install docs](https://docs.wxwidgets.org/3.3/plat_msw_install.html)). Against it: QMP must be hand-written, template errors are long, and no current accessible-first C++ wx application was verified.

Switch to C++ if any of these is true:

- You already write C++ fluently and have no Rust experience. The researchers flagged your existing fluency as probably the single largest factor, and they did not know it.
- The spike shows wxDragon cannot do something essential: frame-level key hooks, list activation events, a custom accessible object with child elements, or passing results from a worker thread to the UI.
- You need wxWidgets 3.3.4 accessibility functions before wxDragon exposes them, or you decide to stay on the stable 3.2 line.

The whole toolchain can be installed from the command line with winget and rustup, which is the most screen-reader-friendly path. That is inferred from documented silent-install options ([rustup MSVC prerequisites](https://rust-lang.github.io/rustup/installation/windows-msvc.html)); no first-hand report from a blind developer was found. NVDA's own source build requires the same Visual Studio C++ workload, which is a precedent. Nothing was built on your machine for this report, so build time, executable size and antivirus behavior are unmeasured.

Python is out, as you decided. The only cost is that OpenCore's recovery downloader (`macrecovery.py`) is Python and must be reimplemented; the protocol is one GET, one POST and two downloads, described below.

## Making the wx interface fully accessible

Nobody drove a screen reader during this research. The control advice comes from source code, issue trackers and a structural probe of wxWidgets 3.3.3 run on your machine, not from listening to NVDA. JAWS and Narrator evidence is nearly absent. Test every screen yourself.

### Ground rules

- Use wx classes that wrap real Win32 controls: list box, list control, tree, choice, text field, button, check box, gauge, spin control, notebook. Windows then exposes them to screen readers itself.
- Create a static text label immediately before each control. On Windows, edit fields and combo boxes take their name from the label created just before them, and creation order is also tab order ([wxPython forum](https://discuss.wxpython.org/t/creating-nvda-accessible-applications/40334)). Write one helper that creates label then control, as NVDA's own interface code does ([NVDA guiHelper.py](https://raw.githubusercontent.com/nvaccess/nvda/master/source/gui/guiHelper.py)).
- Avoid the wx-drawn composites unless tested: `wxDataViewCtrl`, `wxSpinCtrlDouble`, `wxFilePickerCtrl`, `wxInfoBar`, `wxListbook`. The local probe found that a data view list reports its name as "dataviewCtrl" and puts column values in the description, not in cells.
- Leave dark mode off for the first release. wxWidgets' opt-in dark mode made check boxes read as plain buttons with no checked state in 3.3.1; it was fixed later, but it shows the risk ([wxWidgets issue 26184](https://github.com/wxWidgets/wxWidgets/issues/26184)). In the default mode, native controls follow Windows high-contrast themes on their own.
- Every action gets a menu item. Shortcuts are conveniences, never the only route.
- Never speak what focus already conveys. Add speech only for things that happen without a focus change.

### Main window and VM list

Use a `wxListBox` with one line per VM, for example "Sequoia test, stopped". It is the lowest-risk control and is read natively with position information. If you want columns, use a report-mode `wxListCtrl` and put the most identifying text in the first column.

The empty-state item is a sound pattern. An empty native list gives a keyboard user nothing to land on, and a static text message is not reachable by keyboard. A single placeholder row with your text, "No virtual machines created yet. To create one, go to File > New Virtual Machine, or press Control plus N.", is discoverable exactly where focus lands at startup. Design points, which are reasoning and not sourced guidance:

- Make Enter on the placeholder open the wizard.
- Track the placeholder with a flag, not by comparing its text. Disable Boot, Settings and Delete while it is shown, and make Control+P say "No virtual machine selected".
- Remove it the moment a real VM exists and select the new VM.
- The screen reader will say "1 of 1" for it; writing it as a sentence makes clear it is not a VM.
- One thing to test: the greater-than sign in "File > New Virtual Machine" may be spoken as a symbol or skipped depending on the user's punctuation level. "File menu, New Virtual Machine" avoids that. This is my suggestion, not a finding.

Enter to boot: `wxListCtrl` has a native activation event that fires on Enter and double-click. `wxListBox` has only a double-click event, so catch Enter with a key handler on the list. Do not make Boot a default button on the frame. This is background knowledge of wx and was not verified.

Control+P for configuration: no comparable VM manager uses it (VirtualBox uses Control+S, VMware Control+D, Hyper-V Control+O), and it clashes with nothing in a program that has no Print command. Keep it, and add Alt+Enter as a second accelerator, since that is the Windows-wide convention for properties (background knowledge). Put the same command in the context menu, and handle the context-menu event so Shift+F10 and the Applications key work.

Use a native `wxNotebook` for the configuration dialog's pages.

### The four-page creation wizard

Build it by hand on a `wxDialog` with a `wxSimplebook`. The stock `wxWizard` changes neither the title nor sends any accessibility event when the page changes, and its button labels contain literal less-than and greater-than characters ([wizard.cpp](https://raw.githubusercontent.com/wxWidgets/wxWidgets/master/src/generic/wizard.cpp)). wxDragon wraps the simplebook but apparently not the wizard, so this choice works in both languages.

On every page change: set the dialog title, for example "New Virtual Machine, step 2 of 4, macOS version"; label the buttons plainly as Back, Next and Finish; and move focus to the first control of the new page, so the screen reader reads the title and then that control. Create controls in reading order with buttons last. Make Next the default button and give Cancel the standard cancel identifier so Escape works.

- Page 1, name and location: a labeled text field for the name; a labeled text field plus a "Browse..." button for the folder, built by hand as NVDA does, not with `wxFilePickerCtrl`.
- Page 2, macOS version and installer source: a `wxChoice` for the version, with each entry saying what to expect, for example "Sequoia 15, speech expected during installation" or "Catalina 10.15, experimental, no speech during installation". A radio box or second choice for the source: download from Apple, or use a local file. Arrow keys move and select in a radio group; Tab leaves it. For a local file, show a labeled text field and Browse button. Be clear about what local files work: a recovery image with its chunklist, or a prepared installer or existing VM disk image. Apple's full InstallAssistant package (about 17 GiB for Tahoe) is not directly bootable.
- Page 3, RAM, CPU cores and SSD size: integer `wxSpinCtrl` controls with the unit in the label, such as "Memory (GB):". Never `wxSpinCtrlDouble`, which has no accessible name ([wx ticket 19222](https://groups.google.com/g/wx-dev/c/nEaCaK6vHZU)). Avoid sliders. Pre-fill per-version defaults, compute a memory ceiling from free RAM, and warn in a message box when the user exceeds it.
- Page 4, summary: a read-only multi-line text field the user can arrow through (read-only single-line fields are not reachable by Tab until wxWidgets 3.3.4), then the "Boot machine after setup completes" check box as a plain `wxCheckBox`, then Finish.

Validation failures should be spoken and should move focus to the offending field.

### Progress for long downloads

- Use a real, determinate `wxGauge`. It is the native progress bar, so NVDA's own beep and speech settings for progress bars apply ([NVDA behaviors.py](https://raw.githubusercontent.com/nvaccess/nvda/master/source/NVDAObjects/behaviors.py)). Indeterminate bars are silent in NVDA, so always supply a real value when a byte count is known and announce the start and end of phases whose length is unknown.
- Beside it, keep a text line the user can read on demand, such as "Downloading macOS Sequoia recovery image: 420 of 843 megabytes, 50 percent, about 3 minutes remaining."
- Add a Cancel button, with Escape mapped to it and a confirmation.
- Never move focus when progress changes. Do not rewrite the window title or a focused label several times a second.
- Speak only at start, on stage change (download, verify, build disk, done), on error and on completion.
- Keep all wx calls on the main thread; run downloads and hashing on workers and throttle UI updates to a few per second.

### Speaking custom messages

The recommended primary channel is a UI Automation notification event raised from a real provider attached to orchord's window. Raising it on the default host provider returns success and reaches nobody; the live-region crate documents the working technique and maps three priorities (queue, replace stale messages, interrupt) ([live-region source](https://raw.githubusercontent.com/trypsynth/live-region/master/src/windows.rs)). It was verified with NVDA only.

Two limits to design around:

- NVDA ignores these notifications when orchord is not the focused application ([NVDA issue 16862](https://github.com/nvaccess/nvda/issues/16862)). For "download finished" while you are elsewhere, also raise a toast notification or speak through the NVDA controller client.
- After a menu command or a dialog closes, NVDA's focus announcements talk over anything said at the same moment. Paperback delays such announcements by about 30 ms after a dialog and 100 ms after a popup menu, and merges two messages into one ([Paperback pull request 943](https://github.com/trypsynth/paperback/pull/943)). Build one "announce for command" helper from the start.

## Reusable open-source components and their licenses

Where a license is listed as not checked, the research did not fetch it; confirm before shipping.

Engine, firmware and boot:

- QEMU: the hypervisor front end and device models, run as a separate process. The notes describe it as GPL version 2 from background knowledge, not from a fetched page. Running it as a child process keeps orchord's own code free of that license.
- OVMF (EDK2) UEFI firmware: comes with QEMU builds. License not checked.
- OpenCorePkg: boot loader, plus the macrecovery downloader and board database. BSD 3-clause. Release 1.0.8 dated 2026-09-27 ([OpenCorePkg license](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/LICENSE.txt)).
- Lilu and VirtualSMC: the minimal required kexts. WhateverGreen, VoodooPS2 and USBToolBox are conveniences. Licenses not checked.
- VoodooHDA: sound for 10.13 to 11.x only. License not checked.
- OSX-KVM: the reference command line and OpenCore configuration. It has no license file, so read it but do not redistribute its prebuilt OpenCore image or firmware; build your own image from official releases.
- quickemu: per-version device choices and download logic to study. MIT. Docker-OSX: GPL version 3 or later.
- MacHyperVSupport and OSX-Hyper-V: only if a Hyper-V backend is ever added. Both BSD 3-clause.
- OC4VM: an OpenCore disk for VMware without patching VMware. License not checked.
- Reims vGPU: experimental graphics. LGPL 3.0 or later upstream; the Windows fork's license statements are inconsistent.

Installer download:

- The recovery protocol in `macrecovery.py` (part of OpenCorePkg, BSD 3-clause) is plain HTTP: get a session cookie from Apple's recovery server, post a short text body with a board identifier, receive the image and chunklist addresses with tokens, download both, and verify the image against the chunklist ([macrecovery.py](https://raw.githubusercontent.com/acidanthera/OpenCorePkg/master/Utilities/macrecovery/macrecovery.py)). Reimplement it in Rust or C++. The transport is unencrypted, so implement the chunklist verification.
- Keep the board identifier table in a data file and sanity-check the result at run time, because the mapping changes only when Apple changes what it serves.
- gibMacOS: an alternative that reads Apple's software update catalog. MIT. Python, so a reference only.
- No conversion step is needed: OpenCore boots the downloaded image directly from a folder named `com.apple.recovery.boot` on a FAT32 volume next to the EFI folder ([Dortania guide](https://dortania.github.io/OpenCore-Install-Guide/installer-guide/windows-install.html)). That avoids dmg2img, which is GPL 2.0 and a Unix tool. A Rust FAT library is believed to exist but was not verified.

User interface and speech:

- wxWidgets 3.3.3. License not checked in the research.
- wxDragon: Rust bindings. Apache 2.0 or MIT.
- live-region: UI Automation announcements for wxDragon applications. MIT.
- NVDA controller client: direct NVDA speech and braille from any program. LGPL 2.1; ship the DLL beside the executable ([controller client readme](https://github.com/nvaccess/nvda/blob/master/extras/controllerClient/readme.md)).
- Prism: the 2026 successor to Tolk and SRAL, covering several screen readers and speech engines. MPL 2.0. It has no tagged releases, its Rust bindings are unsettled, and it needs a C++23 compiler ([Prism](https://github.com/ethindp/prism)).
- Tolk (LGPL 3) and SRAL (MIT, archived) are stale; do not start on them.
- Paperback and hn-blind: both MIT, the best architecture references.
- NVDA's own interface source: control-level patterns that apply to wx in any language.

Control protocol:

- `qapi` crate version 0.15.0 for QMP in Rust. License not checked. In C++ you write QMP yourself over a JSON library.
- The official `windows` crate for Win32, UI Automation and the hypervisor interface. License not checked.

File sharing and file systems:

- SMBLibrary: a user-mode SMB 1, 2 and 3 server library in C#. Described as open source; license not checked. From Rust or C++ it would run as a helper process.
- nfsserve and nfs3_server: user-mode NFS version 3 servers in Rust. Licenses not checked.
- apfs-fuse: read-only APFS driver for Linux. GPL 2.0. Experimental.
- libfsapfs: read-only APFS library. LGPL 3 or later. Experimental.
- The Rust `apfs` crate: read-only parser. MIT. No encryption, snapshots or compressed extents.
- HFSExplorer: reads HFS and HFS+ volumes, extract only. GPL 3 or later. Java.
- WinFsp: user-mode file systems on Windows. GPL 3 with an exception for open-source projects; its installer adds a kernel driver and needs administrator rights.
- Dokan: similar. LGPL for the library and driver, MIT for tools.
- There is no trustworthy open-source APFS writer.

Screen reading:

- The Windows OCR interface (`Windows.Media.Ocr`) is part of Windows, works offline and depends on installed language packs ([Microsoft OcrEngine](https://learn.microsoft.com/uwp/api/windows.media.ocr.ocrengine)).

## The Mac's disk cannot be a Windows folder; the closest design keeps the feel

### Why the literal idea fails

macOS boots only from a block device holding a real APFS structure (or HFS+ on very old releases). From Big Sur on, it boots from a cryptographically sealed snapshot whose top-level hash is signed by Apple ([Eclectic Light](https://eclecticlight.co/2020/11/30/is-big-surs-system-volume-sealed/)). Nothing in QEMU, Hyper-V, VMware or VirtualBox manufactures HFS+ or APFS from a folder. QEMU's one folder-as-disk driver produces FAT only, and its documentation forbids exactly your use case: writing to the folder from the host while the guest has it mounted ([QEMU block drivers](https://www.qemu.org/docs/master/system/qemu-block-drivers.html)).

The deeper reason, which is the researchers' reasoning: a disk is a contract in which the guest's file system owns every block and caches it in memory. Changing blocks underneath a mounted volume means the guest either does not notice or sees corruption. Live two-way sharing needs a file-level protocol, where the guest asks the host for each operation. A third party also cannot produce Apple's seal, and an NTFS folder cannot natively hold Unix permissions, extended attributes, resource forks or names containing characters Windows forbids.

No project was found that implements a folder-backed HFS+ or APFS disk, and no existing product gives a macOS guest a "disk is a folder" experience. Network boot does not help either: it is deprecated and delivered a disk image, not a folder.

The honest product message is: the Mac's system disk is one file; your documents folder is a real folder.

### The closest workable design

Each VM stays one folder, with one substitution:

- `MyMac/vmconfig/` holds the VM's configuration, the OpenCore boot image, the firmware variable store and logs.
- `MyMac/disk/macos.qcow2` is the Mac's SSD: system and home folder in one growing image file.
- `MyMac/drivename/` is a real Windows folder that the running Mac sees as a mounted volume. Give it subfolders named Downloads, Documents and Desktop if you use the mirroring agent below.

Keep the Mac's home folder on the local disk image. Redirecting the home folder, or Downloads, Documents and Desktop, onto a network share is a known source of breakage: sandbox containers, privacy-protected folders, Spotlight and login hangs.

To honor your exact example, "drop a file into the Mac's Downloads from Windows and it appears", run a small agent in the guest that two-way syncs `drivename/Downloads` with the Mac's real Downloads folder, and likewise Documents and Desktop. The Mac folders stay real local folders, and Explorer shows a normal folder. The costs: files exist twice, conflicts need handling, and the agent needs a one-time privacy consent in macOS. An agent can also play a sound or announce "new file from Windows", which hides Finder's unreliable refreshing. This is the closest achievable approximation and it is unvalidated by experiment.

### How the folder reaches the Mac

The mechanism every Linux-host project uses, virtio-9p, is not available: QEMU's 9p server does not build for Windows hosts, confirmed against the QEMU source in October 2026 ([QEMU 9pfs build file](https://github.com/qemu/qemu/blob/master/hw/9pfs/meson.build)). virtio-fs needs a Linux-only host daemon. QEMU's built-in SMB option needs Samba on the host.

Options without administrator rights, in the researchers' order of preference:

- An SMB server embedded in orchord, listening on the loopback address at a high port, reached by the guest through QEMU's built-in user-mode network. Best Finder integration and the only candidate with change notification. Must be tested: whether Apple's SMB client accepts a non-standard port (documented only for its BSD ancestor), and compatibility with Apple's client. If the port is the problem, QEMU's `guestfwd` option can present the server to the guest on the standard port. In a Rust or C++ program the known library, SMBLibrary, is C# and would run as a helper process.
- An embedded WebDAV server. Easiest to build and mounts without root, but Finder is slow with WebDAV and it has no change notification.
- An embedded NFS version 3 server. Available as Rust crates, so it fits a Rust program in-process. Needs a root mount in the guest, has no change notification, and litters the Windows folder with dot-underscore sidecar files.
- Windows' built-in file sharing. The most robust server, but creating a share needs elevation, the Mac must log in with a real Windows account and password (anonymous access is effectively gone on Windows 11 24H2 Pro), and many users sign in with a PIN and do not know a usable password.

Windows' own file server owns port 445, which is why an embedded server must use another port ([zzz.buzz](https://zzz.buzz/2020/04/26/forward-multiple-remote-smb-servers-on-windows/)). Finder's live refresh of SMB folders has been unreliable in recent releases, with reports through macOS 26.3 ([TidBITS Talk](https://talk.tidbits.com/t/26-3-finder-no-longer-live-updates-smb-shares/32950)); the sync agent sidesteps that.

Mount the share with a per-user launch agent after login, retrying until the host answers, and name it something like "Windows Shared". Automounting through the system automounter is fragile and reportedly broke on Tahoe.

A complementary reverse direction is worth prototyping: turn on macOS File Sharing in the guest and let Windows browse the Mac's real folders while the VM runs, the way Windows reaches WSL files.

### The disk image itself

- Format: qcow2 by default. It supports snapshots and stays compact when copied by Explorer. Raw is simpler and may be faster, but copying a sparse raw file with Explorer inflates it. No benchmark was found.
- Attachment: AHCI (SATA) with discard enabled and the disk marked as an SSD, then `sudo trimforce enable` in the guest so the image can shrink. Offer VirtIO block as a speed option for Catalina and later. Sources conflict on whether VirtIO block passes TRIM, so test it. macOS has no SCSI disk support on Intel.
- Offline browsing: an in-app, read-only "Mac disk browser" is feasible with the open-source APFS parsers. It must never open the image for writing and should not open it while the VM runs. Pushing a file into a powered-off Mac by writing APFS is not available; put the file in `drivename/` and the guest sees it at next boot.

## From Finish to a talking desktop: the chain and its unproven links

No one has documented this chain end to end. Every building block exists separately. Two links are unproven: booting under WHPX, covered above, and audio inside the installer.

### Guest audio for VoiceOver

For macOS 12 to 26, use `-device virtio-sound-pci` attached to a host audio backend. It needs QEMU 8.2 or later. On Windows the available QEMU audio backends are DirectSound (`dsound`) and SDL; a check of QEMU's build files found no WASAPI backend, which corrects an assumption elsewhere in the notes ([QEMU audio build file](https://github.com/qemu/qemu/blob/master/audio/meson.build)). All existing reports of this sound device with macOS come from Linux or Mac hosts; there is no data at all from a Windows host, and none on quality or latency.

The critical unknown: Recovery is the installer here, because OpenCore boots the recovery image to install macOS. Whether Apple's VirtIO sound driver is present in that image is untested. If it is not, there is no speech until after installation, and the OCR safety net below becomes the main path.

For 10.13 to 10.15 the only documented route is an HDA device plus VoodooHDA, which would have to be injected by OpenCore to work in the installer. Sources conflict on whether it works with QEMU's emulated device at all, and everyone describes the sound as choppy. Big Sur 11.3 and later is the worst case. On Tahoe, Apple removed its HDA driver, so VirtIO sound is the only clean path ([Dortania Tahoe notes](https://dortania.github.io/OpenCore-Install-Guide/extras/tahoe.html)).

Two practical tools:

- QEMU's `wav` audio backend writes guest audio to a file. Orchord can boot a guest, trigger VoiceOver and check whether the file is silent. That is a guest-audio self-test that needs no ears.
- For speech, dropouts hurt more than a little latency. Buffer and latency options exist for both backends; the right values must be found by experiment.

### VoiceOver in the installer and Recovery

Apple documents Command+F5 as the VoiceOver toggle ([Apple VoiceOver guide](https://support.apple.com/guide/voiceover/turn-voiceover-on-or-off-vo2682/mac)). Blind users report it works in Recovery on real Macs after waiting for Recovery to finish loading, somewhere between 30 seconds and 2 minutes, and that Command+Tab does not work there. Those reports are old forum threads seen only as search snippets. There is no per-version confirmation and nothing on which voice the recovery image contains.

On a PC keyboard the Windows key is Command and Alt is Option, so Command+F5 is Windows+F5, and VoiceOver's modifier is Control+Alt or Caps Lock. That mapping is general knowledge, not cited.

What orchord should do:

- Detect that Recovery has loaded (the screen stops changing, or OCR finds the utilities window text) and say so through NVDA. Silence is otherwise ambiguous: still loading, no audio, or VoiceOver failed.
- Offer a one-key "Start VoiceOver in guest" command that injects the Command and F5 keys over QMP.
- Then verify: listen to guest audio for a few seconds and report "VoiceOver is speaking" or "No guest audio detected".

The install itself: in Recovery, erase the target disk as APFS in Disk Utility, then choose Reinstall macOS. The installer downloads the full system inside the VM, on the order of 12 to 18 GB, and reboots two or three times. No figures for duration or failure rate were found. Setup Assistant follows, where you turn VoiceOver on again and create the account.

### The OpenCore boot picker

OpenCore has a real pre-boot screen reader, `PickerAudioAssist`, toggled with Command+F5, but it depends on a firmware audio driver that its maintainers still describe as choppy on QEMU for exactly the multi-sample speech the picker uses ([acidanthera bugtracker 740](https://github.com/acidanthera/bugtracker/issues/740)). It also only speaks entry types, not volume names, and works only with HDA devices, not VirtIO sound.

Do not depend on it. Make the picker unnecessary:

- Generate the OpenCore configuration per phase. During install, attach only the recovery image and the empty target, hide the picker or give it a short timeout, and default to the installer. Afterwards, default to the macOS volume.
- The fragile part is the reboots during installation, when the default entry must switch. OpenCore normally follows the startup disk choice macOS writes to firmware variables, which requires the variable store file to be writable and persistent.
- Replace picker choices with manager commands, such as "Boot to Recovery next time".
- Send OpenCore's log to a QEMU serial port and show it in a text pane. That is a fully screen-reader-readable view of the pre-kernel phase.

### Keyboard pass-through and NVDA

Three separate problems:

- NVDA eats Insert, Caps Lock and its own commands unless it is asleep for the console. NVDA supports this per application: an app module containing `sleepMode = True`, which is exactly how NVDA handles the self-voicing DOSVOX program ([NVDA dosvox.py](https://github.com/nvaccess/nvda/blob/master/source/appModules/dosvox.py)). NVDA ships no module for any VM console. Ship a small NVDA add-on with orchord; without it, the user presses NVDA+Shift+S in the console each session. Put the console in a separate process from the manager so the manager's dialogs are not put to sleep.
- Windows reserves Windows+L and Control+Alt+Delete; no program can intercept them ([Microsoft documentation](https://learn.microsoft.com/en-us/uwp/api/windows.ui.input.keyboarddeliveryinterceptor)). So "Windows key is Command" can never be complete: Command+L would lock your PC. Orchord needs a configurable alternative Command key that avoids the Windows key, for example swapping Alt and Windows, plus menu items for the affected combinations. This is a requirement, not a nicety.
- QEMU's own GTK window hard-codes Control+Alt as its hotkey modifier, the same chord as VoiceOver's modifier ([QEMU gtk.c](https://github.com/qemu/qemu/blob/master/ui/gtk.c)). The SDL window can use Right Control instead, but exposes nothing to a screen reader beyond its title.

The recommended design avoids QEMU's windows entirely. While "keyboard captured" is on and the console is in the foreground, orchord's own low-level keyboard hook swallows every key except the release gesture and forwards separate key-down and key-up events over QMP's `input-send-event` command ([QEMU QMP interface](https://github.com/qemu/qemu/blob/master/qapi/ui.json)). Separate events matter because Control+Alt must be held while other keys are tapped.

Untested: whether NVDA's hook or orchord's wins for Insert and Caps Lock when both are installed, and whether the NVDA controller client still speaks while the focused program is in sleep mode. If it does not, capture and release must be signaled with distinct sounds. JAWS equivalents were not verified, and Narrator was not researched.

### Capture and release keys

Capture: Control+G, as you prefer and as VMware users expect. It is safe as the default because it only acts while the keyboard is released, when keys go to orchord's own window and not to the Mac. Once captured, Control+G passes through to the guest like any other key.

Release: a bare Control+Alt needs care. On a PC keyboard Control+Alt is Control+Option, VoiceOver's default modifier chord. A release that fires the moment Control and Alt are both down would fire on every VoiceOver command. The workable variants:

- (a) Release only when Control and Alt are pressed and then released on their own, with no other key in between. Control+Alt plus any key still passes through to VoiceOver. This keeps your VMware habit and works on every keyboard. Orchord's hook already sees separate key-down and key-up events, so it can tell the two cases apart. Costs: the guest has already received the Control and Alt presses by the time orchord knows it is a release, so orchord must send the matching key-up events to avoid stuck modifiers; a VoiceOver user who presses the modifier and then changes their mind releases the keyboard by accident; and what VoiceOver does with a bare Control+Option tap (it may interrupt speech) is untested.
- (b) One dedicated host key tapped alone, such as Right Control. This is the VirtualBox convention and the alternative QEMU's own SDL window offers; it is not an NVDA or JAWS modifier and it shadows no VoiceOver command. Not every laptop keyboard has a Right Control key, so check yours.
- (c) A three-key chord, such as Control+Alt+Shift+G, which QEMU's SDL window also offers. It has the lowest risk of accidental release, but it is the hardest to press and it takes one Control+Option combination away from VoiceOver.

Recommended default: variant (a), because it matches your stated preference, pairs naturally with Control+G, and needs no special key. Make the release key configurable, with (b) and (c) as the other choices, so anyone who finds accidental releases annoying can switch. Whatever is chosen must differ from the alternative Command key. Users who set Caps Lock as their VoiceOver modifier rarely press Control+Alt at all and are least affected.

Always announce both events, "Keyboard captured" and "Keyboard released", in speech and with two distinct sounds, and mirror the state in the window title. A blind user must never have to guess which system their keys are going to.

### The console window

Run QEMU with `-display none` and have orchord own the console. Give it:

- A title that carries state: the VM name, "macOS console", and "keyboard captured" or "keyboard released".
- A focusable read-only status line: running, paused or shut down; boot phase; last event.
- A read-only log holding the OpenCore serial log and manager events.
- Spoken and sound announcements for capture and release, and for shutdown, reset, pause and guest panic events, which QMP reports.
- A menu bar, reachable when released, with every guest action as an ordinary menu item: Start VoiceOver in guest, Read guest screen, Send Command+Tab, Boot to Recovery, Power off, Reset. Nothing should require capture mode.
- An optional picture of the guest screen for low-vision users and for sighted help when wanted, with a sensible accessible name.

### The OCR safety net

A built-in "Read guest screen" command is feasible with stock parts: QMP `screendump` to a PNG file, then Windows' offline OCR, then the text into a read-only field and into speech. NVDA's own OCR command cannot be the mechanism because NVDA is asleep in the console. Prior art exists: a shipped macOS VM tool reads Recovery prompts by OCR to automate them, and a blind developer has proposed the same idea for Hyper-V ([Lume](https://cua.ai/docs/concepts/how-lume-unattended-setup-works), [TheWorkBench issue 150](https://github.com/kellylford/TheWorkBench/issues/150)).

Build three things on it:

- An on-demand reader, with its hotkey recognized even in captured mode.
- A screen-change watcher that announces only new text, and says "still working" or "screen unchanged for some minutes". This answers the complaint blind users repeat about every VM installer: there is no reliable way to tell when an install finishes.
- Phase detection from keywords, which gives orchord a state to speak from, independent of guest audio. Pin the guest's language to a language Windows OCR has installed.

OCR accuracy on macOS installer screens, with thin fonts on translucent backgrounds, was not measured. Word positions from OCR allow a later "click this text" feature for the few steps keyboard navigation cannot reach.

After installation, SSH is the fully accessible channel, but there is no supported way to have it on at first boot; enabling Remote Login is a manual in-guest step. Orchord can offer an "Enable SSH in guest" command that types the Terminal commands over QMP; the exact commands for current macOS still need verifying. Screen sharing carries no VoiceOver speech, so speech must always come from the emulated sound device.

## Beyond the wizard: how the rest of the program should work

You left this open. The following is a design synthesis from the notes, not a tested specification.

### Shape of the program

- A manager process: the wx window with the VM list, menus, wizard, configuration dialog and progress views.
- One headless QEMU process per running VM, with a QMP control channel and a serial port for the boot log.
- One console process per running VM, separate so NVDA's sleep mode applies only to it.
- Logic for QMP, downloads and image building kept in modules with no window code, so most tests need no UI. hn-blind is structured this way.
- A backend boundary in the code, with QEMU on WHPX as the only implementation at first. That keeps a VMware or WSL2 fallback possible without a rewrite.

### Suggested menus

- File: New Virtual Machine (Control+N), Exit.
- Machine: Boot (Enter), Settings (Control+P and Alt+Enter), Shut down guest, Force power off, Reset, Open VM folder, Open shared folder, Delete.
- Guest, in the console: Start VoiceOver in guest, Read guest screen, send special key combinations, Boot to Recovery next time, Enable SSH in guest.
- Tools: Guest audio self-test, View logs.
- Help: documentation opened in the browser or shown in a read-only text field.

### What happens after Finish

- Check prerequisites: the Windows Hypervisor Platform feature is enabled (turning it on needs administrator rights and a reboot, once), QEMU is present and new enough, free memory and disk space are sufficient.
- Download the recovery image and chunklist with resume support, and verify.
- Build the OpenCore image with a generated configuration for the install phase, including the CPU identity spoof where the QEMU build needs it.
- Create the disk image and write the VM's configuration into `vmconfig/`.
- If "Boot machine after setup completes" is checked, start QEMU and open the console, announcing each phase.

### Configuration dialog pages

- General: name, macOS version, folder.
- Hardware: memory, virtual CPUs.
- Storage: disk size, disk attachment type, shared folder on or off.
- Audio: device, host backend, buffer size, self-test button.
- Keyboard: Command key mapping, release key, whether Caps Lock is passed to the guest.
- Advanced: accelerator options, CPU identity spoofing, extra QEMU arguments.

### After first boot

Offer a one-time tuning step that applies the consistently recommended guest settings: Reduce Motion and Reduce Transparency on, Spotlight indexing off, automatic updates off so the image does not grow, TRIM enabled, guest sleep disabled, VoiceOver set to start at login, and the shared-folder agent installed.

Tell the truth in the interface. Label each macOS version with its speech-during-install status, show a neutral license notice in the wizard, and never present the experimental GPU as a default.

### Testing the interface

Manual NVDA passes remain the ground truth. Structural checks can be automated: Paperback's Windows tests assert on what UI Automation reports. A test that walks each dialog and checks that every focusable element has a non-empty name, and that none is a wx default such as "dataviewCtrl", is cheap. Driving NVDA in automated builds is possible but brittle; treat it as a later extra.

## First hands-on experiments, in order

Each hardware experiment takes under an hour once a guest boots. Experiment 1 is independent of the rest and can run in parallel.

1. Toolchain and UI spike, about a day. Install the Visual Studio C++ tools, CMake, Ninja and rustup from the command line. Build a wxDragon window with a menu bar, a list box holding the placeholder row, Enter and context-menu handling, a dialog with a labeled text field, spin control and check box on a simplebook with Back and Next, a gauge fed from a worker thread, and one spoken announcement. Test with NVDA and once with Narrator. Note first-build time, executable size and how readable the build errors are. If anything essential fails, build the same skeleton in C++ against prebuilt wxWidgets 3.3.3 and compare.
2. QEMU inventory. Install the newest QEMU 11.x build for Windows. Confirm the Windows Hypervisor Platform feature is enabled and that QEMU lists WHPX as an accelerator. Confirm the build includes the devices macOS needs (Apple SMC, VMware display, VirtIO sound, vmxnet3) and the DirectSound and SDL audio backends; whether common Windows builds include them was not verified. Confirm that the 9p folder-sharing option is rejected.
3. The WHPX boot test. This is the gate. Boot a Monterey recovery image through OpenCore with `-accel whpx`, 6 GB and 4 virtual CPUs. Vary one thing at a time: the Comet Lake CPU identity spoof, timestamp-counter flags, merged versus separate firmware files. If upstream QEMU fails, try a current development build, then the fork build. Run the same VM under pure emulation as a control. Success here resolves the conflict in the notes for your hardware.
4. Audio in Recovery. Boot the Monterey recovery image with VirtIO sound and the `wav` audio backend, wait for the Recovery screen, inject Command+F5 over QMP, and check whether the recording is silent. Repeat with the Sequoia recovery image. This decides whether a blind user gets speech during installation.
5. Full install and VoiceOver responsiveness. Install the version that passed, with live audio. Time the phases. Go through Setup Assistant with VoiceOver. Then compare 2 against 4 virtual CPUs, 6 against 8 GB, and audio buffer sizes, listening for stutter. Try restricting QEMU to the performance-core threads.
6. Keyboard and NVDA. With a sleep-mode app module and orchord's own keyboard hook, check that Insert, Caps Lock, Windows+F5 and Control+Alt chords all reach the guest, that Control+G captures, that a bare Control+Alt tap releases without misfiring during fast VoiceOver use and leaves no modifier stuck in the guest, and whether the NVDA controller client still speaks while NVDA sleeps.
7. OCR accuracy. Run Windows OCR over screenshots of Recovery, the installer and Setup Assistant from experiments 4 and 5.
8. Shared folder. Test whether the Mac mounts an SMB share on a non-standard port, how long a file created on Windows takes to appear in Finder and to a file watcher, copy speed for a multi-gigabyte file over QEMU's user-mode network, and how VoiceOver behaves in Finder on the share. Test whether the disk image shrinks after deleting files.
9. Wider version coverage. Ventura, then Tahoe. Then, if wanted, Catalina with VoodooHDA injected by OpenCore, to learn whether any pre-Monterey version can talk during install.
10. Baseline comparison, optional. The same guest in VMware Workstation 26, including whether there is sound in Recovery without any kext. This both measures QEMU against something and qualifies the fallback.

Decision gates:

- If experiment 3 fails in every variation, QEMU on Windows is not a backend for your CPU today. Move to the VMware comparison and the WSL2 route before writing engine-specific code.
- If experiment 4 is silent on every version, plan for OCR-guided installation as the main path, with speech arriving only after install, and test the VMware sound device in Recovery.
- If experiment 1 goes badly in Rust, switch to C++ early; the cost of switching is lowest before the first real screen exists.

## Apple software license position

Apple's macOS license grants installation only on Apple-branded computers, and permits virtualization only on an Apple-branded computer that is already running macOS ([Apple macOS Tahoe license](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf)). Running macOS in a VM on a Windows PC is therefore outside the license terms. Existing projects respond by shipping no Apple software, downloading from Apple's servers on the user's machine at run time, stating the restriction plainly and leaving acceptance to the user. The one documented enforcement against such a project was Apple's 2024 copyright complaint that removed Docker-OSX images, which contained macOS installer content, from Docker Hub; the source repositories remain available ([SecurityOnline](https://securityonline.info/docker-osx-shut-down-apples-copyright-claim-impacts-macos-containers/)). Some projects also embed a 64-character Apple key string that QEMU's Apple SMC device requires; its legal status was not researched. Orchord's "download from Apple" design matches the pattern the surviving projects follow. The research did not cover enforceability in any particular country, and none of this is legal advice.

## Conclusion

The research changed the shape of the question. "Which engine is fast" turned out to have no good answer on Windows, because no engine gives macOS a graphics card; the useful question for a blind user is "which engine can talk, and how early". That single criterion removes Hyper-V, demotes VMware to a fallback, and makes QEMU's sound devices and scriptable control channel worth more than raw speed. It also moves the hard problems out of the emulator and into places nobody has built yet: knowing when the guest is ready, getting the first Command+F5 to produce sound, and keeping two screen readers from fighting over one keyboard.

Orchord would be the first tool of its kind, so its real contribution is ending ambiguous silence. Even if both unproven links fail on your laptop, the manager's announcements, OCR reader and progress watcher still deliver something no existing VM front end offers a screen-reader user. Build those parts to be independent of the engine, run the two gate experiments first, and let their results choose between the QEMU plan and the fallbacks.
