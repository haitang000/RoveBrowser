import assert from 'node:assert/strict';
import { join } from 'node:path';

export async function checkCommand(ui,pages,name,{output,waitFor}){
  const tabCount=await ui.getByRole('tab').count();
  const before=await Promise.all(pages.map(page=>page.evaluate(()=>{
    window.__commandVisibility=[];
    document.addEventListener('visibilitychange',()=>window.__commandVisibility.push(document.visibilityState));
    window.__commandMarker=crypto.randomUUID();
    return {marker:window.__commandMarker,scroll:scrollY};
  })));
  await ui.keyboard.press('Control+l');
  await ui.locator('.command-scrim[data-presented="true"]').waitFor();
  await ui.getByRole('combobox').waitFor();
  assert.equal(await ui.locator('.command-background').count(),pages.length);
  for(const image of await ui.locator('.command-background').all())
    assert(await image.evaluate(img=>img.complete&&img.naturalWidth>0));
  const area=await ui.locator('.page-area').boundingBox();
  await ui.locator('.command-scrim').evaluate(e=>e.getAnimations({subtree:true}).forEach(a=>a.finish()));
  const panel=await ui.locator('.command-panel').boundingBox();
  assert(Math.abs(panel.x+panel.width/2-area.x-area.width/2)<1);
  assert(await ui.getByRole('combobox').evaluate(e=>e===document.activeElement));
  await ui.screenshot({path:join(output,`native-command-${name}.png`)});
  for(const page of pages)assert.equal(await page.evaluate(()=>document.visibilityState),'visible');
  await ui.keyboard.press('Escape');
  await ui.locator('.command-scrim').waitFor({state:'detached'});
  await waitFor(async()=>await ui.locator('.command-background').count()===0,'command snapshots released');
  assert.equal(await ui.getByRole('tab').count(),tabCount);
  for(let i=0;i<pages.length;i++){
    assert.deepEqual(await pages[i].evaluate(()=>({marker:window.__commandMarker,scroll:scrollY})),before[i]);
    assert.deepEqual(await pages[i].evaluate(()=>window.__commandVisibility),[]);
  }
  assert(await pages.at(-1).evaluate(()=>document.hasFocus()));
}
export async function checkCommandHandoff(ui,web,{waitFor}){
  await ui.evaluate(()=>{
    const decode=HTMLImageElement.prototype.decode;
    window.__commandReleases=[];
    HTMLImageElement.prototype.decode=function(){
      const decoded=decode.call(this);
      return this.classList.contains('command-background')
        ? decoded.then(()=>new Promise(resolve=>window.__commandReleases.push(resolve))) : decoded;
    };
    window.__restoreCommandDecode=()=>{HTMLImageElement.prototype.decode=decode};
  });
  await ui.keyboard.press('Control+l');
  await waitFor(()=>ui.evaluate(()=>window.__commandReleases.length===1),'command decode held');
  assert.equal(await ui.locator('.command-scrim').getAttribute('data-presented'),'false');
  assert.equal(await web.evaluate(()=>document.visibilityState),'visible');
  await ui.keyboard.press('Escape');
  await ui.locator('.command-scrim').waitFor({state:'detached'});
  await ui.keyboard.press('Control+l');
  await waitFor(()=>ui.evaluate(()=>window.__commandReleases.length===2),'reopened command decode held');
  await ui.evaluate(()=>window.__commandReleases.shift()());
  assert.equal(await ui.locator('.command-scrim').getAttribute('data-presented'),'false');
  await ui.evaluate(()=>{window.__restoreCommandDecode();window.__commandReleases.splice(0).forEach(resolve=>resolve())});
  await ui.locator('.command-scrim[data-presented="true"]').waitFor();
  await ui.keyboard.press('Escape');
  await ui.locator('.command-scrim').waitFor({state:'detached'});
  await ui.evaluate(()=>{
    const decode=HTMLImageElement.prototype.decode;
    HTMLImageElement.prototype.decode=function(){return this.classList.contains('command-background')?Promise.reject(Error('fixture decode failure')):decode.call(this)};
    window.__restoreCommandDecode=()=>{HTMLImageElement.prototype.decode=decode};
  });
  await ui.keyboard.press('Control+l');
  await ui.getByRole('alert').filter({hasText:'无法准备命令栏'}).waitFor();
  await ui.locator('.command-scrim').waitFor({state:'detached'});
  assert.equal(await web.evaluate(()=>document.visibilityState),'visible');
  await ui.evaluate(()=>window.__restoreCommandDecode());
  const tabs=await ui.getByRole('tab').count();
  await ui.keyboard.press('Control+t');
  await ui.locator('.command-scrim[data-presented="true"]').waitFor();
  assert.equal(await ui.getByRole('combobox').inputValue(),'');
  await ui.locator('.command-scrim').click({position:{x:8,y:8}});
  await ui.locator('.command-scrim').waitFor({state:'detached'});
  assert.equal(await ui.getByRole('tab').count(),tabs);
  assert(await web.evaluate(()=>document.hasFocus()));
}
