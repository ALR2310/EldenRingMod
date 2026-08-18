<#
.SYNOPSIS
    Builds one mod's release DLL and copies it next to its .ini into build/,
    so both files sit in one flat, easy-to-find folder instead of buried
    among the many other files cargo puts in target/release/.

.PARAMETER Mod
    The mod's PascalCase name, matching both its [lib] name in Cargo.toml
    (the built DLL's filename) and its .ini filename - e.g. "AutoRegen" for
    AutoRegen.dll/AutoRegen.ini. The crate/package name is derived by
    lowercasing this (AutoRegen -> autoregen), which matches how every mod
    crate in this workspace is named.

.EXAMPLE
    pwsh -File scripts/build-mod.ps1 -Mod AutoRegen
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Mod
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

Copy-Item $DllPath -Destination $BuildDir -Force
Copy-Item $IniPath -Destination $BuildDir -Force

Write-Host "==> build\$Mod.dll + build\$Mod.ini ready" -ForegroundColor Green
