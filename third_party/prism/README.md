# Prism 0.18.3

The speech library Orchord uses to talk through NVDA, JAWS and other screen readers:
<https://github.com/ethindp/prism>, MPL-2.0. These are the unmodified `dynamic/release` files
from the official `prism-windows-x64.zip` of release v0.18.3, which is the version the
`prism-sys` 0.1.4 crate is written against.

- `lib\prism.lib` is linked at build time (`PRISM_LIB_DIR` in `.cargo\config.toml`).
- `bin\prism.dll` is copied next to `orchord.exe` by `build.rs` and must ship with it.
- `LICENSES\` holds Prism's licence and those of the libraries inside it.

A prebuilt copy is used because building Prism from source needs Microsoft's ATL headers,
which the Visual Studio Build Tools do not install by default.
