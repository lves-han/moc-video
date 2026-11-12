// Official Video.js controls and media. The only custom feature bridges desktop-window fullscreen.
import { createPlayer as createVideoJsPlayer } from '@videojs/html';
import { videoFeatures } from '@videojs/html/video';
import { controlsFeature, definePlayerFeature, fullscreenFeature, timeFeature } from '@videojs/core/dom';
import '@videojs/html/ui/container';
import '@videojs/html/ui/controls';
import '@videojs/html/ui/controls-content';
import '@videojs/html/ui/play-button';
import '@videojs/html/ui/seek-button';
import '@videojs/html/ui/time-slider';
import '@videojs/html/ui/slider-track';
import '@videojs/html/ui/slider-buffer';
import '@videojs/html/ui/slider-fill';
import '@videojs/html/ui/slider-thumb';
import '@videojs/html/ui/slider-preview';
import '@videojs/html/ui/slider-value';
import '@videojs/html/ui/time';
import '@videojs/html/ui/mute-button';
import '@videojs/html/ui/volume-slider';
import '@videojs/html/ui/playback-rate-button';
import '@videojs/html/ui/fullscreen-button';
import '@videojs/html/ui/captions-button';
import '@videojs/html/ui/captions-radio-group';
import '@videojs/html/ui/audio-track-radio-group';
import '@videojs/html/ui/playback-rate-radio-group';
import '@videojs/html/ui/menu';
import '@videojs/html/ui/menu-content';
import '@videojs/html/ui/menu-radio-item';
import '@videojs/html/ui/buffering-indicator';
import '@videojs/html/ui/hotkey';
import '@videojs/html/ui/gesture';
import '@videojs/html/ui/status-announcer';
import '@videojs/html/icons/element/compat';
import '@videojs/html/ui/tooltip';
import '@videojs/html/ui/tooltip-label';
import '@videojs/html/ui/tooltip-group';
import '@videojs/html/i18n';
import '@videojs/html/i18n/locales/zh-CN/register';
import { nativeInvoke } from './player.ts';

// Keep official seek/lifecycle behavior while matching the application's 250ms
// position cadence. Metadata, seeking and settled pause updates remain immediate.
const coalescedTime: typeof timeFeature = {
  ...timeFeature,
  attach(context) {
    let lastPosition = -Infinity;
    return timeFeature.attach!({ ...context, set(update) {
      if (Object.keys(update).length === 1 && 'currentTime' in update) {
        const now = performance.now();
        if (now - lastPosition < 250) return;
        lastPosition = now;
      }
      context.set(update);
    } });
  },
};

// 10.3.0 fixes the official idle delay at 2s. A one-second activity lock
// extends it to the product's 3s while retaining menu/drag/touch ownership.
const immersiveControls: typeof controlsFeature = {
  ...controlsFeature,
  attach(context) {
    const cleanup = controlsFeature.attach!(context);
    const { target, signal, get } = context;
    let release: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const activity = (event?: Event) => {
      if (event instanceof PointerEvent && event.pointerType === 'touch') return;
      clearTimeout(timer);
      release ??= get().requestControlsLock();
      timer = setTimeout(() => { release?.(); release = undefined; }, 1000);
    };
    for (const type of ['pointermove', 'pointerup', 'keydown', 'keyup', 'focusin']) {
      target.container?.addEventListener(type, activity, { signal });
    }
    signal.addEventListener('abort', () => { clearTimeout(timer); release?.(); }, { once: true });
    activity();
    return cleanup;
  },
};

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
        window.dispatchEvent(new CustomEvent('moc-player-action-error', { detail: String(error) }));
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
        if (!signal.aborted) window.dispatchEvent(new CustomEvent('moc-player-action-error', { detail: String(error) }));
      }
    };
    window.addEventListener('resize', () => { void refresh(); }, { signal });
    void refresh();
  },
});
const { PlayerElement } = createVideoJsPlayer({
  features: videoFeatures.map(feature => feature === timeFeature ? coalescedTime
    : feature === controlsFeature ? immersiveControls
    : feature === fullscreenFeature && window.__TAURI__ ? desktopFullscreen : feature) as typeof videoFeatures,
});
if (!customElements.get('video-player')) customElements.define('video-player', PlayerElement);
