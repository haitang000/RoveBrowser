// FindBar (ported Zigzag Window/FindBar.swift) against real WebView2 pages and
// an isolated temporary profile. Focuses on the host-owned find session:
// WebView2's accelerator keys cannot be synthesized over CDP, so opening the bar
// is driven through the same trusted-UI IPC the shell uses.
import { chromium } from '@playwright/test';
import { spawn, execFile } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import assert from 'node:assert/strict';

const execFileAsync=promisify(execFile);
const profile=await mkdtemp(join(tmpdir(),'rove-find-smoke-'));
const output=resolve(process.env.ERA_SMOKE_OUTPUT||'test-results/native-find');
const port=Number(process.env.ERA_SMOKE_PORT||19666);
await mkdir(output,{recursive:true});
const server=createServer((req,res)=>{
  const name=req.url==='/second'?'Second':'First';
  res.setHeader('Content-Type','text/html; charset=utf-8');
  res.end(`<!doctype html><title>${name} find fixture</title><style>html{background:#164e63;color:white;font:16px/1.6 sans-serif}</style><h1>${name} find fixture</h1><p>alpha beta alpha gamma alpha delta</p><p>one unique needle here</p><div style="height:1600px"></div>`);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
let child,uiBrowser,pageBrowser,ui,popup;
const checks=[];
async function waitFor(fn,label){
  for(let i=0;i<150;i++){
    try{const value=await fn();if(value)return value}catch{}
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  throw Error('Timed out: '+label);
}
async function send(command){await ui.evaluate(command=>window.ipc.postMessage(JSON.stringify(command)),command)}
async function site(path){
  return waitFor(async()=>pageBrowser.contexts().flatMap(context=>context.pages()).find(page=>page.url()===base+path),'page '+path);
}
async function bar(){return popup.getByRole('search',{name:'页内查找'})}
async function count(){return (await popup.locator('.find-count').count())?popup.locator('.find-count').innerText():''}
async function waitCount(expected,label){
  await waitFor(async()=>await count()===expected,label||('find count '+expected));
  checks.push(label||('匹配数 '+expected));
}
try{
  child=spawn(resolve(process.env.ERA_SMOKE_EXE||'native/target/debug/rove.exe'),[],{
    env:{...process.env,ERA_DATA_DIR:profile,ERA_DEBUG_PORT:String(port)},windowsHide:true,stdio:'pipe',
  });
  child.stderr.on('data',data=>process.stderr.write(data));
  uiBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port}`),'UI debugger');
  ui=await waitFor(async()=>uiBrowser.contexts().flatMap(context=>context.pages()).find(page=>page.url()==='https://era.ui/index.html'),'UI page');
  await ui.waitForSelector('.sidebar');
  await ui.evaluate(()=>window.ipc.postMessage(JSON.stringify({type:'settingsUpdate',settings:{...window.__ERA_INITIAL_STATE__.settings,askBeforeQuit:false}})));
  popup=await waitFor(()=>ui.context().pages().find(page=>page.url().endsWith('?popup')),'popup renderer');
  // Keep the find-related host events so a failure report can tell a missing
  // command from a state race.
  for(const renderer of [ui,popup])await renderer.evaluate(()=>{
    window.__findEvents=[];
    const previous=window.__ERA_RECEIVE__;
    window.__ERA_RECEIVE__=(event)=>{
      if(['findOpen','findClosed','findFocus','findMatches'].includes(event.type))window.__findEvents.push({...event});
      previous?.(event);
    };
  });
  await send({type:'navigate',url:base+'/first'});
  pageBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port+1}`),'page debugger');
  const first=await site('/first');await first.waitForSelector('h1');

  // Open from trusted UI (same host path Ctrl+F takes), type, and read WebView2's own count.
  await send({type:'findOpen'});
  await (await bar()).waitFor();
  assert.equal(await popup.locator('.find-bar input').evaluate(element=>element===document.activeElement),true,'find field owns focus when the bar opens');
  checks.push('页内查找条在弹窗层打开并聚焦输入框');
  await popup.locator('.find-bar input').fill('alpha');
  await waitCount('1 / 3','WebView2 报告 3 个匹配，第 1 个为当前');
  // The popup HWND is clipped to the reported regions. Read them from the real
  // Win32 window: the layout box alone is 320x40 (ratio 8); the 20px shadow
  // bleed makes it 360x80 (ratio 4.5) in any uniform DPI scale.
  const {stdout:regionOutput}=await execFileAsync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/window-regions.ps1'),'-ProcessId',String(child.pid)],{windowsHide:true,timeout:30000});
  const regions=JSON.parse(regionOutput.trim());
  const widened=regions.some(region=>{
    const width=region.right-region.left,height=region.bottom-region.top,ratio=width/height;
    return ratio>4.2&&ratio<4.8&&width>300&&height>50;
  });
  assert.equal(widened,true,`native popup region includes the find bar shadow bleed (${regionOutput.trim()})`);
  checks.push('Win32 弹窗区域为查找条阴影外扩（360x80），阴影不再被方角裁切');
  await popup.getByRole('button',{name:'下一个匹配',exact:true}).click();
  await waitCount('2 / 3','下一个匹配前进');
  await popup.locator('.find-bar input').press('Enter');
  await waitCount('3 / 3','Enter 前进');
  await popup.locator('.find-bar input').press('Shift+Enter');
  await waitCount('2 / 3','Shift+Enter 后退');
  // Closing keeps the outer box mounted for the 120ms exit fade and only then
  // detaches, so the native region still covers the animating pill.
  const leaving=await popup.evaluate(()=>new Promise(resolve=>{
    const start=performance.now();
    let mountedAt30=false;
    const observer=new MutationObserver(()=>{
      if(document.querySelector('.find-bar'))return;
      observer.disconnect();
      resolve({mountedAt30,leaveMs:performance.now()-start});
    });
    observer.observe(document.body,{childList:true,subtree:true});
    document.querySelector('.find-done')?.dispatchEvent(new MouseEvent('click',{bubbles:true}));
    setTimeout(()=>{mountedAt30=Boolean(document.querySelector('.find-bar'));},30);
  }));
  assert.equal(leaving.mountedAt30,true,'find bar stays mounted during its exit fade');
  assert.ok(leaving.leaveMs>=90,`find bar exit lasts a transition (${leaving.leaveMs.toFixed(0)}ms)`);
  checks.push('查找条退场时外框保留到动画结束（'+Math.round(leaving.leaveMs)+'ms）');
  await (await bar()).waitFor({state:'detached'});
  assert.equal(await first.evaluate(()=>document.hasFocus()),true,'closing the bar returns focus to the page');
  checks.push('关闭查找后高亮清除、焦点返回网页');

  // A second open with an empty query can be closed before any session exists.
  await send({type:'findOpen'});
  await (await bar()).waitFor();
  await popup.locator('.find-bar input').press('Escape');
  await (await bar()).waitFor({state:'detached'});
  checks.push('空查询时 Esc 关闭不留下会话');

  // The field owns Ctrl+F: pressing it again closes, as in Zigzag.
  await send({type:'findOpen'});
  await (await bar()).waitFor();
  await popup.locator('.find-bar input').fill('needle');
  await waitCount('1 / 1','唯一匹配');
  await popup.locator('.find-bar input').press('Control+f');
  await (await bar()).waitFor({state:'detached'});
  checks.push('输入框中再按 Ctrl+F 收起查找条');

  // Switching tabs ends the session and hides the bar.
  await send({type:'findOpen'});
  await (await bar()).waitFor();
  await popup.locator('.find-bar input').fill('alpha');
  await waitCount('1 / 3');
  await send({type:'new',url:base+'/second'});
  await site('/second');
  await (await bar()).waitFor({state:'detached'});
  checks.push('切换标签结束查找会话并收起查找条');

  // internal pages have no WebView2 page to search.
  await send({type:'navigate',url:'rove://settings'});
  await ui.locator('.settings-page').waitFor();
  await send({type:'findOpen'});
  await popup.waitForTimeout(300);
  assert.equal(await (await bar()).count(),0,'internal pages refuse find');
  checks.push('内部设置页不打开查找条');

  // A navigation while the bar is open closes it before the old document's
  // highlights can survive on the new one. Close the other tabs first so the
  // only WebView2 page left is the one the bar searched.
  await send({type:'closeOthers'});
  await waitFor(async()=>pageBrowser.contexts().flatMap(context=>context.pages()).filter(candidate=>candidate.url().startsWith(base)).length===0,'other fixture pages closed');
  await send({type:'navigate',url:base+'/first'});
  const visible=await waitFor(async()=>{
    for(const candidate of pageBrowser.contexts().flatMap(context=>context.pages())){
      if(candidate.url()!==base+'/first')continue;
      try{if(await candidate.evaluate(()=>document.visibilityState==='visible'))return candidate}catch{}
    }
  },'visible first page');
  await visible.waitForSelector('h1');
  await send({type:'findOpen'});
  await (await bar()).waitFor();
  await popup.locator('.find-bar input').fill('alpha');
  await waitCount('1 / 3');
  await visible.evaluate(()=>{location.href='/second'});
  await (await bar()).waitFor({state:'detached'});
  checks.push('网页导航开始即结束查找');

  await writeFile(join(output,'native-find-smoke.json'),JSON.stringify({passed:true,profile,checks},null,2));
  console.log(JSON.stringify({passed:true,profile,checks},null,2));
}catch(error){
  await popup?.screenshot({path:join(output,'failure-popup.png')}).catch(()=>{});
  await ui?.screenshot({path:join(output,'failure-ui.png')}).catch(()=>{});
  const state=await Promise.all([ui,popup].map(page=>page?.evaluate(()=>({
    url:location.href,focus:document.hasFocus(),
    findEvents:window.__findEvents||[],
    find:Array.from(document.querySelectorAll('.find-bar'),element=>({html:element.outerHTML.slice(0,500),rect:element.getBoundingClientRect().toJSON()})),
  })).catch(()=>null)));
  await writeFile(join(output,'failure.json'),JSON.stringify({checks,error:String(error),state},null,2));
  throw error;
}finally{
  await pageBrowser?.close();await uiBrowser?.close();
  if(child&&!child.killed)child.kill();
  server.close();
}
