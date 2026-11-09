// Test-only transport; App, MacCMS views, official media and history use the production Host.
import { createApp, h, nextTick } from 'vue';
import App from '../../src/App.vue';
import UiProvider from '../../src/ui/UiProvider.vue';
import { appController as app, appState as state } from '../../src/state';
import { api } from '../../src/api';
import { locale } from '../../src/i18n';
import { nativeInvoke, type Player } from '../../src/player';
import { videoJsControl } from './videojs_controls';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const all = <T extends HTMLElement = HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];
const one = <T extends HTMLElement = HTMLElement>(selector: string) => { const value = document.querySelector<T>(selector); check(value, `Missing ${selector}`); return value; };
async function until(predicate: () => unknown, message: string, seconds = 30) {
  const end = performance.now() + seconds * 1000;
  while (!predicate()) { if (performance.now() > end) throw new Error(`${message}; ${JSON.stringify({ hash: location.hash, details: state.details.phase, error: state.details.error,
    playback: state.playback.phase, playback_error: state.playback.error, alerts: all('[role="alert"]').map(e => e.textContent), errors })}`); await delay(25); }
}
let mounted: ReturnType<typeof createApp> | undefined;
let rpc: (value: unknown) => Promise<any>;
const calls: { command: string; input: any; completed?: boolean }[] = [], errors: string[] = [];
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function measure(selector: string, pseudo?: string) {
  const element = one(selector), style = getComputedStyle(element, pseudo);
  const keys = ['color','background-color','font-size','font-weight','height','width','border-radius','padding-left','padding-right','left','right','bottom','top',
    'object-fit','filter','transform','animation-name','transition-duration','text-overflow','white-space','max-width','opacity','content','background-image','-webkit-backdrop-filter'];
  return { rect: element.getBoundingClientRect().toJSON(), ...Object.fromEntries(keys.map(key => [key, key === 'background-image'
    ? style.getPropertyValue(key).replace(/url\([^)]*\)/g, 'url(<gateway-image>)') : style.getPropertyValue(key)])) };
}
async function browse(category = 'variety') {
  location.hash = `#/browse/${category}`;
  await until(() => document.querySelector<HTMLElement>('.maccms-catalog')?.dataset.category === category && document.querySelector<HTMLElement>('.maccms-catalog')?.dataset.query === ''
    && !document.querySelector('.maccms-catalog .catalog-status') && all('.maccms-catalog .home-card').length > 0, `MacCMS ${category} page missing`);
  await nextTick(); await delay(300);
}
async function filter(key: string, value: string) {
  one<HTMLButtonElement>(`[data-dimension="${key}"]`).click(); await nextTick();
  const option = all<HTMLButtonElement>('.filter-option-grid button').find(button => button.textContent!.trim() === value); check(option, `Missing ${key}=${value}`); option.click();
  await until(() => new URLSearchParams(location.hash.split('?')[1]).get(key) === value && document.querySelector<HTMLElement>('.maccms-catalog')?.dataset.query === location.hash.split('?')[1]
    && !document.querySelector('.maccms-catalog .catalog-status'), `Filter ${key} did not settle`);
}
async function details() {
  await browse(); one<HTMLAnchorElement>('.maccms-catalog .poster-link').click();
  await until(() => state.details.phase === 'ready' && state.details.data?.metadata_ref?.identity.namespace === 'maccms' && !!document.querySelector('.maccms-details .details-episode'), 'MacCMS details missing');
  await delay(250);
}
const media = () => one<HTMLVideoElement>('video');
async function playing() {
  await until(() => state.playback.session && document.querySelector('video') && media().readyState >= 2, 'MacCMS actual media did not load');
  if (media().paused) videoJsControl('play').click();
  await until(() => !media().paused && media().currentTime > .1 && state.playback.phase === 'playing', 'MacCMS official player did not play');
}
async function saveAt(seconds: number) {
  media().currentTime = seconds;
  await until(() => !media().seeking && Math.abs(media().currentTime - seconds) < .3 && Math.abs(state.playback.position_ms / 1000 - media().currentTime) < .4, 'Native seek did not reach business state');
  if (!media().paused) videoJsControl('play').click();
  await until(() => state.playback.phase === 'paused', 'Official pause did not reach history');
}

export async function maccmsScenario(action: string, prototype: { player: Player; base: string; cap: string }) {
  if (!mounted) {
    prototype.player.stop(); const root = one('#app') as HTMLElement & { __vue_app__?: { unmount(): void } }; root.__vue_app__!.unmount();
    const original = window.__TAURI__!.core;
    rpc = async value => { const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(value) });
      check(response.ok, 'TV4 Host transport failed'); return response.json(); };
    const commands = new Set(['runtime_status','runtime_prepare','get_settings','save_settings','get_sources','sync_catalog','get_home','get_details','get_resource_status',
      'get_media_cache','clear_media_cache','list_history','save_history','remove_history','clear_history','cancel_request','match_playback','resolve_episode',
      'open_playback','close_playback','release_resolution','start_interaction','finish_interaction']);
    window.__TAURI__!.core = { ...original, invoke: async <T>(command: string, args?: Record<string, any>): Promise<T> => {
      if (!commands.has(command)) return original.invoke<T>(command, args);
      const call = { command, input: copy(args?.input ?? {}), completed: false }; calls.push(call);
      const value = await rpc({ command, args }); call.completed = true; return value;
    } };
    window.addEventListener('error', event => errors.push(event.message)); window.addEventListener('unhandledrejection', event => errors.push(String(event.reason)));
    location.hash = '#/home'; locale.value = 'en'; await app.initialize();
    mounted = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) }); mounted.mount(root);
    await until(() => !!document.querySelector('.tv-sidebar a[href="#/browse/variety"]'), 'Playback category entry missing');
  }
  if (action === 'styles') {
    await browse(); scrollTo({ top: 0, behavior: 'instant' });
    one<HTMLButtonElement>('.hero-indicators button').click(); await delay(100);
    await until(() => all<HTMLImageElement>('.maccms-catalog .poster-wall img').every(image => image.complete && image.naturalWidth), 'MacCMS posters did not decode');
    const selectors = ['.catalog-hero','.catalog-hero-slide.current img','.hero-cover-poster','.hero-bottom-mask','.hero-left-mask',
      '.maccms-subcategories','.maccms-subcategories .filter-pill.selected','.maccms-subcategories .filter-pill:not(.selected)',
      '.maccms-catalog .media-poster','.maccms-catalog .media-poster img','.media-update-label','[data-dimension="year"]','[data-dimension="sort"]'];
    const styles = Object.fromEntries(selectors.map(selector => [selector, measure(selector)]));
    const rating = all('.maccms-catalog .media-card-detail').map(element => element.textContent);
    const missing = all('.maccms-catalog .media-poster-missing').length;
    all<HTMLButtonElement>('.hero-indicators button')[1]!.click(); await nextTick();
    const zero_rating_hidden = !one('.hero-information').textContent!.includes('0 ★');
    one<HTMLButtonElement>('.hero-indicators button').click(); await nextTick();
    const rect = one('.maccms-subcategories .filter-pill:not(.selected)').getBoundingClientRect();
    return { width: innerWidth, height: innerHeight, styles, rating, missing, overflow: document.documentElement.scrollWidth > innerWidth + 1,
      title: document.title, origin: location.origin, zero_rating_hidden, errors: [...errors], hover: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }, native: await nativeInvoke('t01_control', { action: 'identity' }) };
  }
  if (action === 'hover') return measure('.maccms-subcategories .filter-pill:not(.selected)');
  if (action === 'sports') { await browse('sports'); return { poster: measure('.maccms-catalog .media-poster'), cards: all('.maccms-catalog .home-card').length, errors: [...errors] }; }
  if (action === 'short') { await browse('short'); return { poster: measure('.maccms-catalog .media-poster'), cards: all('.maccms-catalog .home-card').length, errors: [...errors] }; }
  if (action === 'filters') {
    await browse(); await filter('year', '2026'); await filter('region', '中国大陆'); await filter('language', '国语');
    const filtered = { count: all('.maccms-catalog .home-card').length, titles: all('.maccms-catalog .media-card-title').map(e => e.textContent),
      progress: one('.wall-progress').textContent, hash: location.hash, calls: calls.filter(c => c.command === 'get_home' && c.input.filter?.language).map(c => c.input) };
    one<HTMLButtonElement>('.maccms-catalog .wall-more').click();
    await until(() => all('.maccms-catalog .home-card').length === 8 && !document.querySelector('.maccms-catalog .catalog-status'), 'MacCMS bounded continuation did not append the remaining pages');
    const pagination = { count: all('.maccms-catalog .home-card').length, unique: new Set(all('.maccms-catalog .home-card').map(e => e.dataset.cardId)).size, progress: one('.wall-progress').textContent };
    await filter('year', '1900'); await until(() => !!document.querySelector('.catalog-empty h2'), 'MacCMS empty state missing');
    const empty = { heading: measure('.catalog-empty h2'), text: one('.catalog-empty').textContent };
    one<HTMLButtonElement>('.catalog-empty button').click(); await until(() => all('.maccms-catalog .home-card').length > 0 && !document.querySelector('.maccms-catalog .catalog-status'), 'Clearing filters did not recover');
    one<HTMLButtonElement>('.maccms-subcategories .filter-pill:not(.selected)').click();
    await until(() => new URLSearchParams(location.hash.split('?')[1]).get('subcategory') === '港台综艺'
      && document.querySelector<HTMLElement>('.maccms-catalog')?.dataset.query === location.hash.split('?')[1]
      && document.querySelector('.maccms-subcategories .filter-pill.selected')?.textContent === '港台综艺'
      && !document.querySelector('.maccms-catalog .catalog-status'), 'Subcategory did not change');
    return { filtered, pagination, empty, selected: one('.maccms-subcategories .filter-pill.selected').textContent, cards: all('.maccms-catalog .home-card').length, errors: [...errors] };
  }
  if (action === 'switch') {
    const original = state.selected_playback_source!.source_id;
    const limited = state.sources.data!.providers.find(item => item.name === 'Limited fixture')!;
    const select = one<HTMLSelectElement>('.tv-sidebar select[data-role="playback"]');
    select.value = limited.id; select.dispatchEvent(new Event('change', { bubbles: true }));
    await until(() => state.selected_playback_source?.source_id === limited.id && document.querySelector<HTMLElement>('.maccms-catalog')?.dataset.source === limited.id
      && !document.querySelector('.maccms-catalog .catalog-status') && all('.maccms-catalog .home-card').length > 0, 'Selected source category did not reload');
    const result = { links: all<HTMLAnchorElement>('.tv-sidebar nav a').map(a => a.hash), disabled: all('.tv-sidebar a[aria-disabled="true"]').length,
      selected: one('.maccms-subcategories .filter-pill.selected').textContent, error: !!document.querySelector('.maccms-catalog [role="alert"]'),
      calls: calls.filter(c => c.command === 'get_home' && c.input.source_id === limited.id).map(c => c.input) };
    select.value = original; select.dispatchEvent(new Event('change', { bubbles: true }));
    await until(() => state.selected_playback_source?.source_id === original && !!document.querySelector('.tv-sidebar a[href="#/browse/sports"]'), 'Original source entries missing');
    return { ...result, errors: [...errors] };
  }
  if (action === 'retry') {
    await browse(); await rpc({ operation: 'mode', value: 1 }); await filter('year', '2026');
    await until(() => !!document.querySelector('.maccms-catalog [role="alert"]'), 'Actual 503 did not render a retry state');
    const failed = { message: one('.maccms-catalog [role="alert"]').textContent, empty: !!document.querySelector('.maccms-catalog .catalog-empty') };
    await rpc({ operation: 'mode', value: 0 }); one<HTMLButtonElement>('.maccms-retry').click();
    await until(() => !document.querySelector('.maccms-catalog [role="alert"]') && !document.querySelector('.maccms-catalog .catalog-status') && all('.maccms-catalog .home-card').length === 5, 'Source retry did not recover');
    return { failed, recovered: all('.maccms-catalog .home-card').length, errors: [...errors] };
  }
  if (action === 'details-styles') {
    await details(); scrollTo({ top: 0, behavior: 'instant' });
    return { head: measure('.details-head'), background: measure('.details-head','::before'), mask: measure('.details-head','::after'), poster: measure('.details-poster'),
      source: measure('.details-meta','::after'), source_name: one('.maccms-accessible-source').textContent, errors: [...errors] };
  }
  if (action === 'play') {
    if (state.details.data?.metadata_ref?.identity.namespace !== 'maccms' || state.route.name !== 'details') await details();
    const metadata = copy(state.selected_metadata_source), ref = copy(state.details.data!.details.ref);
    const videos = all<HTMLButtonElement>('.details-episode').map(button => button.dataset.videoId!); check(videos.length === 2, 'Two same-source episodes required');
    one<HTMLButtonElement>('.details-episode').click(); await playing(); await saveAt(5);
    const first = { session: state.playback.session!.session_id, video: state.playback.context!.metadata_video_id, position: state.playback.position_ms, decoded: media().videoWidth > 0 };
    location.hash = `#/details/${encodeURIComponent(JSON.stringify(ref))}`;
    await until(() => state.route.name === 'details' && state.details.phase === 'ready' && all('.details-episode').length === 2, 'Details did not return for episode switch');
    all<HTMLButtonElement>('.details-episode')[1]!.click(); await playing(); await saveAt(7);
    const second = { session: state.playback.session!.session_id, video: state.playback.context!.metadata_video_id, position: state.playback.position_ms, decoded: media().videoWidth > 0 };
    location.hash = '#/home';
    await until(() => state.route.name === 'home' && !!document.querySelector('.continue-card img'), 'MacCMS continue-watching poster missing');
    const record = (await api.call('list_history', {})).entries.find(item => item.content_id === state.details.data!.details.id)!;
    check(record, 'MacCMS history row missing');
    const history = { namespace: record.metadata_identity.namespace, video: record.video_id, position: record.position_ms, content_id: record.content_id,
      poster: one<HTMLImageElement>('.continue-card img').src.startsWith('http://127.0.0.1:') };
    one<HTMLButtonElement>('.continue-poster').click();
    await playing();
    await until(() => state.playback.context?.metadata_video_id === second.video && media().currentTime >= record.position_ms / 1000 - .3, 'MacCMS resume did not restore the saved episode and position');
    const resumed = { session: state.playback.session!.session_id, video: state.playback.context!.metadata_video_id, position: media().currentTime * 1000, decoded: media().videoWidth > 0 };
    location.hash = '#/home'; await until(() => state.route.name === 'home' && !!document.querySelector('.continue-card'), 'Final history flush did not finish');
    return { first, second, history, resumed, metadata_kept: JSON.stringify(metadata) === JSON.stringify(state.selected_metadata_source),
      matches: calls.filter(c => c.command === 'match_playback').map(c => ({ namespace: c.input.metadata_ref.identity.namespace, same_source: c.input.metadata_ref.content_ref.source_id === c.input.playback_source.source_id, video: c.input.video_id })),
      saves: calls.filter(c => c.command === 'save_history' && c.completed).length, errors: [...errors] };
  }
  if (action === 'errors') return errors;
  throw new Error(`Unknown TV4 action ${action}`);
}
