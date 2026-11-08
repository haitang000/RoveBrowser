// Website reference: rove_www/src/style.css and src/i18n.ts.
[CustomMessages]
english.SetupTitle=Install Rove
english.SetupSubtitle=A little space for your next discovery.
english.OptionsStep=Installation options
english.InstallStep=Install Rove
english.DoneStep=Start browsing
english.BrandLine=Room to explore.
english.BrandHeadline=Light by Design.%nFast by Nature.
english.BrandDescription=Free and open-source.%nMade for Windows.
english.LocationTitle=Choose a home for Rove
english.LocationSubtitle=Pick an installation folder to get started.
english.LocationLabel=Installation folder
english.PersonalInstall=Just for you. No administrator access needed.
english.ProfileNote=Your tabs, workspaces and sign-ins stay with you, even after uninstalling.
english.ShortcutsTitle=Make yourself at home
english.ShortcutsSubtitle=Keep Rove within easy reach.
english.ShortcutsLabel=A shortcut in the Start menu is included.
english.ReadyTitle=Ready when you are
english.ReadySubtitle=One more click, and Rove is yours.
english.InstallAction=Install Rove
english.InstallingTitle=Getting Rove ready
english.InstallingSubtitle=This will only take a moment.
english.FinishedTitle=You're all set
english.FinishedSubtitle=Rove is ready for your next discovery.
english.FinishedNote=Your browsing data stays on this computer.
english.FinishAction=Finish
chinesesimplified.SetupTitle=安装 Rove
chinesesimplified.SetupSubtitle=为下一次探索，留一点空间。
chinesesimplified.OptionsStep=安装选项
chinesesimplified.InstallStep=安装 Rove
chinesesimplified.DoneStep=开始浏览
chinesesimplified.BrandLine=浏览，自在一点。
chinesesimplified.BrandHeadline=轻盈设计。%n天生迅捷。
chinesesimplified.BrandDescription=免费开源，%n专为 Windows 打造。
chinesesimplified.LocationTitle=为 Rove 选个位置
chinesesimplified.LocationSubtitle=选好安装位置，就可以开始了。
chinesesimplified.LocationLabel=安装位置
chinesesimplified.PersonalInstall=仅为当前用户安装，无需管理员权限。
chinesesimplified.ProfileNote=标签、工作区和登录状态会保留，卸载后也一样。
chinesesimplified.ShortcutsTitle=让 Rove 触手可及
chinesesimplified.ShortcutsSubtitle=按你的习惯，安排一个入口。
chinesesimplified.ShortcutsLabel=开始菜单快捷方式会自动创建。
chinesesimplified.ReadyTitle=准备好，就出发
chinesesimplified.ReadySubtitle=点击“安装 Rove”，开始安装。
chinesesimplified.InstallAction=安装 Rove
chinesesimplified.InstallingTitle=正在准备你的 Rove
chinesesimplified.InstallingSubtitle=很快就好，稍等片刻。
chinesesimplified.FinishedTitle=一切就绪
chinesesimplified.FinishedSubtitle=Rove 已准备好，随时开始浏览。
chinesesimplified.FinishedNote=你的浏览数据会保留在这台电脑上。
chinesesimplified.FinishAction=完成

[Code]
const
  RoveFrame = $FFFFFF;
  RoveInk = $2F2622;   // #22262f
  RoveQuiet = $808080;
  RoveAccent = $F65569; // #6955f6
  RoveWhite = $FFFFFF;

var
  RoveCard: TPanel;
  RoveRail: TPanel;
  RoveTitle: TNewStaticText;
  RoveSubtitle: TNewStaticText;
  RoveFootnote: TNewStaticText;
  RoveProgressText: TNewStaticText;
  RoveSteps: array[0..2] of TNewStaticText;
  RoveStepImages: array[0..2] of TBitmapImage;
  RoveProgress: TBitmapImage;
  RoveCustomDesign: Boolean;
  RoveFontLoaded: Boolean;

function CreateRoundRectRgn(Left, Top, Right, Bottom, Width, Height: Integer): THandle;
  external 'CreateRoundRectRgn@gdi32.dll stdcall';
function SetWindowRgn(Wnd: HWND; Region: THandle; Redraw: Boolean): Integer;
  external 'SetWindowRgn@user32.dll stdcall';
function DwmSetWindowAttribute(Wnd: HWND; Attribute: Integer; var Value: Integer;
  Size: Integer): Integer;
  external 'DwmSetWindowAttribute@dwmapi.dll stdcall';
function AddFontResourceEx(const Filename: String; Flags: LongWord; Reserved: Longint): Integer;
  external 'AddFontResourceExW@gdi32.dll stdcall';
function RemoveFontResourceEx(const Filename: String; Flags: LongWord; Reserved: Longint): Boolean;
  external 'RemoveFontResourceExW@gdi32.dll stdcall';

function RoveLabel(Parent: TWinControl; const Text: String;
  X, Y, W, H, Size: Integer; Color: TColor): TNewStaticText;
begin
  Result := TNewStaticText.Create(WizardForm);
  Result.Parent := Parent;
  Result.AutoSize := False;
  Result.WordWrap := True;
  Result.ShowAccelChar := False;
  Result.SetBounds(ScaleX(X), ScaleY(Y), ScaleX(W), ScaleY(H));
  Result.Caption := Text;
  Result.Font.Size := Size;
  Result.Font.Color := Color;
  Result.StyleElements := [];
end;

procedure DrawStep(Index: Integer; Active: Boolean);
var
  Fill: TColor;
begin
  Fill := RoveFrame;
  if Active then Fill := $FFF0EB;
  with RoveStepImages[Index] do
  begin
    Bitmap.Width := Width;
    Bitmap.Height := Height;
    Bitmap.Canvas.Brush.Color := RoveFrame;
    Bitmap.Canvas.Pen.Color := RoveFrame;
    Bitmap.Canvas.Rectangle(0, 0, Width, Height);
    Bitmap.Canvas.Brush.Color := Fill;
    Bitmap.Canvas.Pen.Color := Fill;
    Bitmap.Canvas.RoundRect(0, 0, Width, Height, ScaleX(12), ScaleY(12));
  end;
  RoveSteps[Index].Color := Fill;
  RoveSteps[Index].Font.Color := RoveQuiet;
  if Active then RoveSteps[Index].Font.Color := RoveAccent;
end;

procedure LayoutRoveButtons;
begin
  WizardForm.CancelButton.SetBounds(ScaleX(28), ScaleY(420), ScaleX(72), ScaleY(34));
  WizardForm.BackButton.SetBounds(ScaleX(286), ScaleY(420), ScaleX(84), ScaleY(34));
  WizardForm.NextButton.SetBounds(ScaleX(378), ScaleY(420), ScaleX(120), ScaleY(34));
end;

#include "Button.iss"

procedure InitializeWizard;
var
  Logo: TBitmapImage;
  Brand: TNewStaticText;
  Index, FrameColor, DarkCaption: Integer;
  StepName: String;
begin
  // Keep the standard accessible system design in high contrast mode.
  RoveCustomDesign := not HighContrastActive;
  if not RoveCustomDesign then Exit;
  ExtractTemporaryFile('Outfit.ttf');
  RoveFontLoaded := AddFontResourceEx(ExpandConstant('{tmp}\Outfit.ttf'), $10, 0) > 0;
  WizardForm.Font.Name := 'Outfit';
  if not RoveFontLoaded then WizardForm.Font.Name := 'Segoe UI';
  if ActiveLanguage = 'chinesesimplified' then
    WizardForm.Font.Name := 'Microsoft YaHei UI';
  WizardForm.Font.Size := 9;
  WizardForm.Font.Color := RoveInk;
  WizardForm.Color := RoveFrame;
  WizardForm.ClientWidth := ScaleX(860);
  WizardForm.ClientHeight := ScaleY(500);
  WizardForm.Caption := CustomMessage('SetupTitle');

  // Match the website's white canvas while retaining native window controls.
  FrameColor := RoveFrame;
  DarkCaption := 0;
  DwmSetWindowAttribute(WizardForm.Handle, 20, DarkCaption, 4);
  DwmSetWindowAttribute(WizardForm.Handle, 35, FrameColor, 4);
  FrameColor := RoveInk;
  DwmSetWindowAttribute(WizardForm.Handle, 36, FrameColor, 4);

  WizardForm.MainPanel.Visible := False;
  WizardForm.Bevel.Visible := False;
  WizardForm.Bevel1.Visible := False;
  WizardForm.BeveledLabel.Visible := False;
  RoveRail := TPanel.Create(WizardForm);
  RoveRail.Parent := WizardForm;
  RoveRail.BevelOuter := bvNone;
  RoveRail.Color := RoveFrame;
  RoveRail.StyleElements := [];
  RoveRail.SetBounds(0, 0, ScaleX(322), ScaleY(500));

  ExtractTemporaryFile('icon.png');
  Logo := TBitmapImage.Create(WizardForm);
  Logo.Parent := RoveRail;
  Logo.SetBounds(ScaleX(34), ScaleY(32), ScaleX(38), ScaleY(38));
  Logo.Stretch := True;
  Logo.BackColor := RoveFrame;
  Logo.PngImage.LoadFromFile(ExpandConstant('{tmp}\icon.png'));
  Brand := RoveLabel(RoveRail, 'Rove', 84, 33, 180, 42, 22, RoveInk);
  if RoveFontLoaded then Brand.Font.Name := 'Outfit';
  RoveLabel(RoveRail, CustomMessage('BrandHeadline'), 34, 112, 272, 112, 25, RoveInk);
  RoveLabel(RoveRail, CustomMessage('BrandDescription'), 34, 232, 260, 60, 10, RoveQuiet);

  for Index := 0 to 2 do
  begin
    RoveStepImages[Index] := TBitmapImage.Create(WizardForm);
    RoveStepImages[Index].Parent := RoveRail;
    RoveStepImages[Index].SetBounds(ScaleX(24), ScaleY(318 + Index * 38), ScaleX(260), ScaleY(32));
    case Index of
      0: StepName := CustomMessage('OptionsStep');
      1: StepName := CustomMessage('InstallStep');
      2: StepName := CustomMessage('DoneStep');
    end;
    RoveSteps[Index] := RoveLabel(RoveRail, IntToStr(Index + 1) + '    ' + StepName,
      37, 325 + Index * 38, 234, 23, 9, RoveQuiet);
  end;
  RoveLabel(RoveRail, '{#AppVersion}  /  Windows x64', 34, 460, 270, 24, 8, RoveQuiet);

  RoveCard := TPanel.Create(WizardForm);
  RoveCard.Parent := WizardForm;
  RoveCard.BevelOuter := bvNone;
  RoveCard.StyleElements := [];
  RoveCard.Color := RoveWhite;
  RoveCard.SetBounds(ScaleX(322), ScaleY(10), ScaleX(526), ScaleY(478));
  SetWindowRgn(RoveCard.Handle,
    CreateRoundRectRgn(0, 0, RoveCard.Width + 1, RoveCard.Height + 1, ScaleX(28), ScaleY(28)), True);
  RoveTitle := RoveLabel(RoveCard, CustomMessage('LocationTitle'), 28, 34, 470, 44, 20, RoveInk);
  RoveSubtitle := RoveLabel(RoveCard, CustomMessage('LocationSubtitle'), 28, 90, 470, 38, 9, RoveQuiet);

  WizardForm.OuterNotebook.Parent := RoveCard;
  WizardForm.OuterNotebook.SetBounds(ScaleX(28), ScaleY(148), ScaleX(470), ScaleY(248));
  WizardForm.InnerPage.Color := RoveWhite;
  WizardForm.FinishedPage.Color := RoveWhite;
  WizardForm.InnerNotebook.Align := alClient;
  for Index := 0 to WizardForm.InnerNotebook.PageCount - 1 do
    WizardForm.InnerNotebook.Pages[Index].Color := RoveWhite;
  WizardForm.CancelButton.Parent := RoveCard;
  WizardForm.BackButton.Parent := RoveCard;
  WizardForm.NextButton.Parent := RoveCard;
  WizardForm.NextButton.Anchors := [akLeft, akTop];
  WizardForm.BackButton.Anchors := [akLeft, akTop];
  WizardForm.CancelButton.Anchors := [akLeft, akTop];
  LayoutRoveButtons;

  WizardForm.SelectDirBitmapImage.Visible := False;
  WizardForm.SelectDirBrowseLabel.Visible := False;
  WizardForm.SelectDirLabel.Caption := CustomMessage('LocationLabel');
  WizardForm.SelectDirLabel.SetBounds(0, 0, ScaleX(470), ScaleY(24));
  WizardForm.SelectDirLabel.Anchors := [akLeft, akTop];
  WizardForm.DirEdit.Anchors := [akLeft, akTop];
  WizardForm.DirBrowseButton.Anchors := [akLeft, akTop];
  WizardForm.DirEdit.SetBounds(0, ScaleY(34), ScaleX(372), ScaleY(32));
  WizardForm.DirBrowseButton.SetBounds(ScaleX(382), ScaleY(34), ScaleX(88), ScaleY(32));
  WizardForm.DiskSpaceLabel.Font.Color := RoveQuiet;
  WizardForm.DiskSpaceLabel.SetBounds(0, ScaleY(216), ScaleX(470), ScaleY(24));
  RoveLabel(WizardForm.SelectDirPage, CustomMessage('PersonalInstall'), 0, 88, 470, 30, 9, RoveQuiet);
  RoveFootnote := RoveLabel(WizardForm.SelectDirPage, CustomMessage('ProfileNote'), 0, 150, 470, 52, 9, RoveQuiet);

  WizardForm.SelectTasksLabel.Caption := CustomMessage('ShortcutsLabel');
  WizardForm.SelectTasksLabel.SetBounds(0, 0, ScaleX(470), ScaleY(36));
  WizardForm.TasksList.SetBounds(0, ScaleY(46), ScaleX(470), ScaleY(148));
  WizardForm.TasksList.Color := RoveWhite;
  WizardForm.TasksList.BorderStyle := bsNone;
  WizardForm.TasksList.Anchors := [akLeft, akTop];
  WizardForm.TasksList.MinItemHeight := ScaleY(32);

  WizardForm.ReadyLabel.Visible := False;
  WizardForm.ReadyMemo.SetBounds(0, 0, ScaleX(470), ScaleY(248));
  WizardForm.ReadyMemo.Color := RoveWhite;
  WizardForm.ReadyMemo.BorderStyle := bsNone;
  WizardForm.ReadyMemo.Font.Size := 9;
  WizardForm.ReadyMemo.Anchors := [akLeft, akTop];
  WizardForm.ReadyMemo.WordWrap := True;
  WizardForm.ReadyMemo.ScrollBars := ssVertical;

  WizardForm.StatusLabel.SetBounds(0, ScaleY(20), ScaleX(470), ScaleY(36));
  WizardForm.FilenameLabel.SetBounds(0, ScaleY(120), ScaleX(470), ScaleY(48));
  WizardForm.FilenameLabel.Font.Color := RoveQuiet;
  WizardForm.ProgressGauge.Visible := False;
  RoveProgress := TBitmapImage.Create(WizardForm);
  RoveProgress.Parent := WizardForm.InstallingPage;
  RoveProgress.SetBounds(0, ScaleY(76), ScaleX(470), ScaleY(6));
  RoveProgressText := RoveLabel(WizardForm.InstallingPage, '', 0, 94, 470, 26, 9, RoveQuiet);

  WizardForm.WizardBitmapImage2.Visible := False;
  WizardForm.FinishedHeadingLabel.Visible := False;
  WizardForm.FinishedLabel.Caption := CustomMessage('FinishedNote');
  WizardForm.FinishedLabel.SetBounds(0, 0, ScaleX(470), ScaleY(60));
  WizardForm.RunList.SetBounds(0, ScaleY(78), ScaleX(470), ScaleY(120));
  WizardForm.RunList.Color := RoveWhite;
  WizardForm.RunList.Anchors := [akLeft, akTop];
  StyleRovePrimaryButton;
end;

procedure CurPageChanged(CurPageID: Integer);
var
  ActiveStep, Index: Integer;
begin
  if not RoveCustomDesign then Exit;
  ActiveStep := 0;
  case CurPageID of
    wpSelectDir: begin
      RoveTitle.Caption := CustomMessage('LocationTitle');
      RoveSubtitle.Caption := CustomMessage('LocationSubtitle');
    end;
    wpSelectTasks: begin
      RoveTitle.Caption := CustomMessage('ShortcutsTitle');
      RoveSubtitle.Caption := CustomMessage('ShortcutsSubtitle');
    end;
    wpReady: begin
      ActiveStep := 1;
      RoveTitle.Caption := CustomMessage('ReadyTitle');
      RoveSubtitle.Caption := CustomMessage('ReadySubtitle');
      WizardForm.NextButton.Caption := CustomMessage('InstallAction');
    end;
    wpPreparing, wpInstalling: begin
      ActiveStep := 1;
      RoveTitle.Caption := CustomMessage('InstallingTitle');
      RoveSubtitle.Caption := CustomMessage('InstallingSubtitle');
    end;
    wpFinished: begin
      ActiveStep := 2;
      RoveTitle.Caption := CustomMessage('FinishedTitle');
      RoveSubtitle.Caption := CustomMessage('FinishedSubtitle');
      WizardForm.NextButton.Caption := CustomMessage('FinishAction');
    end;
  end;
  for Index := 0 to 2 do DrawStep(Index, Index = ActiveStep);
  if CurPageID = wpSelectDir then
  begin
    WizardForm.DirEdit.SetBounds(0, ScaleY(34), ScaleX(372), ScaleY(32));
    WizardForm.DirBrowseButton.SetBounds(ScaleX(382), ScaleY(34), ScaleX(88), ScaleY(32));
  end;
  LayoutRoveButtons;
end;

procedure CurInstallProgressChanged(CurProgress, MaxProgress: Integer);
var
  Filled, Percent: Integer;
begin
  if not RoveCustomDesign or (MaxProgress = 0) then Exit;
  Percent := Round(CurProgress * 100.0 / MaxProgress);
  RoveProgressText.Caption := IntToStr(Percent) + '%';
  with RoveProgress do
  begin
    Bitmap.Width := Width;
    Bitmap.Height := Height;
    Bitmap.Canvas.Brush.Color := $F3EFEB;
    Bitmap.Canvas.Pen.Color := $F3EFEB;
    Bitmap.Canvas.RoundRect(0, 0, Width, Height, Height, Height);
    Filled := Round(CurProgress * 1.0 / MaxProgress * Width);
    Bitmap.Canvas.Brush.Color := RoveAccent;
    Bitmap.Canvas.Pen.Color := RoveAccent;
    if Filled > 0 then
      Bitmap.Canvas.RoundRect(0, 0, Filled, Height, Height, Height);
  end;
end;

procedure DeinitializeSetup;
begin
  // VCL destroys controls after the script engine is released. Detach first.
  if RoveButtonCallback <> 0 then
    RemoveWindowSubclass(WizardForm.NextButton.Handle, RoveButtonCallback, 1);
  if RoveFontLoaded then
    RemoveFontResourceEx(ExpandConstant('{tmp}\Outfit.ttf'), $10, 0);
end;
