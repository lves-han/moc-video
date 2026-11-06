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
    if (['t01_control', 'player_fullscreen', 'window_bounds_now', 'app_close_started', 'app_close_aborted', 'app_close_ready'].includes(name)) return original.invoke<T>(name, args);
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
    await until(() => appState.home.phase !== 'loading', 'Public home did not settle', 90000);
    const homeItems = appState.home.data!.page.sections.flatMap(section => section.items);
    const recommendations = [...new Map(homeItems.filter(card => card.kind === 'series' && /[一-鿿]/.test(card.title)).map(card => [card.id, { title: card.title, href: '#/details/' + encodeURIComponent(JSON.stringify(card.ref)) }])).values()];
    const playback = appState.sources.data!.providers.find(source => source.name === '红牛' && source.runtime_status === 'ready');
    check(playback, 'Public playback source is not ready');
    await appController.selectSource(playback!.id, 'playback');
    const observations: any[] = [];
    let result: any = null;
    for (const card of recommendations.slice(0, 8)) {
      if (!/[一-鿿]/.test(card.title)) continue;
      location.hash = card.href;
      await delay(100);
      await until(() => (appState.details.phase === 'ready' && appState.details.data?.details.title === card.title) || appState.details.phase === 'failed', 'Public detail did not settle');
      await nextTick();
      if (appState.details.phase !== 'ready') { observations.push({ title: card.title, stage: 'details', status: appState.details.error?.code }); continue; }
      const group = appState.details.data!.details.groups.find(group => group.episodes.length >= 2);
      if (!group) { observations.push({ title: card.title, stage: 'videos', status: 'fewer_than_two' }); continue; }
      all<HTMLButtonElement>('.details-group').find(button => button.dataset.groupId === group.id)?.click();
      await nextTick();
      const episodes = all<HTMLButtonElement>('.details-episode');
      check(episodes.length >= 2, 'Selected metadata season did not render');
      const videoId = episodes[0]!.dataset.videoId;
      const before = calls.filter(c => c === 'open_playback').length;
      episodes[0]!.click();
      await nextTick(); await delay(100);
      await until(() => appController.matching.pending === null && (appController.matching.result !== null || appState.playback.phase === 'failed'), 'Public search did not settle');
      await nextTick();
      if (!one('.details-candidate')) { observations.push({ title: card.title, stage: 'match', status: appController.matching.result?.status ?? 'failed', code: appState.playback.error?.code }); continue; }
      check(calls.filter(c => c === 'open_playback').length === before, 'Playback opened before confirmation');
      one<HTMLButtonElement>('.details-candidate')!.click();
      await nextTick(); await delay(100);
      await until(() => all('.details-candidate-episode').length > 0 || !appController.matching.loading, 'Public candidate did not settle');
      const directGroup = all('.details-candidate-group').find(group => /m3u8|hls|mp4/i.test(group.querySelector('h3')?.textContent ?? ''));
      const candidates = directGroup ? [...directGroup.querySelectorAll<HTMLButtonElement>('.details-candidate-episode')] : [];
      const line = directGroup?.querySelector('h3')?.textContent;
      if (candidates.length < 2) { appController.invalidatePlayback(); continue; }
      candidates[0]!.click();
      await until(() => (one<HTMLVideoElement>('video')?.readyState ?? 0) >= 2 || appState.playback.phase === 'failed', 'Public media did not settle', 90000);
      if (appState.playback.phase === 'failed') { observations.push({ title: card.title, line, stage: 'media', status: appState.playback.error?.code, message: appState.playback.error?.message }); appController.invalidatePlayback(); continue; }
      const video = one<HTMLVideoElement>('video')!; await video.play(); const start = video.currentTime;
      await until(() => video.currentTime >= start + 2, 'Public playback did not advance'); video.pause();

      await until(() => appState.playback.phase === 'paused', 'Initial pause did not settle');
      await nextTick();
      const actions = ['play', 'stop', 'next', 'episodes', 'fullscreen'];
      const iconButtons = actions.map(action => {
        const button = one<HTMLButtonElement>(`[data-action="${action}"]`)!;
        check(button && button.querySelector('svg') && !button.textContent?.trim(), `Playback ${action} must be icon-only`);
        check(button.title && button.title === button.getAttribute('aria-label'), `Playback ${action} needs matching hover and accessible text`);
        return { action, icon: true, tooltip: button.title, accessible: true, width: button.getBoundingClientRect().width };
      });
      const playButton = one<HTMLButtonElement>('[data-action="play"]')!;
      playButton.focus();
      check(document.activeElement === playButton, 'Playback icon button cannot receive keyboard focus');
      playButton.click(); await until(() => !video.paused && playButton.title === '暂停', 'Icon play action or pause tooltip did not update');
      playButton.click(); await until(() => video.paused && playButton.title === '播放', 'Icon pause action or play tooltip did not update');
      const mute = one<HTMLInputElement>('input[aria-label="静音"]')!;
      mute.click(); await until(() => video.muted && mute.parentElement!.title === '取消静音', 'Mute icon did not update its action');
      mute.click(); await until(() => !video.muted, 'Mute icon did not restore audio');
      await nextTick();
      check(iconButtons.every(button => button.width >= 44), 'Playback icons must retain usable hit targets');
      const iconLayouts = [];
      for (const width of [960, 390]) {
        await original.invoke('t01_control', { action: 'width', value: width }); await delay(250);
        const controls = one('.playback-controls')!;
        const fits = document.documentElement.scrollWidth <= innerWidth + 1 && controls.scrollWidth <= controls.clientWidth + 1;
        check(fits, 'Playback icon controls overflowed the viewport');
        iconLayouts.push({ width: innerWidth, fits });
      }
      await original.invoke('t01_control', { action: 'width', value: 1200 });

      await historyRepository.drain();
      const first = appState.playback.session!.session_id;
      one<HTMLButtonElement>('[data-action="next"]')!.click();
      await until(() => one('.details-candidate') || (appState.playback.session && appState.playback.session.session_id !== first), 'Next episode did not request confirmation');
      if (one('.details-candidate')) {
        one<HTMLButtonElement>('.details-candidate')!.click(); await until(() => all('.details-candidate-episode').length > 1, 'Next candidate episodes missing');
        const nextGroup = all('.details-candidate-group').find(group => /m3u8|hls|mp4/i.test(group.querySelector('h3')?.textContent ?? ''));
        check(nextGroup, 'Direct media group missing on next confirmation');
        nextGroup!.querySelectorAll<HTMLButtonElement>('.details-candidate-episode')[1]!.click();
      }
      await until(() => (appState.playback.session?.session_id !== first && (one<HTMLVideoElement>('video')?.readyState ?? 0) >= 2) || appState.playback.phase === 'failed', 'Public next episode did not settle', 90000);
      check(appState.playback.phase !== 'failed', 'Public next episode failed: ' + JSON.stringify({ phase: appState.playback.phase, code: appState.playback.error?.code, message: appState.playback.error?.message, video: appState.playback.context?.metadata_video_id, has_session: Boolean(appState.playback.session) }));
      const next = one<HTMLVideoElement>('video')!; await next.play(); const nextStart = next.currentTime;
      await until(() => next.currentTime > nextStart + 1, 'Public next did not advance'); next.pause(); await delay(200); await historyRepository.drain();
      const rows = await historyRepository.list();
      result = { icon_layouts: iconLayouts, icon_buttons: iconButtons, icon_play_pause: true, icon_mute: true, keyboard_focus: true, title: card.title, line, chinese: true, metadata_video: videoId, confirmed: true, played: true, next: appState.playback.context!.metadata_video_id !== videoId,
        gateway: next.currentSrc === appState.playback.session!.media_url, saved: rows.some(r => r.record?.video_id === appState.playback.context!.metadata_video_id) };
      break;
    }
    check(result, 'No public series completed playback and switching; series=' + recommendations.length + '; observations=' + JSON.stringify(observations));
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
    const backupUrl = 'https://muthukumarkr-stremio-muthu.hf.space/H4sIAAAAAAAC_6tWyk0tSUxJLEkMzi8tSk5VslIqyU1JUtIBUz6Jeemliekg0aoMXWc_oHBxamJRcgZEcbGSVTREeWwtALGe9bRJAAAA/manifest.json';
    const metadataBeforeBackup = appState.selected_metadata_source!.source_id;
    await appController.saveSourceConfig({ config_id: null, kind: 'stremio', name: 'TMDB 中文备份', urls: backupUrl, enabled: true });
    const backupSource = appState.sources.data!.providers.find(source => source.name === 'TMDB 中文备份')!;
    check(backupSource?.runtime_status === 'ready', 'Backup metadata registration failed');
    const backupDetail = await rpc({ command: 'get_details', args: { request_id: crypto.randomUUID(), input: { ref: {
      source_id: backupSource.id, source_revision: backupSource.source_revision,
      data: JSON.stringify(['stremio', 'bcbbd55c35fbd9e6d91b7f311bf0e51cfabe2f731f865db97a9cb7f9a73eccf0', 'series', 'tmdb:1399'])
    } } } });
    check(backupDetail.data?.details.title === '权力的游戏' && backupDetail.data.details.groups.some((group: any) => group.episodes.length > 0), 'Backup Chinese detail/videos failed');
    const backup = { registered: true, chinese_details: true, videos: true, home: backupSource.capabilities.home, primary_kept: appState.selected_metadata_source!.source_id === metadataBeforeBackup };
    await rpc({ operation: 'restart' });
    return { public: result, recommendations: recommendations.length, observations, layouts, backup, errors };
  } finally { app.unmount(); window.removeEventListener('error', onError); window.removeEventListener('unhandledrejection', onRejection); window.__TAURI__!.core = original; }
}
