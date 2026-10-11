# The boot folder Orchord's Macs start from

`EFI\` is built by `orchord-efi` (`crates\orchord-core\src\bin\orchord-efi.rs`) from official
releases only. Nothing in it is edited by hand; to change it, change the tool and run it again:

```
orchord-efi <unpacked OpenCore release> <folder of unpacked kexts> third_party\opencore
```

What is inside, and where it came from:

- **OpenCore 1.0.8** (`BOOTx64.efi`, `OpenCore.efi`, `OpenRuntime.efi`, `OpenHfsPlus.efi`,
  `SSDT-EC.aml`): <https://github.com/acidanthera/OpenCorePkg>, BSD-3-Clause.
- **`config.plist`**: OpenCore's own `Docs\Sample.plist` with the changes listed, with
  reasons, in `orchord-efi.rs`.
- **Lilu 1.7.2**: <https://github.com/acidanthera/Lilu>, BSD-3-Clause.
- **VirtualSMC 1.3.8**: <https://github.com/acidanthera/VirtualSMC>, BSD-3-Clause.
- **WhateverGreen 1.7.1**: <https://github.com/acidanthera/WhateverGreen>, BSD-3-Clause.
- **VoodooPS2Controller 2.3.7**: <https://github.com/acidanthera/VoodooPS2>, Apple Public
  Source License 2.0. Its source is at that address. The release carries **VoodooInput**
  inside it (<https://github.com/acidanthera/VoodooInput>, GPL-2.0), which Orchord does not
  load.

The licence text of each is in `LICENSES\`, taken from its repository.

The Apple SMC key string that QEMU's Apple SMC device needs is not part of this folder or of
this repository.
