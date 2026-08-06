# LiveDownloader - 本地编译与 Fast Release 二进制一键上传发布脚本 (Windows PowerShell)

Param (
    [string]$Tag = ""
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    Write-Host "[ERROR] 未检测到 GitHub CLI (gh)！" -ForegroundColor Red
    Write-Host "请先安装 gh (如: winget install GitHub.cli) 并运行 'gh auth login' 授权登录。" -ForegroundColor Yellow
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
Write-Host "[INFO] 准备发布 Linux amd64 二进制到 GitHub Release Tag: [ $Tag ]" -ForegroundColor Cyan

$BinSrc = ""
if (Test-Path "src-tauri/target/x86_64-unknown-linux-musl/release/LiveDownloader") {
    $BinSrc = "src-tauri/target/x86_64-unknown-linux-musl/release/LiveDownloader"
} elseif (Test-Path "src-tauri/target/release/LiveDownloader") {
    $BinSrc = "src-tauri/target/release/LiveDownloader"
} elseif (Test-Path "src-tauri/target/release/LiveDownloader.exe") {
    $BinSrc = "src-tauri/target/release/LiveDownloader.exe"
} elseif (Test-Path "./LiveDownloader") {
    $BinSrc = "./LiveDownloader"
} elseif (Test-Path "./$TargetFile") {
    $BinSrc = "./$TargetFile"
}

if (-not $BinSrc -or -not (Test-Path $BinSrc)) {
    Write-Host "[INFO] 未找到已编译的二进制，开始在本地调用 cargo 编译..." -ForegroundColor Cyan
    Push-Location src-tauri
    cargo build --release --no-default-features --features server
    Pop-Location
    $BinSrc = "src-tauri/target/release/LiveDownloader"
}

Copy-Item $BinSrc "./$TargetFile" -Force
Write-Host "[SUCCESS] 准备就绪: ./$TargetFile" -ForegroundColor Green

Write-Host "[INFO] 正在直接上传至 GitHub Release ($Tag)..." -ForegroundColor Cyan
$releaseExists = $false
try {
    gh release view $Tag | Out-Null
    $releaseExists = $true
} catch {
    $releaseExists = $false
}

if ($releaseExists) {
    gh release upload $Tag "./$TargetFile" --clobber
    Write-Host "[SUCCESS] 成功！二进制 ./$TargetFile 已覆盖更新至已有 Release: $Tag" -ForegroundColor Green
} else {
    gh release create $Tag "./$TargetFile" --title "LiveDownloader $Tag" --notes "LiveDownloader Linux Server Binary ($Tag)"
    Write-Host "[SUCCESS] 成功！已新建 Release $Tag 并上传二进制文件！" -ForegroundColor Green
}

Remove-Item "./$TargetFile" -Force -ErrorAction SilentlyContinue
