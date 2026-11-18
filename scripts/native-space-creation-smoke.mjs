// Space creation across the shell, floating renderer, real pages and SQLite.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium, expect } from '@playwright/test';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import assert from 'node:assert/strict';

const profile = await mkdtemp(join(tmpdir(), 'rove-space-creation-'));
createLegacyNativeProfile(profile);
const executable = resolve(process.env.ERA_SPACE_CREATION_EXE || 'native/target/debug/rove.exe');
const output = resolve('test-results/native-space-creation');
const port = Number(process.env.ERA_SPACE_CREATION_PORT || 19876);
await mkdir(output, { recursive: true });
const server = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end('<!doctype html><title>Space creation fixture</title><style>html{background:#164e63;color:white}</style><h1>Space creation fixture</h1><div style="height:1800px"></div>');
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const url = `http://127.0.0.1:${server.address().port}/fixture`;
let app, browser, webBrowser, ui, popup;
const checks = [];
const execFileAsync = promisify(execFile);
async function visibleWebviews() {
  const { stdout } = await execFileAsync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', resolve('scripts/window-regions.ps1'), '-ProcessId', String(app.pid)], { windowsHide: true, timeout: 30000 });
  return JSON.parse(stdout.trim()).filter(row => row.class === 'WRY_WEBVIEW' && row.visible).length;
}
async function waitFor(fn, label) {
  const end = Date.now() + 20000;
  while (Date.now() < end) {
    try { const value = await fn(); if (value) return value; } catch { /* WebViews are starting. */ }
    await new Promise(resolve => setTimeout(resolve, 80));
  }
  throw Error(`Timed out: ${label}`);
}
async function send(command) { await ui.evaluate(c => window.ipc.postMessage(JSON.stringify(c)), command); }
async function state() { return ui.evaluate(() => window.__spaceCreationState); }
async function launch() {
  app = spawn(executable, [], { env: { ...process.env, ERA_DATA_DIR: profile, ERA_DEBUG_PORT: String(port) }, windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
  app.stderr.on('data', data => process.stderr.write(data));
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`), 'shell debugger');
  ui = await waitFor(() => browser.contexts().flatMap(c => c.pages()).find(p => p.url() === 'https://era.ui/index.html'), 'shell');
  await ui.locator('.sidebar').waitFor();
  popup = await waitFor(() => ui.context().pages().find(p => p.url().endsWith('?popup')), 'popup renderer');
  await popup.locator('.popup-page-area').waitFor({ state: 'attached' });
  await waitFor(async () => {
    const { stdout } = await execFileAsync('powershell.exe', ['-NoProfile', '-Command', `(Get-Process -Id ${app.pid}).MainWindowHandle.ToInt64()`], { windowsHide: true });
    return Number(stdout.trim()) > 0;
  }, 'native window presented');
  await ui.evaluate(() => {
    window.__spaceCreationState = window.__ERA_INITIAL_STATE__;
    window.__spaceCreationPopup = {};
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => {
      if (event.type === 'state') window.__spaceCreationState = event.state;
      if (event.type === 'popupState') Object.assign(window.__spaceCreationPopup, event.popup);
      receive(event);
    };
  });
  const current = await state();
  await send({ type: 'settingsUpdate', settings: { ...current.settings, askBeforeQuit: false } });
}
async function close() {
  await webBrowser?.close(); webBrowser = undefined;
  if (app && app.exitCode === null) {
    const done = once(app, 'exit');
    await send({ type: 'window', action: 'close' }).catch(() => app.kill());
    await done;
  }
  await browser?.close(); browser = undefined;
}
async function openFromMenu() {
  await ui.getByRole('button', { name: '浏览器菜单', exact: true }).click();
  await popup.getByRole('menuitem', { name: '新建 Space', exact: true }).click();
  await ui.getByRole('heading', { name: '创建 Space', exact: true }).waitFor();
}
try {
  await launch();
  await send({ type: 'navigate', url });
  webBrowser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port + 1}`), 'website debugger');
  const source = await waitFor(() => webBrowser.contexts().flatMap(c => c.pages()).find(p => p.url() === url), 'source page');
  await source.locator('h1').waitFor();
  await source.evaluate(() => { window.__spaceCreationMarker = 'preserved'; scrollTo(0, 240); });
  const before = await state();
  const visibleBefore = await visibleWebviews();

  await ui.getByRole('button', { name: '浏览器菜单', exact: true }).click();
  const menu = popup.getByRole('menu', { name: '浏览器菜单', exact: true });
  await expect(menu.getByRole('group', { name: '新建' }).getByRole('menuitem')).toHaveCount(3);
  await expect(menu.getByRole('menuitem', { name: '下载', exact: true })).toBeVisible();
  await expect(menu.getByRole('menuitem', { name: '设置', exact: true })).toBeVisible();
  await expect(ui.getByRole('menu')).toHaveCount(0);
  await menu.screenshot({ path: join(output, 'browser-menu.png') });
  await popup.mouse.click(500, 192);
  await expect(popup.getByRole('menu')).toHaveCount(0);
  await waitFor(() => source.evaluate(() => document.visibilityState === 'visible'), 'source remains visible after menu');
  checks.push('浏览器菜单使用原生浮层显示，包含新建、下载、设置与工作区；点击网页区域可关闭菜单');
  console.log(checks.at(-1));

  await ui.getByRole('button', { name: '浏览器菜单', exact: true }).click();
  await menu.getByRole('menuitem', { name: '下载', exact: true }).click();
  const downloads = popup.getByRole('dialog', { name: '下载', exact: true });
  await expect(downloads).toBeVisible();
  await expect(menu).toHaveCount(0);
  await popup.keyboard.press('Escape');
  await expect(downloads).toHaveCount(0);
  checks.push('聚合菜单中的下载入口可打开原生下载弹窗，Esc 可关闭弹窗');

  await openFromMenu();
  assert.equal(await visibleWebviews(), visibleBefore - 1, 'creation hides the real website HWND');
  await expect(ui.getByRole('textbox', { name: '工作区名称', exact: true })).toBeFocused();
  await ui.getByRole('textbox', { name: '工作区名称', exact: true }).fill('取消的空间');
  await ui.getByRole('button', { name: '主题…', exact: true }).click();
  await ui.getByRole('button', { name: '薄荷', exact: true }).click();
  await ui.getByRole('button', { name: '关闭主题', exact: true }).click();
  await ui.getByRole('button', { name: '取消', exact: true }).click();
  await waitFor(() => source.evaluate(() => document.visibilityState === 'visible'), 'source restored after cancel');
  assert.equal(await visibleWebviews(), visibleBefore, 'cancel restores the website HWND');
  assert.deepEqual((await state()).spaces, before.spaces);
  assert.equal((await state()).activeTab, before.activeTab);
  assert.deepEqual(await source.evaluate(() => [window.__spaceCreationMarker, scrollY]), ['preserved', 240]);
  checks.push('创建页隐藏真实网页；取消不新增或修改 Space，原网页 DOM、滚动位置与活动标签保持');
  console.log(checks.at(-1));

  await ui.locator('.sidebar-header .space-name').click();
  const editor = ui.getByRole('dialog', { name: '编辑空间', exact: true });
  await expect(editor.getByRole('textbox', { name: '工作区名称', exact: true })).toHaveValue('Personal');
  assert.equal(await visibleWebviews(), visibleBefore - 1, 'editing hides the real website HWND');
  const themeButton = editor.getByRole('button', { name: '主题…', exact: true });
  const theme = ui.getByRole('region', { name: 'Space 主题', exact: true });
  await themeButton.click();
  await theme.getByRole('button', { name: '薄荷', exact: true }).click();
  await ui.keyboard.press('Escape');
  await expect(themeButton).toBeFocused();
  await ui.screenshot({ path: join(output, 'edit-page.png') });
  await editor.getByRole('button', { name: '完成', exact: true }).click();
  await waitFor(() => source.evaluate(() => document.visibilityState === 'visible'), 'source restored after editing');
  assert.deepEqual(await source.evaluate(() => [window.__spaceCreationMarker, scrollY]), ['preserved', 240]);
  await ui.locator('.sidebar-header .space-name').click();
  await themeButton.click();
  await expect(theme.getByRole('button', { name: '薄荷', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await ui.keyboard.press('Escape');
  await ui.keyboard.press('Escape');
  await expect(editor).toHaveCount(0);
  checks.push('编辑使用同一侧栏布局和主题面板；主题即时保存，完成后恢复原网页');
  console.log(checks.at(-1));

  await ui.keyboard.press('Control+t');
  await popup.getByRole('combobox').fill('new space');
  await popup.keyboard.press('Enter');
  await ui.getByRole('heading', { name: '创建 Space', exact: true }).waitFor();
  await ui.getByRole('textbox', { name: '工作区名称', exact: true }).fill('原生研究空间');
  await ui.getByRole('button', { name: '选择图标', exact: true }).click();
  await ui.getByRole('button', { name: '书', exact: true }).click();
  await ui.getByRole('button', { name: '主题…', exact: true }).click();
  await ui.getByRole('button', { name: '薄荷', exact: true }).click();
  await ui.getByRole('button', { name: '关闭主题', exact: true }).click();
  await ui.screenshot({ path: join(output, 'creation-page.png') });
  await ui.getByRole('button', { name: '创建 Space', exact: true }).click();
  await expect(ui.locator('.sidebar-header .space-name')).toHaveText('📖原生研究空间');
  const created = (await state()).spaces.find(s => s.name === '原生研究空间');
  assert.equal((await state()).spaces.length, before.spaces.length + 1);
  assert.equal((await state()).activeSpace, created.id);
  assert.equal(created.icon, '📖');
  assert.equal(typeof created.hue, 'number');
  assert.equal(typeof created.hue2, 'number');
  await ui.getByRole('button', { name: '浏览器菜单', exact: true }).click();
  await popup.getByRole('menuitem', { name: '工作区 Personal', exact: true }).click();
  await waitFor(() => source.evaluate(() => document.visibilityState === 'visible'), 'source visible after switching back');
  await expect(ui.locator('.sidebar-header .space-name')).toHaveText('Personal');
  await ui.getByRole('button', { name: '浏览器菜单', exact: true }).click();
  await popup.getByRole('menuitem', { name: '工作区 原生研究空间', exact: true }).click();
  await expect(ui.locator('.sidebar-header .space-name')).toHaveText('📖原生研究空间');
  await waitFor(async () => (await state()).activeSpace === created.id, 'new Space selected before exit');
  checks.push('命令面板的新 Space 也进入创建页；名称、图标和主题经原生 IPC 创建且自动切换');
  console.log(checks.at(-1));

  await close();
  await waitFor(async () => {
    try {
      const targets = await fetch(`http://127.0.0.1:${port}/json/list`, { signal: AbortSignal.timeout(500) }).then(r => r.json());
      return !targets.some(target => target.url.startsWith('https://era.ui/'));
    } catch { return true; }
  }, 'old renderer shutdown');
  await launch();
  await expect(ui.locator('.sidebar-header .space-name')).toHaveText('📖原生研究空间');
  assert.deepEqual((await state()).spaces.find(s => s.id === created.id), created);
  await openFromMenu();
  await ui.getByRole('button', { name: '取消', exact: true }).click();
  await expect(ui.locator('.new-tab-page')).toBeVisible();
  checks.push('进程重启后 SQLite 恢复新 Space、图标、主题与选择状态；创建页本身不被恢复');
  const result = { passed: true, profile, checks };
  await writeFile(join(output, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  await writeFile(join(output, 'failure.json'), JSON.stringify({
    checks, error: String(error),
    shell: await ui?.evaluate(() => ({ popup: window.__spaceCreationPopup, text: document.body.innerText })).catch(() => null),
    popup: await popup?.evaluate(() => document.body.innerText).catch(() => null),
  }, null, 2));
  throw error;
} finally {
  await close();
  if (app && app.exitCode === null) app.kill();
  server.close();
}
