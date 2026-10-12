# Measurements taken on the project's own PC

These are direct measurements, not web research. They were taken on 12 October 2026 by the coordinator while the web research ran, to give the report real numbers to weigh advice against. No other note in this folder contains a measurement.

## Set-up

- Host: Intel Core i7-1355U laptop (2 performance cores with hyper-threading, 8 efficiency cores, 12 logical processors), 16 GB RAM, Windows 11 Pro build 26200, on a Balanced power plan, virtualization-based security running.
- QEMU 11.1.0 with `-accel whpx`, started by Orchord 0.1.4's command-line runner (`orchord-vm.exe`, release build) exactly as the product starts it, except that the Mac's sound was set to "none" so nothing would be heard. So the sound device and its timer are NOT included in any figure below.
- Guest: the project's installed macOS Big Sur 11 test Mac. 4 virtual processors (4 sockets, 1 core each), 4096 MB, Penryn processor model, screen 1280 by 800, boot arguments `-v keepsyms=1 debug=0x100 serial=3`.
- No Orchord window was open, so nothing was fetching the screen unless stated.
- Method: QEMU's total processor time (and each thread's) was read before and after a timed window, usually 60 seconds. Figures are a share of ONE logical processor; 100 percent means one logical processor fully busy, and the PC has 12. Five research programs were also running on the PC during the first half of the session, which only used the network, not much processor.

## Readings

Start-up, from the kernel starting to a settled login window: about 4 minutes of heavy work (296, 172, 30, 73, 140, 100 percent in successive 30-second windows), then calm.

For about 5 minutes after first reaching the login window the load came and went (102, 131, 126, 68 percent in one-minute windows) while QEMU's working set grew from 2.8 GB to 4.0 GB. This is macOS doing background work after start-up. It then settled.

Settled at the login window, password box empty with its text cursor blinking: 14 to 18 percent of one logical processor, in ten separate one-minute readings (31, 17, 17, 14, 18, 17, 14, 16, 15, 15). That is about 1.2 to 1.5 percent of the whole PC. The four virtual-processor threads shared it about equally (roughly 3 to 4 percent each).

Login window with text sitting selected in the password box (a wrong password had been typed, so the box held highlighted dots and showed no blinking cursor): 2 to 3 percent of one logical processor, in four one-minute readings (3, 3, 2, 3). The only visible difference from the 14 to 18 percent state was that nothing on the screen was changing. The coordinator's reading: a single blinking text cursor, redrawn in software about twice a second, accounts for roughly 12 percent of one logical processor. This was not isolated as a controlled experiment, but it is the plainest evidence in the folder that redrawing the screen, however small the change, is what costs processor time in this guest.

Fetching the screen with QMP `screendump` twice a second (PPM, 3,072,016 bytes per picture, 130 pictures in 66 seconds), Mac idle at the login window: total 20 percent, against 14 to 17 percent without. QEMU's main thread used 5.5 percent of one logical processor during the fetches and about 1 percent without. So Orchord's twice-a-second screen fetch costs about 5 percent of one logical processor (0.4 percent of the PC) at 1280 by 800. It is not a significant source of load.

Logging in: 383 percent in the first minute (all four virtual processors close to flat out), then 95 and 120 percent, then settled after about 3 minutes.

Logged in, desktop showing with a Terminal window open, nothing pressed: 14 to 17 percent of one logical processor (17, 14, 14).

VoiceOver: Command+F5 was sent. With sound off there was no way to confirm VoiceOver was speaking, so treat these as weak. 29 percent in the 45 seconds after the key press, 12 percent with nothing pressed afterwards, and 15 percent while Control+Option+Right Arrow was sent twice a second for a minute in the Terminal window. No sound device was attached, so speech output cost is not captured.

Opening Safari from Spotlight: 250 percent for the first 30 seconds, 253 percent for the next 30 seconds, then 135 and 151 percent in the following one-minute windows with Safari open on its start page and nothing pressed. Safari left alone kept about one and a half logical processors busy; the bare desktop kept about one seventh of one busy.

Memory: QEMU's private memory stayed at about 4.5 to 4.8 GB for a 4096 MB guest from early in start-up (all guest memory is committed up front). Its working set (memory actually in use by the PC) moved between 2.7 and 4.1 GB.

## What these readings do and do not show

- A Mac that is sitting still is cheap on this PC: about one seventh of one logical processor, a little over 1 percent of the whole PC. Idle virtual processors do rest properly under WHPX on Windows 11. There is no idle spin to fix on this host.
- The load is in activity: starting up, logging in and running applications each kept two and a half to four logical processors busy for a minute or more, and an open Safari window kept about one and a half busy indefinitely. With no graphics hardware, anything that draws is processor work.
- Orchord's own overhead (the screen fetch) is small. The serial log, firmware log and boot arguments were not varied, so their cost is not measured here.
- Not measured: a Windows 10 host (where QEMU emulates the interrupt controller itself and idle cost should be higher, according to the QEMU notes), an AMD host, macOS 12, 14 or 15, the effect of sound being on, the effect of any proposed setting (no before-and-after test of `serial=1`, `hpet=off`, removing the EHCI controller, `ssd=off`, Reduce Motion, Reduce Transparency or a lower resolution was run), and the delay between a key press and VoiceOver speaking.
- Every figure is from one session on one PC and one guest. Treat them as a baseline, not as a benchmark.
