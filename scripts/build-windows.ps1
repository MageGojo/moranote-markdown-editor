<#
.SYNOPSIS
    Build a portable Windows package (and optional installer) for MoraNote.

.DESCRIPTION
    Mirrors scripts/build-dmg.sh for Windows. Steps:
      1. Build the release binary (cargo build --release).
      2. Assemble a portable folder: MoraNote.exe + bundled theme/font assets.
      3. Produce a versioned .zip in dist/.
      4. If Inno Setup (iscc.exe) is available, also produce a Setup .exe installer.

.NOTES
    Run from PowerShell on Windows:
        powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1

    Requirements:
      - Rust toolchain (https://rustup.rs) with the MSVC target.
      - (Optional) Inno Setup 6 for the installer: https://jrsoftware.org/isinfo.php
#>

$ErrorActionPreference = "Stop"

# --- Paths & metadata ---------------------------------------------------------
$RepoDir   = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$AppName   = "MoraNote"
$BinaryName = "moranote"
$ExeName   = "MoraNote.exe"

$CargoToml = Join-Path $RepoDir "Cargo.toml"
$Version   = (Select-String -Path $CargoToml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches.Groups[1].Value
if ([string]::IsNullOrWhiteSpace($Version)) { $Version = "0.0.0" }

$Arch      = $env:PROCESSOR_ARCHITECTURE.ToLower()  # e.g. amd64 / arm64
$DistDir   = Join-Path $RepoDir "dist"
$StageDir  = Join-Path $RepoDir "target\windows\$AppName"
$ZipPath   = Join-Path $DistDir "$AppName-$Version-windows-$Arch.zip"

Set-Location $RepoDir

Write-Host "==> Building $AppName $Version ($Arch) ..." -ForegroundColor Cyan

# --- 1. Build release ---------------------------------------------------------
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$ReleaseExe = Join-Path $RepoDir "target\release\$BinaryName.exe"
if (-not (Test-Path $ReleaseExe)) { throw "Release binary not found: $ReleaseExe" }

# --- 2. Assemble portable folder ---------------------------------------------
if (Test-Path $StageDir) { Remove-Item $StageDir -Recurse -Force }
New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
New-Item -ItemType Directory -Path $DistDir  -Force | Out-Null

Copy-Item $ReleaseExe (Join-Path $StageDir $ExeName) -Force

# Bundle theme + fonts next to the exe (portable layout resolved at runtime).
$ThemeSrc = Join-Path $RepoDir "assets\themes\morandigarden"
$ThemeDst = Join-Path $StageDir "assets\themes\morandigarden"
New-Item -ItemType Directory -Path $ThemeDst -Force | Out-Null
Copy-Item (Join-Path $ThemeSrc "*") $ThemeDst -Recurse -Force

# Optional: include a license / readme if present.
foreach ($doc in @("README.md", "LICENSE")) {
    $docPath = Join-Path $RepoDir $doc
    if (Test-Path $docPath) { Copy-Item $docPath (Join-Path $StageDir $doc) -Force }
}

# --- 3. Zip the portable package ---------------------------------------------
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Compress-Archive -Path (Join-Path $StageDir "*") -DestinationPath $ZipPath -CompressionLevel Optimal
Write-Host "==> Portable zip: $ZipPath" -ForegroundColor Green

# --- 4. Optional Inno Setup installer ----------------------------------------
$Iscc = Get-Command "iscc.exe" -ErrorAction SilentlyContinue
if ($Iscc) {
    Write-Host "==> Inno Setup found, building installer ..." -ForegroundColor Cyan
    $IssPath = Join-Path $env:TEMP "moranote-$Version.iss"
    $InstallerOut = $DistDir
    $InstallerBase = "$AppName-$Version-windows-$Arch-setup"

    $iss = @"
[Setup]
AppId={{8E9F3C2A-7B5D-4E1A-9C6F-MORANOTE0001}
AppName=$AppName
AppVersion=$Version
AppPublisher=ApiZero (apizero.cn)
AppPublisherURL=https://apizero.cn/
DefaultDirName={autopf}\$AppName
DefaultGroupName=$AppName
OutputDir=$InstallerOut
OutputBaseFilename=$InstallerBase
Compression=lzma2/max
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
DisableProgramGroupPage=yes
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Files]
Source: "$StageDir\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs

[Icons]
Name: "{group}\$AppName"; Filename: "{app}\$ExeName"
Name: "{autodesktop}\$AppName"; Filename: "{app}\$ExeName"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Run]
Filename: "{app}\$ExeName"; Description: "{cm:LaunchProgram,$AppName}"; Flags: nowait postinstall skipifsilent
"@

    Set-Content -Path $IssPath -Value $iss -Encoding UTF8
    & $Iscc.Source $IssPath
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup (iscc) failed" }
    Write-Host "==> Installer: $InstallerOut\$InstallerBase.exe" -ForegroundColor Green
} else {
    Write-Host "==> Inno Setup (iscc.exe) not found; skipped installer. Portable zip is ready." -ForegroundColor Yellow
}

Write-Host "==> Done." -ForegroundColor Cyan
