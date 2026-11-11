// Zigzag's Clear ladder, tab aging data and the completed tab context menu,
// against the real native browser: rename, duplicate, copy link, explicit
// Close Others/Below, both Clear steps with Undo, and restart persistence.
// The faded-tab step itself is covered by the Rust model tests because aging a
// tab takes six hours; here the UI flow of steps two and three is exercised.
import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';

const execFileAsync = promisify(execFile);
const server = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  const name = req.url === '/b' ? 'B' : req.url === '/c' ? 'C' : 'A';
  res.end(`<!doctype html><title>${name} fixture</title><h1>${name} fixture</h1>`);
});
await new Promise((done) => server.listen(0, '127.0.0.1', done));
const port = Number(process.env.ERA_SMOKE_PORT || 19806);
const base = `http://127.0.0.1:${server.address().port}`;
const sites = [`${base}/a`, `${base}/b`, `${base}/c`];
const profile = await mkdtemp(join(tmpdir(), 'rove-clear-menu-smoke-'));
const output = resolve(process.env.ERA_SMOKE_OUTPUT || 'test-results/native-clear-menu');
await mkdir(output, { recursive: true });

let child, uiBrowser, ui, popup;
const checks = [];
async function waitFor(fn, label) {
  for (let i = 0; i < 220; i++) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await new Promise((done) => setTimeout(done, 100));
  }
  throw Error('Timed out: ' + label);
}
async function send(command) {
  await ui.evaluate((c) => window.ipc.postMessage(JSON.stringify(c)), command);
}
const lastState = () => ui.evaluate(() => window.__lastState);
const regularRows = () => ui.locator('.space-list > .tab-list:not(.pinned-tabs) .tab-row');
const rowCount = () => regularRows().count();
async function rowsEventually(expected, label) {
  await waitFor(async () => (await rowCount()) === expected, `${label}: ${expected}`);
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
  await ui.evaluate(() => {
    window.__lastState = window.__ERA_INITIAL_STATE__;
    const previous = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = (event) => {
      if (event.state) window.__lastState = event.state;
      previous?.(event);
    };
  });
  await rowCount().then((count) => (typeof count === 'number' ? count : 0));
  await ui.locator('.tab-row').first().waitFor();
}
async function stop() {
  await uiBrowser?.close().catch(() => {});
  const exited = new Promise((done) => child.once('exit', done));
  child.kill();
  await Promise.race([exited, new Promise((done) => setTimeout(done, 4000))]);
  uiBrowser = ui = popup = child = undefined;
  await new Promise((done) => setTimeout(done, 700));
}
async function openTabMenu(row = '.tab-row.selected') {
  await ui.locator(row).first().click({ button: 'right' });
  await popup.locator('.context-menu').waitFor();
}
const menuItem = (name) => popup.getByRole('menuitem', { name, exact: false });
const toastText = () => popup.locator('.action-toast').innerText();
async function clickUndo() {
  await popup.locator('.action-toast-undo').click();
}
async function clearStepOnce() {
  // The button only becomes hit-testable while the pointer is inside the list
  // and only arms after 350ms on screen; mirror a careful click.
  await ui.locator('.space-list .section-label').first().hover();
  const clear = ui.locator('.clear-tabs').first();
  await clear.hover();
  await ui.waitForTimeout(450);
  await clear.click();
}
/** A recent batch shows Undo in the Clear slot; wait for the window to lapse. */
async function waitForClearSlot() {
  for (let i = 0; i < 80; i++) {
    const text = await ui.locator('.clear-tabs').first().innerText().catch(() => '');
    if (text.includes('Clear')) return;
    await ui.waitForTimeout(100);
  }
  throw Error('Clear slot did not return after the undo window');
}
async function openTab(url) {
  await send({ type: 'new', url });
  // Page creation can take a moment under load; wait for the command to land
  // before issuing the next one so a slow batch cannot be mistaken for a drop.
  await waitFor(async () => (await lastState()).tabs.some((tab) => tab.url === url), 'tab ' + url);
  await ui.waitForTimeout(120);
}

try {
  await launch();
  const baseline = await rowCount();
  for (const site of sites) await openTab(site);
  await rowsEventually(baseline + 3, 'three new regular tabs');
  const afterCreate = await rowCount();
  const activeUrl = () => lastState().then((state) => state.tabs.find((tab) => tab.id === state.activeTab)?.url);
  assert.equal(await activeUrl(), sites[2]);

  // The completed context menu is rendered by the trusted popup layer.
  await openTabMenu();
  for (const label of ['重命名标签页', '复制标签页', '复制链接', '关闭其他标签页', '关闭下方标签页']) {
    assert.ok(await (await menuItem(label)).count(), `menu item ${label}`);
  }
  checks.push('标签右键菜单包含重命名、复制标签页、复制链接、关闭其他/下方');

  // Copy Link reaches the Windows clipboard through the host.
  await (await menuItem('复制链接')).click();
  await waitFor(async () => (await toastText()).includes('已复制链接'), 'copy toast');
  const { stdout: clipboard } = await execFileAsync(
    'powershell.exe',
    ['-NoProfile', '-Command', 'Get-Clipboard -Raw'],
    { windowsHide: true },
  );
  assert.equal(clipboard.trim(), sites[2]);
  checks.push('复制链接写入系统剪贴板');

  // Rename from the menu; the tab row switches to the custom name.
  await openTabMenu();
  await (await menuItem('重命名标签页')).click();
  await popup.getByRole('textbox', { name: '标签名称' }).fill('重命名页');
  await popup.getByRole('button', { name: '保存名称' }).click();
  await waitFor(async () => {
    const state = await lastState();
    return state.tabs.find((tab) => tab.id === state.activeTab)?.customTitle === '重命名页';
  }, 'custom title in state');
  await ui.getByRole('tab', { name: '重命名页' }).waitFor();
  checks.push('菜单重命名标签页，侧栏立即显示自定义名称');

  // Duplicate from the menu opens a second tab with the same URL.
  await openTabMenu();
  await (await menuItem('复制标签页')).click();
  const afterDuplicate = afterCreate + 1;
  await rowsEventually(afterDuplicate, 'duplicate tab');
  const duplicated = await lastState();
  assert.equal(duplicated.tabs.find((tab) => tab.id === duplicated.activeTab).url, sites[2]);
  checks.push('菜单复制标签页，新标签页沿用当前网址');

  // Explicit Close Others from a menu closes the other tabs and Undo restores.
  await openTabMenu();
  await (await menuItem('关闭其他标签页')).click();
  await rowsEventually(1, 'close others');
  await waitFor(async () => /已关闭 \d+ 个标签/.test(await toastText()), 'close others toast');
  await clickUndo();
  await rowsEventually(afterDuplicate, 'undo close others');
  checks.push('关闭其他标签页可撤销，批量恢复原标签');

  // Close Below targets the tab the menu was opened on, not the active tab.
  for (const site of sites.slice(0, 2)) await openTab(site);
  const beforeBelow = await rowCount();
  assert.ok(beforeBelow > 2);
  const firstRowId = await ui
    .locator('.space-list > .tab-list:not(.pinned-tabs) .tab-row')
    .first()
    .getAttribute('data-tab-id');
  await openTabMenu(`[data-tab-id="${firstRowId}"]`);
  await (await menuItem('关闭下方标签页')).click();
  await rowsEventually(1, 'close below');
  await clickUndo();
  await rowsEventually(beforeBelow, 'undo close below');
  checks.push('关闭下方标签页以右键的目标标签为基准，并可撤销');

  // Clear step two: the other tabs go first, and the Clear slot turns Undo.
  const beforeClear = await rowCount();
  assert.ok(beforeClear > 1);
  await waitForClearSlot();
  await clearStepOnce();
  await rowsEventually(1, 'clear others');
  await waitFor(async () => /已关闭 \d+ 个标签/.test(await toastText()), 'clear toast');
  await ui.locator('.clear-tabs').getByText('Undo').waitFor();
  await ui.locator('.clear-tabs').click();
  await rowsEventually(beforeClear, 'undo clear');
  checks.push('Clear 后原按钮变为 Undo，点击即恢复批量关闭的标签');

  // The Undo window expires: the slot returns to Clear while the host keeps
  // the batch (used here through the same IPC to put the tabs back).
  await openTabMenu();
  await (await menuItem('关闭其他标签页')).click();
  await rowsEventually(1, 'close others for expiry');
  await ui.locator('.clear-tabs').getByText('Undo').waitFor();
  await ui.waitForTimeout(4300);
  await waitFor(async () => (await ui.locator('.clear-tabs').innerText()).includes('Clear'), 'undo window expires');
  await send({ type: 'undoBatch' });
  await rowsEventually(beforeClear, 'undo after expiry');
  checks.push('Undo 窗口（4 秒）结束后按钮恢复为 Clear，宿主仍保留最近批次');

  // Clear step three: only the current tab left; Clear removes it entirely.
  await openTabMenu();
  await (await menuItem('关闭其他标签页')).click();
  await rowsEventually(1, 'single tab before clear');
  const solo = await lastState();
  const soloUrl = solo.tabs.find((tab) => tab.id === solo.activeTab).url;
  await waitForClearSlot();
  await clearStepOnce();
  await waitFor(async () => (await toastText()).includes('已关闭 1 个标签'), 'clear last toast');
  const blanked = await lastState();
  assert.notEqual(blanked.tabs.find((tab) => tab.id === blanked.activeTab)?.url, soloUrl);
  await clickUndo();
  await waitFor(async () => {
    const state = await lastState();
    const restored = state.tabs.find((tab) => tab.url === soloUrl);
    return Boolean(restored) && state.activeTab === restored.id;
  }, 'undo clear last');
  checks.push('Clear 第三档关闭当前页并可撤销，撤销后回到原页');

  // Rename survives a restart through the SQLite session.
  const active = (await lastState()).activeTab;
  await send({ type: 'renameTab', id: active, title: '持久名称' });
  await new Promise((done) => setTimeout(done, 400));
  await stop();
  await launch();
  await waitFor(async () => {
    const state = await lastState();
    return state.tabs.some((tab) => tab.customTitle === '持久名称');
  }, 'renamed tab after restart');
  await ui.getByRole('tab', { name: '持久名称' }).waitFor();
  checks.push('自定义标签名经 SQLite 会话在重启后恢复');

  const result = { passed: true, profile, checks };
  await mkdir(output, { recursive: true });
  await writeFile(join(output, 'native-clear-menu-smoke.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await mkdir(output, { recursive: true }).catch(() => {});
  await ui?.screenshot({ path: join(output, 'failure-ui.png') }).catch(() => {});
  await popup?.screenshot({ path: join(output, 'failure-popup.png') }).catch(() => {});
  const state = await Promise.all(
    [ui, popup].map((renderer) =>
      renderer
        ?.evaluate(() => ({ url: location.href, lastState: window.__lastState }))
        .catch(() => null),
    ),
  );
  await writeFile(join(output, 'failure.json'), JSON.stringify({ checks, error: String(error), state }, null, 2));
  throw error;
} finally {
  await uiBrowser?.close().catch(() => {});
  if (child && !child.killed) child.kill();
  server.close();
}
