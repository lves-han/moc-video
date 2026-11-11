// The real App and official player use TV5's production Host, gateway and SQLite.
import { nextTick } from 'vue';
import { api } from '../../src/api';
import { appController as app, appState as state } from '../../src/state';
import { locale } from '../../src/i18n';
import { nativeInvoke, type Player } from '../../src/player';
import { maccmsScenario } from './tv4_webview';
import { videoJsControl } from './videojs_controls';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const all = <T extends HTMLElement = HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)].filter(element => !element.closest('.page-transition-old'));
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const one = <T extends HTMLElement = HTMLElement>(selector: string) => { const value = all<T>(selector)[0]; check(value, `Missing ${selector}`); return value; };
async function until(predicate: () => unknown, message: string, seconds = 30) {
  const end = performance.now() + seconds * 1000;
  while (!predicate()) { if (performance.now() > end) throw new Error(`${message}; ${JSON.stringify({ hash: location.hash, details: state.details.phase, playback: state.playback.phase, error: state.playback.error, alerts: all('[role="alert"]').map(e => e.textContent) })}`); await delay(15); }
}
async function settled() { await until(() => !document.querySelector('.page-transition-old'), 'Crossfade did not clean up'); await delay(450); }
function measure(selector: string, pseudo?: string) {
  const element = one(selector), style = getComputedStyle(element, pseudo);
  const keys = ['color','background-color','font-size','font-weight','line-height','height','min-height','width','min-width','max-width','max-height','border-radius','padding-left','padding-right','padding-top','padding-bottom',
    'left','right','top','bottom','object-fit','filter','transform','display','overflow-x','animation-name','animation-duration','animation-delay','transition-duration','-webkit-line-clamp','grid-template-columns','column-gap','view-transition-name','-webkit-backdrop-filter','background-image'];
  return { rect: element.getBoundingClientRect().toJSON(), ...Object.fromEntries(keys.map(key => [key, key === 'background-image' ? style.getPropertyValue(key).replace(/url\([^)]*\)/g,'url(<gateway-image>)') : style.getPropertyValue(key)])) };
}
let initialized = false;
const transitions: any[] = [];
async function home() {
  location.hash = '#/home';
  await until(() => state.route.name === 'home' && all('.home-catalog .media-poster').length > 0 && !document.querySelector('.home-catalog > .catalog-status'), 'Home cards missing'); await settled();
}
async function open(ready: any) {
  const target = `#/details/${encodeURIComponent(JSON.stringify(ready.ref))}`;
  if (location.hash !== target) {
    await home();
    const poster = all<HTMLAnchorElement>('.media-poster').find(element => element.getAttribute('href') === target); check(poster,'Seeded series poster missing'); poster.click();
  }
  await until(() => state.route.name === 'details' && state.details.phase === 'ready' && state.details.data?.details.id === ready.record.content_id
    && all('.details-play').some(e => e.textContent?.includes('Episode 3')), 'Details history label missing'); await settled();
}

export async function detailsScenario(action: string, ready: any, prototype: { player: Player; base: string; cap: string }) {
  if (!initialized) {
    await maccmsScenario('errors', prototype);
    const start = document.startViewTransition?.bind(document);
    if (start) document.startViewTransition = ((update: () => Promise<void>) => {
      const before = all('.poster-link,.details-head').filter(e => getComputedStyle(e).viewTransitionName === 'details-art').length;
      const transition = start(update);
      void transition.ready.then(() => transitions.push({ ready: true, before, after: all('.poster-link,.details-head').filter(e => getComputedStyle(e).viewTransitionName === 'details-art').length,
        duration: getComputedStyle(document.documentElement,'::view-transition-group(details-art)').animationDuration }), error => transitions.push({ ready: false, reason: String(error) }));
      return transition;
    }) as typeof document.startViewTransition;
    initialized = true;
  }
  if (action === 'styles') {
    locale.value = 'en'; await open(ready); scrollTo({ top: 0, behavior: 'instant' });
    await until(() => one<HTMLImageElement>('.details-backdrop').complete && one<HTMLImageElement>('.details-backdrop').naturalWidth > 0, 'Backdrop did not decode');
    const selectors = ['.details-head','.details-backdrop','.details-bottom-mask','.details-left-mask','.details-copy','.details-copy h1','.details-meta','.details-rating','.details-plot','.details-play','.details-play-progress','.details-watchlist','.watchlist-symbol',
      '.details-back','.details-groups','.details-group[aria-pressed="true"]','.details-group[aria-pressed="false"]','.details-episode','.details-episode-art','.details-episode strong','.details-episode-plot','.details-episode-duration','.details-watching','.details-episode-progress',
      '.details-episode-fallback','.details-episode-number','.details-actor img','.details-actor-initial','.details-actor p','.details-actor-role','.details-information dl','.details-information dt','.details-information dd','.details-related .media-poster','.details-content'];
    return { width: innerWidth, height: innerHeight, styles: Object.fromEntries(selectors.map(selector => [selector, measure(selector)])),
      label: one('.details-play').textContent, history: (await api.call('list_history',{})).entries.find(e => e.content_id === ready.record.content_id),
      meta_order: all('.details-meta > span').map(e => e.textContent), episodes: all('.details-episode').map(e => ({ text: e.textContent, video: e.dataset.videoId })),
      cast: all('.details-actor').length, related: all('.details-related .home-card').length, more: !!all('.details-plot-toggle').length,
      title: document.title, origin: location.origin, overflow: document.documentElement.scrollWidth > innerWidth + 1,
      native: await nativeInvoke('t01_control',{action:'identity'}), errors: await maccmsScenario('errors',prototype) };
  }
  if (action === 'navigation') {
    await home(); scrollTo({ top:320, behavior:'instant' }); await delay(100);
    const original = scrollY;
    await open(ready); const a = location.hash;
    const related = all<HTMLAnchorElement>('.details-related .poster-link').find(e => e.getAttribute('href') !== a);
    check(related, 'Related B missing'); const b = related.getAttribute('href')!; related.click();
    await until(() => location.hash === b && state.details.phase === 'ready', 'B did not load'); await settled();
    const backToA = all<HTMLAnchorElement>('.details-related .poster-link').find(e => e.getAttribute('href') === a);
    check(backToA, 'Related A missing'); backToA.click();
    await until(() => location.hash === a && state.details.phase === 'ready', 'A revisit did not load'); await settled();
    const chain: string[] = [];
    for (let i = 0; i < 3; i++) {
      one<HTMLAnchorElement>('.details-back').click(); await delay(700); await settled();
      chain.push(location.hash === a ? 'A' : location.hash === b ? 'B' : location.hash === '#/home' ? 'home' : 'other');
    }
    const restored = scrollY, browser: string[] = [];
    if (chain.at(-1) === 'home') {
      history.forward(); await until(() => location.hash === a && state.details.phase === 'ready', 'Browser Forward lost A'); await settled(); browser.push('A');
      history.forward(); await until(() => location.hash === b && state.details.phase === 'ready', 'Browser Forward lost B'); await settled(); browser.push('B');
      history.back(); await until(() => location.hash === a && state.details.phase === 'ready', 'Browser Back lost A'); await settled(); browser.push('A');
      one<HTMLAnchorElement>('.details-back').click(); await until(() => state.route.name === 'home', 'Page Back after browser navigation lost home'); await settled(); browser.push('home');
    }
    return { chain, browser, original, restored, clean:!document.querySelector('.page-transition-old'), errors:await maccmsScenario('errors',prototype) };
  }
  if (action === 'lower') {
    one('.details-episodes').scrollIntoView({ block:'start',behavior:'instant' }); await delay(300);
    return { scroll: scrollY, casts: all('.details-actor').length, related: all('.details-related .home-card').length };
  }
  if (action === 'states') {
    one<HTMLButtonElement>('.details-plot-toggle').click(); await until(() => !!all('.details-plot-modal').length,'Plot dialog missing'); await delay(300);
    const plot = { style: measure('.details-plot-modal'), content: measure('.details-plot-modal .n-card-content'), text: one('.details-plot-modal').textContent };
    one<HTMLElement>('.details-plot-modal .n-card-header__close').click(); await until(() => !all('.details-plot-modal').length,'Plot dialog did not close');
    all<HTMLButtonElement>('.details-group')[1]!.click(); await nextTick(); const season = { episodes: all('.details-episode').length, selected: one('.details-group[aria-pressed="true"]').textContent };
    all<HTMLButtonElement>('.details-group')[0]!.click(); await nextTick();
    one<HTMLButtonElement>('.details-episode').click();
    await until(() => app.matching.result?.status === 'needs_confirmation' || state.route.name === 'playback','Match did not settle');
    check(app.matching.result?.status === 'needs_confirmation','Controlled cross-source match must ask for confirmation'); await delay(300);
    one<HTMLButtonElement>('.details-candidate').click(); await until(() => all('.details-candidate-episode').length > 0,'Candidate did not load'); await delay(350);
    const confirm = { style: measure('.details-confirm'), candidate: measure('.details-candidate'), text: one('.details-confirm').textContent };
    one<HTMLButtonElement>('.details-confirm-cancel').click(); await until(() => !all('.details-confirm').length,'Match cancel did not close');
    return { plot, season, confirm, errors: await maccmsScenario('errors',prototype) };
  }
  if (action === 'add') {
    one<HTMLButtonElement>('.details-watchlist').click(); await until(() => one('.details-watchlist').getAttribute('aria-pressed') === 'true','Watchlist add failed');
    const icon = measure('.watchlist-symbol'), saved = await api.call('list_watchlist',{});
    await home();
    await until(() => all('.watchlist-card').length === 1 && all<HTMLImageElement>('.watchlist-card img').some(img => img.complete && img.naturalWidth > 0),'Watchlist home poster missing');
    const order = !!(one('.continue-watching').compareDocumentPosition(one('.watchlist-row')) & Node.DOCUMENT_POSITION_FOLLOWING);
    return { saved, icon, count: all('.watchlist-card').length, title: one('.watchlist-card').textContent, poster: measure('.watchlist-poster'), after_history: order };
  }
  if (action === 'after-restart') {
    await until(() => all('.watchlist-card').length === 1,'Restart lost the watchlist home row'); await settled();
    const before = { count: all('.watchlist-card').length, epoch: state.runtime.engine_epoch, entries: (await api.call('list_watchlist',{})).entries };
    one<HTMLButtonElement>('.watchlist-poster').click(); await until(() => state.route.name === 'details' && all('.details-watchlist').some(e => e.getAttribute('aria-pressed') === 'true'),'Saved poster did not reopen details'); await settled();
    const label = one('.details-play').textContent;
    one<HTMLButtonElement>('.details-watchlist').click(); await until(() => one('.details-watchlist').getAttribute('aria-pressed') === 'false','Watchlist removal failed');
    await home(); await until(() => !all('.watchlist-row').length,'Empty watchlist row remains');
    return { before, label, after: await api.call('list_watchlist',{}), hidden: !all('.watchlist-row').length };
  }
  if (action === 'watchlist-images') {
    await settled();
    one('.watchlist-poster').scrollIntoView({ block:'center', behavior:'instant' }); await delay(300);
    const core = window.__TAURI__!.core, calls: { command: string; input: any; response: any }[] = [], imageErrors: boolean[] = [], expiredStatuses: number[] = [];
    window.__TAURI__!.core = { ...core, invoke: async <T>(command: string, args?: Record<string, any>): Promise<T> => {
      const response = await core.invoke<T>(command, args);
      if (command === 'get_details' || command === 'get_resource_status') calls.push({ command, input: args?.input, response });
      return response;
    } };
    const details = () => calls.filter(c => c.command === 'get_details' && c.input.ref.data === ready.ref.data).length;
    const statuses = () => calls.filter(c => c.command === 'get_resource_status' && c.input.kind === 'image');
    const fail = async () => {
      const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method:'POST', headers:{'Content-Type':'application/json'}, body:JSON.stringify({operation:'evict-images'}) });
      check(response.ok && (await response.json()).ok, 'Image LRU eviction failed');
      const image = one<HTMLImageElement>('.watchlist-poster img');
      const source = image.src, expired = await fetch(source, {cache:'no-store'});
      expiredStatuses.push(expired.status);
      await expired.arrayBuffer(); check(expired.status === 410, 'Evicted watchlist image must return real HTTP 410');
      image.src = '/mark.svg'; await image.decode();
      image.addEventListener('error', event => imageErrors.push(event.isTrusted), {once:true});
      // A different request mode bypasses WKWebView's already-decoded no-CORS image.
      // The resource URL/capability stays exact and the failure remains a native event.
      image.crossOrigin = 'anonymous';
      image.src = source;
    };
    try {
      const href = one('.watchlist-poster').getAttribute('href')!;
      check(href.startsWith('#/details/'), 'Watchlist transition link missing');
      const ref = JSON.parse(decodeURIComponent(href.slice('#/details/'.length)));
      check(['source_id','source_revision','data'].every(key => ref[key] === ready.ref[key]), 'Watchlist transition reference differs');
      await fail();
      await until(() => details() === 1 && statuses().length === 1 && all<HTMLImageElement>('.watchlist-poster img').some(img => img.complete && img.naturalWidth > 0), 'Expired watchlist poster did not refresh its owning DTO once');
      const recovered = { details:details(), queries:statuses().length };
      await fail();
      await until(() => statuses().length === 2 && !all('.watchlist-poster img').length && !!all('.watchlist-row [role="alert"]').length, 'Second expiry did not retain an explicit error/fallback');
      await delay(200);
      check(details() === 1 && imageErrors.length === 2 && imageErrors.every(Boolean) && statuses().every(c => c.response.data?.target_id === c.input.target_id && c.response.data?.error?.code === 'SESSION_EXPIRED'), 'Image recovery repeated, lacked native failures or queried another target');
      const refused = { details:details(), queries:statuses().length, fallback:one('.watchlist-poster').textContent };
      one<HTMLButtonElement>('.watchlist-row [role="alert"] button').click();
      await until(() => details() === 2 && all<HTMLImageElement>('.watchlist-poster img').some(img => img.complete && img.naturalWidth > 0) && !all('.watchlist-row [role="alert"]').length, 'Explicit watchlist retry did not start a new attempt');
      return { recovered, refused, retried:{details:details(),queries:statuses().length}, native_image_errors:imageErrors, expired_http_status:expiredStatuses, errors:await maccmsScenario('errors',prototype) };
    } catch (cause) {
      const image = all<HTMLImageElement>('.watchlist-poster img')[0];
      throw new Error(`${String(cause)}; image recovery evidence=${JSON.stringify({ details:details(), queries:statuses().length, native_image_errors:imageErrors, expired_http_status:expiredStatuses,
        statuses:statuses().map(call => ({ kind:call.response.data?.kind, target_matches:call.response.data?.target_id === call.input.target_id, code:call.response.data?.error?.code })),
        image:image ? { complete:image.complete, decoded:image.naturalWidth > 0, connected:image.isConnected, loading:image.loading, top:image.getBoundingClientRect().top } : null })}`);
    } finally { window.__TAURI__!.core = core; }
  }
  if (action === 'resume') {
    await open(ready); const label = one('.details-play').textContent; one<HTMLButtonElement>('.details-play').click();
    await until(() => state.route.name === 'playback' || !!all('.details-confirm').length,'Primary resume did not match');
    if (all('.details-confirm').length) {
      one<HTMLButtonElement>('.details-candidate').click(); await until(() => all('.details-candidate-episode').length >= 3,'Candidate episodes missing');
      all<HTMLButtonElement>('.details-candidate-episode')[2]!.click();
    }
    await until(() => state.playback.session && document.querySelector<HTMLVideoElement>('video')?.readyState! >= 2,'Resume video did not load');
    const video = one<HTMLVideoElement>('video'); if (video.paused) videoJsControl('play').click();
    await until(() => !video.paused && !video.seeking && state.playback.phase === 'playing' && video.currentTime >= 6.8 && state.playback.context?.metadata_video_id === ready.record.video_id,'Actual player did not restore the history video and position');
    const resumedAt = video.currentTime;
    await until(() => state.playback.phase === 'playing' && !video.seeking && video.currentTime > resumedAt + .1,'Resumed media clock did not advance');
    const played = { video: state.playback.context!.metadata_video_id, position: video.currentTime * 1000, decoded: video.videoWidth > 0, session: state.playback.session!.session_id, label };
    videoJsControl('play').click(); await until(() => state.playback.phase === 'paused','Pause did not flush history');
    location.hash = '#/home'; await until(() => state.route.name === 'home' && !!all('.continue-card').length,'Resume exit did not finish'); await settled();
    return { played, history: (await api.call('list_history',{})).entries[0], errors: await maccmsScenario('errors',prototype) };
  }
  if (action === 'transitions') {
    await home(); transitions.length = 0; scrollTo({top:320,behavior:'instant'}); await delay(100); const original = scrollY;
    await open(ready); one<HTMLAnchorElement>('.details-back').click(); await until(() => state.route.name === 'home','Native transition return failed'); await settled();
    const shared = { supported: !!document.startViewTransition, observations: [...transitions], original, restored: scrollY };
    const descriptor = Object.getOwnPropertyDescriptor(document,'startViewTransition');
    Object.defineProperty(document,'startViewTransition',{configurable:true,value:undefined});
    let fallback;
    try {
      const target = `#/details/${encodeURIComponent(JSON.stringify(ready.ref))}`;
      const poster = all<HTMLAnchorElement>('.media-poster').find(e => e.getAttribute('href') === target); check(poster,'Fallback origin poster missing'); poster.click();
      await until(() => state.details.phase === 'ready' && !!document.querySelector('.page-transition-old') && one('#main-content').getAnimations().length > 0,'Disabled API did not start fallback animations');
      const old = document.querySelector<HTMLElement>('.page-transition-old')!;
      fallback = { old: old.getAnimations().map(a => ({ duration:a.effect?.getComputedTiming().duration, frames:(a.effect as KeyframeEffect).getKeyframes().map(f => f.opacity) })),
        next: one('#main-content').getAnimations().map(a => ({ duration:a.effect?.getComputedTiming().duration, frames:(a.effect as KeyframeEffect).getKeyframes().map(f => f.opacity) })) };
      await settled(); one<HTMLAnchorElement>('.details-back').click(); await until(() => state.route.name === 'home','Fallback return failed'); await settled();
      return { shared, fallback, restored: scrollY, clean: !document.querySelector('.page-transition-old'), errors: await maccmsScenario('errors',prototype) };
    } finally { if (descriptor) Object.defineProperty(document,'startViewTransition',descriptor); else delete (document as any).startViewTransition; }
  }
  if (action === 'errors') return maccmsScenario('errors',prototype);
  throw new Error(`Unknown TV5 action ${action}`);
}
