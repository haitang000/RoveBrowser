// Paint the existing native button; its keyboard, focus, accessibility and click behavior remain native.
[Code]
function SetWindowSubclass(Wnd: HWND; Callback: Longint; Id, Data: Longint): Boolean;
  external 'SetWindowSubclass@comctl32.dll stdcall';
function RemoveWindowSubclass(Wnd: HWND; Callback: Longint; Id: Longint): Boolean;
  external 'RemoveWindowSubclass@comctl32.dll stdcall';
function DefSubclassProc(Wnd: HWND; Msg: LongWord; WParam: Longint; LParam: Longint): Longint;
  external 'DefSubclassProc@comctl32.dll stdcall';
function ButtonGetDC(Wnd: HWND): THandle;
  external 'GetDC@user32.dll stdcall';
function ButtonReleaseDC(Wnd: HWND; DC: THandle): Integer;
  external 'ReleaseDC@user32.dll stdcall';
function ButtonGetClientRect(Wnd: HWND; var Bounds: TRect): Boolean;
  external 'GetClientRect@user32.dll stdcall';
function ButtonFillRect(DC: THandle; var Bounds: TRect; Brush: THandle): Integer;
  external 'FillRect@user32.dll stdcall';
function ButtonBrush(Color: Integer): THandle;
  external 'CreateSolidBrush@gdi32.dll stdcall';
function ButtonDeleteObject(Obj: THandle): Boolean;
  external 'DeleteObject@gdi32.dll stdcall';
function ButtonSelectObject(DC, Obj: THandle): THandle;
  external 'SelectObject@gdi32.dll stdcall';
function ButtonSaveDC(DC: THandle): Integer;
  external 'SaveDC@gdi32.dll stdcall';
function ButtonRestoreDC(DC: THandle; Saved: Integer): Boolean;
  external 'RestoreDC@gdi32.dll stdcall';
function ButtonClip(DC, Region: THandle): Integer;
  external 'SelectClipRgn@gdi32.dll stdcall';
function ButtonTextColor(DC: THandle; Color: Integer): Integer;
  external 'SetTextColor@gdi32.dll stdcall';
function ButtonBkMode(DC: THandle; Mode: Integer): Integer;
  external 'SetBkMode@gdi32.dll stdcall';
function ButtonDrawText(DC: THandle; Text: String; Count: Integer; var Bounds: TRect; Format: LongWord): Integer;
  external 'DrawTextW@user32.dll stdcall';
function ButtonGetFocus: HWND;
  external 'GetFocus@user32.dll stdcall';
function ButtonIsEnabled(Wnd: HWND): Boolean;
  external 'IsWindowEnabled@user32.dll stdcall';
function ButtonFocusRect(DC: THandle; var Bounds: TRect): Boolean;
  external 'DrawFocusRect@user32.dll stdcall';

var
  RoveButtonCallback: Longint;

procedure PaintRoveButton(Wnd: HWND; DC: THandle);
var
  Bounds, Stripe: TRect;
  Brush, Region: THandle;
  Saved, Y, R, G, B, Height, State: Integer;
  Text: String;
  Pressed, Enabled: Boolean;
begin
  Saved := ButtonSaveDC(DC);
  ButtonGetClientRect(Wnd, Bounds);
  Brush := ButtonBrush(RoveWhite);
  ButtonFillRect(DC, Bounds, Brush);
  ButtonDeleteObject(Brush);
  Region := CreateRoundRectRgn(0, 0, Bounds.Right, Bounds.Bottom, ScaleX(16), ScaleY(16));
  ButtonClip(DC, Region);
  ButtonDeleteObject(Region);
  State := SendMessage(Wnd, $00F2, 0, 0); // BM_GETSTATE
  Pressed := (State and 4) <> 0;
  Enabled := ButtonIsEnabled(Wnd);
  Height := Bounds.Bottom;
  // Website button: #363a43 -> #22252d.
  for Y := 0 to Height - 1 do
  begin
    R := 54 - (20 * Y div Height);
    G := 58 - (21 * Y div Height);
    B := 67 - (22 * Y div Height);
    if Pressed then begin R := 30; G := 33; B := 41; end;
    if not Enabled then begin R := 170; G := 173; B := 180; end;
    Brush := ButtonBrush(R + G * 256 + B * 65536);
    Stripe := Bounds;
    Stripe.Top := Y;
    Stripe.Bottom := Y + 1;
    ButtonFillRect(DC, Stripe, Brush);
    ButtonDeleteObject(Brush);
  end;
  ButtonSelectObject(DC, SendMessage(Wnd, $0031, 0, 0)); // WM_GETFONT
  ButtonTextColor(DC, RoveWhite);
  ButtonBkMode(DC, 1);
  Text := WizardForm.NextButton.Caption;
  StringChangeEx(Text, '&', '', True);
  ButtonDrawText(DC, Text, Length(Text), Bounds, $00000825);
  if ButtonGetFocus = Wnd then
  begin
    Bounds.Left := ScaleX(4);
    Bounds.Top := ScaleY(4);
    Bounds.Right := Bounds.Right - ScaleX(4);
    Bounds.Bottom := Bounds.Bottom - ScaleY(4);
    ButtonFocusRect(DC, Bounds);
  end;
  ButtonRestoreDC(DC, Saved);
end;

function RoveButtonProc(Wnd: HWND; Msg: LongWord; WParam: Longint;
  LParam: Longint; Id, Data: Longint): Longint;
var
  DC: THandle;
begin
  if Msg = $0082 then // WM_NCDESTROY
    RemoveWindowSubclass(Wnd, RoveButtonCallback, 1);
  Result := DefSubclassProc(Wnd, Msg, WParam, LParam);
  if Msg = $000F then // WM_PAINT
  begin
    DC := ButtonGetDC(Wnd);
    if DC <> 0 then
    begin
      PaintRoveButton(Wnd, DC);
      ButtonReleaseDC(Wnd, DC);
    end;
  end
  else if Msg = $0318 then // WM_PRINTCLIENT
    PaintRoveButton(Wnd, WParam);
end;

procedure StyleRovePrimaryButton;
begin
  RoveButtonCallback := CreateCallback(@RoveButtonProc);
  if not SetWindowSubclass(WizardForm.NextButton.Handle, RoveButtonCallback, 1, 0) then
    Log('Could not customize the primary button; retaining the native button.');
end;
