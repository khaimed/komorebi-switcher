# Exit if komorebi-switcher.exe is not found in ./dist
if (-not (Test-Path -Path "./dist/glazewm-switcher.exe")) {
    Write-Host -ForegroundColor Red "Error: glazewm-switcher.exe not found in ./dist, run build.ps1 first"
    exit 1
}

# Copy the glazewm-switcher.exe to the installer directory
Copy-Item -Force "./dist/glazewm-switcher.exe" "./installer/glazewm-switcher.exe"

# Copy the icon.ico to the installer directory
Copy-Item -Force "./assets/icon.ico" "./installer/icon.ico"

# Create the installer
makensis /V4 "./installer/installer.nsi"

# Move the installer to the dist directory
Move-Item -Force "./installer/glazewm-switcher-setup.exe" "./dist/glazewm-switcher-setup.exe"

# Compress the glazewm-switcher.exe to glazewm-switcher.zip
Compress-Archive -Update "./dist/glazewm-switcher.exe" "./dist/glazewm-switcher.zip"

# Remove artifacts
Remove-Item -Force "./installer/glazewm-switcher.exe"
Remove-Item -Force "./installer/icon.ico"
