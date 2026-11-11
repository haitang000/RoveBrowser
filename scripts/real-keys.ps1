param(
  [Parameter(Mandatory = $true)][int]$ProcessId,
  [Parameter(Mandatory = $true)][ValidateSet('click-page', 'ctrl-j', 'escape')][string]$Action,
  [double]$xFrac = 0.62,
  [double]$yFrac = 0.5
)
# Sends trusted OS input to a Rove test window. CDP-injected keys bypass
# WebView2's AcceleratorKeyPressed, so native smoke tests that exercise page
# accelerators use real keybd_event input through this helper.
Add-Type -Name Win -Namespace Native -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
[DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
[DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, int dx, int dy, uint dwData, UIntPtr dwExtraInfo);
[DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
public struct POINT { public int X; public int Y; }
'@
$handle = (Get-Process -Id $ProcessId).MainWindowHandle
if ($handle -eq [IntPtr]::Zero) { Write-Output 'no-window'; exit 1 }
[Native.Win]::ShowWindow($handle, 9) | Out-Null
[Native.Win]::SetWindowPos($handle, [IntPtr](-1), 0, 0, 0, 0, 0x0013) | Out-Null
[Native.Win]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 400
$rect = New-Object Native.Win+RECT
[Native.Win]::GetWindowRect($handle, [ref]$rect) | Out-Null
$cursor = New-Object Native.Win+POINT
[Native.Win]::GetCursorPos([ref]$cursor) | Out-Null
$oldX = $cursor.X; $oldY = $cursor.Y
$x = [int]($rect.Left + ($rect.Right - $rect.Left) * $xFrac)
$y = [int]($rect.Top + ($rect.Bottom - $rect.Top) * $yFrac)
# Click into the page so the page WebView owns keyboard focus.
[Native.Win]::SetCursorPos($x, $y) | Out-Null
[Native.Win]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
[Native.Win]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 350
function Send-Key([byte]$key, [bool]$ctrl) {
  if ($ctrl) {
    [Native.Win]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 30
  }
  [Native.Win]::keybd_event($key, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 60
  [Native.Win]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
  if ($ctrl) { [Native.Win]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero) }
  Start-Sleep -Milliseconds 300
}
switch ($Action) {
  'click-page' { }
  'ctrl-j' { Send-Key 0x4A $true }   # Ctrl+J
  'escape' { Send-Key 0x1B $false }  # Esc
}
[Native.Win]::SetCursorPos($oldX, $oldY) | Out-Null
[Native.Win]::SetWindowPos($handle, [IntPtr](-2), 0, 0, 0, 0, 0x0013) | Out-Null
Write-Output "action=$Action targetPid=$ProcessId click=$x,$y"
