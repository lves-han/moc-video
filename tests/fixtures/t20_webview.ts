// Ordinary public Stremio metadata and JSON playback through the production App and Host.
import { createApp, h, nextTick } from 'vue';
import App from '../../src/App.vue';
import UiProvider from '../../src/ui/UiProvider.vue';
import { appController, appState } from '../../src/state.ts';
import { SOURCE_PRESETS } from '../../src/data/sourcePresets.ts';
import { historyRepository } from '../../src/data/history.ts';
import { locale } from '../../src/i18n';
import type { Player } from '../../src/player.ts';
const PRIMARY = 'https://tmdb.elfhosted.com/N4IgNghgdg5grhGBTEAuEAvAFgWgMIByIAvkA/manifest.json';
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const one = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);
const all = <T extends HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
async function until(condition: () => unknown, message: string, timeout = 45000) {
  const end = performance.now() + timeout;
  while (!condition()) { check(performance.now() < end, message); await delay(50); }
}
export async function publicScenario(_ready: unknown, prototype: { player: Player; base: string; cap: string }) {
  prototype.player.stop();
  const root = document.getElementById('app') as HTMLElement & { __vue_app__?: { unmount(): void } };
  root.__vue_app__!.unmount();
  const original = window.__TAURI__!.core;
  const rpc = async (value: unknown) => {
    const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(value) });
    check(response.ok, 'Host transport failed'); return response.json();
  };
  const calls: string[] = [], errors: string[] = [];
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener('error', onError); window.addEventListener('unhandledrejection', onRejection);
  window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
    if (name === 't01_control' || name === 'player_fullscreen') return original.invoke<T>(name, args);
    calls.push(name); return rpc({ command: name, args });
  } };
  location.hash = '#/home';
  await appController.initialize();
  await appController.updateSettings(s => ({ ...s, ui_locale: 'zh-CN', auto_next: false })); locale.value = 'zh-CN';
  await appController.saveSourceConfig({ config_id: null, kind: 'stremio', name: 'TMDB 中文', urls: PRIMARY, enabled: true });
  for (const name of ['红牛', '光速']) {
    const source = SOURCE_PRESETS.find(s => s.name === name)!;
    await appController.saveSourceConfig({ config_id: null, kind: 'maccms', name, urls: source.api_url, enabled: true });
  }
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) }); app.mount(root);
  try {
    await until(() => all('.home-card-title').length > 0, 'Public recommendations missing', 90000);
    const recommendations = all<HTMLAnchorElement>('.home-card-title').map(a => ({ title: a.textContent!.trim(), href: a.href }));
    const observations: any[] = [];
    let result: any = null;
    for (const card of recommendations.slice(0, 30)) {
      if (!/[一-鿿]/.test(card.title)) continue;
      location.href = card.href;
      await until(() => (appState.details.phase === 'ready' && appState.details.data?.details.title === card.title) || appState.details.phase === 'failed', 'Public detail did not settle');
      await nextTick();
      if (appState.details.phase !== 'ready' || appState.details.data?.details.kind === 'movie') continue;
      const episodes = all<HTMLButtonElement>('.details-episode');
      if (episodes.length < 2) continue;
      const videoId = episodes[0]!.dataset.videoId;
      const before = calls.filter(c => c === 'open_playback').length;
      episodes[0]!.click();
      await until(() => !appController.matching.loading, 'Public search did not settle');
      if (!one('.details-candidate')) { observations.push({ title: card.title, stage: 'match', status: 'no_candidates' }); continue; }
      check(calls.filter(c => c === 'open_playback').length === before, 'Playback opened before confirmation');
      one<HTMLButtonElement>('.details-candidate')!.click();
      await until(() => all('.details-candidate-episode').length > 0 || !appController.matching.loading, 'Public candidate did not settle');
      const candidates = all<HTMLButtonElement>('.details-candidate-episode');
      if (candidates.length < 2) { appController.invalidatePlayback(); continue; }
      candidates[0]!.click();
      await until(() => (one<HTMLVideoElement>('video')?.readyState ?? 0) >= 2 || appState.playback.phase === 'failed', 'Public media did not settle', 90000);
      if (appState.playback.phase === 'failed') { observations.push({ title: card.title, stage: 'media', status: appState.playback.error?.code }); appController.invalidatePlayback(); continue; }
      const video = one<HTMLVideoElement>('video')!; await video.play(); const start = video.currentTime;
      await until(() => video.currentTime >= start + 2, 'Public playback did not advance'); video.pause();

      await historyRepository.drain();
      const first = appState.playback.session!.session_id;
      one<HTMLButtonElement>('[data-action="next"]')!.click();
      await until(() => one('.details-candidate') || (appState.playback.session && appState.playback.session.session_id !== first), 'Next episode did not request confirmation');
      if (one('.details-candidate')) {
        one<HTMLButtonElement>('.details-candidate')!.click(); await until(() => all('.details-candidate-episode').length > 1, 'Next candidate episodes missing');
        all<HTMLButtonElement>('.details-candidate-episode')[1]!.click();
      }
      await until(() => appState.playback.session?.session_id !== first && (one<HTMLVideoElement>('video')?.readyState ?? 0) >= 2, 'Public next episode did not play', 90000);
      const next = one<HTMLVideoElement>('video')!; await next.play(); const nextStart = next.currentTime;
      await until(() => next.currentTime > nextStart + 1, 'Public next did not advance'); next.pause(); await delay(200); await historyRepository.drain();
      const rows = await historyRepository.list();
      result = { title: card.title, chinese: true, metadata_video: videoId, confirmed: true, played: true, next: appState.playback.context!.metadata_video_id !== videoId,
        gateway: next.currentSrc === appState.playback.session!.media_url, saved: rows.some(r => r.record?.video_id === appState.playback.context!.metadata_video_id) };
      break;
    }
    check(result, 'No public series completed playback and switching; observations=' + JSON.stringify(observations));
    appController.invalidatePlayback(); location.hash = '#/history';
    await until(() => all('.history-card').length > 0, 'Public history missing');
    await until(() => { const img = one<HTMLImageElement>('.history-poster'); return img && !img.src.endsWith('/mark.svg') && img.complete && img.naturalWidth > 0; }, 'Public history poster missing');
    result.poster = one<HTMLImageElement>('.history-poster')!.complete && one<HTMLImageElement>('.history-poster')!.naturalWidth > 0;
    result.label = !one('.history-card')!.textContent!.includes('Feature');
    const layouts = [];
    for (const width of [960, 390]) for (const language of ['en', 'zh-CN'] as const) for (const theme of ['light', 'dark'] as const) {
      await original.invoke('t01_control', { action: 'width', value: width });
      await original.invoke('t01_control', { action: 'theme', value: theme === 'dark' ? 1 : 0 });
      await appController.updateSettings(s => ({ ...s, ui_locale: language })); locale.value = language; await delay(250); await nextTick();
      const dark = matchMedia('(prefers-color-scheme: dark)').matches;
      check(dark === (theme === 'dark'), 'Native theme did not change');
      layouts.push({ width: innerWidth, language, theme, dark, overflow: document.documentElement.scrollWidth > innerWidth + 1 });
    }
    await original.invoke('t01_control', { action: 'width', value: 1200 });
    await rpc({ operation: 'restart' });
    return { public: result, recommendations: recommendations.length, observations, layouts, errors };
  } finally { app.unmount(); window.removeEventListener('error', onError); window.removeEventListener('unhandledrejection', onRejection); window.__TAURI__!.core = original; }
}
