; Compile through scripts/installer.ps1 so versions, payload and dependencies agree.
; The wizard intentionally uses the stock Inno Setup modern style.
#ifndef AppVersion
  #error Run scripts/installer.ps1 to build the installer.
#endif
#ifndef AppRegistrationName
  #define AppRegistrationName "Rove"
#endif
#ifndef SetupAppId
  #define SetupAppId "{{76F5D0C8-2824-42C5-A964-CAD38178E0BA}"
#endif
#ifndef AppTitle
  #define AppTitle "Rove"
#endif

[Setup]
AppId={#SetupAppId}
AppName={#AppTitle}
AppVersion={#AppVersion}
AppVerName={#AppTitle} {#AppVersion}
VersionInfoVersion={#FileVersion}
VersionInfoProductVersion={#FileVersion}
VersionInfoProductTextVersion={#AppVersion}
VersionInfoTextVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Rove
DefaultGroupName={#AppTitle}
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
english.InstallWebView2=Installing Microsoft Edge WebView2 Runtime...
english.WebView2Failed=WebView2 Runtime could not be installed (exit code %1). Connect to the internet and retry, or install the runtime from https://developer.microsoft.com/microsoft-edge/webview2/ and run Setup again.
chinesesimplified.InstallWebView2=正在安装 Microsoft Edge WebView2 运行时……
chinesesimplified.WebView2Failed=无法安装 WebView2 运行时（退出代码 %1）。请连接网络后重试，或从 https://developer.microsoft.com/microsoft-edge/webview2/ 安装运行时后重新运行安装程序。

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PayloadDir}\Rove.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#PayloadDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#BootstrapperPath}"; Flags: dontcopy

[Icons]
Name: "{group}\Rove"; Filename: "{app}\Rove.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Rove"; Filename: "{app}\Rove.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\Rove.exe"; Description: "{cm:LaunchProgram,Rove}"; Flags: nowait postinstall skipifsilent

; Offer HTTP/HTTPS handling for this user. Never overwrite UserChoice or
; Software\Classes\http(s): Windows Default Apps owns the final decision.
[Registry]
Root: HKCU; Subkey: "Software\Classes\{#AppRegistrationName}URL"; ValueType: string; ValueName: ""; ValueData: "Rove 网页链接"
Root: HKCU; Subkey: "Software\Classes\{#AppRegistrationName}URL"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\{#AppRegistrationName}URL\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\Rove.exe"",0"
Root: HKCU; Subkey: "Software\Classes\{#AppRegistrationName}URL\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\Rove.exe"" -- ""%1"""
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}"; ValueType: string; ValueName: ""; ValueData: "Rove"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\Rove.exe"",0"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\Rove.exe"""
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Rove"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Rove 网页浏览器"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\Capabilities"; ValueType: string; ValueName: "ApplicationIcon"; ValueData: """{app}\Rove.exe"",0"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\Capabilities\URLAssociations"; ValueType: string; ValueName: "http"; ValueData: "{#AppRegistrationName}URL"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\Capabilities\URLAssociations"; ValueType: string; ValueName: "https"; ValueData: "{#AppRegistrationName}URL"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppRegistrationName}"; ValueData: "Software\Clients\StartMenuInternet\{#AppRegistrationName}\Capabilities"

; Intentionally no UninstallDelete: the existing EraBrowser profile belongs to the user.
[Code]
const
  WebView2Key = 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

procedure SHChangeNotify(EventId: Integer; Flags: Cardinal; Item1, Item2: Integer);
  external 'SHChangeNotify@shell32.dll stdcall';

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    SHChangeNotify($08000000, 0, 0, 0);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command, Expected: String;
begin
  if CurUninstallStep <> usUninstall then Exit;
  Expected := '"' + ExpandConstant('{app}\Rove.exe') + '" -- "%1"';
  { Preserve a registration subsequently claimed by another portable copy. }
  if RegQueryStringValue(HKCU, 'Software\Classes\{#AppRegistrationName}URL\shell\open\command', '', Command) and
    (CompareText(Command, Expected) = 0) then
  begin
    RegDeleteValue(HKCU, 'Software\RegisteredApplications', '{#AppRegistrationName}');
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Clients\StartMenuInternet\{#AppRegistrationName}');
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Classes\{#AppRegistrationName}URL');
    SHChangeNotify($08000000, 0, 0, 0);
  end;
end;

function RuntimeInRegistry(RootKey: Integer): Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(RootKey, WebView2Key, 'pv', Version) and
    (Version <> '') and (Version <> '0.0.0.0');
end;

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
