# Orchord

Orchord is a virtual machine manager for Windows that runs Intel versions of macOS, built to
be used with a screen reader and the keyboard alone. It was designed by a blind user for blind
users: every command is a menu item, every control is a native Windows control with a proper
name, and what happens to a Mac is spoken and signalled with sounds.

It does not emulate a Mac itself. It drives [QEMU](https://www.qemu.org/) with the Windows
Hypervisor Platform accelerator, boots macOS with [OpenCore](https://github.com/acidanthera/OpenCorePkg),
and adds the window, the installer automation, the speech and the file sharing around them.

## What it does

- **A list of Macs.** Enter starts the selected one. The window title names the running Mac.
- **A wizard for new Macs (Control+N).** Four pages: name and location; macOS version and
  where the installer comes from; memory, processor cores and disk size; an overview.
- **Easy install.** Orchord downloads the installer from Apple, prepares the disk, works the
  installer by itself, and tells you when macOS has reached its first set-up screen, where it
  turns VoiceOver on. You agree to Apple's licence in the wizard.
- **Your own installer file.** Instead of downloading, the wizard takes an installer you
  already have: a full installer saved as `.iso` or `.cdr` (what `createinstallmedia` and
  `hdiutil` make on a Mac), or Apple's `BaseSystem.dmg`. An ISO is used where it is, not
  copied, so it has to stay there until macOS is installed. Easy install works with it too.
- **The Mac's screen as something a screen reader can read.** While a Mac runs, the window
  holds only its screen, so NVDA's text recognition (NVDA+R) reads the Mac. The mouse works
  over the screen, so a recognised word can be clicked.
- **Keyboard capture.** Control+G sends every key to the Mac, including the Windows key as
  Command. Pressing and releasing Control+Alt by themselves gives the keyboard back.
- **Speech and sounds.** Status messages are spoken through the running screen reader.
  Separate sounds mark a Mac starting, restarting, finishing installation, crashing and
  failing to start, and the keyboard being captured and released.
- **Restarts that work.** Under the Windows hypervisor a Mac cannot restart itself; Orchord
  notices and starts it again.
- **A shared folder.** A Windows folder appears in the Mac as a network drive, live in both
  directions, with no administrator rights.
- **USB devices.** Control+U lists the USB devices plugged into the PC and asks QEMU to hand
  one to the running Mac. Windows only lets go of a device when the UsbDk driver is installed,
  or when the device uses the generic WinUSB driver; otherwise Orchord says so and the device
  stays with Windows.

## Keys

- Control+N: new virtual machine.
- Enter: start the selected Mac.
- Control+G: send the keyboard to the Mac. Control+Alt, pressed and released alone: get it back.
- Alt+P: the Power menu (Restart, Shut Down, Suspend).
- Control+L: switch between the Mac's screen and the status log.
- Control+P: settings for the Mac.
- Control+U: USB devices.
- Control+Comma: Orchord's own settings (checking and setting up the PC, the QEMU folder,
  where Macs are kept, the Apple SMC key, light or dark appearance, checking for updates).
- F11: full screen, and back. The title, menus and status line are hidden in full screen.
- Delete: delete the selected Mac. Orchord asks first, and Enter answers No.
- F5: read the list of Macs again.

While the keyboard is captured, Right Control and the Menu key work as extra Command keys,
because Windows keeps Windows+L for itself. This can be turned off in the Machine menu.
While it is captured every key goes to the Mac, F11 and Delete included.

## Updates

When Orchord starts it asks GitHub whether a newer version has been published, and says so if
there is one. File, Check for Updates shows what is new and, if you agree, downloads the new
version, checks it against the checksum GitHub gives for it, puts it in place and restarts
Orchord. The question at start-up sends nothing about you or your PC; it can be turned off
in Orchord Settings.

## Deleting a Mac

Delete removes the Mac's disk and its `vmconfig` folder, and nothing else. Files in its
shared folder, and anything else you put in its folder, are yours and are left where they
are; Orchord says so when that happens.

## When a Mac will not start

Machine, Save Diagnostic Report writes one text file into the Mac's folder and shows it in
File Explorer. It holds the PC's processor, the QEMU version, how the Mac is started and the
ends of its logs, which is what someone helping from afar needs. It does not hold the Apple
SMC key; it does hold folder names, which may include your Windows user name.

Make the report **while the Mac is stuck**, not after powering it off: only then can it
also say what each of the Mac's processors is doing at that moment, which shows where it
stopped. The logs it draws on are in the Mac's `vmconfig` folder: `firmware.log` (the
firmware, up to the hand-over to macOS), `serial.log` (macOS itself) and `qemu.log`.

## macOS versions

Tested on an Intel Core i7-1355U laptop:

- macOS Big Sur 11, Monterey 12 and Sequoia 15 install and run, with VoiceOver speaking.
- VoiceOver cannot speak inside Monterey's installer, so Monterey needs easy install.
- Catalina 10.15 boots but has no sound. Lion 10.7 and Mountain Lion 10.8 do not boot.
- Apple silicon versions of macOS are out of scope.

If a Mac you are installing by hand goes quiet after a restart, read its screen with NVDA+R.
"OpenCore Boot Menu" means the boot loader is waiting for a key: send the keyboard to the Mac
and press Enter. Easy install does this by itself.

There is no graphics acceleration: macOS draws everything on the processor. Speech is
responsive; animation is not.

## Known limits

- Everything above was tested on that one Intel laptop.
- **AMD processors do not work yet.** On an AMD PC a Mac stops on a black screen of start-up
  text: macOS dies in its first moments. 0.1.2 made QEMU answer requests for registers only
  Intel processors have, and that was not enough. 0.1.3 adds a second boot folder,
  `opencore-amd`, used when the processor is not Intel's, which takes one write out of the
  Mac's kernel that AMD processors refuse; and it makes QEMU write every refused register
  request into its log. None of this could be tried here, where the only PC is an Intel one.
  If a Mac does not start on your AMD PC, a diagnostic report (Machine menu) attached to an
  issue shows exactly what stopped it.
- Easy install has run from start to finish for Big Sur, from Apple's download and from an
  ISO. For Monterey and Sequoia only its first stage has been run.
- Handing a USB device to a Mac has not worked yet: it needs the UsbDk driver, which was not
  installed on the test PC.
- Whether a screen reader on Windows lets go of its own keys while the keyboard is sent to
  the Mac has not been checked.

## What you need

Windows 10 or 11 on an Intel processor (AMD: see Known limits). Orchord's "Check and set up
this PC" button looks for the rest and offers to get what is missing:

- **QEMU 11 or newer.** The button installs it with winget, Windows' own package manager,
  which downloads QEMU's installer from QEMU's site; Windows asks for permission. By hand:
  `winget install SoftwareFreedomConservancy.QEMU`.
- **The Windows Hypervisor Platform feature.** The button asks QEMU whether it can use it,
  and if not, turns the feature on; Windows asks for permission and has to be restarted. By
  hand: Turn Windows features on or off, check Windows Hypervisor Platform, restart.
- **The Apple SMC key,** a 64-character string that QEMU's Apple SMC device has to present
  before macOS will start. Orchord does not come with it; the button fetches it after asking.
  See "Getting the Apple SMC key" below.
- **A text recognition language in Windows,** for easy install. The button says whether
  there is one, and where to add one.

The button changes nothing without a Yes from you.

## Getting started

1. Download `Orchord-<version>-windows-x64.zip` from the Releases page and unzip it anywhere.
   Keep the files together: `prism.dll` and the `opencore` and `opencore-amd` folders must
   stay beside `orchord.exe`.
2. Start `orchord.exe`. The first time, it says that it is not fully set up.
3. Open File, Orchord Settings (Control+Comma). The first button there is "Check and set up
   this PC". Press it and answer its questions; it ends by saying whether the PC is ready.
4. Choose where virtual machines are kept, if the folder offered does not suit you, and
   press OK.
5. Press Control+N to create a Mac.

The settings are kept in `%APPDATA%\orchord\settings.toml`. Two lines can be added to that
file by hand for experiments, and are not needed otherwise: `accelerator = "..."` replaces
what Orchord gives QEMU's `-accel`, and `extra_qemu_args = ["...", "..."]` adds arguments to
the end of QEMU's command line.

## Getting the Apple SMC key

macOS asks the Mac it is starting on for a 64-character key, held in a chip called the SMC,
and stops if the answer is wrong. QEMU's Apple SMC device gives whatever key it is handed, so
Orchord has to be told it. The key is the same on every Intel Mac. It is Apple's text, so it
is not in this repository or in the release. There are three ways to get it:

- **Let Orchord fetch it.** In File, Orchord Settings, press "Get the key from the
  internet". After asking you, Orchord downloads a public start-up script of the OSX-KVM
  project from GitHub (or quickemu's, if that fails), takes the key out of it and puts it in
  the key field. Nothing about you or your PC is sent. Orchord knows the key's fingerprint,
  not the key, so it can tell that what it found is right.
- **Read it from an Intel Mac.** Gabriel Somlo's page
  [Running Mac OS X as a QEMU/KVM Guest](https://www.contrib.andrew.cmu.edu/~somlo/OSXKVM/)
  offers a small program, `SmcDumpKey.c`.
  Compiled and run on an Intel Mac, it prints the SMC's two values, OSK0 and OSK1; joined
  together they are the key. Apple silicon Macs have no such chip.
- **Look it up.** Because it never changes, other projects that run macOS under QEMU carry
  it in their start-up scripts. In `OpenCore-Boot.sh` of
  [OSX-KVM](https://github.com/kholia/OSX-KVM) it is on the line containing
  `isa-applesmc,osk=`, and reads as an English sentence that ends with Apple's copyright
  notice. [quickemu](https://github.com/quickemu-project/quickemu) has it too, but with its
  letters scrambled, so that one is no use for reading by hand.

Enter it in File, Orchord Settings. Orchord refuses anything that is not 64 characters long,
and asks before saving 64 characters that are not the key.

## Building

Rust 1.85 or newer with the MSVC toolchain, plus CMake, Ninja and LLVM for the first build of
wxWidgets:

```
cargo build
cargo test --workspace
```

The program is `target\debug\orchord.exe`. `target\debug\orchord-vm.exe` runs one Mac from the
command line and prints what happens to it. After `cargo build --release`, `package.ps1`
makes the zip for the Releases page.

## Layout

- `src\`: the window. `main_window.rs`, `wizard.rs`, `program_settings.rs`,
  `settings_dialog.rs`, `usb_dialog.rs`, `updater.rs`, `keyboard_capture.rs`, `speech.rs`,
  and `busy.rs`, which keeps the window answering while slow work is done elsewhere.
- `crates\orchord-core\`: everything without a window. The QEMU launcher, the supervisor that
  keeps a Mac running across restarts, easy install and its text recognition, the installer
  download, the handling of installer files, the shared-folder server, USB hand-over and the
  sound cues.
- `third_party\prism\`: the [Prism](https://github.com/ethindp/prism) speech library.
- `third_party\opencore\`: the boot folder the Macs start from, built by `orchord-efi` from
  the official OpenCore release and acidanthera's kernel extensions.
- `reports\` and `research_notes\`: the research the design rests on, and what was found by
  testing on real hardware.

A virtual machine is one folder named after it, holding `disk\` (the Mac's drive) and
`vmconfig\` (settings, firmware variables and logs).

## Licences of the parts

Orchord's own code has no licence chosen yet, which means all rights in it are reserved for
now. The parts it is built from and ships with keep their own licences:

- Prism (MPL-2.0): `third_party\prism\LICENSES\`.
- OpenCore, Lilu, VirtualSMC and WhateverGreen (BSD-3-Clause), VoodooPS2Controller (Apple
  Public Source License 2.0) and the VoodooInput it carries (GPL-2.0):
  `third_party\opencore\LICENSES\`, with where each one's source is in the README beside it.
- wxWidgets, through wxDragon, and the other Rust crates: `third_party\RUST-CRATES.md`.

In the release zip the same files are in its `licences` folder.

macOS is Apple's: its licence allows it only on Apple hardware, and accepting it is each
user's own decision.
