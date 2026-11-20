[CmdletBinding()]
param ([string]$Version)

$ErrorActionPreference = "Stop"
$manifest = Join-Path $PSScriptRoot 'desktop\Cargo.toml'
if (-not $Version) {
    $versionLine = Select-String -LiteralPath $manifest -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    $Version = $versionLine.Matches[0].Groups[1].Value
}

$compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
$iscc = if ($compiler) {
    $compiler.Source
} else {
    @(
        'C:\Program Files\Inno Setup 7\ISCC.exe'
        'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
        'C:\Program Files\Inno Setup 6\ISCC.exe'
    ) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $iscc) {
    throw 'Inno Setup Compiler (ISCC.exe) was not found.'
}

$build = Join-Path $PSScriptRoot 'target\desktop'
cargo build --locked --release --manifest-path $manifest --target-dir $build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $build 'release\omnispot-desktop.exe'
$installer = Join-Path $PSScriptRoot 'installer\installer.iss'
& $iscc "/DAppVersion=$Version" "/DRustExe=$exe" $installer
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
