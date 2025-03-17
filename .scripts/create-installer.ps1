$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { './target' }

$exe = "$targetDir/release/komorebi-switcher.exe"

if (!(Test-Path $exe)) {
  cargo build --release
}

Copy-Item -Force $exe "./installer/komorebi-switcher.exe"

makensis /V4 "./installer/installer.nsi"

New-Item -Force "dist" -Type Directory > $null

Move-Item -Force "./installer/komorebi-switcher.exe" "./dist/komorebi-switcher.exe"
Move-Item -Force "./installer/komorebi-switcher-setup.exe" "./dist/komorebi-switcher-setup.exe"

Compress-Archive -Update "./dist/komorebi-switcher.exe" "./dist/komorebi-switcher.zip"
