# Build an unsigned .app layout for ACowork (Windows x86_64).
# Requires: rustc/cargo for figcraft-acowork; optional trunk for real UI.
# Packing into a signed .app ZIP uses ACowork's `acowork-pack-app` separately.
param(
    [string]$OutDir = "",
    [switch]$WithWebUi,
    # Prebuilt figcraft-web zip (upstream `figcraft-web-<version>.zip`).
    # Install never compiles this; the archive is copied into ui/ as-is.
    [string]$WebArchive = "",
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
    $targetDir = (cargo metadata --format-version 1 --no-deps | ConvertFrom-Json).target_directory
} finally {
    Pop-Location
}

$binSrc = Join-Path $targetDir "release\figcraft-acowork.exe"
if (-not (Test-Path $binSrc)) {
    throw "missing $binSrc (target_directory=$targetDir)"
}

$stage = $OutDir
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force -Path (Join-Path $stage "bin") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage "ui") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $stage "assets") | Out-Null

Copy-Item $binSrc (Join-Path $stage "bin\figcraft-acowork.exe")

$entry = "bin/figcraft-acowork.exe"
$utf8 = New-Object System.Text.UTF8Encoding $false
$tomlPath = Join-Path $PSScriptRoot "app.toml"
$toml = [System.IO.File]::ReadAllText($tomlPath, $utf8)
$toml = $toml -replace 'platforms = \[[^\]]*\]', "platforms = [`"$Platform`"]"
$toml = $toml -replace 'entry = "[^"]*"', "entry = `"$entry`""
[System.IO.File]::WriteAllText((Join-Path $stage "app.toml"), $toml, $utf8)

if ($WebArchive -and $WithWebUi) {
    throw "pass either -WithWebUi or -WebArchive, not both"
}
if ($WebArchive) {
    if (-not (Test-Path $WebArchive)) { throw "missing web archive: $WebArchive" }
    Write-Host "Unpacking prebuilt web UI from $WebArchive"
    $uiDir = Join-Path $stage "ui"
    Expand-Archive -Path $WebArchive -DestinationPath $uiDir -Force
    $index = Get-ChildItem -Path $uiDir -Filter index.html -Recurse -File | Select-Object -First 1
    if (-not $index) { throw "web archive has no index.html" }
    $indexDir = $index.Directory.FullName
    if ($indexDir -ne (Resolve-Path $uiDir).Path) {
        Get-ChildItem -Force $indexDir | Move-Item -Destination $uiDir
        $nested = $index.Directory
        if ($nested.FullName.StartsWith((Resolve-Path $uiDir).Path) -and $nested.FullName -ne (Resolve-Path $uiDir).Path) {
            Remove-Item -Recurse -Force $nested.FullName
        }
    }
} elseif ($WithWebUi) {
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

$iconCandidates = @(
    (Join-Path $Root "assets\icon\icon_512.png"),
    (Join-Path $Root "assets\app-icon\figcraft-512.png")
)
foreach ($icon in $iconCandidates) {
    if (Test-Path $icon) {
        Copy-Item $icon (Join-Path $stage "assets\icon.png")
        break
    }
}

# Licenses and NOTICE belong in the package. Never copy docs/brand —
# ArtCraft marks are not open source and must not identify this adaptation.
Copy-Item (Join-Path $Root "LICENSE-MIT") (Join-Path $stage "LICENSE-MIT")
Copy-Item (Join-Path $Root "LICENSE-APACHE") (Join-Path $stage "LICENSE-APACHE")
Copy-Item (Join-Path $PSScriptRoot "NOTICE") (Join-Path $stage "NOTICE")
$iconLicense = Join-Path $Root "assets\app-icon\LICENSE.txt"
if (Test-Path $iconLicense) {
    Copy-Item $iconLicense (Join-Path $stage "assets\ICON-LICENSE.txt")
}

Write-Host "Staged unsigned app tree at $stage"
Write-Host "Next: acowork-pack-app --app-dir `"$stage`" --output `"$stage.app`""
