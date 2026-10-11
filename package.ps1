# Makes the zip that goes on the Releases page, from a release build that is already there:
#   cargo build --release
#   .\package.ps1
# The zip holds the program, the speech library and the boot folder that must sit beside it,
# the README, and the licences of everything that ships inside it. It is written to
# target\Orchord-<version>-windows-x64.zip, and its name and SHA-256 are printed.
$ErrorActionPreference = "Stop"
$repo = $PSScriptRoot
$version = ([regex]::Match((Get-Content "$repo\Cargo.toml" -Raw), '(?m)^version\s*=\s*"([^"]+)"')).Groups[1].Value
$name = "Orchord-$version-windows-x64"
$stage = "$repo\target\package\$name"
$zip = "$repo\target\$name.zip"
if (Test-Path "$repo\target\package") { [System.IO.Directory]::Delete("$repo\target\package", $true) }
if (Test-Path $zip) { [System.IO.File]::Delete($zip) }
$null = New-Item -ItemType Directory -Path "$stage\licences\prism", "$stage\licences\opencore"

Copy-Item "$repo\target\release\orchord.exe", "$repo\third_party\prism\bin\prism.dll", "$repo\README.md" $stage
# The boot folder comes from the repository, not from the build folder, so it is exactly what was committed.
$null = New-Item -ItemType Directory -Path "$stage\opencore"
Copy-Item "$repo\third_party\opencore\EFI" "$stage\opencore\EFI" -Recurse
Copy-Item "$repo\third_party\prism\LICENSES\*" "$stage\licences\prism" -Recurse
Copy-Item "$repo\third_party\prism\README.md" "$stage\licences\prism\README.md"
Copy-Item "$repo\third_party\opencore\LICENSES\*" "$stage\licences\opencore"
Copy-Item "$repo\third_party\opencore\README.md" "$stage\licences\opencore\README.md"
Copy-Item "$repo\third_party\RUST-CRATES.md" "$stage\licences\RUST-CRATES.md"

Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($stage, $zip, [System.IO.Compression.CompressionLevel]::Optimal, $true)
"{0}  {1:N1} MB  SHA-256 {2}" -f $zip, ((Get-Item $zip).Length / 1MB), (Get-FileHash $zip -Algorithm SHA256).Hash
