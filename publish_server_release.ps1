# LiveDownloader - Fast Release Upload Script (Windows PowerShell)

Param (
    [string]$Tag = ""
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Host "[ERROR] GitHub CLI (gh) is not installed!" -ForegroundColor Red
    Write-Host "Please install gh (winget install GitHub.cli) and run 'gh auth login'." -ForegroundColor Yellow
    exit 1
}

if ([string]::IsNullOrWhiteSpace($Tag)) {
    if (Test-Path "src-tauri/tauri.conf.json") {
        $json = Get-Content "src-tauri/tauri.conf.json" | ConvertFrom-Json
        $Tag = "v" + $json.version
    } else {
        $Tag = "v0.1.1"
    }
}

$TargetFile = "livedownloader-server-linux-amd64"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "  LiveDownloader Linux Server Release Publisher" -ForegroundColor Green
Write-Host "  Target Release Tag: [ $Tag ]" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "Select compilation/build mode:"
Write-Host "  1) [Default] Use existing local binary directly (No re-compile)"
Write-Host "  2) Recompile via 'cargo zigbuild' for x86_64-unknown-linux-musl and upload"
Write-Host "  3) Recompile via 'cargo build' and upload"

$choice = Read-Host "Enter option [1-3] (Default 1)"
if ([string]::IsNullOrWhiteSpace($choice)) { $choice = "1" }

if ($choice -eq "2") {
    Write-Host "[INFO] Recompiling via cargo zigbuild..." -ForegroundColor Cyan
    Push-Location src-tauri
    cargo zigbuild --target x86_64-unknown-linux-musl --release --no-default-features --features server
    Pop-Location
} elseif ($choice -eq "3") {
    Write-Host "[INFO] Recompiling via cargo build..." -ForegroundColor Cyan
    Push-Location src-tauri
    cargo build --release --no-default-features --features server
    Pop-Location
}

$BinSrc = ""
if (Test-Path "src-tauri/target/x86_64-unknown-linux-musl/release/LiveDownloader") {
    $BinSrc = "src-tauri/target/x86_64-unknown-linux-musl/release/LiveDownloader"
} elseif (Test-Path "src-tauri/target/release/LiveDownloader") {
    $BinSrc = "src-tauri/target/release/LiveDownloader"
} elseif (Test-Path "src-tauri/target/release/LiveDownloader.exe") {
    $BinSrc = "src-tauri/target/release/LiveDownloader.exe"
} elseif (Test-Path "./LiveDownloader") {
    $BinSrc = "./LiveDownloader"
} elseif (Test-Path "./livedownloader-server-linux-amd64") {
    $BinSrc = "./livedownloader-server-linux-amd64"
}

if (-not $BinSrc -or -not (Test-Path $BinSrc)) {
    Write-Host "[ERROR] Could not find compiled binary file!" -ForegroundColor Red
    exit 1
}

Copy-Item $BinSrc -Destination $TargetFile -Force
Write-Host "[SUCCESS] Target file ready: $TargetFile" -ForegroundColor Green

Write-Host "[INFO] Uploading to GitHub Release ($Tag)..." -ForegroundColor Cyan
$releaseExists = $false
try {
    gh release view $Tag 2>&1 | Out-Null
    $releaseExists = $true
} catch {
    $releaseExists = $false
}

if ($releaseExists) {
    gh release upload $Tag $TargetFile --clobber
    Write-Host "[SUCCESS] Uploaded $TargetFile to existing Release: $Tag" -ForegroundColor Green
} else {
    gh release create $Tag $TargetFile --title "LiveDownloader $Tag" --notes "LiveDownloader Linux Server Binary ($Tag)"
    Write-Host "[SUCCESS] Created new Release $Tag and uploaded $TargetFile!" -ForegroundColor Green
}

Remove-Item $TargetFile -Force -ErrorAction SilentlyContinue
