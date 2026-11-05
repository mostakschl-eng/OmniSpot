[CmdletBinding()]
param (
    [string]$Version = "1.0.0",
    [switch]$SelfContained,
    [switch]$All
)

$ErrorActionPreference = "Stop"

# 1. Locate Inno Setup Compiler (ISCC.exe)
$isccPath = $null
$candidatePaths = @(
    "C:\Program Files\Inno Setup 7\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)

$cmd = Get-Command "iscc" -ErrorAction SilentlyContinue
if ($cmd) {
    $isccPath = $cmd.Source
} else {
    foreach ($path in $candidatePaths) {
        if (Test-Path $path) {
            $isccPath = $path
            break
        }
    }
}

if (-not $isccPath) {
    Write-Error "Inno Setup Compiler (ISCC.exe) not found. Please install Inno Setup 6+."
    exit 1
}

Write-Host "Using Inno Setup compiler: $isccPath" -ForegroundColor Cyan

function Build-Package {
    param (
        [string]$Profile,
        [string]$Ver
    )

    $pubxml = if ($Profile -eq "folder") { "FolderProfile" } else { "SelfContainedProfile" }
    $publishFolder = "package-$Profile"

    Write-Host "`n>>> Publishing OmniSpot with profile: $pubxml (Version: $Ver) ..." -ForegroundColor Green
    dotnet publish OmniSpot/OmniSpot.csproj -p:PublishProfile=$pubxml -p:PublishDir="bin/publish/$publishFolder/" -c Release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "dotnet publish failed for profile $pubxml"
        exit $LASTEXITCODE
    }

    Write-Host "`n>>> Compiling Inno Setup installer for profile: $Profile ..." -ForegroundColor Green
    Push-Location OmniSpot
    try {
        & $isccPath /DAppVersion="$Ver" /DProfileName="$Profile" /DPublishFolder="$publishFolder" installer.iss
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Inno Setup compilation failed for profile $Profile"
            exit $LASTEXITCODE
        }
    }
    finally {
        Pop-Location
    }
}

if ($All) {
    Build-Package -Profile "folder" -Ver $Version
    Build-Package -Profile "selfcontained" -Ver $Version
} elseif ($SelfContained) {
    Build-Package -Profile "selfcontained" -Ver $Version
} else {
    Build-Package -Profile "folder" -Ver $Version
}

Write-Host "`nAll requested installers built successfully! Output directory: D:\OmniSpot\OmniSpot\Inno_Files" -ForegroundColor Cyan
