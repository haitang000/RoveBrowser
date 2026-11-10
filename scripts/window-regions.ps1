param([int]$ProcessId)
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class RoveRegions {
  private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);
  [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
  [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassName(IntPtr hWnd, StringBuilder name, int length);
  [DllImport("user32.dll")] private static extern IntPtr GetWindow(IntPtr hWnd, uint command);
  [DllImport("user32.dll")] private static extern int GetWindowRgn(IntPtr hWnd, IntPtr region);
  [DllImport("gdi32.dll")] private static extern IntPtr CreateRectRgn(int left, int top, int right, int bottom);
  [DllImport("gdi32.dll")] private static extern int GetRgnBox(IntPtr region, out RECT box);
  [DllImport("gdi32.dll")] private static extern bool DeleteObject(IntPtr handle);
  [StructLayout(LayoutKind.Sequential)] private struct RECT { public int Left, Top, Right, Bottom; }
  private static readonly List<string> Rows = new List<string>();
  private static void Walk(IntPtr window) {
    var name = new StringBuilder(256);
    GetClassName(window, name, 256);
    IntPtr region = CreateRectRgn(0, 0, 0, 0);
    int type = GetWindowRgn(window, region);
    RECT box; int ok = GetRgnBox(region, out box);
    DeleteObject(region);
    Rows.Add(string.Format("{{\"class\":\"{0}\",\"regionType\":{1},\"ok\":{2},\"left\":{3},\"top\":{4},\"right\":{5},\"bottom\":{6}}}", name, type, ok, box.Left, box.Top, box.Right, box.Bottom));
    IntPtr child = GetWindow(window, 5);
    while (child != IntPtr.Zero) { Walk(child); child = GetWindow(child, 2); }
  }
  public static string Dump(uint processId) {
    Rows.Clear();
    EnumWindows((window, _) => { uint owner; GetWindowThreadProcessId(window, out owner); if (owner == processId && IsWindowVisible(window)) Walk(window); return true; }, IntPtr.Zero);
    return "[" + string.Join(",", Rows) + "]";
  }
}
'@
Write-Output ([RoveRegions]::Dump([uint32]$ProcessId))
