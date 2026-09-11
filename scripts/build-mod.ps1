<#
.SYNOPSIS
    Builds one mod's release DLL and copies it next to its .ini into build/,
    so both files sit in one flat, easy-to-find folder instead of buried
    among the many other files cargo puts in target/release/. Optionally
    also zips the pair for uploading (e.g. to Nexus).

.PARAMETER Mod
    The mod's PascalCase name, matching both its [lib] name in Cargo.toml
    (the built DLL's filename) and its .ini filename - e.g. "AutoRegen" for
    AutoRegen.dll/AutoRegen.ini. The crate/package name is derived by
    lowercasing this (AutoRegen -> autoregen), which matches how every mod
    crate in this workspace is named.

.PARAMETER Zip
    When set, also packages build\<Mod>.dll + build\<Mod>.ini into
    build\<Mod>-<version>.zip. No zip is created unless this switch is
    passed - most local test builds don't need one.

.PARAMETER Version
    Version string to use for the zip's filename (only meaningful with
    -Zip). Defaults to the crate's own Cargo.toml (`[package] version`) if
    omitted - but that is NOT necessarily the same as the version published
    on Nexus (this workspace's crates don't keep those in sync). The
    publish-nexus-mod skill always passes the release version it confirmed
    with the user explicitly, rather than relying on this default.

.EXAMPLE
    pwsh -File scripts/build-mod.ps1 -Mod AutoRegen

.EXAMPLE
    pwsh -File scripts/build-mod.ps1 -Mod AutoRegen -Zip -Version 2.5.0
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Mod,

    [switch]$Zip,

    [string]$Version
)

$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Crate = $Mod.ToLower()
$CrateDir = Join-Path $RepoRoot "crates\$Crate"
$IniPath = Join-Path $CrateDir "$Mod.ini"

if (-not (Test-Path $CrateDir)) {
    throw "No crate directory found at '$CrateDir' - is '$Mod' spelled the same as its Cargo package (lowercased)?"
}
if (-not (Test-Path $IniPath)) {
    throw "No ini found at '$IniPath' - expected '$Mod.ini' next to the crate's Cargo.toml."
}

Write-Host "==> Building $Mod (release)..." -ForegroundColor Cyan
& cargo build --release -p $Crate --manifest-path (Join-Path $RepoRoot "Cargo.toml")
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed for '$Crate' (exit code $LASTEXITCODE)."
}

$DllPath = Join-Path $RepoRoot "target\release\$Mod.dll"
if (-not (Test-Path $DllPath)) {
    throw "Build succeeded but '$DllPath' wasn't produced - check the crate's [lib] name in Cargo.toml matches '$Mod'."
}

# One shared flat folder for every mod's output - AutoRegen.dll/.ini and
# SomeTweaks.dll/.ini end up side by side here, never colliding since each
# mod's filenames are unique.
$BuildDir = Join-Path $RepoRoot "build"
New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null

$BuildDllPath = Join-Path $BuildDir "$Mod.dll"
$BuildIniPath = Join-Path $BuildDir "$Mod.ini"
Copy-Item $DllPath -Destination $BuildDllPath -Force
Copy-Item $IniPath -Destination $BuildIniPath -Force

Write-Host "==> build\$Mod.dll + build\$Mod.ini ready" -ForegroundColor Green

if ($Zip) {
    $ZipVersion = $Version
    if (-not $ZipVersion) {
        $CargoTomlPath = Join-Path $CrateDir "Cargo.toml"
        $CargoToml = Get-Content $CargoTomlPath -Raw
        if ($CargoToml -notmatch '(?ms)^\[package\](.*?)(\r?\n\[|\z)') {
            throw "Could not find a [package] section in '$CargoTomlPath'."
        }
        $PackageSection = $Matches[1]
        if ($PackageSection -notmatch 'version\s*=\s*"([^"]+)"') {
            throw "Could not find a 'version' key in '$CargoTomlPath''s [package] section."
        }
        $ZipVersion = $Matches[1]
    }

    $ZipPath = Join-Path $BuildDir "$Mod-$ZipVersion.zip"
    if (Test-Path $ZipPath) {
        Remove-Item $ZipPath -Force
    }
    Compress-Archive -Path $BuildDllPath, $BuildIniPath -DestinationPath $ZipPath -Force

    Write-Host "==> $ZipPath ready" -ForegroundColor Green
}
