param(
    [switch]$SkipBuild,
    [string]$CompilerPath = $env:INNO_SETUP_COMPILER
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
if ($version -notmatch '^(\d+)\.(\d+)\.(\d+)(?:-[0-9A-Za-z.-]+)?$') {
    throw "Unsupported release version: $version"
}
$fileVersion = "$($Matches[1]).$($Matches[2]).$($Matches[3]).0"

if (-not $CompilerPath) {
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe')
    )
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($command) { $candidates += $command.Source }
    $CompilerPath = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $CompilerPath -or -not (Test-Path -LiteralPath $CompilerPath -PathType Leaf)) {
    throw 'Install Inno Setup 6 (https://jrsoftware.org/isinfo.php), or set INNO_SETUP_COMPILER to ISCC.exe.'
}
$CompilerPath = (Resolve-Path -LiteralPath $CompilerPath).Path

$target = 'x86_64-pc-windows-msvc'
$targetRoot = Join-Path $projectRoot "native\target\$target\release"
$binary = Join-Path $targetRoot 'rove.exe'
if (-not $SkipBuild) {
    Push-Location $projectRoot
    $previousRustFlags = $env:RUSTFLAGS
    $previousEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
    try {
        # An explicit target keeps host build scripts/proc macros on their normal CRT.
        # The distributed application needs no separately installed VC++ runtime.
        $env:RUSTFLAGS = "$previousRustFlags -C target-feature=+crt-static".Trim()
        if ($previousEncodedFlags) {
            $separator = [char]31
            $env:CARGO_ENCODED_RUSTFLAGS = "$previousEncodedFlags${separator}-C${separator}target-feature=+crt-static"
        }
        & npm.cmd run build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }
        $cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
        $cargoPath = if ($cargo) { $cargo.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
        & $cargoPath build --release --locked --manifest-path native/Cargo.toml --target $target
        if ($LASTEXITCODE -ne 0) { throw 'Native release build failed.' }
    } finally {
        $env:RUSTFLAGS = $previousRustFlags
        $env:CARGO_ENCODED_RUSTFLAGS = $previousEncodedFlags
        Pop-Location
    }
}
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw 'Release executable not found. Run npm run installer without -SkipBuild first.'
}
# Cargo 1.100+ nests build-script output as build\<package>\<hash>\out;
# older releases use build\<package>-<hash>\out, so search both layouts.
$icon = Get-ChildItem -Path (Join-Path $targetRoot 'build') -Recurse -Filter 'icon.ico' -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
if (-not $icon) { throw 'Build-generated application icon not found.' }

$dependencyRoot = Join-Path $projectRoot 'build\installer\dependencies'
New-Item -ItemType Directory -Force -Path $dependencyRoot | Out-Null
$bootstrapper = Join-Path $dependencyRoot 'MicrosoftEdgeWebview2Setup.exe'
if (-not (Test-Path -LiteralPath $bootstrapper)) {
    # Microsoft's Evergreen bootstrapper chooses the appropriate runtime and installs per user.
    Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $bootstrapper
}
# Use this shell's built-in module even when launched from another PowerShell version.
Import-Module (Join-Path $PSHOME 'Modules\Microsoft.PowerShell.Security\Microsoft.PowerShell.Security.psd1') -ErrorAction Stop
$signature = Get-AuthenticodeSignature -LiteralPath $bootstrapper
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') {
    throw "Microsoft WebView2 bootstrapper signature is invalid: $bootstrapper"
}

& (Join-Path $PSScriptRoot 'package.ps1') -ExecutablePath $binary -SkipArchive | Out-Host
$payload = Join-Path $projectRoot "release\Rove-$version-windows-x64"
$outputRoot = Join-Path $projectRoot 'release'
& $CompilerPath /Q "/DAppVersion=$version" "/DFileVersion=$fileVersion" "/DPayloadDir=$payload" "/DOutputDir=$outputRoot" "/DAppIcon=$($icon.FullName)" "/DBootstrapperPath=$bootstrapper" (Join-Path $projectRoot 'installer\Rove.iss')
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
$installer = Join-Path $outputRoot "Rove-$version-windows-x64-setup.exe"
Get-Item -LiteralPath $installer | Select-Object FullName, Length
