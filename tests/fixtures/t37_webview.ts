// Production App -> current matching -> real MacCMS adapter/Host -> gateway ->
// decoded WKWebView frames. Only the transport and upstream mutations are test code.
import { createApp, h, nextTick } from 'vue';
import App from '../../src/App.vue';
import UiProvider from '../../src/ui/UiProvider.vue';
import { api } from '../../src/api';
import { appController as app, appState as state, metadataVideoId } from '../../src/state';
import { LINE_PREFERENCE_KEY } from '../../src/line-preference';
import { historyRepository } from '../../src/data/history';
import { nativeInvoke, type Player } from '../../src/player';
import type { Bridge_MediaRef } from '../../src/models';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
const all = <T extends HTMLElement = HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)].filter(element => !element.closest('.page-transition-old'));
const one = <T extends HTMLElement = HTMLElement>(selector: string) => { const element = all<T>(selector)[0]; check(element, `Missing ${selector}`); return element; };
const video = () => one<HTMLVideoElement>('video');
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const calls: { command: string; input: any }[] = [], errors: string[] = [];
let rpc: (input: unknown) => Promise<any>;
let mounted = false, holdOpen = false, openHeld = false, releaseOpen: (() => void) | undefined;
let holdDetails = false, detailsHeld = false, releaseDetails: (() => void) | undefined;
let nullCoordinates = false;
let productionUntouched = () => false;
async function until(condition: () => unknown, message: string, seconds = 30) {
  const end = performance.now() + seconds * 1000;
  while (!condition()) {
    check(performance.now() < end, `${message}; ${JSON.stringify({ route: state.route.name, phase: state.playback.phase,
      match: app.matching.result?.status, pending: app.matching.pending, error: state.playback.error?.message, storage: app.lineMemory, errors })}`);
    await delay(20);
  }
}
async function goDetails(ref: Bridge_MediaRef) {
  const expected = `#/details/${encodeURIComponent(JSON.stringify(ref))}`;
  if (location.hash === expected) { app.navigate({ name: 'details', ref }); await app.loadDetails(ref); }
  else location.hash = expected;
  await until(() => state.route.name === 'details' && state.details.phase === 'ready' && state.details.data?.details.ref.data === ref.data && !!all('.details-episode').length, 'Current details did not open');
  await nextTick(); await delay(100);
}
async function decoded(expectSaved = true) {
  await until(() => state.playback.session && all('video').length && video().readyState >= 2, 'Actual video did not load');
  if (video().paused) await video().play();
  await until(() => video().getVideoPlaybackQuality().totalVideoFrames > 1 && video().currentTime > .15 && state.playback.position_ms > 150 && state.playback.phase === 'playing', 'Decoded frames did not advance');
  if (expectSaved) await until(() => app.lineMemory.saved, 'Current decoded playback did not save its line');
}
async function confirm(group: number, episode = 0) {
  await until(() => all('.details-candidate').length, 'Confirmation candidate is missing');
  one<HTMLButtonElement>('.details-candidate').click();
  await until(() => all('.details-candidate-group').length === 3 && !app.matching.loading, 'Three current provider routes did not load');
  const section = all('.details-candidate-group')[group]!;
  nullCoordinates = Boolean(app.matching.details?.details.groups.every(group => group.episodes.every(episode => episode.season === null && episode.number === null)));
  const buttons = [...section.querySelectorAll<HTMLButtonElement>('.details-candidate-episode')];
  check(buttons[episode], 'Requested current episode missing'); buttons[episode]!.click();
}
const routeIndex = () => JSON.parse(state.playback.context!.episode_ref.data)[2] as string;
const handles = () => ({ session: state.playback.session!.session_id, resolution: state.playback.resolution!.id, stream: state.playback.session!.stream_id });
function fresh(before: ReturnType<typeof handles>, after: ReturnType<typeof handles>) {
  return before.session !== after.session && before.resolution !== after.resolution && before.stream !== after.stream;
}
const raw = () => localStorage.getItem(LINE_PREFERENCE_KEY);
async function traces() { return (await rpc({ operation: 'trace' })).media as string[]; }
async function metadataDetails() {
  const source = state.sources.data!.providers.find(source => source.kind === 'stremio')!;
  const home = await api.call('get_home', { source_id: source.id, source_revision: source.source_revision, section_ref: null, page: 1, filter: null });
  const ref = home.page.sections[0]!.items[0]!.ref; await goDetails(ref); return copy(ref);
}
async function directDetails() {
  const source = state.sources.data!.providers.find(source => source.kind === 'maccms' && source.name !== 'Alternate')!;
  return { source, ref: { source_id: source.id, source_revision: source.source_revision, data: JSON.stringify(['maccms', 'show']) } };
}
async function initialize(prototype: { player: Player; base: string; cap: string }) {
  if (mounted) return;
  // Use the actual persistent WebKit backend without clearing or replacing
  // the user's production records. All three fresh windows use this owned key.
  const fixtureKey = `${LINE_PREFERENCE_KEY}.acceptance-t37`;
  const get = Storage.prototype.getItem, set = Storage.prototype.setItem, remove = Storage.prototype.removeItem;
  const productionBefore = get.call(localStorage, LINE_PREFERENCE_KEY);
  productionUntouched = () => get.call(localStorage, LINE_PREFERENCE_KEY) === productionBefore;
  Storage.prototype.getItem = function(key) { return get.call(this, key === LINE_PREFERENCE_KEY ? fixtureKey : key); };
  Storage.prototype.setItem = function(key, value) { set.call(this, key === LINE_PREFERENCE_KEY ? fixtureKey : key, value); };
  Storage.prototype.removeItem = function(key) { remove.call(this, key === LINE_PREFERENCE_KEY ? fixtureKey : key); };
  prototype.player.stop();
  const root = one('#app') as HTMLElement & { __vue_app__?: { unmount(): void } }; root.__vue_app__!.unmount();
  const original = window.__TAURI__!.core;
  rpc = async value => { const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(value) }); check(response.ok, 'Actual Host test transport failed'); return response.json(); };
  window.__TAURI__!.core = { ...original, invoke: async <T>(command: string, args?: Record<string, any>): Promise<T> => {
    if (!args?.request_id) return original.invoke<T>(command, args);
    calls.push({ command, input: copy(args.input) });
    const response = await rpc({ command, args });
    if (holdOpen && command === 'open_playback') { openHeld = true; await new Promise<void>(resolve => { releaseOpen = resolve; }); }
    if (holdDetails && command === 'get_details' && JSON.parse(args.input.ref.data)[0] === 'maccms') { detailsHeld = true; await new Promise<void>(resolve => { releaseDetails = resolve; }); }
    return response;
  } };
  window.addEventListener('error', event => errors.push(event.message));
  window.addEventListener('unhandledrejection', event => errors.push(String(event.reason)));
  location.hash = '#/home'; await app.initialize();
  createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) }).mount(root); mounted = true;
  await until(() => state.selected_playback_source && state.selected_metadata_source && all('.tv-sidebar').length, 'Production app did not initialize');
}

export async function linePreferenceScenario(action: string, _ready: unknown, prototype: { player: Player; base: string; cap: string }) {
  await initialize(prototype);
  const evidence: Record<string, any> = {};
  if (action === 'cross-source') {
    localStorage.removeItem(LINE_PREFERENCE_KEY);
    const ref = await metadataDetails();
    one<HTMLButtonElement>('.details-episode').click(); await confirm(0);
    await until(() => state.playback.phase === 'failed', 'First route must fail actual decoding');
    evidence.first_failure = { saved: raw() !== null, opened: calls.filter(c => c.command === 'open_playback').length, code: state.playback.error?.code };
    await goDetails(ref); one<HTMLButtonElement>('.details-episode').click(); await confirm(1);
    await until(() => state.playback.phase === 'failed', 'Second route must be unplayable');
    evidence.second_failure = { saved: raw() !== null, opened: calls.filter(c => c.command === 'open_playback').length, code: state.playback.error?.code };
    await goDetails(ref); holdOpen = true; one<HTMLButtonElement>('.details-episode').click(); await confirm(2);
    await until(() => openHeld, 'Playable route did not reach a real Host Session');
    evidence.resolve_only = { saved: raw() !== null, opened: calls.filter(c => c.command === 'open_playback').length };
    holdOpen = false; releaseOpen!(); releaseOpen = undefined; await decoded();
    evidence.third = { route: routeIndex(), saved: app.lineMemory.saved, frames: video().getVideoPlaybackQuality().totalVideoFrames, position: video().currentTime,
      null_coordinates: nullCoordinates, record_count: JSON.parse(raw()!).length };
    let previous = handles(); const initialTrace = await traces();
    one<HTMLButtonElement>('[data-action="next"]').click(); await until(() => state.playback.context?.metadata_video_id === 'v2' && state.playback.session, 'Next episode did not use remembered route'); await decoded();
    evidence.second = { route: routeIndex(), fresh: fresh(previous, handles()), dialog: !!all('[role=dialog]').length };
    previous = handles(); await goDetails(ref); all<HTMLButtonElement>('.details-episode')[2]!.click();
    await until(() => state.playback.context?.metadata_video_id === 'v3' && state.playback.session, 'Other episode did not use remembered route'); await decoded();
    evidence.other = { route: routeIndex(), fresh: fresh(previous, handles()), dialog: !!all('[role=dialog]').length };
    const afterTrace = await traces();
    evidence.no_failed_retry = afterTrace.filter(path => path === '/media/broken.mp4').length === initialTrace.filter(path => path === '/media/broken.mp4').length;
    const stable = raw(); evidence.ambiguities = [];
    for (const mode of [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]) {
      await rpc({ operation: 'mode', value: mode }); await goDetails(ref);
      const before = calls.filter(c => c.command === 'open_playback').length;
      one<HTMLButtonElement>('.details-episode').click();
      await until(() => app.matching.result?.status === 'needs_confirmation', `Mutation ${mode} bypassed confirmation`);
      evidence.ambiguities.push({ mode, confirmation: !!all('[role=dialog]').length, opens: calls.filter(c => c.command === 'open_playback').length - before, kept: raw() === stable });
      one<HTMLButtonElement>('.details-confirm-cancel').click(); await nextTick();
    }
    await rpc({ operation: 'mode', value: 13 }); await goDetails(ref);
    const unavailableOpens = calls.filter(call => call.command === 'open_playback').length;
    all<HTMLButtonElement>('.details-episode')[1]!.click();
    await until(() => app.matching.result?.status === 'needs_confirmation', 'Unplayable remembered line bypassed confirmation');
    evidence.unavailable_line = { confirmation: true, opens: calls.filter(call => call.command === 'open_playback').length - unavailableOpens, kept: raw() === stable };
    one<HTMLButtonElement>('.details-confirm-cancel').click(); await nextTick();
    await rpc({ operation: 'mode', value: 0 });
    // Storage read faults keep original confirmation and preserve exact bytes.
    const get = Storage.prototype.getItem, set = Storage.prototype.setItem;
    Storage.prototype.getItem = function(key) { if (key === LINE_PREFERENCE_KEY) throw new DOMException('denied', 'SecurityError'); return get.call(this, key); };
    try { await goDetails(ref); one<HTMLButtonElement>('.details-episode').click(); await until(() => app.matching.result?.status === 'needs_confirmation', 'Unavailable storage did not fall back'); evidence.unavailable_read = { notice: app.lineMemory.error.includes('read failed'), preserved: get.call(localStorage, LINE_PREFERENCE_KEY) === stable }; }
    finally { Storage.prototype.getItem = get; }
    one<HTMLButtonElement>('.details-confirm-cancel').click();
    localStorage.setItem(LINE_PREFERENCE_KEY, '{damaged'); await goDetails(ref); one<HTMLButtonElement>('.details-episode').click();
    await until(() => app.matching.result?.status === 'needs_confirmation', 'Damaged storage did not fall back');
    evidence.corruption = { notice: app.lineMemory.error.includes('damaged'), preserved: raw() === '{damaged' }; one<HTMLButtonElement>('.details-confirm-cancel').click();
    localStorage.setItem(LINE_PREFERENCE_KEY, stable!);
    await goDetails(ref); await app.match('v1', null, false);
    // Explicit confirmation selects the same line while storage is temporarily unwritable.
    Storage.prototype.setItem = function(key, value) { if (key === LINE_PREFERENCE_KEY) throw new DOMException('full', 'QuotaExceededError'); set.call(this, key, value); };
    try {
      await confirm(2); await decoded(false); await until(() => app.lineMemory.error.includes('save failed'), 'Write error was hidden');
      const start = video().currentTime; await delay(250);
      evidence.failed_write = { notice: true, kept: raw() === stable, advanced: video().currentTime > start, saved: app.lineMemory.saved };
    } finally { Storage.prototype.setItem = set; }
    await until(() => app.lineMemory.saved && !app.lineMemory.error, 'Writable storage did not recover on actual playback', 15);
    evidence.recovered_write = true;

    // A playable manual override replaces the preference only after decoding.
    await rpc({ operation: 'mode', value: 12 });
    one<HTMLButtonElement>('[data-tray-tab="streams"]').click(); await nextTick();
    one<HTMLButtonElement>('[data-action="change-line"]').click();
    await confirm(0); await decoded(); await goDetails(ref); all<HTMLButtonElement>('.details-episode')[1]!.click();
    await until(() => state.playback.context?.metadata_video_id === 'v2' && state.playback.session, 'Manual override was not reused'); await decoded();
    evidence.manual_override = { route: routeIndex() };
    // Restore the third line by a real manual confirmation and actual playback.
    await goDetails(ref); await app.match('v1', null, false);
    await confirm(2); await decoded(); await rpc({ operation: 'mode', value: 0 });
    const success = raw();
    // Hold a current response after Host completion; explicit source choice wins.
    holdDetails = true; detailsHeld = false;
    const late = app.match('v2'); await until(() => detailsHeld, 'Late details response was not captured');
    const alternate = state.sources.data!.providers.find(source => source.name === 'Alternate')!;
    await app.selectSource(alternate.id, 'playback');
    holdDetails = false; releaseDetails!(); releaseDetails = undefined; await late;
    evidence.race = { selected: state.selected_playback_source?.source_id === alternate.id, context: state.playback.context === null, kept: raw() === success, pending: app.matching.pending === null };
    const source = state.sources.data!.providers.find(source => source.kind === 'maccms' && source.name !== 'Alternate')!;
    check(source.kind === 'maccms', 'Expected remembered MacCMS source');
    await app.setSourceEnabled(source.origin.config_id, false);
    const missingSource = await app.match('v2');
    evidence.source_unavailable = { confirmation: missingSource?.status === 'needs_confirmation', alternate: state.selected_playback_source?.source_id === alternate.id, kept: raw() === success };
    app.invalidatePlayback(); await app.setSourceEnabled(source.origin.config_id, true);
    evidence.restart_choice = { last_global_is_alternate: state.settings.last_playback_source_key === alternate.id, remembered_source_differs: source.id !== alternate.id };
    evidence.storage = { records: JSON.parse(raw()!).length, sensitive: /T37SECRET|http|session|resolution|stream|mediaTime|fingerprint|Line memory|Reliable/.test(raw()!) };
  } else if (action === 'cross-restart-and-same-source') {
    const ref = await metadataDetails(), before = state.selected_playback_source!.source_id;
    all<HTMLButtonElement>('.details-episode')[1]!.click();
    await until(() => state.playback.context?.metadata_video_id === 'v2' && state.playback.session, 'Restart did not restore cross-source line'); await decoded();
    evidence.cross_restart = { route: routeIndex(), changed_global: state.selected_playback_source?.source_id !== before, records: JSON.parse(raw()!).length };
    const direct = await directDetails();
    await app.selectSource(direct.source.id, 'playback');
    await goDetails(direct.ref);
    const original = state.details.data!.details.groups[0]!.episodes.map(episode => metadataVideoId(episode.ref));
    one<HTMLButtonElement>('.details-episode').click(); await until(() => state.playback.phase === 'failed', 'Same-source first route must fail');
    await goDetails(direct.ref); all<HTMLButtonElement>('.details-group')[1]!.click(); await nextTick(); one<HTMLButtonElement>('.details-episode').click();
    await until(() => state.playback.phase === 'failed', 'Same-source second route must be unavailable');
    await goDetails(direct.ref); all<HTMLButtonElement>('.details-group')[2]!.click(); await nextTick(); one<HTMLButtonElement>('.details-episode').click(); await decoded();
    let previous = handles(), previousUrl = state.playback.session!.media_url;
    await goDetails(direct.ref); all<HTMLButtonElement>('.details-episode')[1]!.click();
    await until(() => state.playback.session && state.playback.context?.metadata_video_id === original[1], 'Same-source requested video did not remap'); await decoded();
    const response = await fetch(previousUrl); await response.arrayBuffer();
    evidence.same_second = { route: routeIndex(), video: state.playback.context!.metadata_video_id === original[1], fresh: fresh(previous, handles()), old_http: response.status };
    previous = handles(); one<HTMLButtonElement>('[data-action="next"]').click();
    await until(() => state.playback.session && state.playback.context?.metadata_video_id === original[2], 'Same-source next video identity changed'); await decoded();
    video().pause(); await historyRepository.drain(); await delay(150); await historyRepository.drain();
    const rows = await api.call('list_history', {}), saved = rows.entries.find(row => row.content_id === state.details.data!.details.id)!;
    evidence.same_third = { route: routeIndex(), video: state.playback.context!.metadata_video_id === original[2], history_video: saved?.video_id === original[2], fresh: fresh(previous, handles()), boundary: app.nextVideo(original[2]!) === null };
    evidence.storage = { records: JSON.parse(raw()!).length, sensitive: /T37SECRET|http|session|resolution|stream|fingerprint/.test(raw()!) };
    await goDetails(ref); // Retire the direct Session before the owned process closes.
  } else if (action === 'same-source-restart') {
    const direct = await directDetails(); await goDetails(direct.ref);
    const original = metadataVideoId(state.details.data!.details.groups[0]!.episodes[1]!.ref);
    all<HTMLButtonElement>('.details-episode')[1]!.click();
    await until(() => state.playback.session && state.playback.context?.metadata_video_id === original, 'Same-source restart did not remap'); await decoded();
    evidence.same_restart = { route: routeIndex(), video: state.playback.context!.metadata_video_id === original, saved: app.lineMemory.saved };
    evidence.original_fallback = [];
    for (const mode of [1, 2]) {
      await rpc({ operation: 'mode', value: mode }); await goDetails(direct.ref);
      const expected = metadataVideoId(state.details.data!.details.groups[0]!.episodes[1]!.ref);
      const result = await app.match(expected);
      evidence.original_fallback.push({ mode, route: JSON.parse(result!.context!.episode_ref.data)[2], video: result!.context!.metadata_video_id === expected, original_group: result!.context!.group_id === state.details.data!.details.groups[0]!.id });
    }
    await rpc({ operation: 'mode', value: 0 }); await goDetails(direct.ref);
    // A revised source has no access to the old source revision's preference.
    check(direct.source.kind === 'maccms', 'Expected MacCMS source');
    const configId = direct.source.origin.config_id;
    const config = state.settings.source_configs.find(config => config.kind === 'maccms' && config.config_id === configId)!;
    check(config.kind === 'maccms', 'Expected JSON configuration');
    const revision = direct.source.source_revision;
    await app.saveSourceConfig({ kind: 'maccms', config_id: config.config_id, name: config.name, enabled: true, urls: `${config.api_url}?revision=2` });
    const current = state.sources.data!.providers.find(source => source.id === direct.source.id)!;
    const changedRef = { ...direct.ref, source_revision: current.source_revision }; await goDetails(changedRef);
    const id = metadataVideoId(state.details.data!.details.groups[0]!.episodes[1]!.ref), result = await app.match(id);
    evidence.revision = { changed: revision !== current.source_revision, original_group: result!.context!.group_id === state.details.data!.details.groups[0]!.id };
    // The owned profile is discarded; restore the original configuration for cleanup.
    await app.saveSourceConfig({ kind: 'maccms', config_id: config.config_id, name: config.name, enabled: true, urls: config.api_url });
    const another = { ...direct.ref, data: JSON.stringify(['maccms', 'another-film']) };
    await goDetails(another);
    const otherVideo = metadataVideoId(state.details.data!.details.groups[0]!.episodes[1]!.ref);
    const other = await app.match(otherVideo);
    evidence.other_film = { different: other!.context!.metadata_ref.identity.canonical_id !== result!.context!.metadata_ref.identity.canonical_id,
      original_group: other!.context!.group_id === state.details.data!.details.groups[0]!.id };
    await goDetails(direct.ref);
    evidence.storage = { records: JSON.parse(raw()!).length, sensitive: /T37SECRET|http|session|resolution|stream|fingerprint/.test(raw()!) };
  } else throw new Error(`Unknown T37 phase ${action}`);
  await historyRepository.drain();
  evidence.production_preserved = productionUntouched();
  if (action === 'same-source-restart') localStorage.removeItem(LINE_PREFERENCE_KEY);
  evidence.errors = [...errors]; evidence.native = await nativeInvoke('t01_control', { action: 'identity' });
  return evidence;
}
