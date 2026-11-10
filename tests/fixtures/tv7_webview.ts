// All successful business responses below come from the production Host.
import { nextTick } from 'vue';
import { api } from '../../src/api';
import { appController as app, appState as state } from '../../src/state';
import { locale } from '../../src/i18n';
import { nativeInvoke, type Player } from '../../src/player';
import { maccmsScenario } from './tv4_webview';

const delay = (ms:number) => new Promise(resolve=>setTimeout(resolve,ms));
function check(value:unknown,message:string):asserts value {if(!value)throw new Error(message);}
const all = <T extends HTMLElement=HTMLElement>(selector:string) => [...document.querySelectorAll<T>(selector)].filter(e=>!e.closest('.page-transition-old'));
const one = <T extends HTMLElement=HTMLElement>(selector:string) => {const element=all<T>(selector)[0];check(element,`Missing ${selector}`);return element;};
async function until(predicate:()=>unknown,message:string) {const end=performance.now()+30000;while(!predicate()){if(performance.now()>end)throw new Error(`${message}; phase=${app.search.phase}; error=${app.search.error?.message}; alerts=${all('[role=alert]').map(e=>e.textContent).join(';')}`);await delay(20);}}
const measure = (selector:string) => {
  const element=one(selector),s=getComputedStyle(element);
  const keys=['color','background-color','font-size','font-weight','height','min-height','width','max-width','border-radius','border-top-width','border-top-color','padding-top','padding-bottom','padding-left','padding-right','margin-bottom','opacity','transform','transition-duration','-webkit-backdrop-filter','outline-width','outline-color','outline-offset','grid-template-columns','position','top'];
  return {rect:element.getBoundingClientRect().toJSON(),...Object.fromEntries(keys.map(key=>[key,s.getPropertyValue(key)]))};
};
type Call = {command:string;id:string;query?:string;target?:string;time:number;done?:boolean;code?:string};
const calls:Call[]=[]; let initialized=false,oldReceived=false,releaseOld:(()=>void)|undefined,holdOld=false;
let genreHover:any=null, disarmHover:(()=>void)|undefined;
function fill(selector:string,value:string) {const input=one<HTMLInputElement>(selector);input.value=value;input.dispatchEvent(new Event('input',{bubbles:true}));}
function choose(selector:string,value:string) {const input=one<HTMLSelectElement>(selector);input.value=value;input.dispatchEvent(new Event('change',{bubbles:true}));}
async function route(hash:string,selector:string) {location.hash=hash;await until(()=>location.hash===hash&&!!all(selector).length,`Route ${hash}`);await delay(450);}
async function search(query:string) {fill('.search-input',query);await until(()=>app.search.query===query&&['ready','empty'].includes(app.search.phase),'Search did not finish');await nextTick();await delay(100);}
async function searchPage(ready:any) {
  await route('#/search','.search-input');
  choose('.search-provider select',ready.metadata.id);await nextTick();
}
function snapshot(selectors:string[]) {return {styles:Object.fromEntries(selectors.map(s=>[s,measure(s)])),width:innerWidth,height:innerHeight,title:document.title,origin:location.origin,overflow:document.documentElement.scrollWidth>innerWidth+1};}
async function tab(name:string) {one<HTMLButtonElement>(`#library-tab-${name}`).click();await nextTick();await delay(100);}

export async function searchLibraryScenario(action:string,ready:any,prototype:{player:Player;base:string;cap:string}) {
  if(!initialized){
    await maccmsScenario('errors',prototype);
    const original=window.__TAURI__!.core;
    window.__TAURI__!.core={...original,invoke:async<T>(command:string,args?:Record<string,any>):Promise<T>=>{
      const call:Call={command,id:args?.request_id,query:args?.input?.query,target:args?.input?.target_request_id,time:performance.now()};calls.push(call);
      let response:any;
      if(command==='search_catalog') {
        const http=await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({command,args})});
        check(http.ok,'Search transport failed');response=await http.json();
      } else response=await original.invoke<T>(command,args);
      call.done=true;call.code=response?.error?.code;
      if(command==='search_catalog'&&args?.input.query==='old'&&holdOld){oldReceived=true;await new Promise<void>(resolve=>{releaseOld=resolve;});}
      return response;
    }};
    locale.value='en';initialized=true;
  }
  if(action==='prepare-shortcut'){await route('#/home','.home-catalog');return {ready:true};}
  if(action==='shortcut'){
    await until(()=>location.hash==='#/search'&&document.activeElement===one('.search-input'),'Native Cmd+F did not open/focus Search');
    return {hash:location.hash,focused:true,title:document.title};
  }
  if(action==='search-behavior'){
    await searchPage(ready);await search('baseline');
    const before=app.search.items.map(c=>c.id);calls.length=0;holdOld=true;oldReceived=false;
    fill('.search-input','o');await delay(90);fill('.search-input','ol');await delay(90);const typed=performance.now();fill('.search-input','old');await delay(240);
    const early=calls.filter(c=>c.command==='search_catalog').length;check(early===0,'Search fired before debounce');
    await until(()=>oldReceived,'Real old response was not held');
    const kept=app.search.items.map(c=>c.id),pending=getComputedStyle(one('.search-results')).opacity;
    fill('.search-input','fresh');
    await until(()=>app.search.query==='fresh'&&app.search.phase==='ready','Fresh search missing');
    check(releaseOld,'No old response to release');releaseOld();releaseOld=undefined;holdOld=false;await delay(150);
    const request=calls.find(c=>c.command==='search_catalog'&&c.query==='old')!;
    const rows=all('.search-results .media-row').map(e=>({kind:e.dataset.kind,title:e.querySelector('h2')!.textContent,count:e.querySelectorAll('.media-card').length}));
    const result={early,debounce_ms:request.time-typed,kept:JSON.stringify(before)===JSON.stringify(kept),pending_opacity:pending,
      cancelled:calls.some(c=>c.command==='cancel_request'&&c.target===request.id),latest:app.search.items.every(c=>c.title.startsWith('fresh ')),rows};
    await search('none');const empty={text:one('.search-empty').textContent,style:measure('.search-empty')};
    one<HTMLButtonElement>('.search-clear').click();await nextTick();const genres=all('.search-genre').length;
    choose('.search-provider select',ready.playback.id);await nextTick();await search('wd');
    const local={count:app.search.items.length,titles:app.search.items.map(c=>c.title),kinds:app.search.items.map(c=>c.kind)};
    one<HTMLButtonElement>('.search-clear').click();await nextTick();
    return {...result,empty,genres,local,errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='search-styles'){
    await searchPage(ready);fill('.search-input','');await nextTick();one<HTMLInputElement>('.search-input').focus();await nextTick();
    const focused=measure('.search-bar');one<HTMLInputElement>('.search-input').blur();
    const result=snapshot(['.search-bar','.search-input','.search-genre','.search-genre span','.search-genres']);
    const r=one('.search-genre').getBoundingClientRect();
    return {...result,focused,genres:all('.search-genre').length,hover:{x:r.x+r.width/2,y:r.y+r.height/2},errors:await maccmsScenario('errors',prototype),native:await nativeInvoke('t01_control',{action:'identity'})};
  }
  if(action==='search-hover-arm'){
    disarmHover?.();genreHover=null;const element=one('.search-genre');
    const entered=async(event:PointerEvent)=>{
      // Flush the native :hover style before waiting on this element's actual
      // transitions; no synthetic event or test-only class sets the state.
      getComputedStyle(element).transform;
      await Promise.allSettled(element.getAnimations().map(animation=>animation.finished));
      genreHover={...measure('.search-genre'),native_event:event.isTrusted,hovered:element.matches(':hover'),focused:document.hasFocus()};
    };
    element.addEventListener('pointerenter',entered,{once:true});disarmHover=()=>element.removeEventListener('pointerenter',entered);
    return {armed:true};
  }
  if(action==='search-hover'){await until(()=>genreHover!==null,'Native pointer never entered genre card');return genreHover;}
  if(action==='results-styles'){
    await searchPage(ready);await search('visual');
    await until(()=>all<HTMLImageElement>('.search-results .media-poster img').some(i=>i.complete&&i.naturalWidth>0),'Search posters did not decode');
    return {...snapshot(['.search-results','.search-results .media-row','.search-results .media-poster']),rows:all('.search-results .media-row').length,errors:await maccmsScenario('errors',prototype),native:await nativeInvoke('t01_control',{action:'identity'})};
  }
  if(action==='library-styles'){
    await route('#/history','.library-tabs');await until(()=>all('.history-card').length===2,'Continue list missing');
    const continuing={count:all('.history-card').length,poster:measure('.history-poster')};
    await tab('watchlist');await until(()=>all<HTMLImageElement>('.watchlist-poster img').some(i=>i.complete&&i.naturalWidth>0),'Watchlist wall missing');
    const watchlist={count:all('.watchlist-card').length,track:measure('.watchlist-track'),poster:measure('.watchlist-poster')};
    await tab('history');await until(()=>all('.history-card').length===3,'History missing');
    const r=one('.history-card').getBoundingClientRect();
    return {...snapshot(['.library-heading h1','.library-tabs','#library-tab-history','.history-group > h2','.history-poster','.history-delete','.history-clear']),continuing,watchlist,tabs:all('[role=tab]').map(t=>t.textContent),groups:all('.history-group > h2').map(e=>e.textContent),hover:{x:r.x+r.width/2,y:r.y+40},native:await nativeInvoke('t01_control',{action:'identity'}),errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='history-hover'){await delay(250);return measure('.history-delete');}
  if(action==='history-actions'){
    await route('#/history','.library-tabs');await tab('history');
    one<HTMLButtonElement>('.history-delete').click();await until(()=>!!all('.history-confirmation').length,'Delete dialog missing');await delay(200);
    const modal=measure('.history-confirmation');
    one<HTMLButtonElement>('.history-confirmation .n-dialog__action button:first-child').click();await until(()=>!all('.history-confirmation').length,'Cancel dialog missing');
    const cancelled=(await api.call('list_history',{})).entries.length;
    one<HTMLButtonElement>('.history-delete').click();await until(()=>!!all('.history-confirmation').length,'Delete confirmation missing');
    one<HTMLButtonElement>('.history-confirmation .n-dialog__action button:last-child').click();await until(()=>all('.history-card').length===2,'Single deletion not reflected');
    const removed=(await api.call('list_history',{})).entries.length;
    one<HTMLButtonElement>('.history-clear').click();await until(()=>!!all('.history-confirmation').length,'Clear dialog missing');
    one<HTMLButtonElement>('.history-confirmation .n-dialog__action button:last-child').click();await until(()=>!all('.history-card').length,'History clear did not finish');
    return {modal,cancelled,removed,cleared:(await api.call('list_history',{})).entries.length,watchlist:(await api.call('list_watchlist',{})).entries.length,errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='sources-styles'){
    await route('#/sources','.native-sources');
    const result=snapshot(['.native-sources','.native-sources h1','.settings-group > h2','.settings-panel','.sources-row','.sources-row h3','.sources-row p','.source-status','.sources-pickers select']);
    one<HTMLButtonElement>('.sources-add').click();await until(()=>!!all('.sources-dialog').length,'Source glass dialog missing');await delay(250);
    const dialog=measure('.sources-dialog'),input=measure('.sources-name');one<HTMLButtonElement>('.sources-form-cancel').click();await until(()=>!all('.sources-dialog').length,'Source cancel failed');
    return {...result,dialog,input,actions:all('.sources-row-actions button').length,native:await nativeInvoke('t01_control',{action:'identity'}),errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='settings-styles'){
    await route('#/settings','.tv-settings');await until(()=>!!all('.media-cache-used').length,'Cache status missing');
    return {...snapshot(['.tv-settings','.tv-settings h1','.settings-group > h2','.settings-panel','.setting-row','.setting-row h3','.setting-row p','.setting-select:not(.narrow-language)','.palette-swatch','.palette-swatch[aria-checked=true]','.settings-language','.settings-language [aria-checked=true]']),swatches:all('.palette-swatch').length,native:await nativeInvoke('t01_control',{action:'identity'}),errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='settings-actions'){
    await route('#/settings','.tv-settings');
    const autoBefore=state.settings.auto_next;one<HTMLButtonElement>('.setting-row .n-switch').click();await until(()=>state.settings.auto_next!==autoBefore,'Auto-next did not persist');
    choose('select[aria-label="Default playback speed"]','1.5');await nextTick();const speed=state.playback.speed;
    const palettes:string[]=[];for(const button of all<HTMLButtonElement>('.palette-swatch')){button.click();await until(()=>button.getAttribute('aria-checked')==='true','Palette did not persist');palettes.push(state.settings.color_palette!);}
    one<HTMLButtonElement>('.settings-language button:last-child').click();await until(()=>state.settings.ui_locale==='zh-CN'&&document.documentElement.lang==='zh-CN','Chinese did not persist');
    const chinese=document.title;one<HTMLButtonElement>('.settings-language button:first-child').click();await until(()=>state.settings.ui_locale==='en','English did not persist');
    fill('#media-cache-max','-1');one<HTMLButtonElement>('[data-action=cache-save]').click();await until(()=>!!all('.media-cache-error').length,'Invalid cache size accepted');const invalid=one('.media-cache-error').textContent;
    fill('#media-cache-max','64');one<HTMLButtonElement>('[data-action=cache-save]').click();await until(()=>state.settings.media_cache_max_bytes===67108864&&!!all('.media-cache-notice').length,'Cache limit did not persist');
    one<HTMLButtonElement>('[data-action=cache-clear]').click();await until(()=>all('.media-cache-notice').some(e=>e.textContent?.includes('cleared')),'Cache clear missing');
    const saved=await api.call('get_settings',{}),cache=await api.call('get_media_cache',{});
    return {auto_before:autoBefore,auto:saved.auto_next,speed,palettes,chinese,invalid,max:saved.media_cache_max_bytes,cache: {used:cache.used_bytes,pending:cache.pending_bytes},errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='sources-actions'){
    await route('#/sources','.native-sources');const before=state.settings.source_configs.length;
    one<HTMLButtonElement>('.sources-add').click();await until(()=>!!all('.sources-dialog').length,'Add dialog missing');
    one<HTMLButtonElement>('.sources-save').click();await until(()=>!!all('.sources-errors').length,'Source validation missing');const invalid=one('.sources-errors').textContent;
    fill('.sources-name','TV7 added source');fill('.sources-urls',ready.playback.origin.api_url.replace(/\/api$/,'/fail'));one<HTMLButtonElement>('.sources-save').click();
    await until(()=>state.settings.source_configs.length===before+1&&!all('.sources-dialog').length&&!!all('.sources-row-error').length,'Failed source row missing');
    let config=state.settings.source_configs.find(c=>c.name==='TV7 added source')!;check(config,'Added configuration missing');
    const row=()=>one(`[data-config-id="${config.config_id}"]`);
    const idle=()=>until(()=>!row().querySelector<HTMLButtonElement>('.sources-toggle')!.disabled,'Source operation did not finish');
    await idle();
    const failed={dot:measure(`[data-config-id="${config.config_id}"] .source-status`),error:measure(`[data-config-id="${config.config_id}"] .sources-row-error`)};
    row().querySelector<HTMLButtonElement>('.sources-edit')!.click();await until(()=>!!all('.sources-dialog').length,'Edit dialog missing');
    fill('.sources-name','TV7 edited source');fill('.sources-urls',ready.playback.origin.api_url);one<HTMLButtonElement>('.sources-save').click();await until(()=>state.settings.source_configs.some(c=>c.name==='TV7 edited source')&&!all('.sources-dialog').length&&!all('.sources-row-error').length,'Edit source failed');
    await idle();
    row().querySelector<HTMLButtonElement>('.sources-toggle')!.click();await until(()=>state.settings.source_configs.find(c=>c.config_id===config.config_id)?.enabled===false,'Disable source failed');const disabled=measure(`[data-config-id="${config.config_id}"] .source-status`);
    await idle();
    row().querySelector<HTMLButtonElement>('.sources-toggle')!.click();await until(()=>state.settings.source_configs.find(c=>c.config_id===config.config_id)?.enabled===true,'Enable source failed');
    await idle();
    row().querySelector<HTMLButtonElement>('.sources-delete')!.click();await until(()=>!!all('.sources-confirm').length,'Delete dialog missing');one<HTMLButtonElement>('.sources-confirm-cancel').click();await until(()=>!all('.sources-confirm').length,'Delete cancel failed');
    const cancelled=state.settings.source_configs.length;row().querySelector<HTMLButtonElement>('.sources-delete')!.click();await until(()=>!!all('.sources-confirm').length,'Delete dialog missing');one<HTMLButtonElement>('.sources-confirm-delete').click();await until(()=>state.settings.source_configs.length===before,'Delete source failed');
    one<HTMLButtonElement>('.sources-copy').click();await until(()=>!!all('.sources-copy-state').length,'Copy diagnostics feedback missing');
    return {before,added:before+1,cancelled,after:state.settings.source_configs.length,invalid,disabled,failed,presets:all('.sources-preset-add').length,diagnostics:!!one('.sources-report').textContent,copy_feedback:one('.sources-copy-state').textContent,errors:await maccmsScenario('errors',prototype)};
  }
  if(action==='errors')return maccmsScenario('errors',prototype);
  throw new Error(`Unknown TV7 action ${action}`);
}
