// Downloads page against real WebView2: the panel reveals itself when a file
// lands, progress/pause/resume/cancel follow the operation, Clear keeps
// unfinished rows, the two keyboard routes work, and the history survives a
// restart. Playwright's CDP denies downloads by default, so the page session
// restores WebView2's own behavior without choosing a destination.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:http';
import { access, mkdtemp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';

const execFileAsync = promisify(execFile);

const smallText = 'Rove downloads fixture\n'.repeat(1200);
const fastText = 'Fast known-length download\n';
// Serve an actual unsigned PE executable, like the installer in the reported
// case. Renaming text to .exe does not exercise the executable download path.
const executable = await readFile(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'));
const slowSize = 3 * 1024 * 1024;
const server = createServer((req, res) => {
  if (req.url === '/fixture.exe') {
    res.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Disposition': 'attachment; filename="fixture.exe"',
      'Content-Length': String(executable.length),
    });
    res.end(executable);
    return;
  }
  if (req.url === '/fast.txt' || req.url === '/empty.bin') {
    const empty = req.url === '/empty.bin';
    const body = empty ? '' : fastText;
    res.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Disposition': `attachment; filename="${empty ? 'empty.bin' : 'fast.txt'}"`,
      'Content-Length': String(Buffer.byteLength(body)),
    });
    res.end(body);
    return;
  }
  if (req.url === '/small.txt') {
    res.writeHead(200, {
      'Content-Type': 'text/plain',
      'Content-Disposition': 'attachment; filename="small.txt"',
    });
    res.end(smallText);
    return;
  }
  if (req.url === '/slow.bin') {
    res.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Disposition': 'attachment; filename="slow.bin"',
      'Content-Length': String(slowSize),
    });
    let sent = 0;
    const chunk = Buffer.alloc(64 * 1024, 7);
    const tick = () => {
      if (sent >= slowSize) { res.end(); return; }
      res.write(chunk);
      sent += chunk.length;
      setTimeout(tick, 80);
    };
    tick();
    return;
  }
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  res.end(
    '<!doctype html><title>Downloads fixture</title><h1>Downloads fixture</h1>' +
      '<a id="small" href="/small.txt" download>small</a> ' +
      '<a id="fast" href="/fast.txt" download>fast</a> ' +
      '<a id="empty" href="/empty.bin" download>empty</a> ' +
      '<a id="exe" href="/fixture.exe" download>exe</a> ' +
      '<a id="slow" href="/slow.bin" download>slow</a>',
  );
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));

const port = Number(process.env.ERA_SMOKE_PORT || 19846);
const base = `http://127.0.0.1:${server.address().port}`;
const profile = await mkdtemp(join(tmpdir(), 'rove-downloads-smoke-'));
createLegacyNativeProfile(profile);
const downloadsDir = join(profile, 'Downloads');
await mkdir(downloadsDir, { recursive: true });
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-downloads');
await mkdir(output, { recursive: true });

let child, uiBrowser, pageBrowser, ui, popup, page;
const checks = [];
async function waitFor(fn, label) {
  for (let i = 0; i < 240; i++) {
    try { const value = await fn(); if (value) return value; } catch {}
    await new Promise((done) => setTimeout(done, 100));
  }
  throw Error('Timed out: ' + label);
}
const send = (command) => ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command);
/** Real OS keys, because CDP-injected input bypasses WebView2 accelerators. */
async function realKeys(action, xFrac = 0.62, yFrac = 0.5) {
  await execFileAsync(
    'powershell.exe',
    [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      resolve('scripts/real-keys.ps1'),
      '-ProcessId',
      String(child.pid),
      '-Action',
      action,
      '-xFrac', String(xFrac),
      '-yFrac', String(yFrac),
    ],
    { windowsHide: true },
  );
  await new Promise((done) => setTimeout(done, 400));
}
const items = () => ui.evaluate(() => window.__lastDownloads || []);
const panel = () => popup.locator('.downloads-panel');
const rows = () => panel().locator('.download-row');
async function launch() {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true,
    stdio: 'pipe',
  });
  child.stderr.on('data', (data) => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(async () =>
    uiBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === 'https://era.ui/index.html'),
  'UI page');
  await ui.waitForSelector('.sidebar');
  popup = await waitFor(async () =>
    uiBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === 'https://era.ui/index.html?popup'),
  'popup renderer');
  await ui.evaluate(() => {
    window.__lastDownloads = [];
    const previous = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.type === 'downloads' && event.items) window.__lastDownloads = event.items;
      previous?.(event);
    };
  });
  // The smoke installs its own folder through the real settings command.
  const settings = await ui.evaluate(() => window.__ERA_INITIAL_STATE__.settings);
  await send({
    type: 'settingsUpdate',
    settings: { ...settings, downloadFolder: downloadsDir, askBeforeQuit: false },
  });
  await ui.evaluate((folder) => (window.__expectedFolder = folder), downloadsDir);
  await send({ type: 'new', url: `${base}/page` });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  page = await waitFor(async () =>
    pageBrowser.contexts().flatMap((c) => c.pages()).find((p) => p.url() === `${base}/page`),
  'fixture page');
  await page.waitForSelector('#small');
  const session = await page.context().newCDPSession(page);
  await session.send('Browser.setDownloadBehavior', { behavior: 'default' });
  await ui.waitForTimeout(400);
}
async function stop() {
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  const exited = new Promise((done) => child.once('exit', done));
  child.kill();
  await Promise.race([exited, new Promise((done) => setTimeout(done, 4000))]);
  child = uiBrowser = ui = popup = pageBrowser = page = undefined;
  await new Promise((done) => setTimeout(done, 700));
}
async function downloadSmall() {
  const known = new Set((await items()).map((entry) => entry.id));
  await page.click('#small');
  await waitFor(async () => {
    const list = await items();
    const item = list.find(
      (entry) => !known.has(entry.id) && entry.filename.startsWith('small') && entry.state === 'finished',
    );
    return item && item.state === 'finished';
  }, 'small download finished');
}

try {
  await launch();

  // A download lands, saves to the configured folder and reveals the panel.
  await downloadSmall();
  await panel().waitFor();
  const first = (await items())[0];
  assert.equal(first.state, 'finished');
  assert.equal(first.total, Buffer.byteLength(smallText));
  assert.equal(await readFile(join(downloadsDir, 'small.txt'), 'utf8'), smallText);
  assert.equal(await rows().count(), 1);
  await popup.screenshot({ path: join(output, 'panel.png') });
  checks.push('下载完成后自动展开面板，文件保存到设置中的下载文件夹');

  // The popup really covers the page HWND for outside clicks, then releases it
  // after the leave transition; CDP input alone cannot prove native hit testing.
  await realKeys('click-page');
  await waitFor(async () => (await panel().count()) === 0, 'native page click closes panel');
  await ui.locator('.downloads-button').click();
  await panel().waitFor();
  await waitFor(() => ui.locator('.downloads-button').getAttribute('aria-expanded').then((value) => value === 'true'), 'shared open state');
  checks.push('真实系统鼠标点击网页关闭浮窗，侧栏按钮与原生浮层开关状态同步');
  for (const [x, y] of [[0.08, 0.25], [0.4, 0.03]]) {
    await realKeys('click-page', x, y);
    await waitFor(async () => (await panel().count()) === 0, 'native sidebar/title click closes panel');
    await ui.locator('.downloads-button').click();
    await panel().waitFor();
  }
  checks.push('真实系统鼠标点击侧栏与标题栏空白也能关闭浮窗');

  // A second identical name gets Zigzag's " (1)" collision name.
  await panel().getByRole('button', { name: '关闭下载面板' }).click();
  await downloadSmall();
  const names = (await items()).map((entry) => entry.filename);
  assert.ok(names.includes('small (1).txt'), 'duplicate filename');
  await waitFor(async () => {
    const files = (await readdir(downloadsDir)).filter((name) => name.startsWith('small'));
    return files.length === 2;
  }, 'two small files on disk');
  checks.push('同名下载自动改名为 "small (1).txt"');

  // Very short known-length downloads may finish around handler registration.
  // Check both native state and the popup's actions, including zero-byte files.
  for (const selector of ['#fast', '#fast', '#fast', '#empty']) {
    const known = new Set((await items()).map((entry) => entry.id));
    await panel().getByRole('button', { name: '关闭下载面板' }).click();
    await page.click(selector);
    const done = await waitFor(async () => (await items()).find(
      (entry) => !known.has(entry.id) && entry.state === 'finished',
    ), `${selector} reaches completed state`);
    const expected = selector === '#empty' ? '' : fastText;
    assert.equal(done.total, Buffer.byteLength(expected));
    assert.equal(done.received, done.total);
    assert.equal(done.exists, true);
    assert.equal(await readFile(done.path, 'utf8'), expected);
    const row = panel().locator('.download-row', { hasText: done.filename });
    await row.getByRole('button', { name: '打开文件', exact: true }).waitFor();
    assert.equal(await row.locator('[role="progressbar"]').count(), 0);
    // A late click from an old row must not turn a completed download active.
    await send({ type: 'downloadAction', id: done.id, action: 'pause' });
    await waitFor(async () => (await items()).find((entry) => entry.id === done.id)?.state === 'finished', 'late pause preserves completion');
  }
  checks.push('连续快速下载和零字节文件均转为已完成，可打开文件且不残留进度条');

  await panel().getByRole('button', { name: '关闭下载面板' }).click();
  await page.click('#exe');
  const exe = await waitFor(async () => (await items()).find(
    (entry) => entry.filename === 'fixture.exe' && entry.state === 'finished',
  ), 'real executable reaches completed state');
  assert.equal(exe.received, executable.length);
  assert.equal(exe.total, executable.length);
  assert.equal(exe.exists, true);
  assert.deepEqual(await readFile(exe.path), executable);
  const exeRow = panel().locator('.download-row', { hasText: 'fixture.exe' });
  await exeRow.getByRole('button', { name: '打开文件', exact: true }).waitFor();
  assert.equal(await exeRow.locator('[role="progressbar"]').count(), 0);
  checks.push('真实未签名 EXE 下载后转为已完成，字节内容一致且不残留进度条（不执行文件）');

  // Pause holds the received byte count, Resume continues it.
  await page.click('#slow');
  await waitFor(async () => {
    const item = (await items()).find((entry) => entry.filename === 'slow.bin');
    return item && item.received > 0;
  }, 'slow download progress');
  const slow = () => items().then((list) => list.find((entry) => entry.filename === 'slow.bin'));
  await panel().locator('.download-row', { hasText: 'slow.bin' }).getByTitle('暂停下载').click();
  await waitFor(async () => (await slow()).state === 'paused', 'paused state');
  const pausedBytes = (await slow()).received;
  await ui.waitForTimeout(700);
  assert.equal((await slow()).received, pausedBytes, 'bytes stay stable while paused');
  await panel().getByTitle('继续下载').click();
  await waitFor(async () => (await slow()).received > pausedBytes, 'resumed bytes');
  checks.push('下载可暂停（字节数停止增长）并继续');

  // Cancel; the partial file must not be left behind, and Clear removes rows.
  await panel().locator('.download-row', { hasText: 'slow.bin' }).getByTitle('取消下载').click();
  await waitFor(async () => (await slow()).state === 'cancelled', 'cancelled state');
  await waitFor(async () => {
    const names = await readdir(downloadsDir);
    return !names.includes('slow.bin');
  }, 'partial file removed');

  // Also let a known-length transfer run through its final bytes to completion.
  await panel().getByRole('button', { name: '关闭下载面板' }).click();
  await page.click('#slow');
  const complete = await waitFor(async () => (await items()).find(
    (entry) => entry.filename === 'slow.bin' && entry.state === 'finished',
  ), 'known-length download completes after its final bytes');
  assert.equal(complete.received, slowSize);
  assert.equal(complete.total, slowSize);
  assert.deepEqual(await readFile(complete.path), Buffer.alloc(slowSize, 7));
  await panel().locator('.download-row.finished', { hasText: 'slow.bin' })
    .getByRole('button', { name: '打开文件', exact: true }).waitFor();
  checks.push('已知大小的大文件经过最后字节后进入已完成，文件内容完整');

  await panel().getByRole('button', { name: '更多下载操作' }).click();
  await panel().getByRole('menuitem', { name: '清除已结束的下载' }).click();
  await waitFor(async () => (await rows().count()) === 0, 'clear finished downloads');
  checks.push('取消后清除已结束下载，未完成文件不残留');

  // Keyboard: Ctrl+J in the shell opens the panel; with the page focused, real
  // Ctrl+J and Esc go through WebView2's page accelerator (CDP keys do not).
  await ui.keyboard.press('Control+j');
  await panel().waitFor();
  await ui.keyboard.press('Escape');
  await waitFor(async () => (await panel().count()) === 0, 'UI escape closes panel');
  await realKeys('ctrl-j');
  await panel().waitFor();
  await ui.waitForTimeout(300);
  await popup.keyboard.press('Escape');
  await waitFor(async () => (await panel().count()) === 0, 'page escape closes panel');
  checks.push('Ctrl+J 从外壳及真实网页打开下载面板，外壳与浮层内的 Esc 都能关闭');

  await send({ type: 'sidebar' });
  await waitFor(() => ui.locator('.sidebar-track').getAttribute('aria-hidden').then((value) => value === 'true'), 'sidebar collapsed');
  await realKeys('ctrl-j');
  await panel().waitFor();
  await waitFor(() => ui.locator('.sidebar-track').getAttribute('aria-hidden').then((value) => value === 'false'), 'download shortcut reveals sidebar once');
  await popup.keyboard.press('Escape');
  await waitFor(async () => (await panel().count()) === 0, 'close after collapsed sidebar shortcut');
  checks.push('侧栏折叠时从真实网页按 Ctrl+J 可展开侧栏并打开浮窗');

  // History survives a restart, and the file is still there.
  await downloadSmall();
  await waitFor(async () => (await items()).some((entry) => entry.state === 'finished'), 'download before restart');
  await stop();
  await launch();
  await ui.locator('.downloads-button').click();
  await panel().waitFor();
  await waitFor(async () =>
    (await rows().count()) === 1 &&
    (await rows().first().innerText()).includes('small'),
  'restored download row');
  await access(join(downloadsDir, 'small (2).txt')).catch(async () => access(join(downloadsDir, 'small.txt')));
  checks.push('下载历史经 SQLite 在重启后恢复，面板仍能打开原文件');

  const result = { passed: true, profile, checks };
  await writeFile(join(output, 'native-downloads-smoke.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await ui?.screenshot({ path: join(output, 'failure-ui.png') }).catch(() => {});
  await popup?.screenshot({ path: join(output, 'failure-popup.png') }).catch(() => {});
  await writeFile(join(output, 'failure.json'), JSON.stringify({
    checks,
    error: String(error),
    items: await ui?.evaluate(() => window.__lastDownloads).catch(() => null),
    files: await readdir(downloadsDir).catch(() => []),
  }, null, 2));
  throw error;
} finally {
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  if (child && !child.killed) child.kill();
  server.close();
}
