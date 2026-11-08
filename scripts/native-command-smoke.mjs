// Command bar integration with real native pages and an isolated temporary profile.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';
import { checkCommand, checkCommandHandoff } from './native-command-checks.mjs';

const profile=await mkdtemp(join(tmpdir(),'rove-command-smoke-'));
const output=resolve(process.env.ERA_SMOKE_OUTPUT||'test-results/native-command');
const port=Number(process.env.ERA_SMOKE_PORT||19646);
await mkdir(output,{recursive:true});
const server=createServer((req,res)=>{
  const name=req.url==='/second'?'Second':req.url==='/peek'?'Peek':'First';
  res.setHeader('Content-Type','text/html; charset=utf-8');
  res.end(`<!doctype html><title>${name} command fixture</title><style>html{background:${name==='Peek'?'#7c2d12':name==='First'?'#164e63':'#385743'};color:white}</style><h1>${name} command fixture</h1><div style="height:1600px"></div>`);
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;
let child,uiBrowser,pageBrowser,ui;
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
try{
  child=spawn(resolve(process.env.ERA_SMOKE_EXE||'native/target/debug/rove.exe'),[],{
    env:{...process.env,ERA_DATA_DIR:profile,ERA_DEBUG_PORT:String(port)},windowsHide:true,stdio:'pipe',
  });
  child.stderr.on('data',data=>process.stderr.write(data));
  uiBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port}`),'UI debugger');
  ui=await waitFor(async()=>uiBrowser.contexts().flatMap(context=>context.pages()).find(page=>page.url().startsWith('https://era.ui/')),'UI page');
  await ui.waitForSelector('.sidebar');
  await send({type:'navigate',url:base+'/first'});
  pageBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port+1}`),'page debugger');
  const first=await site('/first');await first.waitForSelector('h1');
  await first.evaluate(()=>scrollTo(0,240));
  await checkCommand(ui,[first],'page',{output,waitFor});
  checks.push('单页：真实网页背景已解码，浮窗位于网页中央，DOM/滚动/可见性不变，Esc关闭恢复网页焦点');
  await checkCommandHandoff(ui,first,{waitFor});
  checks.push('延迟解码时保留网页；快速重开拒绝旧回调；坏图片取消浮层并恢复；Ctrl+T和点击外侧关闭不创建空白页');
  const firstId=await ui.getByRole('tab',{selected:true}).getAttribute('data-tab-id');
  await send({type:'new',url:base+'/second'});
  const second=await site('/second');await second.waitForSelector('h1');
  const secondId=await ui.getByRole('tab',{selected:true}).getAttribute('data-tab-id');
  await send({type:'split',id:secondId,other:firstId});
  await ui.locator('.split-divider').waitFor();
  await checkCommand(ui,[first,second],'split',{output,waitFor});
  checks.push('分屏：命令浮窗保留两侧网页像素与DOM，关闭后恢复主页面焦点');
  await send({type:'peekOpen',url:base+'/peek'});
  await waitFor(async()=>await ui.locator('.peek-layer').getAttribute('data-phase')==='open','Peek opening completed');
  const peek=await site('/peek');await peek.waitForSelector('h1');
  await checkCommand(ui,[first,second,peek],'peek',{output,waitFor});
  assert.equal(await ui.locator('.peek-layer').getAttribute('data-phase'),'open');
  checks.push('分屏+Peek：三个真实网页背景均保留，Esc只关闭命令栏并将焦点返回Peek');
  await writeFile(join(output,'native-command-smoke.json'),JSON.stringify({passed:true,profile,checks},null,2));
  console.log(JSON.stringify({passed:true,profile,checks},null,2));
}catch(error){
  await ui?.screenshot({path:join(output,'failure.png')}).catch(()=>{});
  await writeFile(join(output,'failure.json'),JSON.stringify({checks,error:String(error)},null,2));
  throw error;
}finally{
  await pageBrowser?.close();await uiBrowser?.close();
  if(child&&!child.killed)child.kill();
  server.close();
}
