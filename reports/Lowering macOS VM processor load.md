# Drawing, not idling, loads Orchord's Macs

The processor load in an Orchord Mac comes from activity that draws on the screen, not from the Mac sitting still, so the changes worth making are the ones that cut drawing work, make each trip between the Mac and QEMU cheaper, and stop Windows treating QEMU as background work. Measurements taken on the project's own Windows 11 Intel PC on 12 October 2026 show that a Mac at rest uses about one seventh of one logical processor (a little over 1 percent of the whole PC), and that Orchord's own screen fetch adds only about 5 percent of one logical processor. Starting up, logging in and opening Safari each kept two and a half to four logical processors busy for a minute or more, and Safari left open kept about one and a half busy for as long as it stayed open. No setting in this report was tested before and after, so every recommendation is a candidate, ranked by reasoning from source code, documentation and other projects' practice. The best-value changes for Orchord are to tell Windows that QEMU is foreground work, to offer QEMU's new `ssd=off` switch as a choice, to shorten the sound buffer that delays VoiceOver's speech, and to change the boot argument `serial=3` to `serial=1`. The best-value changes for a user are Reduce Transparency, Reduce Motion, turning off VoiceOver's caption panel, turning off Spotlight indexing, and keeping few applications open. The tester's memory complaint is expected behaviour: QEMU on Windows claims all of the Mac's memory at start. Windows 10 PCs are on a slower path in QEMU 11.1 than Windows 11 PCs. Real graphics acceleration has exactly one route on Windows, an experimental project called reims-vgpu, and it is not ready to ship.

## How to read the evidence labels in this report

Every claim below carries one of these labels.

- Measured: taken on the project's own PC on 12 October 2026, recorded in `research_notes\Lowering macOS VM processor load\local_measurements.md`. This is the only measured evidence. It is one session, one PC (Intel Core i7-1355U, Windows 11, 16 GB), one Mac (Big Sur 11, four virtual processors, 4096 MB, screen 1280 by 800), with the Mac's sound switched off.
- Source: read from QEMU's source code, Apple's published kernel source, or official documentation. This tells us what the software does. It does not tell us how much time a change saves.
- Project logs: seen in Orchord's own serial logs or source code. These are observations, not timed measurements.
- Hearsay: what other projects do, or what forum posts and guides say, with no numbers behind it. Most of it comes from Linux hosts.
- Reasoning: the researchers' or my own inference. Untested.

Where another party published numbers, I say "measured by others" and name the workload, because none of those tests was a virtual Mac.

Some terms used throughout:

- QEMU is the program that pretends to be the Mac's hardware. WHPX is the part of Windows (the Windows Hypervisor Platform) that lets QEMU run the Mac's code directly on the real processor.
- A logical processor is what Windows counts as one processor. The project's PC has 12. "100 percent of one logical processor" means one of the 12 fully busy.
- An exit is what happens each time the Mac touches a pretend device such as its disk, network card or serial port. The processor has to leave the Mac and hand control to QEMU. Exits are the main overhead of this kind of virtual machine.
- Software rendering means the Mac draws every window with the processor because it has no graphics card it can use. WindowServer is the macOS program that does that drawing.

## Measured: a still Mac costs about 1 percent of the PC, a busy one costs two to four processors

All figures in this section are Measured, from the file named above.

- Settled at the login window, with the text cursor blinking in the password box: 14 to 18 percent of one logical processor, across ten one-minute readings. That is about 1.2 to 1.5 percent of the whole PC.
- The same login window with nothing on screen changing (text selected in the box, so no blinking cursor): 2 to 3 percent of one logical processor.
- Logged in, desktop showing, a Terminal window open, nothing pressed: 14 to 17 percent.
- Orchord's screen fetch, twice a second at 1280 by 800: adds about 5 percent of one logical processor.
- Start-up, from the kernel starting to a settled login window: about 4 minutes of heavy work, peaking near three logical processors. Then about 5 more minutes of background work at roughly one logical processor before it settles.
- Logging in: 383 percent in the first minute, meaning all four virtual processors nearly flat out, settling after about 3 minutes.
- Opening Safari: about 250 percent for the first minute. Safari then sat on its start page, with nothing pressed, at 135 to 151 percent.
- VoiceOver readings were weak, because sound was off and nobody could confirm it was speaking: 29 percent just after Command+F5, 12 percent idle, 15 percent while Control+Option+Right Arrow was sent twice a second in Terminal.

Three conclusions follow.

First, there is no idle problem to fix on a Windows 11 Intel PC. Idle virtual processors rest properly.

Second, redrawing is what costs. The only visible difference between the 2 to 3 percent reading and the 14 to 18 percent reading was one blinking text cursor. That was not a controlled experiment, but it is the plainest evidence in the folder.

Third, the 2 to 3 percent reading puts a ceiling on every "always running" overhead at once. The serial port polling, the HPET timer device, the unused USB controller and the pretend Apple SMC chip were all present during that reading. Together they cost less than 3 percent of one logical processor on this PC (Reasoning from the measurement). So changes aimed at idle overhead cannot give much back on Windows 11. They may matter more on Windows 10, which was not measured.

What was not measured: a Windows 10 PC, an AMD PC, macOS 12, 14 or 15, sound switched on, the delay between a key press and VoiceOver speaking, and the effect of any proposed setting.

## What the two testers' reports mean

### The 8 GB Mac that took more than half its memory at the installer

This is explained by the notes and is not a fault. QEMU on Windows asks Windows for the whole of the Mac's memory in one go at start ([Source: QEMU `util/oslib-win32.c`](https://github.com/qemu/qemu/blob/v11.1.0/util/oslib-win32.c#L47-L63)). Windows counts all of it against its memory limit immediately, though real memory is only used as the Mac touches it. Measured on the project's PC: for a 4096 MB Mac, QEMU's private memory was 4.5 to 4.8 GB from early in start-up, and memory really in use grew from 2.8 GB at the login window to 4.0 GB within five minutes. So macOS touches nearly all of what it is given, quickly. An 8 GB Mac showing more than 4 GB at the installer is what this design does.

The notes did not look at how VMware accounts for memory, so they do not explain why VMware showed less. QEMU on Windows has no switch that changes this: large pages and file-backed memory are not built into the Windows version (Source, same file). The option `-mem-prealloc` would make it worse, because it touches every page at start. The practical answer is to choose a Mac memory size that fits the PC, and to leave the Windows page file alone.

### "Laggy", and keyboard navigation worse than VMware

The notes cannot explain this with evidence. Nobody, anywhere, has published a measurement of VoiceOver's response time in a virtual Mac, and VMware gives a Mac no graphics acceleration either ([Hearsay, vendor staff on a forum](https://community.broadcom.com/vmware-cloud-foundation/discussion/sequoia-graphics-configuration-not-supported)). The candidates the notes do offer, all untested:

- Sound buffering. QEMU's Windows sound output holds about 93 milliseconds of sound by default, plus 10 more, plus up to 10 for its timer (Source, cited below). That is a tenth of a second added to every word VoiceOver speaks. For a screen reader user this is felt as lag.
- Cost per exit. Every disk, network and serial access goes out to QEMU.
- Disk stalls. Orchord's Sequoia log shows the Mac's disk driver timing out and resetting the disk during a normal session (Project logs). Each time-out freezes disk work for a moment.
- Drawing done by VoiceOver itself, such as its caption panel.

### The AMD Ryzen 7 5700X on Windows 10

That Mac does not start yet, which is a separate problem. What the notes say about Windows 10 is gathered in its own section below.

## What Orchord can change, ranked from best value to least

Ranked by likely benefit set against risk and effort. Orchord 0.1.4 does none of these today; I checked `crates\orchord-core\src\qemu.rs` and the boot folder builder.

1. Tell Windows that QEMU is foreground work. Windows rates each program for "quality of service", which decides which cores it gets and how fast they run. A program whose window is minimized or fully covered is rated Low, and so are its child programs. On battery, Low means the slow efficiency cores at their most economical speed ([Source: Microsoft, Quality of Service](https://learn.microsoft.com/en-us/windows/win32/procthread/quality-of-service)). QEMU has no window and is Orchord's child, so it drops to Low whenever Orchord is minimized or covered, for example during a long easy install while the user works elsewhere. Windows 11 can also ignore a hidden program's request for a fine timer ([Source: Microsoft, timeBeginPeriod](https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timebeginperiod)), which would make sound choppy. Microsoft does not say whether that second rule reaches a windowless child such as QEMU; the call below removes the doubt. The fix is one Windows call on the QEMU process: `SetProcessInformation` with `ProcessPowerThrottling`, `ControlMask` set to `PROCESS_POWER_THROTTLING_EXECUTION_SPEED` and `PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION`, and `StateMask` set to 0 ([Source: Microsoft, SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation)). On Windows 10 make it two calls and ignore a failure of the timer one. Benefit: removes the worst case, where the Mac is pushed onto slow cores. It does not help while Orchord is the window in front, because then QEMU is already rated High. Risk: none to the Mac; shorter battery life. Effort: small, no administrator rights. Forum reports of the slow-core effect are Hearsay; one gives a figure of about half speed for a different program.

2. Offer `ssd=off` as a choice. QEMU 11.1 added `-accel whpx,ssd=off`. QEMU's documentation says it "results in a significant MMIO performance increase at the detriment of security mitigations" ([Source: QEMU WHPX documentation](https://github.com/qemu/qemu/blob/v11.1.0/docs/system/whpx.rst)). In plain terms, each exit gets cheaper because Windows stops clearing part of the processor's state on every trip. Every pretend device Orchord uses is reached through exits. Benefit: unknown in size; nobody has published a number. It is the single most promising QEMU switch in the notes. Risk: it weakens the wall between the Mac and Windows against one family of processor attacks, so it should be a setting the user turns on, not a silent default. Effort: almost none. It can be tried today through the `accelerator` line in Orchord's settings file, set to `whpx,ssd=off`.

3. Shorten the sound buffer. The defaults are `out.buffer-length` 92880 microseconds, `latency` 10000 and `timer-period` 10000 ([Source: QEMU `qapi/audio.json`](https://github.com/qemu/qemu/blob/v11.1.0/qapi/audio.json#L185-L202), [Source: `audio/dsound_template.h`](https://github.com/qemu/qemu/blob/v11.1.0/audio/dsound_template.h#L198)). A line to start testing from, with values that are mine and not from any source:

   `-audiodev dsound,id=snd0,out.buffer-length=40000,latency=5000`

   Benefit: speech arrives sooner. This targets lag, not load. Risk: too small a buffer crackles when the PC is busy. Orchord already has a way to count sound drop-outs in a recording without ears, so the smallest clean value can be found by test. Effort: small. Evidence for the delay is Source plus arithmetic; nobody measured it.

4. Change `serial=3` to `serial=1` in the boot arguments. The 3 means output plus input. The input half starts a kernel thread that wakes every 16 milliseconds for as long as the Mac runs, to check a serial port that nobody ever types into ([Source: Apple kernel, `serial_general.c`](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/console/serial_general.c#L52-L97)). The output half alone still sends crash text to the log Orchord watches ([Source: Apple kernel, `printf.c`](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/printf.c#L916-L938)). The new line would be:

   `-v keepsyms=1 debug=0x100 serial=1`

   Benefit: small on Windows 11, given the ceiling measured above. Possibly more on Windows 10, where each wake-up passes through QEMU. Risk: very low; crash detection should be re-checked once with a deliberate crash. Effort: one string.

5. Give large disks a bigger look-up cache. A qcow2 disk image keeps a table of where data lives. On Windows QEMU caps the in-memory copy at 8 MB, enough for a 64 GB disk; Linux gets 32 MB ([Source: QEMU `docs/qcow2-cache.txt`](https://github.com/qemu/qemu/blob/v11.1.0/docs/qcow2-cache.txt)). Orchord's Sequoia disk is 96 GB. Adding `l2-cache-size=16777216` (16 MB, enough for 128 GB) to the main disk's `-drive` line removes extra reads on scattered disk work. Benefit: unmeasured, likely modest. Risk: very low. Effort: trivial.

6. Hold a "do not sleep" request while a Mac runs. `PowerCreateRequest` and `PowerSetRequest` with `PowerRequestSystemRequired` stop Windows sleeping from idleness during an overnight install ([Source: Microsoft, PowerSetRequest](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-powersetrequest)). This is reliability, not load. On a laptop on battery Windows cancels it 5 minutes after the sleep timer, and closing the lid always wins.

7. Use the newer processor model for Big Sur and Monterey. Those two get the old `Penryn` model, which lacks the AVX2 instructions; Sonoma and Sequoia get `Skylake-Client,-hle,-rtm`. macOS's drawing code has faster paths with AVX2, and the newer model also arms timers with one register write where the older needs three ([Source: Apple kernel, `lapic_native.c`](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/i386/lapic_native.c#L790-L802)). Benefit: Reasoning only. Risk: medium; an unexpected feature mix can crash macOS at start, so test each version. Effort: small.

8. Move new Macs' main disk and network card to virtio devices. Virtio devices are designed for virtual machines and need fewer exits per piece of work. quickemu uses `virtio-blk-pci` and `virtio-net-pci` for Big Sur and later, and OSX-KVM says the disk change gives "some performance gain" ([Hearsay: quickemu script](https://github.com/quickemu-project/quickemu/blob/master/quickemu), [Hearsay: OSX-KVM notes](https://github.com/kholia/OSX-KVM/blob/master/notes.md)). Start-up, log-in and application launch are disk-heavy and are where the measured load is, and this might sidestep the disk time-outs in the Sequoia log. Risk: high for a setting. Existing Macs must not change, because the firmware remembers the start-up disk by its position. The boot loader's scan policy lists only SATA disks today and would have to change. Whether the installer sees a virtio disk needs testing per version. All the evidence is from Linux. Effort: medium to large. Do this after the cheaper items have been measured.

9. Turn off the HPET timer device and drop the unused USB 2 controller. `-machine q35,hpet=off` removes a device that can only add exits; quickemu does this for macOS (Hearsay). Removing `usb-ehci` removes a controller that can run a 1000-times-a-second timer in QEMU ([Source: QEMU `hcd-ehci.c`](https://github.com/qemu/qemu/blob/v11.1.0/hw/usb/hcd-ehci.c#L2322-L2362)); Docker-OSX runs without it (Hearsay). Benefit: bounded below 3 percent of one logical processor on Windows 11 by the measurement. Risk: low for HPET; for the controller, check that existing Macs still find their start-up disk. Effort: small.

10. Keep four virtual processors, and steer users towards enough memory. One guide says 16 virtual processors "does not make macOS faster" ([Hearsay: OrelSokolov gist](https://gist.github.com/OrelSokolov/556f626b98ccfecca8760bcc4bd0d86f)). Orchord's defaults are 4096 MB for Big Sur and Monterey and 5120 MB for Sonoma and Sequoia. At those sizes the newer systems will swap to disk under modest use, and disk work is the expensive path (Reasoning). The quickemu maintainer's view is that virtual Macs want 8 GB ([Hearsay: quickemu discussion](https://github.com/orgs/quickemu-project/discussions/1372)). On a 16 GB PC that is a trade against Windows' own needs, so the wizard should suggest, not force.

11. Offer the lighter-Mac settings from the next section as one step, through the set-up script Orchord already uses for the shared folder (Reasoning; it would need the user's password for some of them).

Also worth knowing: fixes for WHPX were picked for the 11.1.x point releases, so the newest 11.1.x is preferable to 11.1.0. I saw that only as a search summary (Hearsay).

## What a user can change inside macOS

No source measured any of these in a virtual Mac. They are ranked by how consistently they are recommended and how little they risk. None needs System Integrity Protection (Apple's lock on system files) turned off. To measure a change yourself, run this in Terminal before and after:

`top -l 5 -stats pid,command,cpu -o cpu`

1. Wait after start-up, log-in and installation. Measured: about 9 minutes from start to a truly settled login window, and about 3 minutes after logging in. After a fresh install or update, indexing can run for hours (Hearsay). Judging speed during those periods misleads.

2. Turn on Reduce Transparency and Reduce Motion. On macOS 14 and 15: System Settings, Accessibility, Display. On macOS 11 and 12: System Preferences, Accessibility, Display. Every source aimed at virtual Macs recommends them ([Hearsay: osx-optimizer](https://github.com/sickcodes/osx-optimizer)). They are purely visual, so VoiceOver loses nothing. The Terminal commands for the same thing are reported blocked on current macOS unless Terminal has Full Disk Access ([Hearsay: forum report](https://github.com/mathiasbynens/dotfiles/issues/1039)), so use the settings window.

3. Turn off VoiceOver's caption panel. It is a floating panel that redraws with every phrase spoken. The key is VO+Fn+Command+F10, or VoiceOver Utility, Visuals, "Show caption panel" ([Source: Apple VoiceOver guide](https://support.apple.com/guide/voiceover/cpvouviscap/mac)). That it saves processor time is Reasoning, backed by the blinking-cursor measurement.

4. Keep few applications open, and close Safari when not using it. Measured: Safari idle on its start page used about ten times what the bare desktop used.

5. Turn off Spotlight indexing, if you can live without Command+Space. The osx-optimizer project says this will "heavily speed up" virtual Macs (Hearsay, no numbers).

   `sudo mdutil -i off -a`

   To undo: `sudo mdutil -i on -a`

   Downside: Command+Space stops finding applications, and search in Mail and Finder stops. Open applications from Finder with Command+Shift+A instead.

6. Pick a light voice. On macOS 14 and 15, Eloquence is reported as very responsive. On macOS 11 and 12, Fred is the community's fastest choice, and the Vocalizer voices such as Samantha had a sluggishness bug that was only fixed in macOS 13 ([Hearsay: AppleVis bug tracker](https://applevis.com/bugs/mac/voiceover-sluggish-when-using-speech-rotor-nuance-voices)). All of this comes from real Macs.

7. On Sonoma and Sequoia, use a plain still wallpaper, no desktop widgets, Stage Manager off, and screen saver set to never (Reasoning: these all redraw continuously).

8. Give the background services nothing to do: no iCloud sign-in, no Photos library, Siri off (Reasoning). Orchord's own test Macs were set up this way.

9. Stop the Mac sleeping. This is for stability, not load.

   `sudo pmset -a sleep 0 disksleep 0 displaysleep 0 powernap 0 standby 0 autopoweroff 0 hibernatemode 0 womp 0`

   ([Source: pmset manual page](https://www.dssw.co.uk/reference/pmset/))

10. Optional: automatic log-in (Users and Groups settings), and automatic updates off. Automatic updates off also stops security updates, so treat it as a trade.

On which macOS version is lightest: no benchmark exists. Indirect evidence says older is lighter, because each release from Monterey to Sequoia moved more features onto graphics hardware ([Source: OpenCore Legacy Patcher tracker](https://github.com/dortania/OpenCore-Legacy-Patcher/issues/108)). Against that, VoiceOver gained speed fixes and Eloquence only from macOS 13. Neither side can be called without measuring Orchord's own Macs. Tahoe 26 should be avoided; it is reported as far heavier.

## What a user can change in Windows

1. Keep the Orchord window visible, not minimized or fully covered, while the Mac is doing heavy work. This is the hand-made version of item 1 in the Orchord list, and stops being needed once Orchord makes that call (Source for the rule; unmeasured for QEMU).

2. Plug the laptop in. The slow-core rule is documented for battery power (Source).

3. On Windows 11, try Settings, System, Power and battery, Power mode, Best performance. Reports conflict on whether it helps (Hearsay).

4. Never apply Task Manager's Efficiency mode to QEMU. It lowers priority and forces the slow cores by design ([Source: Microsoft blog](https://devblogs.microsoft.com/performance-diagnostics/reduce-process-interference-with-task-manager-efficiency-mode/)).

5. Leave memory free. Close WSL, Docker, other virtual machines and heavy programs. Do not shrink the page file. Do not run memory-cleaning tools; the earlier Orchord research saw one stall an install for about an hour.

6. Keep Macs on a solid-state drive, and do not turn on NTFS compression for the folder (general guidance, not sourced to a test).

7. Optional, administrator only, unproven: stop Microsoft Defender scanning files that QEMU opens.

   `Add-MpPreference -ExclusionProcess "C:\full\path\qemu-system-x86_64.exe"`

   Microsoft recommends the equivalent for its own virtual machine product, but gives no numbers, and it lowers protection ([Source: Microsoft](https://learn.microsoft.com/en-us/troubleshoot/windows-server/virtualization/antivirus-exclusions-for-hyper-v-hosts)). Orchord should never do this silently.

## What is not worth doing

- Rewriting how Orchord fetches the screen. Measured at about 5 percent of one logical processor. QEMU's VNC and D-Bus display routes would be a large change for a tiny gain.
- Changing the pretend display card, or giving it more video memory. macOS has no driver for any of them, so all behave as a plain picture buffer (Source and Hearsay agree).
- Lowering the screen resolution further. Orchord's test Mac already runs at 1280 by 800. Dropping to 1024 by 768 leaves about three quarters of the pixels, a modest saving, and risks Orchord's screen reading during easy install (Reasoning).
- Old third-party display drivers such as VMsvga2. They stop at macOS 10.15 or earlier and never offered modern acceleration.
- Turning off Windows Memory Integrity. Measured by others at roughly 5 percent in games, never in a virtual machine, and it weakens Windows security ([Tom's Hardware](https://www.tomshardware.com/news/windows-11-gaming-benchmarks-performance-vbs-hvci-security)). WHPX needs the Windows hypervisor running regardless.
- High or Realtime priority for QEMU. Four busy Mac processors could then starve NVDA and Windows sound. Use the quality-of-service call instead.
- Pinning QEMU to the two fast cores of the i7-1355U. Four virtual processors plus QEMU's own threads would crowd onto two cores (Reasoning).
- `-overcommit cpu-pm=on`. Not implemented for WHPX, and where it works it makes an idle Mac look fully busy (Source).
- Hyper-V "enlightenments" for the Mac. macOS does not know Hyper-V, so it ignores them (Source: Apple kernel).
- `serverperfmode=1`. Real and documented by Apple, but it raises server limits and does not reduce work.
- Turning off App Nap. That makes background applications use more, not less.
- Power management add-ons (CPUFriend, the XCPM settings, `plugin-type`). A virtual Mac cannot change the real processor's speed.
- Hiding the hypervisor from macOS, adding `debug=0x8`, or adding `-vsmccomp`. Each makes things worse (Source for what each does; that it would be slower is Reasoning).
- `-mem-prealloc`. Smoother first minutes, but all memory is taken at once.
- Setting `SetApfsTrimTimeout` to 0. It would save about two seconds per start (Project logs), and that start-up pass is what tells the disk image which space is free to reuse.
- More than four virtual processors.
- Changing the Mac model from iMac19,1. Unproven, and it changes the Mac's identity.
- Moving to VMware for speed. It draws in software too.
- Fighting Apple's background services with System Integrity Protection off. Reports say it does not stick and can make things worse (Hearsay).
- Screen Curtain as a load saver. Nothing shows it stops drawing. It costs nothing to test once.

## Windows 10 and AMD: a slower path, and one setting to re-check

QEMU 11.1 checks whether Windows is "modern". Windows 10 fails the check ([Source: QEMU `whpx-all.c`](https://github.com/qemu/qemu/blob/v11.1.0/target/i386/whpx/whpx-all.c#L3110-L3126)). The earlier Orchord research found the tell-tale line, "Failed to get performance monitoring features", in the AMD tester's own QEMU log, so that PC is on this path.

On that path the Mac's interrupt controller, the part that delivers timer ticks and wake-ups, is run by QEMU itself instead of by Windows. Every timer set, every rest and every wake-up becomes an exit ([Source: QEMU WHPX documentation](https://github.com/qemu/qemu/blob/v11.1.0/docs/system/whpx.rst)). The expected result is more load and less smoothness than on Windows 11 for the same Mac. Nobody has measured how much.

What bears on Windows 10 PCs:

- The idle findings above do not carry over. The "no idle problem" result is for Windows 11 only.
- `serial=1`, `hpet=off` and the newer processor model should matter more there, because each removes timer work that Windows 10 routes through QEMU (Reasoning).
- QEMU documents `-machine q35,pic=off` as the way to get the faster interrupt controller on Windows 10. Whether OpenCore and macOS start with it is untested. It is worth one experiment once that Mac starts at all.
- `ssd=off` may be unavailable on old Windows 10 releases; QEMU treats that as harmless (Source).
- QEMU's Windows support is only tested from Windows 10 version 2004.
- The AMD tester's Ryzen 7 5700X has no slow cores, so the quality-of-service problem mostly does not apply. Moving that PC to Windows 11 would be the largest single host-side change, but its benefit is unmeasured.
- Another project found WHPX blocked on a different AMD PC by missing register handling and other AMD-specific gaps ([Hearsay: reims-vgpu issue 86](https://github.com/steelbrain/reims-vgpu/issues/86)).

One thing to re-check. The notes for this report read QEMU 11.1's source as having `ignore-unknown-msr` on by default ([Source: `whpx-common.c`](https://github.com/qemu/qemu/blob/v11.1.0/accel/whpx/whpx-common.c#L545-L560)). The earlier Orchord research said it was off unless given. If the new reading is right, the `ignore-unknown-msr=on` that Orchord adds on AMD changes nothing. The `intercept-msr-gp=on` it also adds is a logging aid that adds exits (Reasoning), and should come out once the AMD Mac starts.

## Real graphics acceleration: one experimental route, not ready to ship

Every release of macOS from 11 on contains Apple's own driver for a virtual graphics card. Apple's matching host software exists only on Macs, so stock QEMU's `apple-gfx-pci` device cannot work on Windows ([Source: QEMU patch series](https://patchew.org/QEMU/20241223221645.29911-1-phil@philjordan.eu/)).

An open-source project, reims-vgpu, re-creates that host side and translates the Mac's drawing commands for an ordinary graphics card ([Hearsay: project README](https://github.com/steelbrain/reims-vgpu/blob/master/README.md)). Its state on Windows in October 2026, all from the project's own pages and issue reports:

- First Windows support was merged on 31 August 2026, described by the maintainer as likely to run "albeit with glitches".
- A Windows-focused fork reports about 90 to 115 frames per second in a macOS 13 Mac on an NVIDIA RTX 4060, against 5 to 7 before ([MAXZVER fork](https://github.com/MAXZVER/reims-vgpu)). The page does not say whether "before" means no acceleration or an earlier slow version. Only macOS 13 has been tested on Windows, and Orchord offers 11, 12, 14 and 15 but not 13.
- Mac kernel crashes on Windows are still open and need workarounds ([issue 71](https://github.com/steelbrain/reims-vgpu/issues/71)).
- On Tahoe 26 it runs at 0.3 to 1.9 frames per second, worse than no acceleration.
- It needs a patched QEMU built from source, not the QEMU that Orchord installs.
- Only NVIDIA cards are reported working on Windows. Intel built-in graphics, which the project's laptop has, has open bugs.
- Nobody has reported processor load or VoiceOver response with it. Frame rate is not the goal for a blind user, and the translation work itself kept four cores busy in one report. It is not established that it lowers load at all.

The sensible position is to try it on a side branch with a macOS 13 Mac on a PC with an NVIDIA card, look again every few weeks, and keep it out of releases.

The other routes, briefly:

- reims-vgpu inside WSL (Linux inside Windows): works for one user with AMD graphics, unreliable with NVIDIA, and adds a second layer of virtual machine.
- A Linux PC with a second graphics card handed to the Mac: mature, but not Windows.
- VMware's Mac acceleration: Mac hosts only.
- Windows' own graphics sharing for virtual machines, and the virtio graphics card: no macOS driver exists for either.
- A real Mac reached over the network: works, with sound delay as the concern.

## Conclusion

The most useful thing this research produced is a way to stop guessing. The method used for the measurements, reading QEMU's processor time over a fixed window, costs minutes per test. Four fixed scenes (start to login window, log in, open Safari, navigate with VoiceOver) run before and after each candidate would turn the ranked list above into measured fact, starting with `ssd=off` because it is one word to change.

The tester's complaint was about lag, and lag is not the same as load. The measured Mac was cheap at rest and still, by a tester's account, slow to answer the keyboard. The delay between a key press and speech has never been measured in any virtual Mac. Orchord already has both halves of the instrument: it can send a key at a known instant and record the Mac's sound to a file. Timing the gap between the two would make Orchord the first project to put a number on what its users actually feel, and would show whether the sound buffer, the drawing, or something not yet suspected is the cause.
