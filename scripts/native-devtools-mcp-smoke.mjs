import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';

assert.ok(process.env.npm_execpath, 'Run with npm run test:native:devtools-mcp');
const exe = resolve(process.env.ERA_SMOKE_EXE || 'native/target/debug/rove.exe');
await access(exe);
const heading = 'Rove DevTools MCP fixture';
const loaded = new Set();
const server = createServer((req, res) => {
  loaded.add(req.url);
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  res.end(`<!doctype html><title>${heading}</title><h1>${heading}</h1>`);
});
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const origin = `http://127.0.0.1:${server.address().port}`;

async function waitFor(check, label) {
  for (let i = 0; i < 150; i++) {
    const result = await check();
    if (result) return result;
    await delay(100);
  }
  throw Error(`Timed out: ${label}`);
}

async function bounded(promise, label, millis) {
  const timeout = new AbortController();
  try {
    return await Promise.race([
      promise,
      delay(millis, null, { signal: timeout.signal }).then(() => {
        throw Error(`Timed out: ${label}`);
      }),
    ]);
  } finally {
    timeout.abort();
  }
}

function discoveryFile(profile, directory) {
  return join(profile, directory, 'EBWebView', 'DevToolsActivePort');
}

async function readDiscovery(file) {
  try {
    return await readFile(file, 'utf8');
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}

async function checkMcp(userDataDir, url) {
  // Run the published MCP server over stdio using the same options as the docs.
  const mcp = spawn(process.execPath, [
    process.env.npm_execpath, 'exec', '--yes', '--package=chrome-devtools-mcp@1.13.0', '--',
    'chrome-devtools-mcp', '--auto-connect', `--user-data-dir=${userDataDir}`,
    '--no-usage-statistics', '--no-performance-crux',
  ], { windowsHide: true, stdio: ['pipe', 'pipe', 'inherit'] });
  const exited = once(mcp, 'close');
  // npm prints diagnostics on stderr. stdout carries newline-delimited MCP messages.
  const lines = createInterface({ input: mcp.stdout });
  const replies = lines[Symbol.asyncIterator]();
  let requestId = 0;
  const send = message => mcp.stdin.write(JSON.stringify({ jsonrpc: '2.0', ...message }) + '\n');

  async function request(method, params) {
    const id = ++requestId;
    const response = (async () => {
      for (;;) {
        const { value, done } = await replies.next();
        assert.ok(!done, `MCP exited during ${method}`);
        const reply = JSON.parse(value);
        if (reply.id !== id) continue;
        assert.equal(reply.error, undefined, JSON.stringify(reply.error));
        return reply.result;
      }
    })();
    send({ id, method, params });
    return bounded(response, `MCP ${method}`, 60000);
  }

  async function callTool(name, args = {}) {
    const result = await request('tools/call', { name, arguments: args });
    assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
    return result.content;
  }

  try {
    await request('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'rove-windows-smoke', version: '1.0.0' },
    });
    send({ method: 'notifications/initialized' });
    const pages = await callTool('list_pages');
    assert.ok(pages.every(item => !item.text?.includes('era.ui')), JSON.stringify(pages));
    const page = pages.flatMap(item => item.text?.split('\n') ?? []).find(line => line.includes(url));
    const pageIdMatch = page?.match(/^(\d+): /);
    assert.ok(pageIdMatch, JSON.stringify(pages));
    const pageId = Number(pageIdMatch[1]);
    const snapshot = await callTool('take_snapshot', { pageId });
    assert.ok(snapshot.some(item => item.text?.includes(heading)), JSON.stringify(snapshot));
    const screenshot = await callTool('take_screenshot', { pageId });
    assert.ok(screenshot.some(item => item.type === 'image' && item.data.length > 0));
    const nextUrl = origin + '/next';
    await callTool('navigate_page', { pageId, url: nextUrl });
    assert.ok(loaded.has('/next'), 'Rove did not load the navigation target');
    const evaluation = await callTool('evaluate_script', {
      pageId,
      function: 'async () => { console.info("Rove MCP console probe"); return (await fetch("/probe")).status; }',
    });
    assert.ok(evaluation.some(item => item.text?.includes('200')), JSON.stringify(evaluation));
    const consoleMessages = await callTool('list_console_messages', { pageId });
    assert.ok(consoleMessages.some(item => item.text?.includes('Rove MCP console probe')), JSON.stringify(consoleMessages));
    const requests = await callTool('list_network_requests', { pageId });
    assert.ok(requests.some(item => item.text?.includes(origin + '/probe')), JSON.stringify(requests));
  } finally {
    lines.close();
    mcp.stdin.end();
    try {
      await bounded(exited, 'MCP shutdown', 5000);
    } finally {
      mcp.kill();
      mcp.stdout.destroy();
    }
  }
}

try {
  for (const enabled of [true, false]) {
    const profile = await mkdtemp(join(tmpdir(), 'rove-devtools-mcp-'));
    createLegacyNativeProfile(profile, enabled ? {} : { devtoolsMcp: false });
    const db = new DatabaseSync(join(profile, 'era.sqlite'));
    try {
      const state = JSON.parse(db.prepare('SELECT json FROM session WHERE id=1').get().json);
      state.favorites = [];
      state.tabs = state.tabs.filter(tab => tab.id === 'start');
      db.prepare('UPDATE session SET json=? WHERE id=1').run(JSON.stringify(state));
    } finally {
      db.close();
    }
    const pathname = enabled ? '/enabled' : '/disabled';
    const url = origin + pathname;
    const env = { ...process.env, ERA_DATA_DIR: profile };
    delete env.ERA_DEBUG_PORT;
    delete env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS;
    delete env.ROVE_DEVTOOLS_MCP;
    const rove = spawn(exe, [url], {
      env, windowsHide: true, stdio: 'inherit',
    });
    const exited = once(rove, 'close');
    const browsingDiscovery = discoveryFile(profile, 'WebView2');
    try {
      await waitFor(() => loaded.has(pathname), `load ${pathname}`);
      if (enabled) {
        const endpoint = await waitFor(() => readDiscovery(browsingDiscovery), 'WebView2 DevToolsActivePort');
        const [port, wsPath] = endpoint.trim().split(/\r?\n/);
        assert.ok(Number(port) > 0 && Number(port) <= 65535);
        assert.match(wsPath, /^\/devtools\/browser\//);
        await checkMcp(join(profile, 'WebView2', 'EBWebView'), url);
      } else {
        assert.equal(await readDiscovery(browsingDiscovery), null);
      }
      assert.equal(await readDiscovery(discoveryFile(profile, 'UI')), null);
    } finally {
      rove.kill();
      await exited;
      await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    }
  }
  console.log('Rove DevTools MCP verified default-on, saved opt-out, webpage isolation, snapshot, screenshot, navigation, console and network.');
} finally {
  server.close();
}
