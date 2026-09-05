# iLauncher GPUI 版打包脚本
# 用法：powershell -File scripts/pack-gpui.ps1 [-Version 1.2.3] [-Features "ilauncher clipboard"] [-SkipBuild]
#
# 流程：cargo build --release → makensis 打安装包 → （可选）minisign 签名
# 产物：target/release/bundle/nsis/iLauncher_<ver>_x64-setup.exe
#       同名 .sig = base64(minisign 签名文件)，latest.json 的 signature 字段取这个内容
#       （签名/验签协议见 crates/ilauncher-gpui/src/updater.rs verify_signature）
# 注意：目录整改为 workspace 后 cargo 统一输出到根 target/，不再是 crate 内 target/
#
# 签名（-Sign）：需要 minisign 在 PATH，且私钥在 $env:MINISIGN_SECRET_KEY_FILE

param(
    [string]$Version = "",
    [string]$Features = "ilauncher clipboard",
    [switch]$Sign,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$gpuiDir = Join-Path $root "crates/ilauncher-gpui"
$installerDir = Join-Path $gpuiDir "installer"
# workspace 统一输出到根 target/
$bundleDir = Join-Path $root "target\release\bundle\nsis"
$exeSrc = Join-Path $root "target\release\ilauncher-gpui.exe"

if (-not $Version) {
    $toml = Get-Content (Join-Path $gpuiDir "Cargo.toml") -Raw
    $Version = [regex]::Match($toml, '(?m)^version = "([^"]+)"').Groups[1].Value
    if (-not $Version) { throw "无法从 crates/ilauncher-gpui/Cargo.toml 解析版本号，请用 -Version 指定" }
}
Write-Host "==> iLauncher GPUI 打包 v$Version (features: $Features)" -ForegroundColor Cyan

if (-not $SkipBuild) {
    Write-Host "==> cargo build --release" -ForegroundColor Cyan
    & cargo build --release --manifest-path (Join-Path $gpuiDir "Cargo.toml") --features $Features
    if ($LASTEXITCODE -ne 0) { throw "cargo build 失败" }
}
if (-not (Test-Path $exeSrc)) { throw "未找到 $exeSrc，请先构建" }

$makensis = Get-Command makensis -ErrorAction SilentlyContinue
if (-not $makensis) {
    $chocoDir = "C:\Program Files (x86)\NSIS\makensis.exe"
    if (Test-Path $chocoDir) { $makensis = Get-Command $chocoDir }
}
if (-not $makensis) { throw "未找到 makensis，请先安装 NSIS（choco install nsis）" }

New-Item -ItemType Directory -Force $bundleDir | Out-Null
Write-Host "==> makensis" -ForegroundColor Cyan
& $makensis.Source /DVERSION=$Version /DEXE=$exeSrc /DOUTDIR=$bundleDir (Join-Path $installerDir "iLauncher.nsi")
if ($LASTEXITCODE -ne 0) { throw "makensis 失败" }

$setup = Join-Path $bundleDir "iLauncher_${Version}_x64-setup.exe"
if (-not (Test-Path $setup)) { throw "未找到产物 $setup" }
Write-Host "==> 产物：$setup ($([math]::Round((Get-Item $setup).Length/1MB, 1)) MB)" -ForegroundColor Green

if ($Sign) {
    if (-not $env:MINISIGN_SECRET_KEY_FILE) { throw "-Sign 需要环境变量 MINISIGN_SECRET_KEY_FILE 指向私钥文件" }
    Write-Host "==> minisign 签名" -ForegroundColor Cyan
    & minisign -Sm $setup -s $env:MINISIGN_SECRET_KEY_FILE -t "iLauncher v$Version"
    if ($LASTEXITCODE -ne 0) { throw "minisign 签名失败" }
    # updater 协议：signature = base64(整个 .minisig 文件内容)
    $sig = [Convert]::ToBase64String([IO.File]::ReadAllBytes("$setup.minisig"))
    [IO.File]::WriteAllText("$setup.sig", $sig)
    Write-Host "==> 签名：$setup.sig" -ForegroundColor Green
}

Write-Host ""
Write-Host "下一步：node scripts/generate-updater-json.js $Version v$Version" -ForegroundColor Yellow
