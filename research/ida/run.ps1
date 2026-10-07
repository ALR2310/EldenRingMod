<#
.SYNOPSIS
    Chạy 1 script IDAPython của research/ida/ trên database eldenring.exe.i64 có sẵn.

.DESCRIPTION
    Dùng idat (headless): idat -A -S"<script> <args>" -L<log> <db>.i64.
    Database nằm ở dumps/ida/<version>/eldenring.exe.i64 (version = FileVersion
    của exe, vd. 2.7.1.0 = patch 1.17.1). Output của script ghi vào
    dumps/ida/<version>/out/ (biến môi trường ER_OUT). Không mở database đang
    được IDA GUI giữ - đóng GUI trước.

.PARAMETER Script
    Tên file trong research/ida/ (vd. decompile.py), có hoặc không có đuôi .py.

.PARAMETER Version
    Thư mục version trong dumps/ida/ (vd. 2.7.1.0), hoặc `all` để chạy lần lượt
    mọi version có .i64. Mặc định: version mới nhất.

.PARAMETER Arguments
    Tham số truyền cho script (RVA, mẫu AOB, cờ --x...). Đặt trong dấu nháy nếu
    có khoảng trắng; với aob.py mỗi mẫu là 1 chuỗi nháy kép riêng.

.EXAMPLE
    pwsh research/ida/run.ps1 decompile 1e92100 --callees
    pwsh research/ida/run.ps1 aob -Version all "48 8B 05 ?? ?? ?? ?? 48 85 C0"
    pwsh research/ida/run.ps1 -Script xrefs.py -Version 2.7.1.0 25e100 --depth=2
#>
param(
    [Parameter(Mandatory, Position = 0)][string]$Script,
    [string]$Version = "",
    [Parameter(ValueFromRemainingArguments)][string[]]$Arguments = @()
)

$ErrorActionPreference = "Stop"
$Here = $PSScriptRoot
$RepoRoot = Split-Path -Parent (Split-Path -Parent $Here)
$IdaRoot = Join-Path $RepoRoot "dumps\ida"

$Idat = if ($env:ER_IDAT) { $env:ER_IDAT } else { "D:\Programs\IDA Professional 9.3\idat.exe" }
if (-not (Test-Path $Idat)) { throw "Không thấy idat.exe ($Idat). Đặt biến môi trường ER_IDAT." }

if (-not $Script.EndsWith(".py")) { $Script += ".py" }
$ScriptPath = Join-Path $Here $Script
if (-not (Test-Path $ScriptPath)) { throw "Không thấy script '$Script' trong $Here" }

$All = Get-ChildItem $IdaRoot -Directory |
    Where-Object { Test-Path (Join-Path $_.FullName "eldenring.exe.i64") } |
    Sort-Object { try { [version]$_.Name } catch { [version]"0.0" } }
if (-not $All) { throw "Không có database nào trong $IdaRoot" }

$Targets = if ($Version -eq "all") { $All }
elseif ($Version) { $All | Where-Object Name -eq $Version }
else { $All | Select-Object -Last 1 }
if (-not $Targets) { throw "Không thấy version '$Version'. Có: $($All.Name -join ', ')" }

# Ghép tham số thành 1 chuỗi cho -S; tham số có khoảng trắng được bọc nháy kép.
$Joined = ($Arguments | ForEach-Object { if ($_ -match '\s') { '\"' + $_ + '\"' } else { $_ } }) -join " "

foreach ($T in $Targets) {
    $Db = Join-Path $T.FullName "eldenring.exe.i64"
    $Out = Join-Path $T.FullName "out"
    New-Item -ItemType Directory -Force $Out | Out-Null
    $Log = Join-Path $Out ("{0}.log" -f [IO.Path]::GetFileNameWithoutExtension($Script))
    $env:ER_OUT = $Out
    $cmdline = '-A -S"{0} {1}" -L"{2}" "{3}"' -f $ScriptPath, $Joined, $Log, $Db
    Write-Host "==> [$($T.Name)] $Script $Joined" -ForegroundColor Cyan
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Idat -ArgumentList $cmdline -Wait -PassThru -NoNewWindow
    Write-Host ("    xong sau {0:N0}s (exit {1}), output: {2}" -f $sw.Elapsed.TotalSeconds, $p.ExitCode, $Out)
    # Dòng RESULT (aob.py, ...) in lại để so sánh nhanh giữa các version.
    Get-ChildItem $Out -Filter "*.txt" |
        Where-Object { $_.LastWriteTime -gt (Get-Date).AddSeconds(-($sw.Elapsed.TotalSeconds + 5)) } |
        ForEach-Object { Select-String -Path $_.FullName -Pattern '^RESULT ' | ForEach-Object { "    [$($T.Name)] $($_.Line)" } }
}
