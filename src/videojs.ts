// Official Video.js controls and media. The only custom feature bridges desktop-window fullscreen.
import { createPlayer as createVideoJsPlayer } from '@videojs/html';
import { videoFeatures } from '@videojs/html/video';
import { definePlayerFeature, fullscreenFeature } from '@videojs/core/dom';
import '@videojs/html/video/skin';
import '@videojs/html/media/hlsjs-video';
import '@videojs/html/i18n/locales/zh-CN/register';
import { nativeInvoke } from './player.ts';

const desktopFullscreen: typeof fullscreenFeature = definePlayerFeature({
  name: 'fullscreen',
  state: ({ set, signals }) => {
    let version = 0;
    async function change(target: boolean) {
      const request = ++version;
      const signal = signals.supersede("native-fullscreen");
      try {
        const active = await nativeInvoke<boolean>('player_fullscreen', { target });
        if (!signal.aborted && request === version) set({ isFullscreen: active });
      } catch (error) {
        if (signal.aborted) return;
        window.dispatchEvent(new CustomEvent('yingxiang-player-action-error', { detail: String(error) }));
        throw error;
      }
    }
    return { isFullscreen: false as boolean, fullscreenAvailability: 'available' as 'available' | 'unavailable' | 'unsupported',
      requestFullscreen: () => change(true), exitFullscreen: () => change(false) };
  },
  attach({ signal, get, set }) {
    let version = 0;
    const refresh = async () => {
      const request = ++version, previous = get().isFullscreen;
      try {
        const active = await nativeInvoke<boolean>('player_fullscreen', { query: true });
        if (!signal.aborted && request === version && get().isFullscreen === previous) set({ isFullscreen: active });
      } catch (error) {
        if (!signal.aborted) window.dispatchEvent(new CustomEvent('yingxiang-player-action-error', { detail: String(error) }));
      }
    };
    window.addEventListener('resize', () => { void refresh(); }, { signal });
    void refresh();
  },
});
const { PlayerElement } = createVideoJsPlayer({
  features: videoFeatures.map(feature => feature === fullscreenFeature && window.__TAURI__ ? desktopFullscreen : feature) as typeof videoFeatures,
});
if (!customElements.get('video-player')) customElements.define('video-player', PlayerElement);
