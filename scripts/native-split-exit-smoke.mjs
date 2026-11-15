// Real WebView2 split exits: both sides, sequential recovery and interruption.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { access, mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { chromium } from '@playwright/test';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-split-exit-'));
createLegacyNativeProfile(profile);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-split-exit');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19926);
const events = [], checks = [];
const server = createServer((req, res) => {
  const right = req.url === '/right';
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>${right ? 'Right' : 'Left'} exit fixture</title>
    <style>html{background:${right ? '#172638' : '#fafaff'};color:${right ? '#edf4ff' : '#172235'};font:18px system-ui}
    body{margin:0}header{padding:28px;background:${right ? '#237a69' : '#c03e72'};color:white}
    main{padding:24px;display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:20px}
    article{padding:20px;border:2px solid #778fae;border-radius:10px}b{font-size:32px}</style>
    <header><h1>${right ? 'Right' : 'Left'} real webpage</h1></header><main>
    ${Array.from({ length: 12 }, (_, i) => `<article><b>${String(i + 1).padStart(2, '0')}</b><p>Sharp outline, blurred content.</p></article>`).join('')}</main>
    <script>window.exitResizes=[];addEventListener('resize',()=>exitResizes.push(innerWidth));</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
async function waitFor(fn, label) {
  for (let i = 0; i < 180; i++) {
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
    for (const line of lines) if (line.startsWith('Rove resize: ')) events.push(JSON.parse(line.slice(13)));
  });
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar');
  const send = c => ui.evaluate(c => window.ipc.postMessage(JSON.stringify(c)), c);
  await send({ type: 'navigate', url: base + '/left' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'webpage debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const selected = async url => {
    await waitFor(async () => (await ui.getByRole('tab', { selected: true }).getAttribute('title')) === url, 'tab state');
    return ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  };
  let left = await site('/left'); await left.waitForSelector('h1');
  let leftId = await selected(base + '/left');
  const addRight = async () => {
    await send({ type: 'new', url: base + '/right' });
    const page = await site('/right'); await page.waitForSelector('h1');
    return { page, id: await selected(base + '/right') };
  };
  let { page: right, id: rightId } = await addRight();
  const activate = async id => {
    await send({ type: 'activate', id });
    await waitFor(async () => (await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id')) === id, 'activation');
  };
  const split = async () => {
    await activate(leftId);
    const mark = events.length;
    await send({ type: 'split', id: leftId, other: rightId });
    await waitFor(() => since(mark, 'restored').length >= 2, 'entry finishes');
    await ui.locator('.split-divider').waitFor();
  };
  const verifyExit = async (command, side, keeper, departing) => {
    await keeper.evaluate(() => { exitResizes = []; });
    await departing.evaluate(() => { exitResizes = []; });
    const mark = events.length;
    await send(command);
    await waitFor(() => since(mark, 'split-exit-done').length === 1 && since(mark, 'split-move-done').length === 1
      && since(mark, 'restored').length >= 2, 'exit and recovery finish');
    const sequence = events.slice(mark);
    assert.equal(since(mark, 'split-exit').length, 1);
    assert.equal(since(mark, 'split-exit')[0].side, side);
    assert.ok(sequence.findIndex(e => e.phase === 'split-exit-done') < sequence.findIndex(e => e.phase === 'split-move'), 'pane exits before survivor expands');
    const move = since(mark, 'split-move')[0];
    assert.ok(move.from[2] < move.to[2]);
    assert.ok(since(mark, 'split-move-frame').some(e => e.bounds[2] > move.from[2] && e.bounds[2] < move.to[2]), 'recovery has continuous intermediate widths');
    assert.ok(since(mark, 'settling').some(e => e.opacity > 0 && e.opacity < 1 && Math.sign(e.slide) === side), 'exit slides toward the correct edge while fading');
    assert.equal(await ui.locator('.webview-slot').count(), 1);
    const slot = await ui.locator('.webview-slot').evaluate(node => { const r = node.getBoundingClientRect(); return { width: r.width, height: r.height }; });
    const viewport = await keeper.evaluate(() => ({ width: innerWidth, height: innerHeight, resizes: exitResizes }));
    assert.ok(Math.abs(slot.width - viewport.width) <= 1 && Math.abs(slot.height - viewport.height) <= 1, 'final viewport matches full window');
    assert.equal(viewport.resizes.length, 1, 'only one final survivor reflow');
    if (!departing.isClosed()) assert.deepEqual(await departing.evaluate(() => exitResizes), [], 'departing page never reflows');
    return { mark, viewport };
  };
  await split();
  if (process.env.ERA_SMOKE_REVIEW === '1') {
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, port, leftId, rightId, output }));
    for (let i = 0; i < 300; i++) {
      if (await access(join(output, 'resume')).then(() => true, () => false)) break;
      await pause(1000);
    }
  }
  await verifyExit({ type: 'unsplit' }, 1, left, right);
  checks.push('unsplit: right exits, then left expands; survivor reflows once');
  await split();
  await verifyExit({ type: 'close', id: rightId }, 1, left, right);
  checks.push('close right: right exits before destruction, left recovers');
  ({ page: right, id: rightId } = await addRight());
  await split();
  await verifyExit({ type: 'close', id: leftId }, -1, right, left);
  checks.push('close left: mirrored left exit, right moves and expands to full width');

  await send({ type: 'new', url: base + '/left' });
  left = await site('/left'); await left.waitForSelector('h1'); leftId = await selected(base + '/left');
  let mark = events.length;
  await send({ type: 'split', id: leftId, other: rightId });
  await waitFor(() => since(mark, 'split-move').length === 1, 'entry begins');
  const exitMark = events.length;
  await send({ type: 'unsplit' });
  await waitFor(() => since(exitMark, 'split-exit-done').length === 1 && since(exitMark, 'split-move-done').length === 1, 'entry reverses into exit');
  await waitFor(() => since(exitMark, 'restored').length >= 2, 'reversal cleans up');
  assert.equal(since(exitMark, 'split-entry').length, 0, 'cancelled entry must never reveal its partner');
  checks.push('exit during entry preserves presentation and cancels staged reveal');

  await split(); mark = events.length;
  await send({ type: 'unsplit' });
  await waitFor(() => since(mark, 'split-exit').length === 1, 'exit begins');
  await activate(rightId); await pause(700);
  assert.equal(await ui.locator('.webview-slot').count(), 1);
  assert.equal(since(mark, 'split-move').length, 0, 'stale completion cannot move the newly selected tab');
  checks.push('tab switch interrupts exit and ignores its queued completion');

  await split();
  await send({ type: 'resizeAppearance', start: [80, 120, 170], end: [110, 90, 160], angle: 135, dark: false, reduceMotion: true });
  // The appearance setting is read when snapshots are created.
  mark = events.length; await send({ type: 'unsplit' });
  await waitFor(() => since(mark, 'split-exit-done').length === 1 && since(mark, 'restored').length >= 2, 'reduced exit completes');
  assert.ok(since(mark, 'settling').every(e => e.scale === 1 && e.depth.every(value => value === 0) && e.slide === 0 && e.blur === 0), 'reduced motion only crossfades');
  checks.push('reduced motion exits and recovers without depth, movement or blur');
  await writeFile(join(output, 'result.json'), JSON.stringify({ passed: true, checks, events, profile }, null, 2));
  console.log(JSON.stringify({ passed: true, checks, output }));
} finally {
  await writeFile(join(output, 'trace.log'), stderr);
  await pageBrowser?.close(); await uiBrowser?.close(); child?.kill(); server.close();
}
