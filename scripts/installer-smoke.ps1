param([Parameter(Mandatory=$true)][string]$CompilerPath)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json).version
$testId = [guid]::NewGuid().ToString()
$registrationName = 'RoveSmoke' + $testId.Replace('-', '')
$testRoot = Join-Path $projectRoot ".local\installer-smoke-$testId"
$appDir = Join-Path $testRoot '中文 安装目录'
$profile = Join-Path $testRoot 'profile'
$payload = Join-Path $projectRoot "release\Rove-$version-windows-x64"
$oldPayload = Join-Path $projectRoot 'release\Rove-0.1.0-beta5-windows-x64'
$targetRoot = Join-Path $projectRoot 'native\target\x86_64-pc-windows-msvc\release'
# Cargo 1.100+ nests build-script output as build\<package>\<hash>\out;
# older releases use build\<package>-<hash>\out, so search both layouts.
$icon = Get-ChildItem -Path (Join-Path $targetRoot 'build') -Recurse -Filter 'icon.ico' -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
$bootstrapper = Join-Path $projectRoot 'build\installer\dependencies\MicrosoftEdgeWebview2Setup.exe'
$checks = [ordered]@{}
New-Item -ItemType Directory -Force -Path $testRoot,$profile | Out-Null
function Check([string]$Name, [bool]$Result) { $checks[$Name] = $Result; if (-not $Result) { throw "Failed: $Name" } }
function UserChoice {
    $result = [ordered]@{}
    foreach ($protocol in @('http','https')) {
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Software\Microsoft\Windows\Shell\Associations\UrlAssociations\$protocol\UserChoice")
        if ($key) { try { $result[$protocol] = @($key.GetValue('ProgId'),$key.GetValue('Hash')) } finally { $key.Dispose() } }
    }
    ConvertTo-Json -InputObject $result -Compress
}
$before = UserChoice
function Compile([string]$Script, [string]$AppVersion, [string]$PayloadDir, [string]$OutputDir) {
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
    & $CompilerPath /Q "/DAppVersion=$AppVersion" '/DFileVersion=0.1.0.0' "/DPayloadDir=$PayloadDir" "/DOutputDir=$OutputDir" "/DAppIcon=$($icon.FullName)" "/DBootstrapperPath=$bootstrapper" "/DSetupAppId={{$testId}" "/DAppRegistrationName=$registrationName" "/DAppTitle=$registrationName" $Script
    if ($LASTEXITCODE -ne 0) { throw 'Smoke installer compilation failed' }
    Join-Path $OutputDir "Rove-$AppVersion-windows-x64-setup.exe"
}
function Install([string]$Setup, [string]$Log, [bool]$First) {
    $arguments = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-',"/LOG=`"$Log`"")
    if ($First) { $arguments += "/DIR=`"$appDir`"" }
    $process = Start-Process -FilePath $Setup -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    Check "Install-$([IO.Path]::GetFileName($Setup))" ($process.ExitCode -eq 0)
}
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{$testId}_is1"
$clientKey = "HKCU:\Software\Clients\StartMenuInternet\$registrationName"
$progKey = "HKCU:\Software\Classes\${registrationName}URL"
$installedExe = Join-Path $appDir 'Rove.exe'
try {
    # Compile beta5 with an isolated AppId/name, then upgrade using the same ID.
    # The production payload bytes are unchanged; real Rove registration is untouched.
    Push-Location $projectRoot
    try {
        $oldScript = (& git show v0.1.0-beta5:installer/Rove.iss) -join "`n"
        if ($LASTEXITCODE -ne 0) { throw 'beta5 tag is required for upgrade validation' }
        $oldScript = $oldScript.Replace('AppId={{76F5D0C8-2824-42C5-A964-CAD38178E0BA}', "AppId={{$testId}").Replace('AppName=Rove',"AppName=$registrationName").Replace('AppVerName=Rove ',"AppVerName=$registrationName ").Replace('DefaultGroupName=Rove',"DefaultGroupName=$registrationName")
        $oldScriptPath = Join-Path $testRoot 'beta5.iss'
        Set-Content -LiteralPath $oldScriptPath -Value $oldScript -Encoding utf8
        Copy-Item -LiteralPath (Join-Path $projectRoot 'installer\ChineseSimplified.isl') -Destination $testRoot
        $oldSetup = Compile $oldScriptPath '0.1.0-beta5' $oldPayload (Join-Path $testRoot 'old')
        $newSetup = Compile (Join-Path $projectRoot 'installer\Rove.iss') $version $payload (Join-Path $testRoot 'new')
        Install $oldSetup (Join-Path $testRoot 'beta5-install.log') $true
        Check 'Beta5Version' ((Get-ItemProperty -LiteralPath $uninstallKey).DisplayVersion -eq '0.1.0-beta5')
        & node (Join-Path $PSScriptRoot 'native-profile-fixture.mjs') $profile
        if ($LASTEXITCODE -ne 0) { throw 'Profile fixture creation failed' }
        Set-Content -LiteralPath (Join-Path $profile 'keep.txt') -Value 'user data marker'
        Install $newSetup (Join-Path $testRoot 'beta6-upgrade.log') $false
    } finally { Pop-Location }
    Check 'Beta6Version' ((Get-ItemProperty -LiteralPath $uninstallKey).DisplayVersion -eq $version)
    Check 'BinaryMatchesRelease' ((Get-FileHash -LiteralPath $installedExe).Hash -eq (Get-FileHash -LiteralPath (Join-Path $payload 'Rove.exe')).Hash)
    $command = (Get-Item -LiteralPath "$progKey\shell\open\command").GetValue('')
    Check 'QuotedUnicodeCommand' ($command -eq "`"$installedExe`" -- `"%1`"")
    foreach ($protocol in @('http','https')) { Check "$protocol-Capability" ((Get-ItemPropertyValue -LiteralPath "$clientKey\Capabilities\URLAssociations" -Name $protocol) -eq "${registrationName}URL") }
    Check 'RegisteredApplication' ((Get-ItemPropertyValue -LiteralPath 'HKCU:\Software\RegisteredApplications' -Name $registrationName) -eq "Software\Clients\StartMenuInternet\$registrationName\Capabilities")
    Check 'Shortcut' (Test-Path -LiteralPath (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$registrationName\Rove.lnk"))
    $previousDataDir = $env:ERA_DATA_DIR
    try {
        $env:ERA_DATA_DIR = $profile
        $process = Start-Process -FilePath $installedExe -ArgumentList @('--','"http://127.0.0.1:9/beta6?source=installer&b=2"') -WindowStyle Hidden -PassThru
        Start-Sleep -Seconds 5
        Check 'InstalledAppRuns' (-not $process.HasExited)
        & node (Join-Path $PSScriptRoot 'native-profile-fixture.mjs') $profile 'http://127.0.0.1:9/beta6?source=installer&b=2'
        Check 'InstalledColdLinkLaunch' ($LASTEXITCODE -eq 0)
    } finally {
        if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force; $process.WaitForExit(5000) | Out-Null }
        $env:ERA_DATA_DIR = $previousDataDir
    }
    $uninstall = Start-Process -FilePath (Join-Path $appDir 'unins000.exe') -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -WindowStyle Hidden -Wait -PassThru
    Start-Sleep -Seconds 2
    Check 'UninstallSucceeded' ($uninstall.ExitCode -eq 0)
    Check 'RegistrationsRemoved' ((-not (Test-Path -LiteralPath $clientKey)) -and (-not (Test-Path -LiteralPath $progKey)) -and (-not (Test-Path -LiteralPath $uninstallKey)))
    Check 'RegisteredApplicationRemoved' (-not (Get-ItemProperty -LiteralPath 'HKCU:\Software\RegisteredApplications').PSObject.Properties[$registrationName])
    Check 'ShortcutRemoved' (-not (Test-Path -LiteralPath (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$registrationName")))
    Check 'AppRemoved' (-not (Test-Path -LiteralPath $installedExe))
    Check 'ProfilePreserved' ((Test-Path -LiteralPath (Join-Path $profile 'era.sqlite')) -and (Test-Path -LiteralPath (Join-Path $profile 'keep.txt')))
    Check 'DefaultChoicesUnchanged' ((UserChoice) -eq $before)
    $checks | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'checks.json') -Encoding utf8
    $checks
    "Results: $testRoot"
} finally {
    if (Test-Path -LiteralPath (Join-Path $appDir 'unins000.exe')) { Start-Process -FilePath (Join-Path $appDir 'unins000.exe') -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART') -WindowStyle Hidden -Wait | Out-Null }
}
