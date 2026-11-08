; Compile through scripts/installer.ps1 so versions, payload and dependencies agree.
#ifndef AppVersion
  #error Run scripts/installer.ps1 to build the installer.
#endif
#if VER < EncodeVer(6, 7, 3) || VER >= EncodeVer(7, 0, 0)
  #error Use Inno Setup 6.7.3 or a newer 6.x compiler for the native theme.
#endif

[Setup]
AppId={{76F5D0C8-2824-42C5-A964-CAD38178E0BA}
AppName=Rove
AppVersion={#AppVersion}
AppVerName=Rove {#AppVersion}
VersionInfoVersion={#FileVersion}
VersionInfoProductVersion={#FileVersion}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoTextVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Rove
DefaultGroupName=Rove
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\Rove.exe
SetupIconFile={#AppIcon}
OutputDir={#OutputDir}
OutputBaseFilename=Rove-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "ChineseSimplified.isl"

[CustomMessages]
english.DesktopShortcut=Create a &desktop shortcut
english.LaunchRove=Launch Rove
english.InstallWebView2=Installing Microsoft Edge WebView2 Runtime...
english.WebView2Failed=WebView2 Runtime could not be installed (exit code %1). Connect to the internet and retry, or install the runtime from https://developer.microsoft.com/microsoft-edge/webview2/ and run Setup again.
chinesesimplified.DesktopShortcut=创建桌面快捷方式(&D)
chinesesimplified.LaunchRove=启动 Rove
chinesesimplified.InstallWebView2=正在安装 Microsoft Edge WebView2 运行时……
chinesesimplified.WebView2Failed=无法安装 WebView2 运行时（退出代码 %1）。请连接网络后重试，或从 https://developer.microsoft.com/microsoft-edge/webview2/ 安装运行时后重新运行安装程序。

[Tasks]
Name: "desktopicon"; Description: "{cm:DesktopShortcut}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PayloadDir}\Rove.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#PayloadDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#BootstrapperPath}"; Flags: dontcopy
Source: "..\native\assets\icon.png"; Flags: dontcopy
Source: "assets\Outfit.ttf"; Flags: dontcopy
Source: "assets\Outfit-LICENSE.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\Rove"; Filename: "{app}\Rove.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Rove"; Filename: "{app}\Rove.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\Rove.exe"; Description: "{cm:LaunchRove}"; Flags: nowait postinstall skipifsilent

; Intentionally no UninstallDelete: the existing EraBrowser profile belongs to the user.
[Code]
const
  WebView2Key = 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

function RuntimeInRegistry(RootKey: Integer): Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(RootKey, WebView2Key, 'pv', Version) and
    (Version <> '') and (Version <> '0.0.0.0');
end;

#include "Design.iss"

function HasWebView2: Boolean;
begin
  Result := RuntimeInRegistry(HKLM32) or RuntimeInRegistry(HKLM64) or
    RuntimeInRegistry(HKCU32) or RuntimeInRegistry(HKCU64);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ExitCode: Integer;
begin
  Result := '';
  if HasWebView2 then
    Exit;

  WizardForm.StatusLabel.Caption := CustomMessage('InstallWebView2');
  ExtractTemporaryFile('MicrosoftEdgeWebview2Setup.exe');
  ExitCode := -1;
  if Exec(ExpandConstant('{tmp}\MicrosoftEdgeWebview2Setup.exe'), '/silent /install',
      '', SW_HIDE, ewWaitUntilTerminated, ExitCode) then
  begin
    Log(Format('WebView2 bootstrapper exit code: %d', [ExitCode]));
    if HasWebView2 then
      Exit;
  end;
  Result := FmtMessage(CustomMessage('WebView2Failed'), [IntToStr(ExitCode)]);
end;
