// Command bar integration with real native pages and an isolated temporary profile.
import { createLegacyNativeProfile } from './native-profile-fixture.mjs';
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import assert from 'node:assert/strict';
import { checkCommand, checkCommandHandoff, popupPage } from './native-command-checks.mjs';

const profile=await mkdtemp(join(tmpdir(),'rove-command-smoke-'));
createLegacyNativeProfile(profile);
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
try{
  child=spawn(resolve(process.env.ERA_SMOKE_EXE||'native/target/debug/rove.exe'),[],{
    env:{...process.env,ERA_DATA_DIR:profile,ERA_DEBUG_PORT:String(port)},windowsHide:true,stdio:'pipe',
  });
  child.stderr.on('data',data=>process.stderr.write(data));
  uiBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port}`),'UI debugger');
  ui=await waitFor(async()=>uiBrowser.contexts().flatMap(context=>context.pages()).find(page=>page.url()==='https://era.ui/index.html'),'UI page');
  await ui.waitForSelector('.sidebar');
  popup=await waitFor(()=>ui.context().pages().find(p=>p.url().endsWith('?popup')),'popup renderer');
  await send({type:'navigate',url:base+'/first'});
  pageBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port+1}`),'page debugger');
  const first=await site('/first');await first.waitForSelector('h1');
  await first.evaluate(()=>scrollTo(0,240));
  await checkCommand(ui,[first],'page',{output,waitFor});
  checks.push('单页：独立透明WebView，入场动画存在，无整页背景截图，浮窗位于网页中央，DOM/滚动/可见性不变，Esc关闭恢复网页焦点');
  await checkCommandHandoff(ui,first,{waitFor});
  checks.push('不依赖背景解码；快速重开和关闭正常；Ctrl+T和点击外侧关闭不创建空白页');
  const firstId=await ui.getByRole('tab',{selected:true}).getAttribute('data-tab-id');
  await send({type:'new',url:base+'/second'});
  const second=await site('/second');await second.waitForSelector('h1');
  const secondId=await ui.getByRole('tab',{selected:true}).getAttribute('data-tab-id');
  await send({type:'split',id:secondId,other:firstId});
  await ui.locator('.split-divider').waitFor();
  await checkCommand(ui,[first,second],'split',{output,waitFor});
  checks.push('分屏：命令浮窗保留两侧网页像素与DOM，关闭后恢复主页面焦点');
  await send({type:'peekOpen',url:base+'/peek'});
  await waitFor(async()=>await popup.locator('.peek-layer').getAttribute('data-phase')==='open','Peek opening completed');
  const peek=await site('/peek');await peek.waitForSelector('h1');
  await checkCommand(ui,[first,second,peek],'peek',{output,waitFor});
  assert.equal(await popup.locator('.peek-layer').getAttribute('data-phase'),'open');
  checks.push('分屏+Peek：三个真实网页背景均保留，Esc只关闭命令栏并将焦点返回Peek');
  await popup.getByRole('button',{name:'关闭 Peek',exact:true}).click();
  await popup.locator('.peek-layer').waitFor({state:'detached'});
  await ui.getByRole('button',{name:'工作区与外观',exact:true}).click();
  await popup.getByRole('dialog',{name:'工作区与外观'}).waitFor();
  assert.equal(await ui.getByRole('dialog').count(),0);
  for(const page of [first,second])assert.equal(await page.evaluate(()=>document.visibilityState),'visible');
  await popup.keyboard.press('Escape');
  await popup.getByRole('dialog',{name:'工作区与外观'}).waitFor({state:'detached'});
  await ui.getByRole('button',{name:'全部标签页',exact:true}).click();
  await popup.getByRole('dialog',{name:'全部标签页'}).waitFor();
  await popup.keyboard.press('Escape');
  await popup.locator('.desk').waitFor({state:'detached'});
  await ui.getByRole('tab',{selected:true}).click({button:'right'});
  await popup.getByRole('menu').waitFor();
  await popup.keyboard.press('Escape');
  await popup.getByRole('menu').waitFor({state:'detached'});
  await ui.locator('.favorite-tile').first().click({button:'right'});
  await popup.locator('.favorite-menu').waitFor();
  await popup.getByRole('menuitem',{name:'编辑常用网站',exact:true}).click();
  await popup.getByRole('textbox',{name:'置顶网站名称'}).waitFor();
  await popup.keyboard.press('Escape');
  await popup.getByRole('menu').waitFor({state:'detached'});
  checks.push('设置、标签总览、标签菜单、常用网站菜单及编辑均在独立浮层，底层网页持续可见');
  await send({type:'peekOpen',url:'file:///C:/Windows'});
  await popup.getByRole('alert').waitFor();
  assert.equal(await ui.getByRole('alert').count(),0);
  await popup.getByRole('button',{name:'关闭提示',exact:true}).click();
  await popup.getByRole('alert').waitFor({state:'detached'});
  checks.push('错误提示在独立浮层显示，关闭后移除鼠标区域');
  await writeFile(join(output,'native-command-smoke.json'),JSON.stringify({passed:true,profile,checks},null,2));
  console.log(JSON.stringify({passed:true,profile,checks},null,2));
}catch(error){
  await ui?.screenshot({path:join(output,'failure.png')}).catch(()=>{});
  const renderers=await Promise.all([ui,popup].map(page=>page?.evaluate(()=>({
    url:location.href, focus:document.hasFocus(),
    command:Array.from(document.querySelectorAll('.command-scrim,.command-panel,[role=combobox]'),element=>({
      html:element.outerHTML.slice(0,300),rect:element.getBoundingClientRect().toJSON(),
      display:getComputedStyle(element).display,visibility:getComputedStyle(element).visibility,
    })),
  })).catch(()=>null)));
  await writeFile(join(output,'failure.json'),JSON.stringify({checks,error:String(error),renderers},null,2));
  throw error;
}finally{
  await pageBrowser?.close();await uiBrowser?.close();
  if(child&&!child.killed)child.kill();
  server.close();
}
