// Deterministic browser-core integration tests. Debug CDP is enabled only by a debug-build env flag.
import { chromium } from '@playwright/test'
import { spawn, execFile } from 'node:child_process'
import { createServer } from 'node:http'
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import assert from 'node:assert/strict'
import { once } from 'node:events'
import { promisify } from 'node:util'
const execFileAsync=promisify(execFile)
const profile=await mkdtemp(join(tmpdir(),'rove-native-smoke-'))
const output=resolve(process.env.ERA_SMOKE_OUTPUT||'test-results');
await mkdir(output,{recursive:true});
const hits=new Map();
const server=createServer((req,res)=>{
  hits.set(req.url,(hits.get(req.url)||0)+1);
  res.setHeader('Content-Type','text/html')
  const name=req.url==='/peek'?'Peek':req.url==='/ephemeral'?'Ephemeral':req.url==='/second'?'Second':'First';
  res.end(`<!doctype html><title>${name} fixture</title><style>html{background:${name==='Peek'?'#7c2d12':'#164e63'};color:white}a{color:#bae6fd}</style><h1>${name} real native page</h1><a href="/second">Next</a><a href="/second" target="_blank">New window</a> <a href="/peek">Peek link</a> <a href="/peek" target="_blank">Peek target link</a><div style="height:1600px"></div>`)
})
await new Promise(r=>server.listen(0,'127.0.0.1',r))
const base=`http://127.0.0.1:${server.address().port}`
const exe=resolve(process.env.ERA_SMOKE_EXE||'native/target/debug/rove.exe'),port=Number(process.env.ERA_SMOKE_PORT||19426)
let child,uiBrowser,pageBrowser
async function waitFor(fn,label){for(let i=0;i<150;i++){try{const r=await fn();if(r)return r}catch{}await new Promise(r=>setTimeout(r,100))}throw new Error('Timed out: '+label)}
async function launch(){
  child=spawn(exe,[],{env:{...process.env,ERA_DATA_DIR:profile,ERA_DEBUG_PORT:String(port)},windowsHide:true,stdio:'pipe'})
  child.stderr.on('data',d=>process.stderr.write(d))
  // Wait for the new renderer target before binding CDP. An old WebView2 browser
  // process may briefly keep the endpoint alive after the previous host exits.
  await waitFor(async()=>{
    const targets=await fetch(`http://127.0.0.1:${port}/json/list`,{signal:AbortSignal.timeout(500)}).then(r=>r.json())
    return targets.some(t=>t.url.startsWith('https://era.ui/'))
  },'new UI renderer target')
  uiBrowser=await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port}`),'UI debugger')
  const ui=await waitFor(async()=>uiBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith('https://era.ui/')),'UI page')
  await ui.waitForSelector('.sidebar')
  // MainWindowHandle is nonzero only for a visible top-level process window.
  // CDP alone can operate an invisible WebView, so also verify the paint handshake.
  await waitFor(async()=>{
    const {stdout}=await execFileAsync('powershell.exe',['-NoProfile','-Command',`(Get-Process -Id ${child.pid}).MainWindowHandle.ToInt64()`],{windowsHide:true});
    return Number(stdout.trim())>0;
  },'native window presented after shell paint')
  return ui
}
async function checkSidebar(ui,pages){
  for(const collapsed of [true,false]){
    await ui.evaluate(()=>{
      const track=document.querySelector('.sidebar-track');
      track.style.transitionDuration='1200ms';
    });
    for(const page of pages)await page.evaluate(()=>{
      window.__sidebarVisibility=[];
      document.addEventListener('visibilitychange',()=>window.__sidebarVisibility.push(document.visibilityState),{once:true});
    });
    await send(ui,{type:'sidebar'});
    await waitFor(()=>ui.locator('.sidebar-track').evaluate(e=>e.getAnimations().length>0),'sidebar transition');
    await ui.locator('.sidebar-track').evaluate(e=>Promise.all(e.getAnimations().map(async a=>{a.pause();await a.ready})));
    for(const time of [240,600,960,1200]){
      await ui.locator('.sidebar-track').evaluate((e,time)=>e.getAnimations().forEach(a=>a.currentTime=time),time);
      const slots=await ui.locator('.page-pane > .webview-slot').evaluateAll(slots=>slots.map(s=>s.getBoundingClientRect().width));
      for(let i=0;i<pages.length;i++)await waitFor(async()=>Math.abs(await pages[i].evaluate(()=>innerWidth)-slots[i])<=1,`native viewport follows intermediate sidebar width (${collapsed}, ${time}ms, pane ${i}, expected ${slots[i]}, initial ${await pages[i].evaluate(()=>innerWidth)})`);
    }
    for(const page of pages)assert.deepEqual(await page.evaluate(()=>window.__sidebarVisibility),[]);
    await ui.locator('.sidebar-track').evaluate(e=>{e.getAnimations().forEach(a=>a.finish());e.style.transitionDuration=''});
    assert.equal(await ui.locator('.browser-shell').evaluate(e=>e.classList.contains('collapsed')),collapsed);
  }
}
async function send(ui,c){await ui.evaluate(c=>window.ipc.postMessage(JSON.stringify(c)),c)}
async function site(){
  pageBrowser ||= await waitFor(()=>chromium.connectOverCDP(`http://127.0.0.1:${port+1}`),'site debugger')
  return waitFor(async()=>pageBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith(base)),'site page')
}
const checks=[]
// Optional manual checkpoints keep the real HWND composition observable by Computer Use.
async function visualCheckpoint(ui,label){
  if(process.env.ERA_VISUAL_CHECK!=='1')return;
  await ui.evaluate(label=>{window.__visualCheckpoint=label;window.__continueVisual=false},label);
  console.log('Visual checkpoint: '+label);
  for(let i=0;i<600;i++){
    if(await ui.evaluate(()=>window.__continueVisual))return;
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  throw Error('Visual checkpoint not released: '+label);
}
try{
  let ui=await launch()
  await ui.evaluate(()=>document.fonts.ready);
  assert(await ui.evaluate(()=>[...document.fonts].some(f=>f.family.includes('Inter Variable')&&f.status==='loaded')));
  checks.push('原生 UI 从内嵌资源加载 Inter 字体');
  const frameRadius=()=>ui.locator('.browser-shell').evaluate(e=>getComputedStyle(e).borderTopLeftRadius);
  assert.equal(await frameRadius(),'12px');
  await send(ui,{type:'window',action:'maximize'});
  await waitFor(async()=>await frameRadius()==='0px','maximized square frame');
  await send(ui,{type:'window',action:'maximize'});
  await waitFor(async()=>await frameRadius()==='12px','restored rounded frame');
  checks.push('整个原生窗口 12px 圆角，最大化恢复直角，还原恢复圆角');
  await send(ui,{type:'navigate',url:base+'/first'})
  let web=await site();await web.waitForSelector('h1');assert.equal(await web.title(),'First fixture');checks.push('真实 WebView2 加载与标题')
  await ui.getByRole('button',{name:'管理顶部常用网站',exact:true}).click();
  const favoritesPanel=ui.getByRole('dialog',{name:'顶部常用网站',exact:true});
  await favoritesPanel.getByRole('button',{name:'编辑 GitHub',exact:true}).click();
  await favoritesPanel.getByRole('textbox',{name:'网站名称'}).fill('我的常用网页');
  await favoritesPanel.getByRole('textbox',{name:'网站网址'}).fill(base+'/first');
  await favoritesPanel.getByRole('button',{name:'保存修改',exact:true}).click();
  await favoritesPanel.getByRole('button',{name:'编辑 我的常用网页',exact:true}).waitFor();
  await favoritesPanel.getByRole('button',{name:'下移 我的常用网页',exact:true}).click();
  await waitFor(async()=>await favoritesPanel.locator('li').nth(1).getByRole('button',{name:'编辑 我的常用网页',exact:true}).count(),'favorite order broadcast');
  await favoritesPanel.getByRole('textbox',{name:'网站名称'}).fill('临时常用网页');
  await favoritesPanel.getByRole('textbox',{name:'网站网址'}).fill(base+'/second');
  await favoritesPanel.getByRole('button',{name:'添加网站',exact:true}).click();
  await favoritesPanel.getByRole('button',{name:'移除 临时常用网页',exact:true}).click();
  await favoritesPanel.getByRole('button',{name:'编辑 临时常用网页',exact:true}).waitFor({state:'detached'});
  await ui.screenshot({path:join(output,'native-favorites.png')});
  await ui.keyboard.press('Escape');
  await web.evaluate(()=>window.__favoriteMarker='reuse existing page');
  await ui.locator('.favorites').getByRole('button',{name:'我的常用网页',exact:true}).click();
  assert.equal(await web.evaluate(()=>window.__favoriteMarker),'reuse existing page');
  checks.push('顶部常用网站经真实 UI 和受限 IPC 添加、编辑、排序、移除；点击复用已有网页');
  await web.evaluate(()=>window.__middleClickMarker='keep active page');
  await send(ui,{type:'new',url:base+'/second'});
  const middleTab=ui.getByRole('tab',{name:'Second fixture'});
  await middleTab.waitFor();
  await ui.getByRole('tab',{name:'First fixture'}).click();
  await middleTab.click({button:'middle'});
  await middleTab.waitFor({state:'detached'});
  assert.equal(await ui.getByRole('tab',{selected:true}).getAttribute('title'),base+'/first');
  assert.equal(await web.evaluate(()=>window.__middleClickMarker),'keep active page');
  await send(ui,{type:'restore'});
  await ui.getByRole('tab',{name:'Second fixture'}).waitFor();
  await send(ui,{type:'pin'});
  const middlePin=ui.locator('.pinned-tabs .tab-row.selected');
  await middlePin.waitFor();
  await middlePin.click({button:'middle'});
  await ui.getByRole('tab',{name:'Second fixture'}).waitFor({state:'detached'});
  await ui.getByRole('tab',{name:'First fixture'}).click();
  checks.push('原生中键关闭后台及固定标签；后台关闭不切换或重载当前网页，关闭后可恢复');
  await send(ui,{type:'thumbnails'});
  await ui.getByRole('button',{name:'全部标签页',exact:true}).click();
  const thumb=ui.locator('.tab-snapshot').first();await thumb.waitFor();
  const captured=await thumb.evaluate(async img=>{await img.decode();const c=document.createElement('canvas');c.width=c.height=1;const x=c.getContext('2d');x.drawImage(img,0,0,img.naturalWidth,img.naturalHeight,0,0,1,1);return {width:img.naturalWidth,height:img.naturalHeight,src:img.src,pixel:[...x.getImageData(0,0,1,1).data]}});
  assert(captured.width<=480&&captured.height<=320&&captured.src.startsWith('data:image/jpeg;base64,'));
  assert(captured.pixel[2]>captured.pixel[0]&&captured.pixel[1]>captured.pixel[0]);
  await ui.screenshot({path:resolve(output,'native-thumbnails.png')});
  await ui.keyboard.press('Escape');checks.push('真实网页像素经 CapturePreview 缩小为有界 JPEG，全部标签页显示截图');
  await checkSidebar(ui,[web]);checks.push('侧栏展开/收起的中间帧同步真实网页宽度，页面持续可见');
  assert.equal(await web.evaluate(()=>typeof window.ipc),'undefined');await web.evaluate(()=>window.chrome.webview.postMessage(JSON.stringify({type:'close'})));assert.equal(await web.title(),'First fixture');checks.push('网页无内部 IPC 桥；伪造消息无效')
  await web.getByRole('link',{name:'Next',exact:true}).click();await web.waitForURL(base+'/second');await send(ui,{type:'back'});await web.waitForURL(base+'/first');await send(ui,{type:'forward'});await web.waitForURL(base+'/second');checks.push('网页导航/后退/前进')
  // CDP keyboard injection cannot generate Win32 AcceleratorKeyPressed; that path is tested with Computer Use.
  await ui.keyboard.press('Control+l');await ui.getByRole('combobox').waitFor();assert.equal(await ui.getByRole('combobox').inputValue(),base+'/second');await ui.keyboard.press('Escape');checks.push('UI Ctrl+L 与浮层恢复；网页原生快捷键另经实机检查')
  const tabCount=await ui.getByRole('tab').count();
  await web.evaluate(()=>window.__openerMarker='opener unchanged');
  const sourceBox=await web.getByRole('link',{name:'Peek link',exact:true}).boundingBox();
  const sourceViewport=await web.evaluate(()=>({width:innerWidth,height:innerHeight}));
  const sourceSlot=await ui.locator('.page-pane > .webview-slot').first().boundingBox();
  await ui.evaluate(()=>{
    window.__backgroundReleases=[];
    const decode=HTMLImageElement.prototype.decode;
    HTMLImageElement.prototype.decode=function(){
      const decoded=decode.call(this);
      return this.classList.contains('peek-background')
        ? decoded.then(()=>new Promise(resolve=>window.__backgroundReleases.push(resolve))) : decoded;
    };
    window.__restoreImageDecode=()=>{HTMLImageElement.prototype.decode=decode};
  });
  await web.evaluate(()=>{
    window.__peekVisibility=[];
    document.addEventListener('visibilitychange',()=>window.__peekVisibility.push(document.visibilityState));
  });
  await web.getByRole('link',{name:'Peek link',exact:true}).click({modifiers:['Shift']});
  await ui.getByRole('dialog',{name:'Peek · Peek fixture',exact:true}).waitFor();
  await waitFor(()=>ui.evaluate(()=>window.__backgroundReleases.length>0),'background decoding deliberately held');
  assert.equal(await ui.locator('.peek-layer').getAttribute('data-moving'),'0');
  assert.equal(await web.evaluate(()=>document.visibilityState),'visible');
  await visualCheckpoint(ui,'background decoding held: live source remains visible');
  await ui.evaluate(()=>{window.__restoreImageDecode();window.__backgroundReleases.splice(0).forEach(resolve=>resolve())});
  await waitFor(async()=>await ui.locator('.peek-layer').getAttribute('data-phase')==='open','native entry animation completed');
  assert.equal(await web.evaluate(()=>document.visibilityState),'visible');
  assert.deepEqual(await web.evaluate(()=>window.__peekVisibility),[]);
  checks.push('延迟背景解码时不启动动画；原网页在入场和预览期间保持可见渲染');
  const origin=await ui.locator('.peek-layer').evaluate(e=>({x:Number(e.getAttribute('data-origin-x')),y:Number(e.getAttribute('data-origin-y'))}));
  assert(Math.abs(origin.x-(sourceSlot.x+(sourceBox.x+sourceBox.width/2)/sourceViewport.width*sourceSlot.width))<1);
  assert(Math.abs(origin.y-(sourceSlot.y+(sourceBox.y+sourceBox.height/2)/sourceViewport.height*sourceSlot.height))<1);
  checks.push('真实链接点击坐标穿过原生事件，入场完成后才显示可交互网页');
  let preview=await waitFor(async()=>pageBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url()===base+'/peek'),'Peek independent page');
  assert.equal(await ui.getByRole('tab').count(),tabCount);assert.equal(web.url(),base+'/second');
  assert.equal(await preview.evaluate(()=>typeof window.ipc),'undefined');
  await preview.evaluate(()=>{window.__peekMarker='same controller';window.chrome.webview.postMessage(JSON.stringify({type:'peekPromote'}))});
  assert.equal(await ui.getByRole('tab').count(),tabCount);checks.push('Shift 点击打开独立临时 Peek，普通网页不能调用内部命令');
  await waitFor(()=>ui.locator('.peek-background').count(),'Peek background snapshot');
  await preview.getByRole('link',{name:'Next',exact:true}).click();await preview.waitForURL(base+'/second');
  await send(ui,{type:'back'});await preview.waitForURL(base+'/peek');checks.push('Peek 独立导航与后退，不改变原标签');
  await preview.evaluate(()=>{window.__peekMarker='same controller';window.scrollTo(0,400)});
  const peekHits=hits.get('/peek');
  await ui.getByRole('button',{name:'将预览转为标签页'}).click();await ui.locator('.peek-panel').waitFor({state:'hidden'});
  await waitFor(async()=>await ui.getByRole('tab',{name:'Peek fixture'}).count(),'promoted Peek tab');
  assert.equal(await preview.evaluate(()=>window.__peekMarker),'same controller');assert(await preview.evaluate(()=>scrollY)>200);assert.equal(hits.get('/peek'),peekHits);
  assert.equal(await web.evaluate(()=>window.__openerMarker),'opener unchanged');checks.push('Peek 晋升复用同一个 WebView2，保留 DOM/滚动/导航栈，无重载');
  await ui.getByRole('tab',{name:'Second fixture'}).click();
  await web.getByRole('link',{name:'Peek target link',exact:true}).click({modifiers:['Shift']});await ui.locator('.peek-panel').waitFor();
  const closingPreview=await waitFor(async()=>pageBrowser.contexts().flatMap(c=>c.pages()).find(p=>p!==preview&&p.url()===base+'/peek'),'replacement Peek');
  await closingPreview.getByRole('link',{name:'Next',exact:true}).click({modifiers:['Shift']});
  await ui.getByRole('dialog',{name:'Peek · Second fixture',exact:true}).waitFor();
  await waitFor(()=>Promise.resolve(closingPreview.isClosed()),'replaced Peek destroyed');
  const replacement=await waitFor(async()=>pageBrowser.contexts().flatMap(c=>c.pages()).find(p=>p!==web&&p.url()===base+'/second'),'nested replacement Peek');
  checks.push('Peek 内 Shift 点击替换临时实例，释放旧控制器且不堆叠');
  await waitFor(async()=>await ui.locator('.peek-layer').getAttribute('data-phase')==='open','replacement entry completed');
  await replacement.waitForLoadState('load');
  await ui.evaluate(()=>{
    window.__pausePeekExit=true;
    const animate=Element.prototype.animate;
    Element.prototype.animate=function(frames,options){const a=animate.call(this,frames,options);if(window.__pausePeekExit&&(this.classList.contains('peek-panel')||this.classList.contains('peek-scrim'))&&this.closest('.peek-layer')?.getAttribute('data-phase')==='closing')a.pause();return a};
  });
  await ui.getByRole('button',{name:'关闭 Peek',exact:true}).click();
  await waitFor(async()=>ui.locator('.peek-layer').evaluate(e=>e.getAttribute('data-phase')==='closing'&&e.getAttribute('data-moving')===e.getAttribute('data-ticket')),'decoded exit snapshot acknowledged');
  assert(!replacement.isClosed());assert(await ui.locator('.peek-surface').evaluate(e=>e.naturalWidth>0));
  assert.equal(await web.evaluate(()=>document.visibilityState),'visible');
  await ui.locator('.peek-panel').evaluate(e=>e.getAnimations().forEach(a=>a.currentTime=90));
  await visualCheckpoint(ui,'exit midpoint: decoded source and preview snapshots');
  await ui.screenshot({path:join(output,'peek-motion-native-exit.png')});
  await ui.locator('.peek-layer').evaluate(e=>e.getAnimations({subtree:true}).filter(a=>['peek-panel','peek-scrim'].some(c=>a.effect.target.classList.contains(c))).forEach(a=>a.finish()));
  await ui.evaluate(()=>window.__pausePeekExit=false);
  await ui.locator('.peek-panel').waitFor({state:'hidden'});checks.push('离场使用已解码网页快照缩放，原生控制器存活到动画结束后才释放');
  assert.equal(await ui.getByRole('tab').count(),tabCount+1);assert.equal(await web.evaluate(()=>window.__openerMarker),'opener unchanged');
  await waitFor(()=>Promise.resolve(replacement.isClosed()),'closed Peek controller');
  assert.equal(await web.evaluate(()=>document.visibilityState),'visible');
  checks.push('离场动画期间及结束后原网页持续可见，不经过原生隐藏/恢复白帧');
  await visualCheckpoint(ui,'exit complete: live source restored without visibility toggle');
  checks.push('target=_blank 的 Shift 点击同样进入 Peek；关闭不添加标签');
  await send(ui,{type:'peekOpen',url:base+'/peek'});await ui.locator('.peek-panel').waitFor();
  await ui.keyboard.press('Escape');await ui.locator('.peek-panel').waitFor({state:'hidden'});checks.push('Peek Esc 关闭');
  await send(ui,{type:'peekOpen',url:base+'/peek'});await ui.locator('.peek-panel').waitFor();
  await ui.locator('.peek-scrim').click({position:{x:2,y:2}});await ui.locator('.peek-panel').waitFor({state:'hidden'});
  await ui.locator('.peek-layer').waitFor({state:'detached'});
  await send(ui,{type:'peekOpen',url:'file:///C:/Windows'});await ui.getByRole('alert').waitFor();assert.equal(await ui.locator('.peek-panel').count(),0);checks.push('点击遮罩关闭；禁止 Peek 导航到本地或内部协议');
  await send(ui,{type:'peekOpen',url:base+'/peek',origin:{x:430,y:240}});await ui.locator('.peek-panel').waitFor();
  const interruptedId=await ui.locator('.peek-layer').getAttribute('data-peek-id');
  await send(ui,{type:'peekClose'});
  await send(ui,{type:'peekOpen',url:base+'/peek',origin:{x:500,y:280}});
  await waitFor(async()=>await ui.locator('.peek-layer').getAttribute('data-peek-id')!==interruptedId,'reopened motion instance');
  await send(ui,{type:'peekMotionComplete',id:interruptedId,ticket:2});
  await waitFor(async()=>await ui.locator('.peek-layer').getAttribute('data-phase')==='open','reopened animation completion');
  await ui.getByRole('button',{name:'关闭 Peek',exact:true}).click();await ui.locator('.peek-panel').waitFor({state:'hidden'});
  checks.push('快速关闭并重开时丢弃旧动画完成消息，未误关新预览');
  await send(ui,{type:'new',url:base+'/first'});await waitFor(async()=>await ui.getByRole('tab',{name:'First fixture'}).count(),'second tab title')
  await ui.getByRole('tab',{name:'Second fixture'}).click({button:'right'});await ui.getByRole('menuitem',{name:'在当前网页旁分屏'}).click();await ui.waitForSelector('.split-divider');assert.equal(await ui.locator('.page-pane').count(),2);checks.push('独立实例左右分屏')
  const first=await waitFor(async()=>pageBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url()===base+'/first'),'split primary page');
  await checkSidebar(ui,[first,web]);checks.push('分屏两侧的原生网页宽度跟随侧栏过渡同步更新');
  await ui.getByRole('button',{name:'固定分屏组合'}).click();
  await send(ui,{type:'peekOpen',url:base+'/ephemeral'});await ui.getByRole('dialog',{name:'Peek · Ephemeral fixture',exact:true}).waitFor();
  await waitFor(async()=>await ui.locator('.peek-background').count()===2,'both split backgrounds captured');
  await send(ui,{type:'theme'});await send(ui,{type:'sidebar'});
  await ui.screenshot({path:join(output,'native-peek.png')});await pageBrowser.close();pageBrowser=null;
  const exited=once(child,'exit');await send(ui,{type:'window',action:'close'}).catch(()=>{});await exited;
  await uiBrowser.close().catch(()=>{});uiBrowser=null;
  await waitFor(async()=>{
    try{const targets=await fetch(`http://127.0.0.1:${port}/json/list`,{signal:AbortSignal.timeout(500)}).then(r=>r.json());return !targets.some(t=>t.url.startsWith('https://era.ui/'))}catch{return true}
  },'old UI renderer shutdown');
  ui=await launch();await ui.waitForSelector('.split-divider');
  assert.deepEqual(await ui.evaluate(()=>({theme:window.__ERA_INITIAL_STATE__.theme,collapsed:window.__ERA_INITIAL_STATE__.sidebarCollapsed})),{theme:'dark',collapsed:true});
  assert.equal(await ui.locator('html').getAttribute('data-theme'),'dark');
  assert.deepEqual(await ui.evaluate(()=>window.__ERA_INITIAL_STATE__.favorites[1]),{title:'我的常用网页',url:base+'/first'});
  assert.equal(await ui.evaluate(()=>window.__ERA_INITIAL_STATE__.favorites.some(f=>f.title==='临时常用网页')),false);
  checks.push('顶部常用网站的名称、网址、顺序和移除结果经 SQLite 保存并在进程重启后恢复');
  assert.equal(await ui.locator('.sidebar-track').evaluate(e=>e.getBoundingClientRect().width),0);
  checks.push('重启首屏直接使用保存的深色主题和折叠布局；窗口在界面准备后实际可见');
  await send(ui,{type:'sidebar'});await waitFor(async()=>await ui.getByRole('tab',{name:'First fixture'}).count(),'restored title');checks.push('进程重启后恢复标签、固定状态和分屏关系')
  assert.equal(await ui.locator('.peek-panel').count(),0);assert.equal(await ui.getByRole('tab',{name:'Ephemeral fixture'}).count(),0);checks.push('Peek 为临时页面，重启不恢复且不污染会话');
  await send(ui,{type:'history',query:'fixture'});await ui.keyboard.press('Control+l');await ui.getByRole('combobox').fill('Second');await ui.getByRole('option').filter({hasText:'Second fixture'}).first().waitFor();checks.push('SQLite 历史查询')
  await writeFile(join(output,'native-smoke.json'),JSON.stringify({profile,checks,passed:true},null,2));console.log(JSON.stringify({profile,checks,passed:true},null,2))
}catch(error){
  if(uiBrowser){
    const ui=uiBrowser.contexts().flatMap(c=>c.pages()).find(p=>p.url().startsWith('https://era.ui/'));
    if(ui){
      await writeFile(join(output,'native-failure.json'),JSON.stringify({checks,error:String(error),ui:await ui.locator('.peek-layer').evaluateAll(es=>es.map(e=>({html:e.outerHTML.slice(0,6000),animations:e.getAnimations({subtree:true}).map(a=>({state:a.playState,time:a.currentTime}))}))).catch(()=>null)},null,2));
      await ui.screenshot({path:join(output,'native-failure.png')}).catch(()=>{});
    }
  }
  throw error;
}finally{
  await pageBrowser?.close();await uiBrowser?.close();if(child&&!child.killed)child.kill();server.close()
}
