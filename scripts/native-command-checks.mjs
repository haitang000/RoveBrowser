import assert from 'node:assert/strict';
import { join } from 'node:path';

export function popupPage(ui) {
  const popup = ui.context().pages().find(page => page.url().endsWith('?popup'));
  assert(popup, 'independent popup WebView exists');
  return popup;
}
export async function checkCommand(ui,pages,name,{output,waitFor}){
  const popup=popupPage(ui);
  const tabCount=await ui.getByRole('tab').count();
  const before=await Promise.all(pages.map(page=>page.evaluate(()=>{
    window.__commandVisibility=[];
    document.addEventListener('visibilitychange',()=>window.__commandVisibility.push(document.visibilityState));
    window.__commandMarker=crypto.randomUUID();
    return {marker:window.__commandMarker,scroll:scrollY};
  })));
  await popup.evaluate(()=>{
    window.__commandFrames=[];
    window.__commandRecording=true;
    const frame=()=>{
      const panel=document.querySelector('.command-panel');
      if(panel)window.__commandFrames.push(getComputedStyle(panel).transform);
      if(window.__commandRecording)requestAnimationFrame(frame);
    };requestAnimationFrame(frame);
  });
  await ui.keyboard.press('Control+l');
  await popup.getByRole('combobox').waitFor();
  await waitFor(()=>popup.evaluate(()=>window.__commandFrames.length>3),'entry frames');
  const transforms=await popup.evaluate(()=>{window.__commandRecording=false;return window.__commandFrames});
  assert(transforms.some(value=>value!=='none'), 'entry transform is visible in the popup renderer');
  assert.equal(await ui.locator('.command-scrim,.command-background').count(),0);
  assert.equal(await popup.locator('.command-background,.peek-background,.tab-preview-background').count(),0);
  const area=await ui.locator('.page-area').boundingBox();
  await popup.locator('.command-scrim').evaluate(e=>e.getAnimations({subtree:true}).forEach(a=>a.finish()));
  const panel=await popup.locator('.command-panel').boundingBox();
  assert(Math.abs(panel.x+panel.width/2-area.x-area.width/2)<1);
  assert(await popup.getByRole('combobox').evaluate(e=>e===document.activeElement));
  await popup.screenshot({path:join(output,'native-command-'+name+'.png')});
  for(const page of pages)assert.equal(await page.evaluate(()=>document.visibilityState),'visible');
  await popup.keyboard.press('Escape');
  await popup.locator('.command-scrim').waitFor({state:'detached'});
  assert.equal(await ui.getByRole('tab').count(),tabCount);
  for(let i=0;i<pages.length;i++){
    assert.deepEqual(await pages[i].evaluate(()=>({marker:window.__commandMarker,scroll:scrollY})),before[i]);
    assert.deepEqual(await pages[i].evaluate(()=>window.__commandVisibility),[]);
  }
  assert(await pages.at(-1).evaluate(()=>document.hasFocus()));
}
export async function checkCommandHandoff(ui,web,{waitFor}){
  const popup=popupPage(ui);
  await popup.evaluate(()=>{
    const decode=HTMLImageElement.prototype.decode;
    HTMLImageElement.prototype.decode=function(){return this.classList.contains('command-background')?Promise.reject(Error('command backgrounds must not be captured')):decode.call(this)};
    window.__restoreCommandDecode=()=>{HTMLImageElement.prototype.decode=decode};
  });
  for(let i=0;i<3;i++){
    await ui.keyboard.press('Control+l');
    await popup.getByRole('combobox').waitFor();
    await popup.keyboard.press('Escape');
    await popup.locator('.command-scrim').waitFor({state:'detached'});
  }
  const tabs=await ui.getByRole('tab').count();
  await ui.keyboard.press('Control+t');
  await popup.getByRole('combobox').waitFor();
  assert.equal(await popup.getByRole('combobox').inputValue(),'');
  await popup.locator('.command-scrim').click({position:{x:8,y:8}});
  await popup.locator('.command-scrim').waitFor({state:'detached'});
  assert.equal(await ui.getByRole('tab').count(),tabs);
  assert(await web.evaluate(()=>document.hasFocus()));
  await popup.evaluate(()=>window.__restoreCommandDecode());
}
