// Test transport only; all visible controls, media, Host DTOs and history use production code.
import { createApp, h, nextTick, type App as VueApp } from 'vue';
import App from '../../src/App.vue';
import UiProvider from '../../src/ui/UiProvider.vue';
import { appController, appState, navigatePlayback } from '../../src/state.ts';
import { api } from '../../src/api.ts';
import { locale } from '../../src/i18n';
import { nativeInvoke, type Player } from '../../src/player.ts';
import { videoJsControl } from './videojs_controls.ts';
import type { AppSourceList, DetailsViewData, PlaybackContext, PlaybackMatchInput } from '../../src/models.ts';

type Bootstrap = { initial_context: PlaybackContext; match_inputs: PlaybackMatchInput[]; details: DetailsViewData; sources: AppSourceList };
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
const one = <T extends HTMLElement = HTMLElement>(selector: string) => { const el = document.querySelector<T>(selector); check(el, `Missing ${selector}`); return el!; };
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
async function until(condition: () => unknown, message: string, timeout = 20000) {
  const end = performance.now() + timeout;
  while (!condition()) { check(performance.now() < end, `${message}; phase=${appState.playback.phase}; error=${JSON.stringify(appState.playback.error)}; plays=${JSON.stringify(playPromises)}`); await delay(20); }
}
const media = () => one<HTMLVideoElement>('video');
const store = () => (one('video-player') as HTMLElement & { store: any }).store;
let app: VueApp | undefined, original: NonNullable<Window['__TAURI__']>['core'];
let payload: Bootstrap, releaseLoading: (() => void) | undefined, loadingStarted = 0, failResolve = false;
const calls: { command: string; video?: string; completed?: boolean }[] = [], errors: string[] = [];
const mediaEvents: Record<string,unknown>[] = [];
const playPromises: Record<string,unknown>[] = [];
const escapeKeys: Record<string,unknown>[] = [];
let removeErrors = () => {};
const matches = () => calls.filter(row => row.command === 'match_playback').length;
const focus = () => one('media-container').focus();
async function playing() {
  await until(() => appState.playback.session && media().readyState >= 2, 'Real media did not load');
  await until(() => store().paused === media().paused, 'Official paused state did not settle');
  if (media().paused) videoJsControl('play').click();
  try { await until(() => !media().paused && media().currentTime > .1 && appState.playback.phase === 'playing', 'Official play did not play'); }
  catch(cause) { throw new Error(`${cause}; media=${JSON.stringify({ paused:media().paused,time:media().currentTime,ready:media().readyState,network:media().networkState,duration:media().duration,storePaused:store().paused,buffered:[...Array(media().buffered.length)].map((_,i)=>[media().buffered.start(i),media().buffered.end(i)]),events:mediaEvents })}`); }
}
async function seek(seconds: number) {
  media().currentTime = seconds;
  await until(() => !media().seeking && Math.abs(media().currentTime - seconds) < .2, 'Real seek did not settle');
  await until(() => Math.abs(appState.playback.position_ms / 1000 - media().currentTime) < .3, 'Settled seek did not reach business state');
}
async function rematch(index = 0) {
  one<HTMLButtonElement>('.playback-tray-close')?.click();
  await appController.match(payload.match_inputs[index]!.video_id);
  await playing(); media().loop = true;
}
function style(selector: string, pseudo?: string) {
  const el = one(selector), s = getComputedStyle(el, pseudo), r = el.getBoundingClientRect();
  const names = ['backgroundColor','color','fontSize','fontWeight','fontVariantNumeric','width','height','borderRadius','borderTopLeftRadius','borderTopRightRadius','paddingTop','paddingRight','paddingBottom','paddingLeft','top','left','right','bottom','opacity','visibility','transitionDuration','filter','webkitBackdropFilter','objectFit','textShadow','transform','cursor'];
  return { ...Object.fromEntries(names.map(name => [name, s.getPropertyValue(name === 'webkitBackdropFilter' ? '-webkit-backdrop-filter' : name.replace(/[A-Z]/g, value => `-${value.toLowerCase()}`))])), rect: { x: r.x, y: r.y, width: r.width, height: r.height } };
}
function snapshot() {
  return { route: appState.route.name, hash: location.hash, phase: appState.playback.phase, position: media()?.currentTime, duration: media()?.duration,
    paused: media()?.paused, volume: media()?.volume, muted: media()?.muted, speed: media()?.playbackRate,
    video: appState.playback.context?.metadata_video_id, matches: matches(), fullscreen: store()?.isFullscreen,
    tray: one('.playback-tray').classList.contains('open'), visible: store()?.controlsVisible,
    countdown: document.querySelector('.playback-countdown')?.textContent ?? null, next: Boolean(document.querySelector('.playback-next')) };
}
async function end() {
  media().loop = false;
  const events: { trusted: boolean; epoch: number; at: number }[] = [];
  const listener = (event: Event) => events.push({ trusted: event.isTrusted, epoch: appState.playback.playback_epoch, at: performance.now() });
  const video = media(); video.addEventListener('ended', listener);
  try { await playing(); await seek(video.duration - .3); await until(() => appState.playback.phase === 'ended', 'Real EOS did not arrive'); }
  finally { video.removeEventListener('ended', listener); }
  check(events.length === 1 && events[0]!.trusted, 'EOS must be one trusted native event');
  return events[0]!;
}

export async function immersiveScenario(command: { action?: string; payload?: unknown }, prototype: { player: Player; base: string; cap: string }) {
  const action = command.action;
  if (action === 'start') {
    payload = command.payload as Bootstrap;
    prototype.player.stop();
    const root = document.getElementById('app') as HTMLElement & { __vue_app__?: VueApp };
    root.__vue_app__!.unmount(); original = window.__TAURI__!.core;
    calls.length = 0; errors.length = 0; mediaEvents.length=0;playPromises.length=0;
    escapeKeys.length=0;
    const gate = new Promise<void>(resolve => { releaseLoading = resolve; });
    let firstResolve = true;
    window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
      if (['t01_control','player_fullscreen','window_bounds_now','app_close_started','app_close_aborted','app_close_ready'].includes(name)) return original.invoke<T>(name, args);
      calls.push({ command: name, video: args?.input?.video_id, completed: args?.input?.record?.completed });
      if (name === 'resolve_episode' && firstResolve) { firstResolve = false; loadingStarted = performance.now(); await gate; }
      if (name === 'resolve_episode' && failResolve) { failResolve = false; return { request_id: args?.request_id, engine_epoch: appState.runtime.engine_epoch, error: { code: 'UPSTREAM_FAILED', message: 'TV6 controlled resolver failure', retryable: true } } as T; }
      const input = name === 'match_playback' && !args?.input?.selection ? { ...args, input: payload.match_inputs.find(row => row.video_id === args?.input?.video_id) ?? args?.input } : args;
      const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: 'POST', headers: { 'Content-Type':'application/json' }, body: JSON.stringify({ command: name, args: input }) });
      check(response.ok, 'Host fixture transport failed');
      const result=await response.json();
      if(name==='resolve_episode'&&result.data)result.data.streams.push({id:'tv6-unplayable',label:'受控不可播放线路',kind:'drm',quality_label:'DRM',has_interceptor:false,playable:false,reason:'当前设备不支持DRM'});
      return result;
    } };
    const onError = (event: ErrorEvent) => errors.push(event.message);
    const onReject = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
    window.addEventListener('error', onError); window.addEventListener('unhandledrejection', onReject);
    const onKey = (event: KeyboardEvent) => {
      if (!['Escape', 'm', 'M'].includes(event.key)) return;
      const row = { key:event.key, type:event.type, trusted:event.isTrusted, target:(event.target as Element)?.tagName, prevented:event.defaultPrevented, at:performance.now() };
      escapeKeys.push(row); if(escapeKeys.length>16)escapeKeys.shift();
      queueMicrotask(()=>{row.prevented=event.defaultPrevented;});
    };
    window.addEventListener('keydown',onKey,true);window.addEventListener('keyup',onKey,true);
    const oldWarn = console.warn, oldError = console.error;
    console.warn = (...args) => { errors.push(`warn: ${args.map(String).join(' ')}`); oldWarn(...args); };
    console.error = (...args) => { errors.push(`error: ${args.map(String).join(' ')}`); oldError(...args); };
    removeErrors = () => { window.removeEventListener('error', onError); window.removeEventListener('unhandledrejection', onReject);window.removeEventListener('keydown',onKey,true);window.removeEventListener('keyup',onKey,true); console.warn = oldWarn; console.error = oldError; };
    await appController.restoreSettings(); await appController.refreshRuntime();
    locale.value = 'zh-CN';
    await appController.updateSettings(s => ({ ...s, ui_locale: 'zh-CN', auto_next: false, volume: .5, muted: false, media_cache_max_bytes: 0 }));
    appState.sources.data = payload.sources;
    const metadata = payload.match_inputs[0]!.metadata_ref;
    appState.selected_metadata_source = { source_id: metadata.content_ref.source_id, source_revision: metadata.content_ref.source_revision };
    appState.selected_playback_source = clone(payload.match_inputs[0]!.playback_source);
    const details = clone(payload.details);
    details.metadata_ref = metadata;
    details.details.title = 'TV6 自有测试影片';
    details.details.groups = [{ ...details.details.groups[0]!, title: '第一季', episodes: payload.match_inputs.slice(0,4).map((input,index) => ({ ...details.details.groups[0]!.episodes[0]!, id: `tv6-${index}`, label: `第${index + 1}集`, index, ref: { ...metadata.content_ref, data: JSON.stringify(['stremio_video','addon','series','meta',input.video_id]) } })) }];
    appState.details.data = details; appState.details.phase = 'ready';
    navigatePlayback(payload.initial_context);
    app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) }); app.mount(root); await nextTick();
    await until(() => loadingStarted && document.querySelector('.playback-backdrop'), 'Resolver did not expose loading backdrop');
    const video=media();
    const play=video.play;
    video.play=function(){
      const row:Record<string,unknown>={at:performance.now(),state:'pending'};playPromises.push(row);if(playPromises.length>32)playPromises.shift();
      const result=play.call(this);void result.then(()=>Object.assign(row,{state:'resolved',settled:performance.now()}),error=>Object.assign(row,{state:'rejected',error:String(error).replace(/https?:\/\/[^\s"'<>]+/g,'<media-resource>'),settled:performance.now()}));return result;
    };
    for(const name of ['loadstart','loadedmetadata','loadeddata','canplay','play','playing','pause','waiting','seeking','seeked','ended','error']) video.addEventListener(name,event=>{
      mediaEvents.push({ type:name,trusted:event.isTrusted,at:performance.now(),time:video.currentTime,paused:video.paused,ready:video.readyState }); if(mediaEvents.length>64)mediaEvents.shift();
    });
    await until(() => one<HTMLImageElement>('.playback-backdrop').complete && one<HTMLImageElement>('.playback-backdrop').naturalWidth > 0, 'Loading backdrop did not decode');
    return { phase: appState.playback.phase, title: one('.playback-loading h1').textContent, episode: one('.playback-loading p').textContent, backdrop: style('.playback-backdrop'), titleStyle: style('.playback-loading h1'), episodeStyle: style('.playback-loading p'), spinner: style('.loading-spinner'), window: { width: innerWidth, height: innerHeight } };
  }
  check(app, 'TV6 scenario not started');
  if (action === 'long-loading') {
    await delay(Math.max(0, 10300 - (performance.now() - loadingStarted)));
    return { elapsed: performance.now() - loadingStarted, text: one('.playback-long-loading').textContent, visible: getComputedStyle(one('.playback-loading')).display !== 'none' };
  }
  if(action === 'loading-no-image') {
    const details=appState.details.data!.details, poster=details.poster, backdrop=details.backdrop;
    details.poster=null;details.backdrop=null;await nextTick();
    const result={ title:one('.playback-loading h1').textContent, background:getComputedStyle(one('.playback-loading')).backgroundColor, images:document.querySelectorAll('.playback-backdrop').length };
    details.poster=poster;details.backdrop=backdrop;await nextTick();return result;
  }
  if (action === 'release') {
    releaseLoading!(); await playing(); media().loop = true; media().pause();
    await until(() => appState.playback.phase === 'paused', 'Pause failed'); await delay(250);
    return { ...snapshot(), error:appState.playback.error, errors:[...errors], plays:playPromises.map(row => ({ ...row })) };
  }
  if (action === 'snapshot') { await delay(400); return snapshot(); }
  if (action === 'focus') { focus(); return snapshot(); }
  if (action === 'styles') {
    media().pause(); await until(() => appState.playback.phase === 'paused', 'Style pause failed'); await delay(250);
    await seek(.5);
    if (!Array.from(media().textTracks).some(track => track.mode === 'showing')) one<HTMLElement>('media-captions-button').click();
    await until(() => Array.from(media().textTracks).some(track => track.mode === 'showing' && track.activeCues?.length), 'Native subtitle cue missing');
    const selectors = ['.playback-view','video','.playback-shade','.playback-header','.playback-back','.playback-header h2','.playback-header p','.playback-bar','.playback-track','media-slider-buffer','media-slider-fill','media-slider-thumb','.playback-preview','media-time','.playback-button','media-play-button media-icon','.playback-volume-slider','.playback-tray','.playback-tray-header > button','.playback-now','.playback-buffering','.playback-buffering .playback-spinner'];
    const styles = Object.fromEntries(selectors.map(selector => [selector, style(selector)]));
    await until(()=>document.querySelector('.playback-captions')?.textContent?.includes('映象'),'Official active cue did not reach the skin');
    const cue = style('.playback-captions'), textTrack = cue;
    return { width: innerWidth, height: innerHeight, styles, cue, textTrack, bufferDelay:(one('.playback-buffering') as HTMLElement&{delay:number}).delay, videoCount: document.querySelectorAll('video').length, controlsCount: document.querySelectorAll('media-controls').length, legacy: Boolean(document.querySelector('video-compat-skin,.app-shell,.playback-preferences:not(.playback-tray *)')), native: await nativeInvoke('t01_control',{ action:'identity' }), errors: [...errors] };
  }
  if (action === 'rect') { return style(String(command.payload)).rect; }
  if (action === 'hover-style') { await delay(250); return { track: style('.playback-seek > .playback-track'), thumb: style('.playback-seek > media-slider-thumb'), preview: style('.playback-preview'), volume: style('.playback-volume-slider') }; }
  if (action === 'controls') {
    focus(); media().pause(); await seek(12);
    check((one('.playback-seek-forward') as HTMLElement & {seconds:number}).seconds===10 && (one('.playback-seek-back') as HTMLElement & {seconds:number}).seconds===-10,'Official seek configuration differs');
    one<HTMLElement>('.playback-seek-forward').click(); await until(() => !media().seeking && media().currentTime >= 21.9, 'Forward 10s failed'); const forward = media().currentTime;
    one<HTMLElement>('.playback-seek-back').click(); await until(() => !media().seeking && media().currentTime < 12.2, 'Back 10s failed'); const backward = media().currentTime;
    if(Array.from(media().textTracks).some(track=>track.mode==='showing')) {one<HTMLElement>('media-captions-button').click();await until(()=>Array.from(media().textTracks).every(track=>track.mode==='disabled'),'Caption initial off state did not settle');}
    one<HTMLElement>('media-captions-button').click(); await until(() => Array.from(media().textTracks).some(track => track.mode === 'showing'), 'Captions did not turn on');
    const shown = Array.from(media().textTracks).some(track => track.mode === 'showing');
    one<HTMLElement>('media-captions-button').click(); await until(() => Array.from(media().textTracks).every(track => track.mode === 'disabled'), 'Captions did not turn off');
    const speeds = [];
    for (const rate of [.5,1,1.25,1.5,2]) { one<HTMLElement>('media-playback-rate-button').click(); await until(() => document.querySelector('media-menu[data-open]'), 'Rate menu did not open'); one<HTMLElement>(`media-menu-radio-item[data-rate="${rate}"]`).click(); await until(() => media().playbackRate === rate, 'Rate did not reach media'); speeds.push(media().playbackRate); }
    media().playbackRate = 1;
    const thumb = videoJsControl('volume').querySelector<HTMLElement>('media-slider-thumb')!;
    videoJsControl('mute').focus(); await delay(250); thumb.focus();
    thumb.dispatchEvent(new KeyboardEvent('keydown',{ key:'Home',bubbles:true,cancelable:true })); await until(() => media().volume === 0, 'Volume minimum failed');
    thumb.dispatchEvent(new KeyboardEvent('keydown',{ key:'End',bubbles:true,cancelable:true })); await until(() => media().volume === 1, 'Volume maximum failed');
    media().volume = .5; focus();
    return { forward, backward, shown, off: Array.from(media().textTracks).every(track => track.mode === 'disabled'), speeds, volume: media().volume, registered: ['media-play-button','media-time-slider','media-seek-button','media-captions-button','media-gesture','media-hotkey','media-buffering-indicator'].every(tag => Boolean(customElements.get(tag))), owner: document.querySelectorAll('video').length, errors: [...errors] };
  }
  if (action === 'autohide') {
    await playing(); focus(); one('media-container').dispatchEvent(new PointerEvent('pointermove',{ pointerType:'mouse',bubbles:true }));
    const started = performance.now(); await delay(2300); const before = snapshot(); await delay(1050); const hidden = snapshot();
    const cursor = getComputedStyle(media()).cursor, opacity = getComputedStyle(one('.playback-chrome')).opacity;
    one('media-container').dispatchEvent(new PointerEvent('pointermove',{ pointerType:'mouse',bubbles:true })); await delay(40); const restored = snapshot();
    await delay(3350); videoJsControl('play').focus(); await delay(40); const focused = snapshot(); media().pause();
    return { before, hidden, cursor, opacity, restored, focused, elapsed: performance.now() - started };
  }
  if (action === 'tray') {
    one<HTMLButtonElement>('[data-player-tray]').click(); await delay(350);
    const opened = snapshot(), trayStyle = style('.playback-tray');
    one<HTMLButtonElement>('[data-tray-tab=streams]').click(); await nextTick();
    const resolution = appState.playback.resolution!.id, session = appState.playback.session!.session_id;
    const lines = [...document.querySelectorAll<HTMLButtonElement>('.playback-stream')].map(el => ({ label: el.textContent, height: el.getBoundingClientRect().height, disabled: el.disabled }));
    const disabled=one<HTMLButtonElement>('.playback-stream:disabled'),disabledStyle=style('.playback-stream:disabled');
    const opensBefore=calls.filter(call=>call.command==='open_playback').length;disabled.click();await delay(50);
    const disabledBlocked=calls.filter(call=>call.command==='open_playback').length===opensBefore;
    one<HTMLButtonElement>('.playback-stream:nth-child(2)').click(); await until(() => appState.playback.session?.session_id !== session && media().readyState >= 2, 'Tray stream did not replace Session');
    const sameResolution = appState.playback.resolution!.id === resolution, selected = document.querySelectorAll('.playback-stream[aria-current=true]').length;
    // Stream reload closes the tray with its epoch; reopen through the user control.
    if (!one('.playback-tray').classList.contains('open')) one<HTMLButtonElement>('[data-player-tray]').click();
    one<HTMLButtonElement>('[data-tray-tab=tracks]').click(); await nextTick();
    const tracks = one('media-captions-radio-group').querySelectorAll('media-menu-radio-item').length;
    one<HTMLButtonElement>('[data-tray-tab=episodes]').click(); await nextTick();
    const beforeMatches = matches(); one<HTMLButtonElement>(`.playback-episode[data-video-id="${payload.match_inputs[1]!.video_id}"]`).click();
    await until(() => appState.playback.context?.metadata_video_id === payload.match_inputs[1]!.video_id && media().readyState >= 2, 'Episode tray did not switch');
    return { opened, trayStyle, lines, disabledStyle,disabledBlocked,unplayableStimulus:'one injected unplayable DTO; playable streams and Session remain real Host',sameResolution, sessionChanged: session !== appState.playback.session!.session_id, selected, tracks, matches: matches()-beforeMatches, episode: snapshot(), businessInTray: [...document.querySelectorAll('[data-action]')].every(el => Boolean(el.closest('.playback-tray'))), error: [...errors] };
  }
  if (action === 'next') {
    await appController.updateSettings(s => ({ ...s, auto_next:false })); await rematch(); media().pause(); await until(()=>appState.playback.phase==='paused','Pause before next card failed'); await seek(media().duration - 20.7); await nextTick(); const before = snapshot();
    await seek(media().duration - 19.7); await nextTick(); const visible = snapshot(), card = style('.playback-next'), image = style('.playback-next > img'), kicker = style('.playback-next-kicker'), title = style('.playback-next h3');
    const cancel = () => one<HTMLButtonElement>('.playback-next-actions > button:nth-child(2)').click();
    const count = matches(); const offEos = await end(); await delay(5400); const off = snapshot();
    await appController.updateSettings(s => ({ ...s, auto_next:true })); await rematch(); const cancelledEos = await end(); const initialCountdown = snapshot(); await delay(1300); cancel(); await delay(4200); const cancelled = snapshot();
    await rematch(); const autoEos = await end(); await until(() => appState.playback.context?.metadata_video_id === payload.match_inputs[1]!.video_id && media().readyState >= 2, 'EOS countdown did not advance',12000); const advanced = { ...snapshot(), elapsed:performance.now()-autoEos.at }; media().loop = true;
    await rematch(); const staleEos = await end(); await rematch(2); const staleCount = matches(); await delay(5400); const stale = snapshot();
    await appController.updateSettings(s => ({ ...s, auto_next:false }));
    return { before,visible,card,image,kicker,title,offEos,off,offMatches:count,initialCountdown,cancelledEos,cancelled,autoEos,advanced,staleEos,stale,staleCount, completedSaves:calls.filter(row => row.command === 'save_history' && row.completed).length };
  }
  if (action === 'error') {
    failResolve = true; await rematchError(); await until(() => document.querySelector('.playback-error'), 'Resolver failure overlay missing');
    const panel = style('.playback-error'), title = style('.playback-error h2'), description = style('.playback-error p');
    const buttons = [...one('.playback-error').querySelectorAll('button')].map(el => el.textContent);
    one<HTMLButtonElement>('.playback-error button:first-child').click(); await playing();
    return { panel,title,description,buttons,recovered:!document.querySelector('.playback-error') && appState.playback.phase === 'playing',stimulus:'injected retryable resolver envelope; retry uses real Host and media' };
  }
  if (action === 'escape-start') {
    escapeKeys.length=0; focus();
    if (!store().isFullscreen) {
      videoJsControl('fullscreen').click(); await until(() => store().isFullscreen, 'Native fullscreen did not enter');
      // As in T01, let AppKit finish the native Space transition before input.
      await delay(1500);
    }
    one<HTMLButtonElement>('[data-player-tray]').click(); await delay(350);
    return { ...snapshot(), native:await nativeInvoke('t01_control',{ action:'identity' }) };
  }
  if (action === 'escape-state' || action === 'input-state' || action === 'dom-focus') {
    if (action === 'dom-focus') {
      window.focus();
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
      focus();
    }
    await delay(1400);
    const native=await nativeInvoke<{visible:boolean;focused:boolean;minimized:boolean}>('t01_control',{action:'identity'});
    const foreground={native,visibility:document.visibilityState,focused:document.hasFocus()};
    if(action==='escape-state') check(native.visible && native.focused && !native.minimized && document.visibilityState === 'visible' && document.hasFocus(),
      `Escape observation requires the same visible focused native window: ${JSON.stringify(foreground)}`);
    if (!document.querySelector('video-player')) return { route:appState.route.name,hash:location.hash,videos:document.querySelectorAll('video').length,errors:[...errors] };
    return { ...snapshot(), keys:escapeKeys.map(row=>({...row})), active:{ tag:document.activeElement?.tagName, class:document.activeElement?.className }, native, foreground, menus:[...document.querySelectorAll('media-menu[data-open],media-tooltip[data-open]')].map(el=>el.tagName) };
  }
  if (action === 'dispose') {
    await appController.updateSettings(s => ({ ...s, ui_locale:'en', auto_next:false })); locale.value = 'en';
    releaseLoading?.(); app!.unmount(); app=undefined;
    await delay(600); window.__TAURI__!.core = original; removeErrors();
    return { videos:document.querySelectorAll('video').length,errors:[...errors] };
  }
  throw new Error(`Unknown TV6 action ${action}`);
}
async function rematchError() { await appController.match(payload.match_inputs[0]!.video_id); }
