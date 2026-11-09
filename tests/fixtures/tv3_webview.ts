// Test-only transport. Every successful envelope comes from the production Rust Host.
import { createApp, h, nextTick } from 'vue';
import App from '../../src/App.vue';
import UiProvider from '../../src/ui/UiProvider.vue';
import { appController as app, appState as state, type HomeFilter } from '../../src/state';
import { nativeInvoke, type Player } from '../../src/player';
import { api } from '../../src/api';
import { locale } from '../../src/i18n';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
async function until(test: () => unknown, message: string | (() => string), timeout = 25000) {
  const end = performance.now() + timeout;
  while (!test()) { if (performance.now() >= end) throw new Error(typeof message === 'string' ? message : message()); await delay(25); }
}
const all = <T extends Element = HTMLElement>(selector: string, root: ParentNode = document) => [...root.querySelectorAll<T>(selector)];
const node = <T extends HTMLElement = HTMLElement>(selector: string) => { const element = document.querySelector<T>(selector); check(element, `Missing ${selector}`); return element; };
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const calls: { command: string; input: any; completed?: boolean; titles?: string[]; error?: string; sections?: number }[] = [];
const errors: string[] = [];
const visibility: { hidden: boolean; time: number; index: string | undefined }[] = [];
const heroTimeline: Record<string, unknown>[] = [];
let mounted: ReturnType<typeof createApp> | undefined;
let rpc: (input: unknown) => Promise<any>;
let releaseRows: (() => void) | undefined;
let pendingEvidence: Record<string, unknown>;
let heldGenre: string | null = null;
const releases: (() => void)[] = [];

async function route(hash: string) {
  location.hash = hash;
  const [path, query = ''] = hash.split('?'), category = path!.startsWith('#/browse/') ? path!.split('/')[2]! : '';
  await until(() => { const page = document.querySelector<HTMLElement>('.home-catalog'); return location.hash === hash && page?.dataset.category === category && page?.dataset.query === query; }, `Route missing: ${hash}`);
  await nextTick(); await delay(450);
}
function row(genre: string) { return all<HTMLElement>('.catalog-lazy-row').find(element => JSON.parse(element.dataset.filter!).genre === genre); }
function currentHero() {
  const element = node('.catalog-hero');
  return { index: Number(element.dataset.index), paused: element.dataset.paused === 'true', hover: element.matches(':hover'),
    hidden: document.hidden, title: element.querySelector('h1')!.textContent!, slides: all('.catalog-hero-slide').length,
    animation: getComputedStyle(node('.catalog-hero-slide.current img')).animationPlayState };
}
function styles(selector: string) {
  const element = node(selector), s = getComputedStyle(element), r = element.getBoundingClientRect();
  const keys = ['height', 'min-height', 'width', 'max-width', 'max-height', 'font-size', 'font-weight', 'color', 'background-color', 'border-radius',
    'padding-left', 'padding-right', 'padding-top', 'padding-bottom', 'margin-top', 'margin-bottom', 'row-gap', 'column-gap', 'top', 'bottom', 'right', 'left',
    'object-fit', 'object-position', 'animation-duration', 'animation-name', 'transition-duration', 'background-image', 'backdrop-filter', '-webkit-backdrop-filter',
    'scroll-snap-type', 'scrollbar-width', 'grid-template-columns', '-webkit-line-clamp', 'opacity', 'transform', 'display', 'position'];
  return { rect: r.toJSON(), ...Object.fromEntries(keys.map(key => [key, s.getPropertyValue(key)])) };
}
async function choose(key: string, value: string, wire = value) {
  node<HTMLButtonElement>(`.filter-pill[data-dimension="${key}"]`).click();
  await until(() => document.querySelector('.filter-panel'), 'Filter panel missing');
  const option = all<HTMLButtonElement>('.filter-option-grid button').find(button => button.textContent!.replace('✓', '').trim() === value);
  check(option, `Missing filter ${key}=${value}`); option.click();
  await until(() => new URLSearchParams(location.hash.split('?')[1]).get(key) === wire, 'Filter address did not update');
  await delay(250);
}
async function wallReady() {
  await until(() => document.querySelector('.poster-wall') && !document.querySelector('.poster-wall [aria-busy="true"]'), 'Wall did not settle');
  await until(() => all('.poster-wall .home-card').length || document.querySelector('.catalog-empty h2') || document.querySelector('.catalog-row-error'), 'Wall has no settled result');
}

export async function catalogScenario(action: string, prototype: { player: Player; base: string; cap: string }) {
  if (!mounted) {
    prototype.player.stop();
    const container = node('#app') as HTMLElement & { __vue_app__?: { unmount(): void } };
    container.__vue_app__!.unmount();
    rpc = async (value: unknown) => {
      const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(value) });
      check(response.ok, 'TV3 Host transport failed'); return response.json();
    };
    const original = window.__TAURI__!.core;
    const hostCommands = new Set(['runtime_status', 'runtime_prepare', 'get_settings', 'save_settings', 'get_sources', 'sync_catalog',
      'get_home', 'get_details', 'get_resource_status', 'get_media_cache', 'list_history', 'save_history', 'remove_history', 'clear_history',
      'cancel_request', 'match_playback', 'resolve_episode', 'open_playback', 'close_playback', 'release_resolution',
      'start_interaction', 'finish_interaction', 'clear_media_cache']);
    const gate = new Promise<void>(resolve => { releaseRows = resolve; });
    window.__TAURI__!.core = { ...original, invoke: async <T>(command: string, args?: Record<string, any>): Promise<T> => {
      if (!hostCommands.has(command)) return original.invoke<T>(command, args);
      const call: typeof calls[number] = { command, input: copy(args?.input ?? {}) }; calls.push(call);
      if (command === 'get_home' && args?.input.section_ref && releaseRows) await gate;
      const response = await rpc({ command, args });
      call.completed = true;
      call.error = response.error?.code;
      call.sections = response.data?.page?.sections.length;
      if (response.data?.page) call.titles = response.data.page.sections.flatMap((section: any) => section.items.map((card: any) => card.title));
      if (command === 'get_home' && heldGenre && args?.input.filter?.genre === heldGenre) {
        await new Promise<void>(resolve => { releases.push(resolve); });
      }
      return response;
    } };
    window.addEventListener('error', event => errors.push(event.message));
    window.addEventListener('unhandledrejection', event => errors.push(String(event.reason)));
    document.addEventListener('visibilitychange', () => visibility.push({ hidden: document.hidden, time: performance.now(), index: document.querySelector<HTMLElement>('.catalog-hero')?.dataset.index }));
    location.hash = '#/home'; locale.value = 'en';
    await app.initialize();
    mounted = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) }); mounted.mount(container);
    new MutationObserver(records => {
      if (!records.some(record => (record.target as Element).matches('.catalog-hero'))) return;
      const hero = document.querySelector<HTMLElement>('.catalog-hero');
      if (hero) heroTimeline.push({ time: performance.now(), index: hero.dataset.index, paused: hero.dataset.paused,
        hidden: document.hidden, hover: hero.matches(':hover'), focused: hero.contains(document.activeElement), active: document.activeElement?.tagName });
      if (heroTimeline.length > 128) heroTimeline.shift();
    }).observe(container, { subtree: true, attributes: true, attributeFilter: ['data-index', 'data-paused'] });
    await until(() => all('.catalog-lazy-row').length >= 13 && calls.some(c => c.command === 'get_home' && c.input.section_ref), 'Description rows did not become viewport candidates');
    const trace = await rpc({ operation: 'trace' });
    pendingEvidence = { phase: state.home.phase, descriptions: state.home.data!.page.sections.length,
      rows: all<HTMLElement>('.catalog-lazy-row').map(element => ({ phase: element.dataset.phase, empty: !!element.querySelector('.catalog-row-empty') })),
      description_calls: calls.filter(c => c.command === 'get_home' && c.input.catalog_only).length,
      content_requests: trace.targets.filter((target: string) => target.includes('/catalog/')).length,
      visible_requests: calls.filter(c => c.command === 'get_home' && c.input.section_ref).map(c => c.input.filter) };
    const release = releaseRows; releaseRows = undefined; release!();
    await until(() => all('.hero-indicators button').length === 5 && all('.home-card').length > 0, 'Hero and first row did not load');
    await until(() => all<HTMLImageElement>('.catalog-hero-slide.current img').every(image => image.complete && image.naturalWidth), 'Hero artwork did not decode');
  }
  if (action === 'pending') return pendingEvidence;
  if (action === 'home') {
    await route('#/home');
    const initial = all<HTMLElement>('.catalog-lazy-row').map(element => ({ title: element.querySelector('h2')!.textContent, filter: JSON.parse(element.dataset.filter!), phase: element.dataset.phase }));
    const before = calls.filter(c => c.command === 'get_home' && c.input.section_ref).length;
    const last = all<HTMLElement>('.catalog-lazy-row').at(-1)!; last.scrollIntoView({ block: 'center', behavior: 'instant' });
    await until(() => last.dataset.phase === 'ready', 'Offscreen row did not load on entry');
    const after = calls.filter(c => c.command === 'get_home' && c.input.section_ref).length;
    scrollTo({ top: 0, behavior: 'instant' }); await delay(200);
    await until(() => document.querySelector('.continue-card img'), 'History poster missing');
    const progress = node('.continue-progress'), fill = progress.firstElementChild!.getBoundingClientRect();
    return { initial, after, before, last_cards: all('.home-card', last).length, hero: currentHero(), history: {
      count: all('.continue-card').length, title: node('.continue-card > p').textContent, episode: node('.continue-episode').textContent,
      remaining: node('.continue-remaining').textContent, ratio: fill.width / progress.getBoundingClientRect().width,
      image: node<HTMLImageElement>('.continue-card img').src.startsWith('http://127.0.0.1:'), calls: calls.filter(c => c.command === 'list_history').length,
    }, errors: [...errors] };
  }
  if (action === 'hero') return currentHero();
  if (action === 'hero-rotation') {
    node('#main-content').focus({ preventScroll: true }); await nextTick(); await delay(50);
    const start = performance.now(), first = currentHero();
    await delay(8300);
    return { first, rotated: currentHero(), elapsed: performance.now() - start, timeline: [...heroTimeline], visibility: [...visibility] };
  }
  if (action === 'hero-focus') { node<HTMLButtonElement>('.hero-primary').focus(); await delay(30); return currentHero(); }
  if (action === 'hero-blur') { node('#main-content').focus({ preventScroll: true }); await delay(30); return currentHero(); }
  if (action === 'hero-dot') { node<HTMLButtonElement>('.hero-indicators button:last-child').click(); await nextTick(); return currentHero(); }
  if (action === 'visibility') return visibility;
  if (action === 'browse') {
    const result = [];
    for (const category of ['movie', 'series', 'anime', 'documentary']) {
      node<HTMLAnchorElement>(`.tv-sidebar a[href="#/browse/${category}"]`).click();
      await until(() => location.hash === `#/browse/${category}` && document.querySelector<HTMLElement>('.browse-catalog')?.dataset.category === category, 'Category sidebar did not navigate');
      const first = () => document.querySelector<HTMLElement>('.catalog-lazy-row');
      await until(() => first()?.dataset.phase === 'ready', `Category ${category} content missing`);
      const element = first()!, key = element.dataset.rowKey!, data = app.rows[key]!.data!.page.sections[0]!;
      result.push({ category, title: node('.hero-eyebrow').textContent, filters: JSON.parse(element.dataset.filter!),
        kind: data.items[0]!.kind, genres: data.items.map(card => card.genres), ids: data.items.map(card => card.id),
        rows: all<HTMLElement>('.catalog-lazy-row').map(e => ({ title: e.querySelector('h2')!.textContent, filter: JSON.parse(e.dataset.filter!) })) });
    }
    await route('#/browse/movie'); return result;
  }
  if (action === 'filters') {
    await route('#/browse/movie');
    await choose('sort', 'Popular', 'popular');
    await wallReady();
    await choose('language', 'Mandarin');
    await wallReady();
    const cleared = { hash: location.hash, notice: node('.filter-notice').textContent, sort_disabled: node<HTMLButtonElement>('[data-dimension="sort"]').disabled };
    await choose('year', String(new Date().getFullYear() - 1));
    await choose('genre', 'Drama'); await wallReady();
    const last = [...calls].reverse().find(c => c.command === 'get_home' && c.input.filter?.genre === 'Drama' && c.input.filter?.year && c.input.filter?.language)!;
    const data = app.catalogRow(last.input.section_ref, last.input.filter).data!.page.sections[0]!;
    const disabled = node<HTMLButtonElement>('[data-dimension="region"]');
    const result = { cleared, hash: location.hash, filter: last.input.filter, items: data.items.map(card => ({ year: card.year, genres: card.genres, title: card.title })),
      region: { disabled: disabled.disabled, reason: disabled.title, text: disabled.parentElement!.textContent }, compact: styles('.catalog-hero'),
      hidden_plot: !document.querySelector('.hero-plot'), hidden_actions: !document.querySelector('.hero-actions') };
    scrollTo({ top: node('.catalog-hero').offsetHeight * .6, behavior: 'instant' }); await delay(50);
    Object.assign(result, { compact_parallax: styles('.catalog-hero-visual') });
    scrollTo({ top: 0, behavior: 'instant' }); await delay(50);
    node<HTMLButtonElement>('[data-dimension="genre"]').click(); await delay(220);
    const buttons = all<HTMLButtonElement>('.filter-option-grid button');
    const before = buttons.indexOf(document.activeElement as HTMLButtonElement);
    node('.filter-panel').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
    const after = buttons.indexOf(document.activeElement as HTMLButtonElement);
    Object.assign(result, { keyboard: { before, after }, panel: styles('.filter-panel'), pill: styles('[data-dimension="genre"]'), option: styles('.filter-option-grid button'), check: styles('.filter-check'), wall: styles('.poster-wall .media-row-track'), progress: styles('.wall-progress') });
    node('.filter-panel').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    return result;
  }
  if (action === 'filter-styles') {
    await route('#/browse/movie');
    node('#main-content').focus({ preventScroll: true }); await nextTick();
    const button = node<HTMLButtonElement>('[data-dimension="genre"]');
    const normal = styles('[data-dimension="genre"]'), header = styles('.catalog-hero');
    button.click(); await delay(220);
    const options = all<HTMLElement>('.filter-option-grid button').map(element => ({ height: getComputedStyle(element).height, radius: getComputedStyle(element).borderRadius,
      rendered_height: element.getBoundingClientRect().height, transform: getComputedStyle(element).transform }));
    const panel = styles('.filter-panel'); button.click(); await nextTick();
    const bar = node('.filter-bar'); scrollTo({ top: bar.offsetTop + 20, behavior: 'instant' }); await delay(100);
    const sticky = styles('.filter-bar'), r = button.getBoundingClientRect();
    const sticky_hit = button.contains(document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2));
    scrollTo({ top: 0, behavior: 'instant' }); await delay(100);
    return { normal, header, options, panel, sticky, sticky_hit, target: { x: normal.rect.x + normal.rect.width / 2, y: normal.rect.y + normal.rect.height / 2 } };
  }
  if (action === 'filter-hover') return styles('[data-dimension="genre"]');
  if (action === 'pagination') {
    await route('#/browse/movie?genre=Action'); await wallReady();
    const before = all('.poster-wall .home-card').length;
    const more = node<HTMLButtonElement>('.wall-more'); more.scrollIntoView({ block: 'center', behavior: 'instant' });
    await until(() => all('.poster-wall .home-card').length > before, 'Wall sentinel did not append the next page');
    const ids = all<HTMLElement>('.poster-wall .home-card').map(e => e.dataset.cardId!);
    return { before, after: ids.length, unique: new Set(ids).size, requests: calls.filter(c => c.command === 'get_home' && c.input.filter?.genre === 'Action').map(c => c.input.page) };
  }
  if (action === 'review-pagination') {
    await route('#/browse/movie?genre=Action'); await wallReady();
    const image = node<HTMLImageElement>('.poster-wall .media-poster img');
    image.scrollIntoView({ block: 'center', behavior: 'instant' });
    await until(() => image.complete && image.naturalWidth && getComputedStyle(image).opacity === '1', 'Original poster did not become visible');
    const before = all('.poster-wall .home-card').length, src = image.src;
    node('.wall-more').scrollIntoView({ block: 'center', behavior: 'instant' });
    await until(() => all('.poster-wall .home-card').length > before, 'Pagination did not append cards');
    image.scrollIntoView({ block: 'center', behavior: 'instant' }); await delay(300);
    return { before, after: all('.poster-wall .home-card').length, reused: image.isConnected, same_url: image.src === src,
      decoded: image.complete && image.naturalWidth > 0, opacity: getComputedStyle(image).opacity, errors: [...errors] };
  }
  if (action === 'review-breakpoint-leave') {
    if (innerWidth >= 960) await route('#/home');
    await until(() => !!document.querySelector('.home-card .poster-link'), 'Home cards missing before leaving');
    node<HTMLAnchorElement>('.home-card .poster-link').click();
    await until(() => state.route.name === 'details' && state.details.phase === 'ready', 'Details did not open before resizing');
    return { width: innerWidth, requests: calls.filter(c => c.command === 'get_home').length };
  }
  if (action === 'review-breakpoint-return') {
    const before = calls.filter(c => c.command === 'get_home').length;
    history.back();
    await until(() => location.hash === '#/home' && state.route.name === 'home', 'Home route did not return');
    await delay(500);
    await until(() => state.home.phase !== 'loading', 'Home reload did not settle after resizing');
    return { width: innerWidth, phase: state.home.phase, cards: all('.home-card').length,
      catalog: !!document.querySelector('.home-catalog'), requests: calls.filter(c => c.command === 'get_home').slice(before).map(c => c.input), errors: [...errors] };
  }
  if (action === 'restore') {
    await route('#/browse/movie?genre=Comedy'); await wallReady();
    node('.poster-wall .home-card').scrollIntoView({ block: 'center', behavior: 'instant' }); await delay(200);
    const hash = location.hash, scroll = scrollY, ids = all<HTMLElement>('.poster-wall .home-card').map(e => e.dataset.cardId!);
    const requests = calls.filter(c => c.command === 'get_home').length;
    node<HTMLAnchorElement>('.poster-wall .poster-link').click();
    await until(() => state.route.name === 'details' && state.details.phase === 'ready', 'Details did not open');
    history.back();
    await until(() => location.hash === hash && document.querySelector('.poster-wall'), 'Back did not restore browse route');
    await delay(500);
    return { hash, restored_hash: location.hash, scroll, restored_scroll: scrollY,
      ids, restored_ids: all<HTMLElement>('.poster-wall .home-card').map(e => e.dataset.cardId!), requests: calls.filter(c => c.command === 'get_home').length - requests };
  }
  if (action === 'see-all') {
    await route('#/home'); const heading = node<HTMLAnchorElement>('.catalog-lazy-row .media-row-heading a');
    const wanted = heading.hash; heading.click(); await until(() => !!document.querySelector('.poster-wall'), 'Row title did not open wall');
    await wallReady(); return { wanted, hash: location.hash, count: all('.poster-wall .home-card').length };
  }
  if (action === 'empty') {
    await route(`#/browse/movie?genre=Documentary&year=${new Date().getFullYear()}&language=Mandarin`); await wallReady();
    const result = { title: node('.catalog-empty h2').textContent, shortcuts: all('.empty-remove').length,
      icon: styles('.catalog-empty > svg'), heading: styles('.catalog-empty h2'), description: styles('.catalog-empty > p') };
    node<HTMLButtonElement>('.empty-remove').click();
    await until(() => !new URLSearchParams(location.hash.split('?')[1]).has('genre'), 'Remove-condition shortcut did not update address');
    await route(location.hash);
    await wallReady(); return { ...result, recovered: all('.poster-wall .home-card').length, hash: location.hash };
  }
  if (action === 'isolation') {
    await route('#/home'); await app.loadCatalog();
    await until(() => all('.catalog-lazy-row').length >= 13, 'Rows missing after refresh');
    const section = state.home.data!.page.sections.find(s => s.kind === 'movie')!;
    heldGenre = 'Drama';
    const slow = app.loadRow(section.section_ref, { genre: 'Drama' });
    await until(() => releases.length, 'Slow response was not held');
    await app.loadRow(section.section_ref, { genre: 'Comedy' });
    const fast = copy(app.catalogRow(section.section_ref, { genre: 'Comedy' }).data!.page.sections[0]!.items.map(c => c.genres));
    const pending = app.catalogRow(section.section_ref, { genre: 'Drama' }).phase;
    heldGenre = null; releases.splice(0).forEach(release => release()); await slow;
    const slowData = copy(app.catalogRow(section.section_ref, { genre: 'Drama' }).data!.page.sections[0]!.items.map(c => c.genres));
    await app.loadRow(section.section_ref, { genre: 'Comedy' }, 2);
    const pagination = { comedy: app.catalogRow(section.section_ref, { genre: 'Comedy' }).data!.page.sections[0]!.items.length, drama: app.catalogRow(section.section_ref, { genre: 'Drama' }).data!.page.sections[0]!.items.length };
    const displayed = ['Comedy', 'Drama'].map(genre => ({ genre, cards: all('.home-card', row(genre)!).length, text: row(genre)!.textContent }));
    heldGenre = 'Drama'; const oldKey = app.rowKey(section.section_ref, { genre: 'Drama' });
    const late = app.loadRow(section.section_ref, { genre: 'Drama' }); await until(() => releases.length, 'Old source response not held');
    const other = state.sources.data!.providers.find(s => s.kind === 'stremio' && s.id !== state.selected_metadata_source!.source_id)!;
    await app.selectSource(other.id, 'metadata'); await app.loadCatalog();
    heldGenre = null; releases.splice(0).forEach(release => release()); await late;
    await until(() => all('.home-card').length && node('.catalog-hero h1').textContent!.includes('Second'), 'New source did not render');
    const switched = { title: node('.catalog-hero h1').textContent, old_key: oldKey, new_key: app.rowKey(section.section_ref, { genre: 'Drama' }), old_result: app.rows[oldKey]?.data ?? null };
    const currentRef = state.home.data!.page.sections.find(s => s.kind === 'movie')!.section_ref;
    heldGenre = 'Drama'; const oldLocaleKey = app.rowKey(currentRef, { genre: 'Drama' });
    const oldLocale = app.loadRow(currentRef, { genre: 'Drama' }); await until(() => releases.length, 'Old locale response not held');
    await app.updateSettings(settings => ({ ...settings, ui_locale: 'zh-CN' }));
    await until(() => state.home.phase === 'ready', 'Locale catalog did not reload');
    heldGenre = null; releases.splice(0).forEach(release => release()); await oldLocale;
    const language = { old_key: oldLocaleKey, new_key: app.rowKey(currentRef, { genre: 'Drama' }), old_result: app.rows[oldLocaleKey]?.data ?? null };
    await app.updateSettings(settings => ({ ...settings, ui_locale: 'en' })); await until(() => state.home.phase === 'ready', 'English catalog missing');
    return { pending, fast, slow: slowData, pagination, displayed, switched, language, errors: [...errors] };
  }
  if (action === 'retry') {
    await route('#/home');
    await rpc({ operation: 'mode', value: 1 });
    const provider = state.sources.data!.providers.find(s => s.id === state.selected_metadata_source!.source_id);
    check(provider?.kind === 'stremio', 'Retry requires the selected Stremio source');
    await rpc({ operation: 'refresh', config_id: provider.origin.config_id }); await app.loadSources();
    await until(() => state.home.phase === 'ready' && all('.catalog-lazy-row').length >= 13,
      () => `Retry rows missing: ${JSON.stringify({ phase: state.home.phase, error: state.home.error?.code, hash: location.hash,
        selected: state.selected_metadata_source, home_source: state.home.data?.page.source_revision, locale: state.settings.ui_locale,
        row_count: all('.catalog-lazy-row').length, sections: state.home.data?.page.sections.length,
        requests: calls.filter(c => c.command === 'get_home').slice(-6) })}`);
    const target = row('Drama')!; target.scrollIntoView({ block: 'center', behavior: 'instant' });
    await until(() => !!target.querySelector('.home-section-retry'), 'Real upstream failure did not render retry');
    const failed = { phase: target.dataset.phase, empty: !!target.querySelector('.catalog-row-empty'), message: target.querySelector('[role="alert"]')!.textContent };
    await rpc({ operation: 'mode', value: 0 }); target.querySelector<HTMLButtonElement>('.home-section-retry')!.click();
    await until(() => target.dataset.phase === 'ready' && all('.home-card', target).length, 'Retry did not recover');
    const result = { failed, phase: target.dataset.phase, cards: all('.home-card', target).length, error: !!target.querySelector('[role="alert"]') };
    scrollTo({ top: 0, behavior: 'instant' }); return result;
  }
  if (action === 'styles') {
    await route('#/home'); scrollTo({ top: 0, behavior: 'instant' });
    await until(() => all('.home-card').length && all('.continue-card').length, 'Real page content missing for visual measurement');
    await delay(300);
    const selectors = ['.catalog-hero', '.catalog-hero-slide', '.catalog-hero-slide.current img', '.hero-bottom-mask', '.hero-left-mask', '.catalog-hero-copy', '.hero-eyebrow',
      '.hero-information', '.hero-plot', '.hero-actions', '.hero-primary', '.hero-secondary', '.hero-indicators', '.hero-indicators button:not(.active)', '.hero-indicators button.active',
      '.home-content-rows', '.continue-card', '.continue-poster', '.continue-progress', '.continue-progress span', '.continue-play',
      '.catalog-lazy-row .media-card', '.catalog-lazy-row .media-poster', '.catalog-lazy-row .media-row-track', '.catalog-lazy-row .media-row-heading', '.catalog-lazy-row .media-row-next'];
    const style = Object.fromEntries(selectors.map(selector => [selector, styles(selector)]));
    scrollTo({ top: node('.catalog-hero').offsetHeight * .6, behavior: 'instant' }); await delay(40);
    const parallax = styles('.catalog-hero-visual'); scrollTo({ top: 0, behavior: 'instant' });
    return { width: innerWidth, height: innerHeight, styles: style, parallax, overflow: document.documentElement.scrollWidth > innerWidth + 1,
      native: await nativeInvoke('t01_control', { action: 'identity' }), errors: [...errors] };
  }
  if (action === 'builtin') {
    const { preset } = await rpc({ operation: 'builtin' });
    await app.updateSettings(settings => ({ ...settings, source_configs: [...settings.source_configs, preset] }));
    await app.loadSources();
    const builtin = state.sources.data!.providers.find(s => s.kind === 'stremio' && s.origin.config_id === preset.config_id)!;
    check(builtin, 'Built-in TMDB provider missing');
    await app.selectSource(builtin.id, 'metadata'); await route('#/home');
    const persisted = await api.call('get_settings', {});
    check(persisted.source_configs.some(source => source.config_id === preset.config_id), 'Built-in preset was lost from the confirmed settings queue');
    const snapshot = () => ({ phase: state.home.phase, error: state.home.error, selected: state.selected_metadata_source,
      scroll: scrollY, category: document.querySelector<HTMLElement>('.home-catalog')?.dataset.category,
      sections: state.home.data?.page.sections.map(s => ({ kind: s.kind, filters: s.filters })),
      rows: all<HTMLElement>('.catalog-lazy-row').map(e => ({ title: e.querySelector('h2')?.textContent, phase: e.dataset.phase, filter: e.dataset.filter, error: e.querySelector('[role="alert"]')?.textContent })),
      requests: calls.filter(c => c.command === 'get_home' && c.input.source_id === builtin.id) });
    const rendered = () => state.home.phase === 'ready' && all('.home-card').length > 0 && all('.hero-indicators button').length === 5;
    const failed = () => state.home.phase === 'failed' || Object.entries(app.rows).some(([key, row]) => key.includes(builtin.id) && row.phase === 'failed');
    await until(() => rendered() || failed(), () => `Actual built-in TMDB did not render: ${JSON.stringify(snapshot())}`, 120000);
    check(rendered(), `Actual built-in TMDB failed: ${JSON.stringify(snapshot())}`);
    return { origin: builtin.origin, sections: state.home.data!.page.sections.length,
      rows: all<HTMLElement>('.catalog-lazy-row').map(e => ({ title: e.querySelector('h2')!.textContent, filter: JSON.parse(e.dataset.filter!) })),
      hero: currentHero(), cards: all('.home-card').length, calls: calls.filter(c => c.command === 'get_home' && c.input.source_id === builtin.id).map(c => ({ input: c.input, completed: c.completed, titles: c.titles })), errors: [...errors] };
  }
  if (action === 'errors') return errors;
  throw new Error(`Unknown TV3 action ${action}`);
}
