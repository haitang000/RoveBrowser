// Exercise painted split viewports with two real WebView2 controllers.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { access, mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { checkSplitResize } from './native-split-checks.mjs';

const profile = await mkdtemp(join(tmpdir(), 'rove-split-'));
const port = Number(process.env.ERA_SMOKE_PORT || 19876);
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-split');
await mkdir(output, { recursive: true });
const hits = {};
const server = createServer((req, res) => {
  hits[req.url] = (hits[req.url] || 0) + 1;
  const right = req.url === '/right';
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(`<!doctype html><title>${right ? 'Right' : 'Left'} split fixture</title>
    <style>html{background:${right ? '#385743' : '#164e63'};color:white;font:24px system-ui}
    body{margin:32px}h1{font-size:32px}</style>
    <h1>${right ? 'Right' : 'Left'} live page</h1><p>Viewport: <b id="width"></b>px</p>
    <p>Frames: <b id="frames"></b></p><script>let frame=0;function tick(){
    document.querySelector('#width').textContent=innerWidth;
    document.querySelector('#frames').textContent=++frame;requestAnimationFrame(tick)}tick();</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
async function waitFor(fn, label) {
  for (let i = 0; i < 150; i++) {
    try { const result = await fn(); if (result) return result; } catch {}
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  throw Error('Timed out: ' + label);
}
let child, uiBrowser, pageBrowser;
try {
  child = spawn(resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe'), [], {
    env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) },
    windowsHide: true, stdio: 'pipe',
  });
  child.stderr.on('data', data => process.stderr.write(data));
  uiBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'UI debugger');
  const ui = await waitFor(() => uiBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.waitForSelector('.sidebar');
  const send = command => ui.evaluate(command => window.ipc.postMessage(JSON.stringify(command)), command);
  await send({ type: 'navigate', url: base + '/left' });
  pageBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'page debugger');
  const site = path => waitFor(() => pageBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === base + path), path);
  const left = await site('/left'); await left.waitForSelector('h1');
  const leftId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'new', url: base + '/right' });
  const right = await site('/right'); await right.waitForSelector('h1');
  const rightId = await ui.getByRole('tab', { selected: true }).getAttribute('data-tab-id');
  await send({ type: 'split', id: leftId, other: rightId });
  await ui.locator('.split-divider').waitFor();
  if (process.env.ERA_SMOKE_REVIEW === '1') {
    await ui.evaluate(() => {
      window.__nativeSplitEvents = [];
      const receive = window.__ERA_RECEIVE__;
      window.__ERA_RECEIVE__ = event => {
        if (event.type === 'splitResize') window.__nativeSplitEvents.push(event);
        receive(event);
      };
    });
    console.log(JSON.stringify({ reviewReady: true, pid: child.pid, port, output }));
    for (let i = 0; i < 300; i++) {
      if (await access(join(output, 'resume')).then(() => true, () => false)) break;
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    console.log(JSON.stringify({ nativeMouseEvents: await ui.evaluate(() => window.__nativeSplitEvents) }));
  }
  const metrics = await checkSplitResize(ui, [left, right], { waitFor, capturePaints: true });
  const report = { passed: true, profile, hits, ...metrics };
  await mkdir(output, { recursive: true });
  await writeFile(join(output, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} finally {
  await pageBrowser?.close(); await uiBrowser?.close(); child?.kill(); server.close();
}
