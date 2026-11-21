// Test the HTML Fullscreen API against the real WebView2 and Windows window.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-fullscreen-'));
createLegacyNativeProfile(profile);
const port = Number(process.env.ERA_SMOKE_PORT || 19742);
const input = promisify(execFile);
const server = createServer((request, response) => {
  response.setHeader('Content-Type', 'text/html; charset=utf-8');
  if (request.url === '/embedded') {
    response.end(`<!doctype html><title>Embedded player</title>
      <style>html,body{height:100%;margin:0}iframe{width:100%;height:100%;border:0}</style>
      <iframe allowfullscreen src="http://localhost:${server.address().port}/video"></iframe>`);
  } else if (request.url === '/video') {
    response.end(`<!doctype html><title>Video player</title>
      <style>button{position:absolute;transform:translate(-50%,-50%)}</style>
      ${[25, 75].flatMap(x => [25, 75].map(y => `<button style="left:${x}%;top:${y}%">Fullscreen</button>`)).join('')}
      <video id="video" width="100" height="70"></video>
      <script>
        window.fullscreenEvents = [];
        document.addEventListener('fullscreenchange', () => fullscreenEvents.push(document.fullscreenElement?.tagName ?? 'exit'));
        for (const button of document.querySelectorAll('button')) {
          button.onclick = () => video.requestFullscreen().catch(error => fullscreenEvents.push(error.message));
        }
      </script>`);
  } else {
    response.end('<!doctype html><title>Other tab</title><h1>Other tab</h1>');
  }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const base = `http://127.0.0.1:${server.address().port}`;

async function waitFor(check, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    try {
      const result = await check();
      if (result) return result;
    } catch (error) {
      if (Date.now() >= deadline) throw new Error(label, { cause: error });
    }
    await new Promise(done => setTimeout(done, 100));
  }
  throw new Error(`Timed out: ${label}`);
}

async function nativeWindow(action = '', targets = []) {
  const command = `
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    Add-Type -Name Window -Namespace Rove -MemberDefinition @'
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd, int flags);
    [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr monitor, ref MONITORINFO info);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int w, int h, uint flags);
    [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT point);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT point);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
    public struct POINT { public int X, Y; }
    public struct RECT { public int Left, Top, Right, Bottom; }
    public struct MONITORINFO { public int Size; public RECT Monitor, Work; public int Flags; }
'@
    [Rove.Window]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
    $hwnd = (Get-Process -Id ${child.pid}).MainWindowHandle
    if ($hwnd -eq [IntPtr]::Zero) { throw 'Test window unavailable' }
    if ('${action}' -in @('click', 'escape')) {
      [Rove.Window]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0013) | Out-Null
      try {
        [Rove.Window]::SetForegroundWindow($hwnd) | Out-Null
        Start-Sleep -Milliseconds 250
        if ('${action}' -eq 'click') {
          $point = $null
          foreach ($target in @(${targets.map(([x, y]) => `@(${x}, ${y})`).join(', ')})) {
            $candidate = New-Object Rove.Window+POINT
            $candidate.X, $candidate.Y = $target
            if (-not [Rove.Window]::ClientToScreen($hwnd, [ref]$candidate)) { continue }
            $hit = [Rove.Window]::WindowFromPoint($candidate)
            if ([Rove.Window]::GetAncestor($hit, 2) -eq $hwnd -or [Rove.Window]::GetAncestor($hit, 3) -eq $hwnd) {
              $point = $candidate
              break
            }
          }
          if ($null -eq $point) { throw 'All fullscreen buttons are obscured by another window' }
          $cursor = New-Object Rove.Window+POINT
          [Rove.Window]::GetCursorPos([ref]$cursor) | Out-Null
          [Rove.Window]::SetCursorPos($point.X, $point.Y) | Out-Null
          [Rove.Window]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
          [Rove.Window]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
          Start-Sleep -Milliseconds 100
          [Rove.Window]::SetCursorPos($cursor.X, $cursor.Y) | Out-Null
        } else {
          if ([Rove.Window]::GetForegroundWindow() -ne $hwnd) { throw 'Test window is not foreground' }
          [Rove.Window]::keybd_event(27, 0, 0, [UIntPtr]::Zero)
          [Rove.Window]::keybd_event(27, 0, 2, [UIntPtr]::Zero)
        }
      } finally {
        [Rove.Window]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, 0x0013) | Out-Null
      }
    }
    $outer = New-Object Rove.Window+RECT
    $client = New-Object Rove.Window+RECT
    $monitor = New-Object Rove.Window+MONITORINFO
    $monitor.Size = 40
    if (-not [Rove.Window]::GetWindowRect($hwnd, [ref]$outer) -or
        -not [Rove.Window]::GetClientRect($hwnd, [ref]$client) -or
        -not [Rove.Window]::GetMonitorInfo([Rove.Window]::MonitorFromWindow($hwnd, 2), [ref]$monitor)) {
      throw 'Could not measure Rove and its monitor'
    }
    @{ outer = $outer; client = $client; monitor = $monitor.Monitor; maximized = [Rove.Window]::IsZoomed($hwnd) } | ConvertTo-Json -Compress
  `;
  const encoded = Buffer.from(command, 'utf16le').toString('base64');
  try {
    const { stdout } = await input('powershell.exe', ['-NoProfile', '-EncodedCommand', encoded], { windowsHide: true, timeout: 10000 });
    return JSON.parse(stdout);
  } catch (error) {
    throw new Error(`Native window ${action || 'measurement'} failed. ${error.stderr || error.code}`);
  }
}

async function expectFullscreen(ui, page, buttons, pane = 0) {
  await buttons.first().waitFor();
  const slot = await ui.locator('.webview-slot').nth(pane).boundingBox();
  const scale = await ui.evaluate(() => devicePixelRatio);
  const targets = await Promise.all((await buttons.all()).map(button => button.boundingBox()));
  // Fullscreen requires user activation in the focused native page controller.
  const nativeTargets = targets.map(target => [
    Math.round((slot.x + target.x + target.width / 2) * scale),
    Math.round((slot.y + target.y + target.height / 2) * scale),
  ]);
  try {
    await nativeWindow('click', nativeTargets);
  } catch (error) {
    if (pane !== 0 || !error.message.includes('obscured')) throw error;
    // The shell's normal find-close action restores focus to the active page.
    await ui.evaluate(() => window.ipc.postMessage(JSON.stringify({ type: 'findClose' })));
    await page.waitForFunction(() => document.hasFocus(), null, { timeout: 5000 });
    await buttons.first().click();
  }
  try {
    await page.waitForFunction(() => document.fullscreenElement && Math.abs(innerWidth - screen.width) <= 1 && Math.abs(innerHeight - screen.height) <= 1, null, { timeout: 5000 });
  } catch (error) {
    const viewport = await buttons.first().evaluate(element => {
      const view = element.ownerDocument.defaultView;
      return { events: view.fullscreenEvents, size: [view.innerWidth, view.innerHeight], screen: [view.screen.width, view.screen.height] };
    });
    throw new Error(`Fullscreen viewport ${JSON.stringify(viewport)}, native window ${JSON.stringify(await nativeWindow())}`, { cause: error });
  }
  const { outer, client, monitor } = await nativeWindow();
  assert.deepEqual(outer, monitor, 'borderless window covers its monitor');
  assert.equal(client.Right, monitor.Right - monitor.Left);
  assert.equal(client.Bottom, monitor.Bottom - monitor.Top);
}

let child, uiBrowser, pageBrowser;
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: 'pipe',
  });
  child.stderr.pipe(process.stderr);
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  await ui.waitForSelector('.sidebar');
  await send({ type: 'navigate', url: base + '/video' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const video = await site('/video');
  await video.getByRole('button').first().waitFor();
  const original = await nativeWindow();

  await expectFullscreen(ui, video, video.getByRole('button'));
  await video.evaluate(() => document.exitFullscreen());
  assert.deepEqual(await nativeWindow(), original, 'page exit restores the normal window');

  await send({ type: 'window', action: 'maximize' });
  const maximized = await waitFor(async () => {
    const state = await nativeWindow();
    return state.maximized && state;
  }, 'window maximized');
  await send({ type: 'navigate', url: base + '/embedded' });
  const embedded = await site('/embedded');
  const embeddedButtons = embedded.frameLocator('iframe').getByRole('button');
  await expectFullscreen(ui, embedded, embeddedButtons);
  await nativeWindow('escape');
  await embedded.waitForFunction(() => !document.fullscreenElement);
  assert.deepEqual(await nativeWindow(), maximized, 'Escape restores the maximized window');

  const embeddedId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await expectFullscreen(ui, embedded, embeddedButtons);
  await send({ type: 'new', url: base + '/other' });
  const other = await site('/other');
  await other.locator('h1').waitFor();
  await embedded.waitForFunction(() => !document.fullscreenElement);
  assert.deepEqual(await nativeWindow(), maximized, 'switching tabs exits fullscreen');

  const otherId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'split', id: otherId, other: embeddedId });
  const secondSlot = ui.locator('.webview-slot').nth(1);
  await secondSlot.waitFor();
  const paneWidth = (await secondSlot.boundingBox()).width;
  const waitForPane = () => embedded.waitForFunction(width => !document.fullscreenElement && Math.abs(innerWidth - width) <= 1, paneWidth, { timeout: 5000 });
  await waitForPane();
  await expectFullscreen(ui, embedded, embeddedButtons, 1);
  await nativeWindow('escape');
  await waitForPane();
  assert.deepEqual(await nativeWindow(), maximized, 'split video returns to its pane');
  console.log('Native fullscreen passed for video, cross-origin iframe, Escape, tab switching, and split view.');
} finally {
  if (child?.exitCode === null) child.kill();
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  server.closeAllConnections();
  server.close();
}
