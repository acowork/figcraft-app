# Build an unsigned .app layout for ACowork (Windows x86_64).
# Requires: rustc/cargo for figcraft-acowork; optional trunk for real UI.
# Packing into a signed .app ZIP uses ACowork's `acowork-pack-app` separately.
param(
    [string]$OutDir = "",
    [switch]$WithWebUi,
    [string]$Platform = "windows-x86_64"
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
if (-not $OutDir) {
    $OutDir = Join-Path $Root "dist\acowork\figcraft-$Platform"
}

Write-Host "Building figcraft-acowork (release)..."
Push-Location $Root
try {
    cargo build -p figcraft-acowork --release
} finally {
    Pop-Location
}

$binSrc = Join-Path $Root "target\release\figcraft-acowork.exe"
if (-not (Test-Path $binSrc)) {
    throw "missing $binSrc"
}

$stage = $OutDir
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path (Join-Path $stage "bin") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage "ui") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage "assets") | Out-Null

Copy-Item $binSrc (Join-Path $stage "bin\figcraft-acowork.exe")

$entry = "bin/figcraft-acowork.exe"
$toml = Get-Content (Join-Path $PSScriptRoot "app.toml") -Raw
$toml = $toml -replace 'platforms = \[[^\]]*\]', "platforms = [`"$Platform`"]"
$toml = $toml -replace 'entry = "[^"]*"', "entry = `"$entry`""
Set-Content -Path (Join-Path $stage "app.toml") -Value $toml -NoNewline

if ($WithWebUi) {
    Write-Host "Building figcraft-web via trunk..."
    Push-Location (Join-Path $Root "apps\figcraft-web")
    try {
        trunk build --release
    } finally {
        Pop-Location
    }
    $webDist = Join-Path $Root "dist\web"
    if (-not (Test-Path $webDist)) { throw "trunk dist missing: $webDist" }
    Copy-Item -Recurse -Force (Join-Path $webDist "*") (Join-Path $stage "ui")
} else {
    Copy-Item -Recurse -Force (Join-Path $PSScriptRoot "ui\*") (Join-Path $stage "ui")
}

$icon = Join-Path $Root "assets\icon\icon_512.png"
if (Test-Path $icon) {
    Copy-Item $icon (Join-Path $stage "assets\icon.png")
}

Write-Host "Staged unsigned app tree at $stage"
Write-Host "Next: acowork-pack-app --app-dir `"$stage`" --output `"$stage.app`""
