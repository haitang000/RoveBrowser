// Cached site icons stay sharp. Real WebView2 + host, isolated profiles.
// 1. A page's declared SVG replaces WebView2's small captured bitmap.
// 2. A small icon is cached at its own size, never stretched to 64.
// 3. A large declared icon is shrunk to 64, never kept larger.
// 4. Icons cached by an older build (blurred by that stretch) are fetched once
//    at each site's own resolution.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import assert from 'node:assert/strict';

const profile = await mkdtemp(join(tmpdir(), 'rove-favicon-'));
const legacyProfile = await mkdtemp(join(tmpdir(), 'rove-favicon-legacy-'));
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-favicon');
await mkdir(output, { recursive: true });
const port = Number(process.env.ERA_SMOKE_PORT || 19656);
const exe = resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe');
const checks = [];

// --- fixtures -------------------------------------------------------------
const tiny16 = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAPklEQVR4nGNgGGjACGPI5N35T4rGJ5NUwHqZKHUBE6UGsGATfDxRGati2fy71HcB06gBDCzEhvbgDUSGAQcApmYIhYgGuP4AAAAASUVORK5CYII=', 'base64');
const large128 = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAADQElEQVR4nO2dW27bQBAEKcH/5oGSOzgnTe6QHCg5QQIboKFIokRyXz3TVf+GV9O1vaQISacpOb/fvvwt+fv5x6/TlJg0L640aFcxwr6I3oFnFSLUotVCzyBDiIVGCT6iCLILjBp6NBnkFpUteHURZBaTPXhVEYYvwi14NRHOI/+5e/gKMzg5vmhV5gFt0L0BCF9rNt2MI3jNNujSAISvO7PmAhC+9uya1QzBxzgSmjQA4ceZaXUBCL8dLWZbVQDCb0/tGVcTgPD7UXPWVQQg/P7UmnmxAIQ/jhqzLxKA8MdTmsHQp4EwnsMCsPt1KMnikACEr8fRTHYLQPi6HMmGawBzdgnA7tdnb0abBSD8OOzJ6mUS5fX7zykbf759ndTY1ADs/nhszeypAIQfly3ZcRdgzkMB2P3xeZYhDWDOqgDs/jw8ypIGMOeuAOz+fKxlSgOYgwDm3AhA/eflXrY0gDn/CcDuz891xrJPA5WfuL0melLJEWDOpwDUvw+XWdMA5iCAOQhgzocAnP9+LJnTAOYggDkIYA4CmIMA5py5A/DlPXsawBwEMAcBzEEAcxDAHAQwBwHMQQBzEMAcBDAHAcxBAHMQwBwEMAcBzDmP/v16GMd79jSAOQhgDgKYgwDmIIA5HwJwJ+DHkjkNYA4CmIMA5nwKwHWAD5dZ0wDmIIA5Nw+CVD4smunLGJV+Nu76qKcBzLkRgIvBvNzLlgYwBwHMuSsAx0A+1jKlAcxZFYAWyMOjLGkAcx4KQAvE51mGNIA5TwWgBeKyJbtNDYAE8diaGUeAObs+FqbypBDqNfauBuAo0GdvRhwB5uwWgBbQ5Ug2hxoACfQ4msnhIwAJdCjJgmsAc4oEoAXGU5pBcQMgwThqzL7KEYAE/ak182rXAEjQj5qzrnoRiATtqT3j6ncBSNCOFrNtchuIBHFm2vxLInmCqL2Zmr8RRBtoz67LO4FIoDuz7t8TzJGgtVm6PwugDbRmM/SbwmmD8Zti6NNA2mAaPgOZ3wpwa4NZ5HcaJBbhJMIsEvyC1GIyizCLBb8guagsMsyioV8iv8CIIswBgl8Is1B1GeZAoV8SctEKQsxBA78mxYtoKcacJOg1/gFzeTfnmSKpIAAAAABJRU5ErkJggg==', 'base64');
// Exactly what an older build cached: a 16px mark stretched to 64, blurred.
const legacy64 = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAACgklEQVR4nO3bwWrbQBAG4H9mR7Fo3FBM7LgQaCGFhvbWU299tNJH60O09BLIwVDVMiFxnUSORrtlFXqLDg5KKzH6Ljqs2NUMsmFnR8BgMBgM7KKmgU+fg3zPv6WSSsolp8KJoEfUl+oTX2ihxbvp++LrF9KH7pOmCfJ8mY4wmpPy3KOa+6oao08IG9Ekc3BZni8zAJudEqCppjH4ALxlcqeBcIgeYbhVAH6QMFT0cucE+NKnBMxBOA0BH4FwjB4JoAURQAFXvvTnTfdJ00DJKqEK40B0WAdPeI0+CSEm4RAhjEunjXEyjGMYxzCOYRzDOIZxDOOktYmYMEoII7m/upZTW3lgWwZsNdRX9aFbCRglhOlzh+mBw+xA8GyvcZ/1KDd3Acu1Il9XyH9X0G3XEiBUB//maA8nswSTsUObLjYVzpbxtbrD+tbjeotuvgEnRwk+vErx8kW7u+efl1pv3q9uKiwuHtzZPoq0NVH8ze+PGJN9Vwd/PGm/fDBZuXqNNv9fGMYxjGMYxzCOYRzDOIZxDOMYxjGMYxjHMI5hHMM4hnEM4xjGMYxjGCdt1u2vtx4X19V9AbNlcc44d1wjrtW5BGzLUNfrz36VsT3jicriZb1GXKt7CdBQH1r8rds/5cFIXKuzb8D61td1+6c+GutcAtSH+riqrRObf4VhHMM4hnEM4xjGMYxjGCdNA4kXvSMfO6xXsfE49t72Cy0IWBHRJsbSdJc0DXDCRVBkdcs51d3XvWqXj8HHZwchY+Fi5wRIIUWAz2K/vQ/VFQfu1QcTnvyGyWVBfSYquydgOp0VeZ5nInLJJZ+zc736ZMZ7ryploapFjOV/P89gMBgM0EF/ADMFIdezVh3JAAAAAElFTkSuQmCC', 'base64');
const vector = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="12" fill="#2f6bd8"/><path d="M18 46 32 18l14 28z" fill="#fff"/></svg>');

function page(iconLinks, title) {
  return `<!doctype html><html><head><meta charset="utf-8"><title>${title}</title>${iconLinks}</head><body><h1>${title}</h1></body></html>`;
}

const requests = { sharp: {}, cheap: {}, large: {} };
// Host icon fetches carry the host's own user agent; WebView2's page requests
// carry the live Edge one, so the two can be told apart.
const fetched = { sharp: {}, cheap: {}, large: {} };
const servers = [];
async function serve(name, routes) {
  const server = createServer((req, res) => {
    const path = (req.url || '/').split('?')[0];
    requests[name][path] = (requests[name][path] || 0) + 1;
    if ((req.headers['user-agent'] || '').includes('Chrome/131.0.0.0')) {
      fetched[name][path] = (fetched[name][path] || 0) + 1;
    }
    const hit = routes[path];
    if (!hit) {
      res.statusCode = 404;
      res.end();
      return;
    }
    res.setHeader('Content-Type', hit.type);
    res.end(hit.body);
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  servers.push(server);
  return `http://127.0.0.1:${server.address().port}`;
}

const sharp = await serve('sharp', {
  '/': { type: 'text/html', body: page('<link rel="icon" type="image/svg+xml" href="/logo.svg">', 'Sharp icon fixture') },
  '/legacy': { type: 'text/html', body: page('<link rel="icon" type="image/svg+xml" href="/logo.svg">', 'Legacy icon fixture') },
  '/second': { type: 'text/html', body: page('<link rel="icon" type="image/svg+xml" href="/logo.svg">', 'Sharp icon fixture two') },
  '/logo.svg': { type: 'image/svg+xml', body: vector },
  '/favicon.ico': { type: 'image/png', body: tiny16 },
});
const cheap = await serve('cheap', {
  '/': { type: 'text/html', body: page('<link rel="icon" type="image/png" href="/small.png">', 'Small icon fixture') },
  '/small.png': { type: 'image/png', body: tiny16 },
  '/favicon.ico': { type: 'image/png', body: tiny16 },
});
const large = await serve('large', {
  '/': { type: 'text/html', body: page('<link rel="icon" sizes="128x128" href="/large.png">', 'Large icon fixture') },
  '/large.png': { type: 'image/png', body: large128 },
  '/favicon.ico': { type: 'image/png', body: tiny16 },
});

// --- native host ----------------------------------------------------------
let child, browser, pageBrowser, ui;
let state;
async function waitFor(fn, label) {
  for (let i = 0; i < 200; i++) {
    try {
      const result = await fn();
      if (result) return result;
    } catch {}
    await new Promise((done) => setTimeout(done, 100));
  }
  throw Error(`Timed out: ${label}`);
}
async function send(command) {
  await ui.evaluate((command) => window.ipc.postMessage(JSON.stringify(command)), command);
}
async function launch(dataDir) {
  child = spawn(exe, [], { env: { ...process.env, ERA_DATA_DIR: dataDir, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: 'pipe' });
  child.stderr.on('data', (data) => process.stderr.write(data));
  await waitFor(async () => {
    const targets = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) }).then((r) => r.json());
    return targets.some((target) => target.url.startsWith('https://era.ui/'));
  }, 'UI renderer');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  ui = await waitFor(() => Promise.resolve(browser.contexts().flatMap((context) => context.pages()).find((page) => page.url() === 'https://era.ui/index.html')), 'UI page');
  await ui.waitForSelector('.sidebar');
  await ui.evaluate(() => {
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.type === 'state') window.__faviconState = event.state;
      receive(event);
    };
  });
  await ui.evaluate(() => window.ipc.postMessage(JSON.stringify({ type: 'settingsUpdate', settings: { ...window.__ERA_INITIAL_STATE__.settings, askBeforeQuit: false } })));
  await send({ type: 'ready' });
}
async function stop() {
  await pageBrowser?.close(); pageBrowser = null;
  const exited = once(child, 'exit');
  await send({ type: 'window', action: 'close' }).catch(() => {});
  await exited;
  await browser.close(); browser = null;
  await waitFor(async () => {
    try {
      const targets = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) }).then((r) => r.json());
      return !targets.some((target) => target.url.startsWith('https://era.ui/'));
    } catch {
      return true;
    }
  }, 'old renderer shutdown');
}

function pngSide(icon) {
  assert.match(icon, /^data:image\/png;base64,/, 'expected a PNG icon');
  const bytes = Buffer.from(icon.slice('data:image/png;base64,'.length), 'base64');
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  return Math.max(bytes.readUInt32BE(16), bytes.readUInt32BE(20));
}
async function iconOfTab(urlPrefix, matches = () => true) {
  const current = await ui.evaluate(() => window.__faviconState);
  const icon = current?.tabs.find((tab) => tab.url.startsWith(urlPrefix))?.favicon;
  return icon && matches(icon) ? icon : undefined;
}
async function openAndWait(url, label, matches) {
  await send({ type: 'new', url });
  return waitFor(() => iconOfTab(url, matches), label);
}

try {
  await launch(profile);
  const isSvg = (icon) => icon.startsWith('data:image/svg+xml;base64,');
  const sharpIcon = await openAndWait(`${sharp}/`, 'declared SVG icon', isSvg);
  assert.equal(sharpIcon, `data:image/svg+xml;base64,${vector.toString('base64')}`);
  checks.push('页面声明的 SVG 图标取代 WebView2 的小位图缓存');

  // A second page of the same site reuses the fetched icon without another request.
  const secondIcon = await openAndWait(`${sharp}/second`, 'second page icon', (icon) => icon === sharpIcon);
  assert.equal(secondIcon, sharpIcon);
  assert.equal(fetched.sharp['/logo.svg'], 1);
  checks.push('同一站点只请求一次图标，其他页面直接复用');

  const smallIcon = await openAndWait(`${cheap}/`, 'small icon', (icon) => pngSide(icon) <= 16);
  assert.equal(pngSide(smallIcon), 16);
  checks.push('16px 小图标按原尺寸缓存，不再放大成 64px 模糊图');

  const bigIcon = await openAndWait(`${large}/`, 'large icon', (icon) => pngSide(icon) === 64);
  assert.equal(pngSide(bigIcon), 64);
  checks.push('128px 大图标缩至 64px，未放大或超出上限');

  await ui.screenshot({ path: join(output, 'icons.png') });
  await stop();

  // Restart with the fixture servers down: the cached icons come from SQLite.
  for (const server of servers) server.close();
  servers.length = 0;
  await launch(profile);
  const reloaded = await waitFor(async () => {
    const current = await ui.evaluate(() => window.__faviconState);
    const sharpTab = current?.tabs.find((tab) => tab.url === `${sharp}/`);
    const cheapTab = current?.tabs.find((tab) => tab.url === `${cheap}/`);
    const largeTab = current?.tabs.find((tab) => tab.url === `${large}/`);
    return sharpTab?.favicon && cheapTab?.favicon && largeTab?.favicon
      ? [sharpTab.favicon, cheapTab.favicon, largeTab.favicon]
      : null;
  }, 'cached icons after restart');
  assert.equal(reloaded[0], sharpIcon);
  assert.equal(pngSide(reloaded[1]), 16);
  assert.equal(pngSide(reloaded[2]), 64);
  checks.push('服务器关闭后重启进程，缓存图标仍原样恢复');
  await ui.screenshot({ path: join(output, 'offline-restart.png') });
  await stop();

  // An older session's blurred 64px icons are fetched again once, at the site's size.
  let DatabaseSync;
  try {
    ({ DatabaseSync } = await import('node:sqlite'));
  } catch {
    checks.push('旧会话图标升级检查已跳过（当前 Node 无 node:sqlite）');
  }
  if (DatabaseSync) {
    const legacySharp = await serve('sharp', {
      '/legacy': { type: 'text/html', body: page('<link rel="icon" type="image/svg+xml" href="/logo.svg">', 'Legacy icon fixture') },
      '/logo.svg': { type: 'image/svg+xml', body: vector },
      '/favicon.ico': { type: 'image/png', body: tiny16 },
    });
    const legacyIcon = `data:image/png;base64,${legacy64.toString('base64')}`;
    const db = new DatabaseSync(join(legacyProfile, 'era.sqlite'));
    db.exec('PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);');
    const oldState = {
      spaces: [{ id: 'personal', name: 'Personal', color: 'sky' }],
      tabs: [{ id: 'legacy-tab', spaceId: 'personal', title: 'Legacy icon fixture', url: `${legacySharp}/legacy`, pinned: false, favicon: legacyIcon }],
      activeSpace: 'personal',
      activeTab: 'legacy-tab',
      favorites: [{ id: 'legacy-fav', title: 'Legacy icon fixture', url: `${legacySharp}/legacy`, favicon: legacyIcon }],
      pins: [],
      closed: [],
      theme: 'light',
      sidebarCollapsed: false,
    };
    db.prepare('INSERT OR REPLACE INTO session(id, json) VALUES(1, ?)').run(JSON.stringify(oldState));
    db.close();

    await launch(legacyProfile);
    const upgraded = await waitFor(async () => {
      const current = await ui.evaluate(() => window.__faviconState);
      const favorite = current?.favorites.find((mark) => mark.title === 'Legacy icon fixture');
      return favorite?.favicon?.startsWith('data:image/svg+xml')
        ? [favorite.favicon, current.tabs.find((tab) => tab.id === 'legacy-tab')?.favicon]
        : null;
    }, 'legacy icons upgraded');
    assert.equal(upgraded[0], `data:image/svg+xml;base64,${vector.toString('base64')}`);
    assert.equal(upgraded[1], upgraded[0]);
    checks.push('旧版本缓存模糊的 64px 图标在启动后按站点真实尺寸重新获取');
    await ui.screenshot({ path: join(output, 'legacy-upgrade.png') });
    await stop();
    // The marker is persisted only after a refresh that found real icons, so the
    // next launch does not ask every site again.
    const stored = JSON.parse(new DatabaseSync(join(legacyProfile, 'era.sqlite')).prepare('SELECT json FROM session WHERE id=1').get().json);
    assert.equal(stored.iconVersion, 1);
    assert.equal(stored.favorites.find((mark) => mark.title === 'Legacy icon fixture')?.favicon, upgraded[0]);
    checks.push('升级完成后写入 iconVersion，重启不再重复请求');
    for (const server of servers) server.close();
    servers.length = 0;
  }

  await writeFile(join(output, 'result.json'), JSON.stringify({ passed: true, profile, legacyProfile, checks }, null, 2));
  console.log(JSON.stringify({ passed: true, profile, legacyProfile, checks }, null, 2));
} catch (error) {
  await ui?.screenshot({ path: join(output, 'failure.png') }).catch(() => {});
  throw error;
} finally {
  await pageBrowser?.close().catch(() => {});
  await browser?.close().catch(() => {});
  if (child && !child.killed) child.kill();
  for (const server of servers) server.close();
}
