[CmdletBinding()]
param(
    [string]$Version = "1.0.0",
    [switch]$SelfContained
)

$ErrorActionPreference = "Stop"
$rustRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$package = Join-Path $rustRoot "package"
$iscc = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $iscc) {
    $candidates = @(
        "C:\Program Files\Inno Setup 7\ISCC.exe",
        "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
        "C:\Program Files\Inno Setup 6\ISCC.exe"
    )
    $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw "Inno Setup Compiler (ISCC.exe) was not found." }

Push-Location $rustRoot
try {
    cargo +stable-x86_64-pc-windows-msvc build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "Rust release build failed." }

    New-Item -ItemType Directory -Force -Path $package | Out-Null
    Copy-Item "target\release\omnispot-rust.exe" "$package\OmniSpot.exe" -Force
    Copy-Item "..\LICENSE.md" "$package\LICENSE.md" -Force
    if (Test-Path "..\OmniSpot\app-switcher.ico") {
        Copy-Item "..\OmniSpot\app-switcher.ico" "$package\app-switcher.ico" -Force
    }

    $profile = if ($SelfContained) { "selfcontained" } else { "folder" }
    & $iscc "/DAppVersion=$Version" "/DProfileName=$profile" "installer.iss"
    if ($LASTEXITCODE -ne 0) { throw "Rust Inno Setup compilation failed." }
} finally {
    Pop-Location
}
