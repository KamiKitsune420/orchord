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
- Control+Comma: Orchord's own settings (QEMU folder, where Macs are kept, the Apple SMC key).
- F5: read the list of Macs again.

While the keyboard is captured, Right Control and the Menu key work as extra Command keys,
because Windows keeps Windows+L for itself. This can be turned off in the Machine menu.

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

- Everything above was tested on that one laptop. AMD processors have not been tried.
- Easy install has run from start to finish for Big Sur, from Apple's download and from an
  ISO. For Monterey and Sequoia only its first stage has been run.
- Handing a USB device to a Mac has not worked yet: it needs the UsbDk driver, which was not
  installed on the test PC.
- Whether a screen reader on Windows lets go of its own keys while the keyboard is sent to
  the Mac has not been checked.

## What you need

- Windows 10 or 11 on an Intel processor, with the Windows Hypervisor Platform feature on
  (Turn Windows features on or off, then restart).
- QEMU 11 or newer (`winget install SoftwareFreedomConservancy.QEMU`).
- The Microsoft Visual C++ Redistributable, which most PCs already have
  (`winget install Microsoft.VCRedist.2015+.x64`). Without it Windows says that
  `VCRUNTIME140.dll` was not found.
- The Apple SMC key: a 64-character string that QEMU's Apple SMC device has to present before
  macOS will start. Orchord does not come with it.
- A text recognition language installed in Windows, for easy install.

## Getting started

1. Download `Orchord-<version>-windows-x64.zip` from the Releases page and unzip it anywhere.
   Keep the files together: `prism.dll` and the `opencore` folder must stay beside
   `orchord.exe`.
2. Start `orchord.exe`. The first time, it says that it is not fully set up.
3. Open File, Orchord Settings (Control+Comma). Check the QEMU folder, choose where virtual
   machines are kept, and enter the Apple SMC key.
4. Press Control+N to create a Mac.

The settings are kept in `%APPDATA%\orchord\settings.toml`.

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
  `settings_dialog.rs`, `usb_dialog.rs`, `keyboard_capture.rs`, `speech.rs`.
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
