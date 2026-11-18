// Per-site zoom, mute, the playing-sound indicator and the two-finger
// back/forward gesture opt-in, against real WebView2 pages. Two host names
// (127.0.0.1 and localhost) serve the same fixture so per-site zoom is real.
// The swipe gesture itself needs a precision touchpad and is not synthesized
// here. Wheel checks use Windows mouse input; menu commands use the UI IPC.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { execFile, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';
import { promisify } from 'node:util';

const run = promisify(execFile);

function toneWav() {
  const rate = 8000, seconds = 1, samples = rate * seconds;
  const buffer = Buffer.alloc(44 + samples * 2);
  buffer.write('RIFF', 0);
  buffer.writeUInt32LE(36 + samples * 2, 4);
  buffer.write('WAVE', 8);
  buffer.write('fmt ', 12);
  buffer.writeUInt32LE(16, 16);
  buffer.writeUInt16LE(1, 20);
  buffer.writeUInt16LE(1, 22);
  buffer.writeUInt32LE(rate, 24);
  buffer.writeUInt32LE(rate * 2, 28);
  buffer.writeUInt16LE(2, 32);
  buffer.writeUInt16LE(16, 34);
  buffer.write('data', 36);
  buffer.writeUInt32LE(samples * 2, 40);
  for (let i = 0; i < samples; i++)
    buffer.writeInt16LE(Math.round(Math.sin((i / rate) * 2 * Math.PI * 440) * 8000), 44 + i * 2);
  return buffer;
}
const server = createServer((req, res) => {
  if (req.url === '/tone.wav') {
    res.setHeader('Content-Type', 'audio/wav');
    res.end(toneWav());
    return;
  }
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  const name = req.url === '/b' ? 'B' : 'A';
  res.end(`<!doctype html><title>${name} fixture</title>
<style>html{background:#164e63;color:white;font:16px/1.6 sans-serif}body{min-height:200vh}</style>
<h1>${name} site controls fixture</h1>
<audio id="music" src="/tone.wav" loop></audio>
<button id="play" onclick="document.getElementById('music').play()">play</button>
<button id="stop" onclick="document.getElementById('music').pause()">stop</button>`);
});
await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen));

const hostPort = server.address().port;
const siteA = `http://127.0.0.1:${hostPort}/a`;
const sameSite = `http://127.0.0.1:${hostPort}/b`;
const freshSameSite = `http://127.0.0.1:${hostPort}/c`;
const wheelDestination = `http://localhost:${hostPort}/wheel`;
const siteB = `http://localhost:${hostPort}/b`;
const profile = await mkdtemp(join(tmpdir(), 'rove-site-controls-smoke-'));
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-site-controls');
const port = Number(process.env.ERA_SMOKE_PORT || 19786);
await mkdir(output, { recursive: true });

let child, uiBrowser, pageBrowser, ui, popup;
const checks = [];
async function waitFor(fn, label) {
  for (let i = 0; i < 200; i++) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  throw Error('Timed out: ' + label);
}
async function send(command) {
  await ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command);
}
async function pageInput(action, xFrac = 0.62) {
  await run('powershell.exe', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/real-keys.ps1'),
    '-ProcessId', String(child.pid), '-Action', action, '-xFrac', String(xFrac),
  ]);
}
async function pageAt(path) {
  return waitFor(async () => {
    try {
      const found = pageBrowser
        ?.contexts()
        .flatMap((context) => context.pages())
        .find((candidate) => candidate.url() === path);
      if (found) return found;
    } catch {}
    // A page destroyed while the tab showed an internal page can leave the
    // CDP connection with no targets; reconnect to the new renderer endpoint.
    try {
      pageBrowser = await chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`);
    } catch {}
    return undefined;
  }, 'page ' + path);
}
const width = (renderer) => renderer.evaluate(() => innerWidth);
async function widthEventually(renderer, expected, label) {
  await waitFor(
    async () => Math.abs((await width(renderer)) - expected) <= 6,
    `${label}: width ${expected} (got ${await width(renderer)})`,
  );
}
async function events(type) {
  return ui.evaluate((wanted) => window.__events.splice(0).filter((event) => event.type === wanted), type);
}
async function lastState() {
  return ui.evaluate(() => window.__lastState);
}
async function launch() {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true,
    stdio: 'pipe',
  });
  child.stderr.on('data', (data) => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  ui = await waitFor(
    async () =>
      uiBrowser
        .contexts()
        .flatMap((context) => context.pages())
        .find((page) => page.url() === 'https://era.ui/index.html'),
    'UI page',
  );
  await ui.waitForSelector('.sidebar');
  await ui.evaluate(() =>
    window.ipc.postMessage(
      JSON.stringify({
        type: 'settingsUpdate',
        settings: { ...window.__ERA_INITIAL_STATE__.settings, askBeforeQuit: false },
      }),
    ),
  );
  popup = await waitFor(() => ui.context().pages().find((page) => page.url().endsWith('?popup')), 'popup renderer');
  // Record the zoom capsule's CSS entrance/exit animations from the trusted
  // floating layer, where the real popup WebView renders them.
  await popup.evaluate(() => {
    window.__toastAnimations = [];
    document.addEventListener(
      'animationstart',
      (event) => {
        const target = event.target;
        if (
          target instanceof Element &&
          (target.classList.contains('status-toast') ||
            target.classList.contains('status-toast-surface'))
        )
          window.__toastAnimations.push(event.animationName);
      },
      true,
    );
  });
  await ui.evaluate(() => {
    window.__events = [];
    window.__lastState = window.__ERA_INITIAL_STATE__;
    const previous = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.state) window.__lastState = event.state;
      if (['zoom', 'zoomNotice', 'audio', 'state'].includes(event.type)) {
        window.__events.push({
          type: event.type,
          id: event.id,
          factor: event.factor,
          playing: event.playing,
        });
        if (window.__events.length > 120) window.__events.shift();
      }
      previous?.(event);
    };
  });
  await send({ type: 'navigate', url: siteA });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const first = await pageAt(siteA);
  await first.waitForSelector('h1');
  return first;
}
async function stop() {
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  const exited = new Promise((done) => child.once('exit', done));
  child.kill();
  await Promise.race([exited, new Promise((done) => setTimeout(done, 4000))]);
  pageBrowser = uiBrowser = undefined;
  child = undefined;
  // Give the profile copy and the CDP endpoint a beat before relaunching.
  await new Promise((done) => setTimeout(done, 700));
}

try {
  const first = await launch();
  const baseWidth = await width(first);
  const activeId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  assert.ok(activeId, 'active tab id');

  // Zigzag's View › Zoom ladder, on WebView2's own zoom factor.
  await send({ type: 'zoom', action: 'in' });
  await widthEventually(first, Math.round(baseWidth / 1.1), 'zoom in to 110%');
  const notices = await events('zoomNotice');
  assert.ok(notices.some((event) => Math.abs(event.factor - 1.1) < 0.001), 'zoom toast reports 110%');
  checks.push('Ctrl+=/缩放命令把页面放大到 110% 并显示百分比提示');
  await send({ type: 'zoom', action: 'in' });
  await widthEventually(first, Math.round(baseWidth / 1.25), 'zoom in to 125%');
  await send({ type: 'zoom', action: 'out' });
  await widthEventually(first, Math.round(baseWidth / 1.1), 'zoom out to 110%');
  await send({ type: 'zoom', action: 'reset' });
  await widthEventually(first, baseWidth, 'reset to 100%');
  checks.push('缩小回到上一档，重置回到 100%');

  await send({ type: 'new', url: sameSite });
  const sibling = await pageAt(sameSite);
  await sibling.waitForSelector('h1');
  await ui.locator('[role=tab][aria-selected=true]').filter({ hasText: 'B fixture' }).waitFor();
  const siblingId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  assert.notEqual(siblingId, activeId);
  await widthEventually(sibling, baseWidth, 'same-site tab starts at 100%');
  await events('zoomNotice');
  await pageInput('ctrl-wheel-up');
  await widthEventually(sibling, Math.round(baseWidth / 1.1), 'Ctrl+wheel up zooms the current tab to 110%');
  await waitFor(async () => {
    const notices = await events('zoomNotice');
    return notices.some((event) => event.id === siblingId && Math.abs(event.factor - 1.1) < 0.001);
  }, 'Ctrl+wheel zoom notice for the target tab');
  await pageInput('ctrl-wheel-down');
  await widthEventually(sibling, baseWidth, 'Ctrl+wheel down zooms the current tab back to 100%');
  await pageInput('wheel-down');
  await waitFor(() => sibling.evaluate(() => scrollY > 0), 'wheel without Ctrl scrolls the page');
  await widthEventually(sibling, baseWidth, 'ordinary scrolling keeps zoom at 100%');
  await pageInput('ctrl-wheel-up');
  await waitFor(async () => (await lastState()).tabs.find((tab) => tab.id === siblingId)?.zoom === 1.1, 'tab stores wheel zoom');
  assert.equal((await lastState()).siteZoom?.['127.0.0.1'], undefined, 'wheel never writes a site-wide setting');

  await send({ type: 'activate', id: activeId });
  await ui.locator(`[data-tab-id="${activeId}"][aria-selected="true"]`).waitFor();
  await widthEventually(first, baseWidth, 'already open tab on the same site keeps 100%');
  await send({ type: 'new', url: freshSameSite });
  const fresh = await pageAt(freshSameSite);
  await fresh.waitForSelector('h1');
  await widthEventually(fresh, baseWidth, 'newly opened tab on the same site keeps 100%');
  await send({ type: 'close' });
  await send({ type: 'activate', id: siblingId });
  await ui.locator(`[data-tab-id="${siblingId}"][aria-selected="true"]`).waitFor();
  await widthEventually(sibling, Math.round(baseWidth / 1.1), 'switching back keeps the tab zoom');
  await send({ type: 'navigate', url: wheelDestination });
  const wheelPage = await pageAt(wheelDestination);
  await wheelPage.waitForSelector('h1');
  await widthEventually(wheelPage, Math.round(baseWidth / 1.1), 'tab zoom survives cross-site navigation');
  await Promise.all([wheelPage.waitForEvent('load'), send({ type: 'reload' })]);
  await widthEventually(wheelPage, Math.round(baseWidth / 1.1), 'tab zoom survives reload');
  await pageInput('ctrl-0');
  await widthEventually(wheelPage, baseWidth, 'Ctrl+0 resets the current tab');
  await pageInput('ctrl-wheel-up');
  await widthEventually(wheelPage, Math.round(baseWidth / 1.1), 'current tab zooms again after Ctrl+0');
  await send({ type: 'activate', id: activeId });
  await ui.locator(`[data-tab-id="${activeId}"][aria-selected="true"]`).waitFor();
  checks.push('真实 Ctrl+滚轮向上放大、向下缩小并提示百分比，普通滚轮继续滚动网页');
  checks.push('已有和新开的同站点标签保持原比例，滚轮缩放随当前标签切换、跨站导航和刷新保留');

  await send({ type: 'split', id: activeId, other: siblingId });
  await ui.locator('.split-divider').waitFor();
  const [leftWidth, rightWidth] = await ui.locator('.page-pane > .webview-slot').evaluateAll((slots) =>
    slots.map((slot) => slot.getBoundingClientRect().width),
  );
  await widthEventually(first, leftWidth, 'left split pane stays at 100%');
  await widthEventually(wheelPage, Math.round(rightWidth / 1.1), 'right split pane starts at its tab zoom');
  await pageInput('ctrl-wheel-up', 0.83);
  await widthEventually(wheelPage, Math.round(rightWidth / 1.25), 'Ctrl+wheel zooms the right split pane');
  await widthEventually(first, leftWidth, 'the left split pane keeps its zoom');
  await pageInput('ctrl-wheel-down', 0.83);
  await widthEventually(wheelPage, Math.round(rightWidth / 1.1), 'right split pane returns to 110%');
  await send({ type: 'unsplit' });
  await ui.locator('.split-divider').waitFor({ state: 'hidden' });
  await widthEventually(first, baseWidth, 'primary tab regains the full-width viewport');
  checks.push('分屏只缩放滚轮所在窗格，Ctrl+0 将当前标签恢复至 100%');

  // Per-site: 150% for 127.0.0.1, 100% for localhost, restored when returning.
  for (let step = 0; step < 3; step++) {
    await send({ type: 'zoom', action: 'in' });
    await new Promise((done) => setTimeout(done, 250));
  }
  await widthEventually(first, Math.round(baseWidth / 1.5), '150% on 127.0.0.1');
  checks.push('按站点保存 150%：127.0.0.1 记入会话的 siteZoom');
  await send({ type: 'navigate', url: siteB });
  const second = await pageAt(siteB);
  await second.waitForSelector('h1');
  await widthEventually(second, baseWidth, 'localhost keeps 100%');
  const zoomState = await lastState();
  assert.equal(zoomState.siteZoom?.['127.0.0.1'], 1.5, 'session stores per-site zoom');
  assert.equal(zoomState.siteZoom?.['localhost'], undefined, 'localhost has no zoom entry');
  checks.push('localhost 同端口仍为 100%，缩放按站点而非按端口');
  await send({ type: 'navigate', url: siteA });
  await waitFor(async () => {
    const pages = pageBrowser.contexts().flatMap((context) => context.pages());
    return pages.some((candidate) => candidate.url() === siteA);
  }, 'return to 127.0.0.1');
  const back = await pageAt(siteA);
  await back.waitForSelector('h1');
  await widthEventually(back, Math.round(baseWidth / 1.5), 'returning restores 150%');
  checks.push('回到 127.0.0.1 时恢复该站的 150%');

  // Mute, and the speaker that a muted tab shows even when silent.
  await send({ type: 'mute', id: activeId, muted: true });
  await waitFor(async () => (await lastState()).tabs.find((tab) => tab.id === activeId)?.muted === true, 'state tab.muted');
  await expectCount('.tab-row.selected .tab-audio.muted', 1, 'muted tab speaker');
  checks.push('静音命令写入会话并在标签行显示静音喇叭');
  await send({ type: 'mute', id: activeId, muted: false });
  await waitFor(async () => (await lastState()).tabs.find((tab) => tab.id === activeId)?.muted === false, 'state tab unmuted');

  // Sound: a trusted click on the page starts the fixture's looped tone.
  const playing = await pageAt(siteA);
  await playing.click('#play');
  await waitFor(async () => {
    const audio = await events('audio');
    return audio.some((event) => event.playing === true);
  }, 'audio playing event');
  await expectCount('.tab-row.selected .tab-audio', 1, 'playing tab speaker');
  const speaker = ui.locator('.tab-row.selected .tab-audio');
  assert.equal(await speaker.evaluate((element) => element.classList.contains('muted')), false);
  checks.push('网页开始播放声音时标签行出现喇叭（WebView2 IsDocumentPlayingAudio）');
  await send({ type: 'mute', id: activeId, muted: true });
  await expectCount('.tab-row.selected .tab-audio.muted', 1, 'muted while playing');
  await send({ type: 'mute', id: activeId, muted: false });
  await expectCount('.tab-row.selected .tab-audio:not(.muted)', 1, 'unmuted while playing');
  checks.push('播放中可静音/取消静音，喇叭状态随会话更新');
  await playing.evaluate(() => document.getElementById('music').pause());
  await waitFor(async () => {
    const audio = await events('audio');
    return audio.some((event) => event.playing === false);
  }, 'audio stopped event');
  await expectCount('.tab-row.selected .tab-audio', 0, 'speaker hides when sound stops');
  checks.push('声音停止后喇叭消失');

  // The zoom percentage capsule: drops in from above and slides up on exit.
  await popup.evaluate(() => {
    window.__toastAnimations = [];
  });
  await send({ type: 'zoom', action: 'reset' });
  await waitFor(async () => (await popup.locator('.status-toast').count()) === 1, 'zoom toast appears');
  assert.match(await popup.locator('.status-toast').innerText(), /100%/);
  await waitFor(
    async () => popup.evaluate(() => window.__toastAnimations.includes('zoom-toast-drop-in')),
    'toast drop-in animation',
  );
  await waitFor(async () => (await popup.locator('.status-toast').count()) === 0, 'zoom toast hides after 1.2s');
  const toastAnimations = await popup.evaluate(() => window.__toastAnimations);
  assert.ok(toastAnimations.includes('zoom-toast-fade-in'), 'toast fades in');
  assert.ok(toastAnimations.includes('zoom-toast-slide-out'), 'toast slides out');
  checks.push('缩放胶囊入场下拉、退场上滑，并在 1.2 秒后自动收起');

  // Internal pages have no controller to zoom or mute.
  await events('zoomNotice');
  await send({ type: 'navigate', url: 'rove://settings' });
  await ui.locator('.settings-page').waitFor();
  await send({ type: 'zoom', action: 'in' });
  await new Promise((done) => setTimeout(done, 400));
  assert.equal((await events('zoomNotice')).length, 0, 'internal page zoom is a no-op');
  await ui.locator('.settings-page').waitFor();
  checks.push('内部设置页不缩放也不报错');

  // Persistence across a restart: 125% for 127.0.0.1, and a muted tab.
  await send({ type: 'navigate', url: siteA });
  await (await pageAt(siteA)).waitForSelector('h1');
  await send({ type: 'zoom', action: 'reset' });
  await new Promise((done) => setTimeout(done, 300));
  await send({ type: 'zoom', action: 'in' });
  await new Promise((done) => setTimeout(done, 300));
  await send({ type: 'zoom', action: 'in' });
  await new Promise((done) => setTimeout(done, 300));
  await send({ type: 'mute', id: activeId, muted: true });
  await new Promise((done) => setTimeout(done, 400));
  await stop();

  const restarted = await launch();
  await widthEventually(restarted, Math.round(baseWidth / 1.25), 'zoom restored after restart');
  const restored = await lastState();
  assert.equal(restored.siteZoom?.['127.0.0.1'], 1.25, 'site zoom survives restart');
  assert.equal(restored.tabs.find((tab) => tab.id === activeId)?.muted, true, 'mute survives restart');
  await expectCount('.tab-row.selected .tab-audio.muted', 1, 'muted speaker after restart');
  checks.push('重启后按站点缩放 125% 与静音状态从 SQLite 会话恢复');
  await send({ type: 'activate', id: siblingId });
  const restoredWheelPage = await pageAt(wheelDestination);
  await restoredWheelPage.waitForSelector('h1');
  await widthEventually(restoredWheelPage, Math.round(baseWidth / 1.1), 'tab zoom restored after restart');
  checks.push('重启后从会话恢复滚轮设置的标签缩放');

  const result = {
    passed: true,
    profile,
    checks,
    manual: ['双指滑动前进后退由 WebView2 SetIsSwipeNavigationEnabled 处理，需要真实精密触控板验证'],
  };
  await writeFile(join(output, 'native-site-controls-smoke.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await popup?.screenshot({ path: join(output, 'failure-popup.png') }).catch(() => {});
  await ui?.screenshot({ path: join(output, 'failure-ui.png') }).catch(() => {});
  const state = await Promise.all(
    [ui, popup].map((renderer) =>
      renderer
        ?.evaluate(() => ({
          url: location.href,
          width: innerWidth,
          audio: Array.from(document.querySelectorAll('.tab-audio')).map((element) => element.outerHTML.slice(0, 200)),
          events: window.__events || [],
          lastState: window.__lastState,
        }))
        .catch(() => null),
    ),
  );
  await writeFile(join(output, 'failure.json'), JSON.stringify({ checks, error: String(error), state }, null, 2));
  throw error;
} finally {
  await pageBrowser?.close().catch(() => {});
  await uiBrowser?.close().catch(() => {});
  if (child && !child.killed) child.kill();
  server.close();
}

async function expectCount(selector, expected, label) {
  await waitFor(
    async () => (await ui.locator(selector).count()) === expected,
    `${label || selector}: ${expected}`,
  );
}
