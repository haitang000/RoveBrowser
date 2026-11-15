// Real WebView2 split entry, including deferred presentation and interruption.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { access, mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-split-entry-'));
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-split-entry');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19886);
const events = [];
const checks = [];
const server = createServer((req, res) => {
  const right = req.url === '/right';
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>${right ? 'Right' : 'Left'} entry fixture</title>
    <style>html{background:${right ? '#172638' : '#fafaff'};color:${right ? '#edf4ff' : '#172235'};font:18px system-ui}
    body{margin:0}header{padding:28px;background:${right ? '#237a69' : '#c03e72'};color:white}
    main{padding:24px;display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:20px}
    article{padding:20px;border:2px solid #778fae;border-radius:10px}b{font-size:32px}</style>
    <header><h1>${right ? 'Right' : 'Left'} real webpage</h1></header><main>
    ${Array.from({ length: 12 }, (_, i) => `<article><b>${String(i + 1).padStart(2, '0')}</b><p>Sharp outline, blurred content.</p></article>`).join('')}</main>
    <script>window.entryResizes=[];addEventListener('resize',()=>entryResizes.push(innerWidth));</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function waitFor(fn, label) {
  for (let i = 0; i < 160; i++) {
    try { const value = await fn(); if (value) return value; } catch {}
    await pause(40);
  }
  throw Error('Timed out: ' + label);
}
const since = (mark, phase) => events.slice(mark).filter(e => e.phase === phase);
let child, uiBrowser, pageBrowser, stderr = '';
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port), ERA_RESIZE_TRACE: '1' },
    windowsHide: true, stdio: 'pipe',
  });
  let partial = '';
  child.stderr.on('data', data => {
    const text = data.toString(); stderr += text; partial += text;
    const lines = partial.split(/\r?\n/); partial = lines.pop();
    for (const line of lines) {
      if (line.startsWith('Rove resize: ')) events.push(JSON.parse(line.slice('Rove resize: '.length)));
    }
  });
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar');
  const send = c => ui.evaluate(c => window.ipc.postMessage(JSON.stringify(c)), c);
  await send({ type: 'navigate', url: base + '/left' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'webpage debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const left = await site('/left'); await left.waitForSelector('h1');
  await waitFor(async () => (await ui.getByRole('tab', { selected: true }).getAttribute('title')) === base + '/left', 'left tab state synchronized');
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/right' });
  const right = await site('/right'); await right.waitForSelector('h1');
  await waitFor(async () => (await ui.getByRole('tab', { selected: true }).getAttribute('title')) === base + '/right', 'right tab state synchronized');
  const rightId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'activate', id: leftId });
  await waitFor(async () => (await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id')) === leftId, 'primary activation');
  if (process.env.ERA_SMOKE_REVIEW === '1') {
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, port, leftId, rightId, output }));
    for (let i = 0; i < 300; i++) {
      if (await access(join(output, 'resume')).then(() => true, () => false)) break;
      await pause(1000);
    }
  }
  const split = () => send({ type: 'split', id: leftId, other: rightId });
  const unsplit = async () => {
    await send({ type: 'unsplit' });
    await ui.locator('.split-divider').waitFor({ state: 'detached' });
  };
  const finished = mark => waitFor(() => since(mark, 'restored').length >= 2, 'both entry previews released');
  const geometry = async () => {
    const slots = await ui.locator('.webview-slot').evaluateAll(nodes => nodes.map(node => {
      const r = node.getBoundingClientRect(); return { width: r.width, height: r.height };
    }));
    const pages = await Promise.all([left, right].map(page => page.evaluate(() => ({ width: innerWidth, height: innerHeight, resizes: entryResizes }))));
    assert.equal(slots.length, 2);
    pages.forEach((page, i) => {
      assert.ok(Math.abs(page.width - slots[i].width) <= 1, 'viewport must match final slot');
      assert.ok(Math.abs(page.height - slots[i].height) <= 1, 'viewport height must match final slot');
    });
    return pages;
  };

  let mark = events.length;
  await split(); await finished(mark);
  const entry = since(mark, 'split-entry');
  assert.equal(entry.length, 1);
  assert.equal(entry[0].edge, -1);
  const move = since(mark, 'split-move');
  assert.equal(move.length, 1);
  assert.ok(move[0].from[2] > move[0].to[2], 'original must move from its unsplit width');
  const frames = since(mark, 'split-move-frame');
  assert.ok(frames.some(e => e.bounds[2] < move[0].from[2] && e.bounds[2] > move[0].to[2]), 'original width must animate continuously');
  const sequence = events.slice(mark);
  const arrived = sequence.findIndex(e => e.phase === 'split-move-done');
  const appeared = sequence.findIndex(e => e.phase === 'split-entry');
  assert.ok(arrived >= 0 && appeared > arrived, 'incoming starts only after original arrives');
  assert.ok(sequence.slice(0, arrived).filter(e => e.phase === 'settling').every(e => e.opacity === 0), 'incoming card remains hidden throughout original movement');
  const samples = since(mark, 'settling');
  assert.ok(samples.some(e => e.scale < 0.99 && Math.abs(e.depth[0]) > 0.01), 'entry must visibly start lifted and tilted');
  assert.ok(samples.some(e => e.blend > 0 && e.blend < 1), 'paint must crossfade');
  assert.ok(samples.some(e => e.scale > 0.97 && e.scale < 1), 'entry must scale back nonlinearly');
  const viewports = await geometry();
  assert.ok(viewports.every(page => page.resizes.length <= 1), 'animation must not repeatedly reflow either webpage');
  assert.ok(samples.some(e => e.opacity > 0 && e.opacity < 1 && e.slide > 0), 'incoming must slide and fade into place');
  checks.push('original moves continuously first; incoming then slides and fades; each viewport commits once');

  mark = events.length;
  const layout = await ui.evaluate(() => {
    const slots = [...document.querySelectorAll('.webview-slot')].map(node => {
      const r = node.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height };
    });
    return { type: 'layout', primary: slots[0], secondary: slots[1], viewport: { width: innerWidth, height: innerHeight } };
  });
  for (let i = 0; i < 5; i++) await send(layout);
  await pause(200);
  assert.equal(since(mark, 'split-entry').length, 0, 'repeated layout cannot replay entry');
  checks.push('ordinary layout reports do not replay entry');

  await unsplit();
  await send({ type: 'overlay', visible: true, command: true });
  mark = events.length;
  await split(); await ui.locator('.split-divider').waitFor(); await pause(100);
  assert.equal(since(mark, 'split-entry').length, 0, 'wait for overlay dismissal');
  assert.equal(since(mark, 'split-move').length, 0, 'original also waits for overlay dismissal');
  await send({ type: 'overlay', visible: false }); await finished(mark);
  assert.equal(since(mark, 'split-entry').length, 1);
  await geometry();
  checks.push('entry deferred until command overlay closes');

  await unsplit(); mark = events.length;
  await split();
  await waitFor(() => since(mark, 'split-move').length === 1, 'original starts moving');
  await unsplit();
  await finished(mark);
  assert.equal(since(mark, 'split-entry').length, 0, 'cancellation cannot reveal the staged pane');
  assert.equal(await ui.locator('.webview-slot').count(), 1);
  const width = await ui.locator('.webview-slot').evaluate(node => node.getBoundingClientRect().width);
  await waitFor(async () => Math.abs(await left.evaluate(() => innerWidth) - width) <= 1, 'unsplit viewport restored');
  checks.push('unsplit interrupts and releases both previews');

  await send({ type: 'resizeAppearance', start: [80, 120, 170], end: [110, 90, 160], angle: 135, dark: false, reduceMotion: true });
  mark = events.length;
  await split(); await finished(mark);
  assert.equal(since(mark, 'split-entry').length, 1);
  assert.ok(since(mark, 'settling').every(e => e.scale === 1 && e.depth.every(value => value === 0)), 'reduced motion uses no perspective or scale');
  await geometry();
  checks.push('reduced motion only fades and reaches final layout');

  await writeFile(join(output, 'result.json'), JSON.stringify({ passed: true, checks, profile, viewports, events }, null, 2));
  console.log(JSON.stringify({ passed: true, checks, output }));
} finally {
  try {
    await mkdir(output, { recursive: true });
    await writeFile(join(output, 'trace.log'), stderr);
  } finally {
    await pageBrowser?.close(); await uiBrowser?.close(); child?.kill(); server.close();
  }
}
