import { createConnection } from 'node:net';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function requestAgent(command, { port = Number(process.env.ROVE_AGENT_PORT), token = process.env.ROVE_AGENT_TOKEN } = {}) {
  if (!Number.isInteger(port) || port < 1024 || port > 65535 || !/^[a-f0-9]{64}$/i.test(token ?? '')) {
    return Promise.reject(new Error('Set ROVE_AGENT_PORT and ROVE_AGENT_TOKEN before connecting.'));
  }
  const payload = JSON.stringify({ token, command }) + '\n';
  return new Promise((resolve, reject) => {
    const socket = createConnection({ host: '127.0.0.1', port });
    const timer = setTimeout(() => socket.destroy(new Error('Rove Agent did not respond in time.')), 15_000);
    const chunks = [];
    let size = 0;
    socket.once('connect', () => socket.write(payload));
    socket.once('error', reject);
    socket.once('close', () => {
      clearTimeout(timer);
      reject(new Error('Rove Agent closed the connection without a response.'));
    });
    socket.on('data', chunk => {
      const end = chunk.indexOf(10);
      const part = end < 0 ? chunk : chunk.subarray(0, end);
      size += part.length;
      if (size > 256 * 1024) {
        socket.destroy(new Error('Rove Agent response is too large.'));
        return;
      }
      chunks.push(part);
      if (end < 0) return;
      try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
      catch { reject(new Error('Rove Agent sent an invalid response.')); }
      socket.destroy();
    });
  });
}

function unsigned(raw, max = 0xffffffff) {
  const value = Number(raw);
  return /^\d+$/.test(raw) && Number.isSafeInteger(value) && value <= max ? value : null;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [op, ...args] = process.argv.slice(2);
  let command;
  if (op === 'list' && args.length === 0) command = { op };
  if (op === 'open' && args.length === 1) command = { op, url: args[0] };
  if (op === 'navigate' && args.length === 2) command = { op, id: args[0], url: args[1] };
  if (op === 'close' && args.length === 1) command = { op, id: args[0] };
  if (op === 'browser-list' && args.length <= 1) {
    const offset = unsigned(args[0] ?? '0', Number.MAX_SAFE_INTEGER);
    if (offset !== null) command = { op: 'browser_list', offset };
  }
  if (op === 'browser-open' && args.length === 1) command = { op: 'browser_open', url: args[0] };
  if (op === 'browser-navigate' && args.length === 2) command = { op: 'browser_navigate', id: args[0], url: args[1] };
  if (op === 'browser-activate' && args.length === 1) command = { op: 'browser_activate', id: args[0] };
  if (op === 'browser-close' && args.length === 1) command = { op: 'browser_close', id: args[0] };
  if (op === 'page-snapshot' && (args.length === 1 || args.length === 2)) {
    const offset = unsigned(args[1] ?? '0');
    if (offset !== null) command = { op: 'page_snapshot', id: args[0], offset };
  }
  if (op === 'page-click' && args.length === 3) {
    const element = unsigned(args[2]);
    if (element) command = { op: 'page_click', id: args[0], snapshot: args[1], element };
  }
  if (op === 'page-fill' && args.length === 4) {
    const element = unsigned(args[2]);
    if (element) command = { op: 'page_fill', id: args[0], snapshot: args[1], element, text: args[3] };
  }
  if (!command) {
    console.error([
      'Usage: node scripts/rove-agent.mjs <command>',
      '  list | open <url> | navigate <id> <url> | close <id>',
      '  browser-list [offset] | browser-open <url> | browser-navigate <id> <url>',
      '  browser-activate <id> | browser-close <id>',
      '  page-snapshot <id> [offset]',
      '  page-click <id> <snapshot> <element> | page-fill <id> <snapshot> <element> <text>',
    ].join('\n'));
    process.exitCode = 1;
  } else {
    try {
      const response = await requestAgent(command);
      console.log(JSON.stringify(response));
      if (response.status !== 'ok') process.exitCode = 1;
    } catch (error) {
      console.error(error.message);
      process.exitCode = 1;
    }
  }
}
