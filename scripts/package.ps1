# 一键出两个发布物：
#   dist\OfficeDesk-<ver>-setup.exe      安装版（Inno Setup）
#   dist\OfficeDesk-<ver>-portable.zip   绿色版（解压即用，数据存 exe 旁的 data\）
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$match = Select-String -Path "crates\app\Cargo.toml" -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1
if (-not $match) { throw "读不到 crates\app\Cargo.toml 里的版本号" }
$version = $match.Matches[0].Groups[1].Value

$dist = Join-Path $root "dist"
$portableDir = Join-Path $dist "OfficeDesk-$version-portable"
$portableZip = Join-Path $dist "OfficeDesk-$version-portable.zip"
$setupExe = Join-Path $dist "OfficeDesk-$version-setup.exe"

if (-not $SkipBuild) {
    # 静态链接 CRT：发布物不依赖 VC++ 运行库，干净机器可直接运行
    $env:RUSTFLAGS = "-C target-feature=+crt-static"
    cargo build -p office_desk --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release 失败" }
}

$exe = "target\release\office_desk.exe"
if (-not (Test-Path $exe)) { throw "缺少 $exe，请先去掉 -SkipBuild" }

New-Item -ItemType Directory -Path $dist -Force | Out-Null

# 绿色版：exe + portable.flag（标记存在时数据写到 exe 旁的 data\）
if (Test-Path $portableDir) { Remove-Item $portableDir -Recurse -Force }
New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
Copy-Item $exe $portableDir
New-Item -ItemType File -Path (Join-Path $portableDir "portable.flag") -Force | Out-Null
if (Test-Path $portableZip) { Remove-Item $portableZip -Force }
Compress-Archive -Path (Join-Path $portableDir "*") -DestinationPath $portableZip

# 安装版：优先找用户目录下（winget 默认）的 Inno Setup
$iscc = @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1

if ($iscc) {
    & $iscc "/DAppVersion=$version" "packaging\office_desk.iss"
    if ($LASTEXITCODE -ne 0) { throw "ISCC 打包安装版失败" }
} else {
    Write-Warning "未找到 Inno Setup（ISCC.exe），已跳过安装版，仅生成绿色版"
}

Get-ChildItem $dist -Filter "OfficeDesk-$version-*" |
    Select-Object Name, @{ n = "MB"; e = { [math]::Round($_.Length / 1MB, 1) } }
