param([int]$ProcessId, [int]$Width, [int]$Height)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class RoveResizeWindow {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll", SetLastError = true)] private static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
  [DllImport("user32.dll")] private static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
  [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr hwnd);
  [DllImport("user32.dll")] private static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  public static string Resize(IntPtr hwnd, int width, int height) {
    SetThreadDpiAwarenessContext(new IntPtr(-4));
    double scale = GetDpiForWindow(hwnd) / 96.0;
    if (!SetWindowPos(hwnd, IntPtr.Zero, 0, 0, (int)Math.Round(width * scale), (int)Math.Round(height * scale), 0x16)) throw new Win32Exception();
    RECT rect; GetClientRect(hwnd, out rect);
    return string.Format(System.Globalization.CultureInfo.InvariantCulture,
      "{{\"width\":{0},\"height\":{1}}}", rect.Right * 96.0 / GetDpiForWindow(hwnd), rect.Bottom * 96.0 / GetDpiForWindow(hwnd));
  }
}
'@
$targetProcess = Get-Process -Id $ProcessId
[RoveResizeWindow]::Resize($targetProcess.MainWindowHandle, $Width, $Height)
