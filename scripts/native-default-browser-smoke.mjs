// Shell launches and per-user capability registration in real WebView2.
// Registration writes are redirected to a private test subtree.
import { chromium } from '@playwright/test';
import { execFile, spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';
const run = promisify(execFile);
const profile = await mkdtemp(join(tmpdir(), 'rove-default-browser-'));
const registry = `Software\\Rove\\Tests\\${randomUUID()}`;
const output = resolve('test-results/native-default-browser');
await mkdir(output, { recursive: true });
const server = createServer((req,res) => { res.setHeader('Content-Type','text/html; charset=utf-8'); res.end('<title>External URL fixture</title><h1>External URL opened</h1>'); });
await new Promise(done => server.listen(0,'127.0.0.1',done));
const base = `http://127.0.0.1:${server.address().port}`;
const first = `${base}/first?a=1&b=%E4%B8%AD%E6%96%87`, second = `${base}/second?url=https%3A%2F%2Fexample.org`, third = `${base}/cold-restart`;
const exe = resolve(process.env.ERA_TEST_EXE || 'native/target/debug/rove.exe');
let child, browser, ui, port = 19900;
const checks = [];
async function userChoice() {
  return Promise.all(['http','https'].map(async protocol => {
    try { return (await run('reg.exe',['query',`HKCU\\Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\${protocol}\\UserChoice`],{windowsHide:true})).stdout; }
    catch { return ''; }
  }));
}
const defaultsBefore = await userChoice();
function env() { return { ...process.env, ERA_DATA_DIR:profile, ERA_DEBUG_PORT:String(port), ERA_IMPORT_FIXTURES:join(profile,'empty'), ERA_ASSOCIATIONS_FIXTURES:registry }; }
async function waitFor(fn,label) {
  const end = Date.now()+25000;
  while (Date.now()<end) { try { const value = await fn(); if(value) return value; } catch {} await new Promise(done => setTimeout(done,100)); }
  throw Error(`Timed out: ${label}`);
}
async function send(command) { await ui.evaluate(c => window.ipc.postMessage(JSON.stringify(c)),command); }
async function state() { return ui.evaluate(() => window.__defaultTest.state); }
async function launch(args) {
  port += 4;
  child = spawn(exe,args,{env:env(),windowsHide:true,stdio:'pipe'});
  child.stderr.on('data',data => process.stderr.write(data));
  browser = await waitFor(() => chromium.connectOverCDP(`http://127.0.0.1:${port}`),'UI debugger');
  ui = await waitFor(() => browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()==='https://era.ui/index.html'),'UI page');
  await ui.locator('.onboarding,.sidebar').first().waitFor();
  await ui.evaluate(() => {
    window.__defaultTest = {state:window.__ERA_INITIAL_STATE__};
    const receive = window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__ = event => { if(event.state) window.__defaultTest.state=event.state; if(event.defaultBrowser) window.__defaultTest.status=event.defaultBrowser; receive(event); };
  });
  await send({type:'ready'});
}
async function forward(args) {
  const result = await run(exe,args,{env:env(),windowsHide:true,timeout:15000});
  assert.equal(result.stderr,'');
}
async function stop() {
  if (child?.exitCode===null) { const exit=once(child,'exit'); child.kill(); await exit; }
  await browser?.close().catch(()=>{}); browser=undefined;
}
try {
  await launch(['--',first]);
  assert.equal((await state()).onboarding.completed,false);
  await forward(['--',second]);
  assert.equal((await state()).tabs.some(t=>t.url===first),false);
  await ui.getByRole('button',{name:'开始使用',exact:true}).click();
  for(let i=0;i<3;i++) await ui.getByRole('button',{name:'跳过',exact:true}).click();
  await ui.getByRole('button',{name:'暂时不用',exact:true}).click();
  await ui.getByRole('button',{name:'开始浏览',exact:true}).click();
  await waitFor(async() => { const s=await state(); return [first,second].every(url=>s.tabs.some(t=>t.url===url)); },'queued shell URLs');
  checks.push('Cold and forwarded URLs wait for onboarding, then open in the existing session');
  const count=(await state()).tabs.length;
  await forward([]);
  assert.equal((await state()).tabs.length,count);
  await forward(['--',`${base}/warm`]);
  await waitFor(async()=>(await state()).tabs.some(t=>t.url===`${base}/warm`),'warm external URL');
  checks.push('Warm link launch and shortcut activation exit without creating another profile owner');
  await ui.getByRole('button',{name:'设置',exact:true}).click();
  await ui.getByRole('button',{name:'设为默认浏览器',exact:true}).click();
  await waitFor(()=>ui.evaluate(()=>!!window.__defaultTest.status),'default status');
  const registration = (await run('reg.exe',['query',`HKCU\\${registry}\\Software\\Classes\\RoveURL\\shell\\open\\command`,'/ve'],{windowsHide:true})).stdout;
  assert(registration.includes(`"${exe}" -- "%1"`));
  for(const protocol of ['http','https']) {
    const result = await run('reg.exe',['query',`HKCU\\${registry}\\Software\\Clients\\StartMenuInternet\\Rove\\Capabilities\\URLAssociations`,'/v',protocol],{windowsHide:true});
    assert(result.stdout.includes('RoveURL'));
  }
  await send({type:'defaultBrowserStatus'});
  checks.push('Settings registers the quoted executable and both protocols; reports Windows default status');
  await ui.screenshot({path:join(output,'settings.png')});
  const tabsBefore=(await state()).tabs.map(t=>t.id);
  await stop(); await launch(['--',third]);
  await waitFor(async()=>(await state()).tabs.some(t=>t.url===third),'restart external URL');
  const restored=await state(); assert(tabsBefore.every(id=>restored.tabs.some(t=>t.id===id)));
  assert.deepEqual(await userChoice(),defaultsBefore);
  checks.push('Restart preserves previous tabs, opens the new URL, and leaves actual Windows UserChoice unchanged');
  await writeFile(join(output,'checks.json'),JSON.stringify({profile,checks},null,2));
  console.log(JSON.stringify({profile,checks},null,2));
} finally {
  await stop(); await new Promise(done=>server.close(done));
  await run('reg.exe',['delete',`HKCU\\${registry}`,'/f'],{windowsHide:true}).catch(()=>{});
}
