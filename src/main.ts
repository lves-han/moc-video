import { createApp, h } from "vue";
import App from "./App.vue";
import UiProvider from "./ui/UiProvider.vue";
import "./style.css";
import "./videojs";
import { appController, appState } from "./state";
import { setAppFont, setFontScale } from "./ui/fonts";
import { locale } from "./i18n";
import { palette, normalizePalette } from "./ui/theme";
import type { EngineStateChanged } from "./models";
import { resetRecoveryInfo } from './resetRecovery';

const recovery = resetRecoveryInfo();
if (!recovery && location.hash !== "#/prototype") {
  const events = (window.__TAURI__ as typeof window.__TAURI__ & {
    event?: { listen<T>(name: string, callback: (event: { payload: T }) => void): Promise<() => void> };
  })?.event;
  if (events) {
    try {
      const unlisten = await events.listen<EngineStateChanged>("engine-state-changed", event => {
        try {
          if (event.payload.engine_epoch !== event.payload.runtime.engine_epoch) throw new Error("Runtime event host epoch differs");
          appController.applyRuntime(event.payload.runtime);
          void appController.loadSources();
        } catch (cause) { appController.diagnostics.push({ code: "ENGINE_NOT_READY", message: String(cause), retryable: true }); }
      });
      window.addEventListener("pagehide", unlisten, { once: true });
    } catch (cause) { appController.diagnostics.push({ code: "ENGINE_NOT_READY", message: `Runtime events: ${String(cause)}`, retryable: true }); }
  }
  // Restore the authoritative Settings before UiProvider's first component frame.
  try {
    const settings = await appController.restoreSettings();
    locale.value = settings.ui_locale ?? "en";
    palette.value = normalizePalette(settings.color_palette);
    document.documentElement.dataset.palette = palette.value;
    document.documentElement.lang = locale.value;
  } catch (cause) {
    appController.diagnostics.push({ code: "INTERNAL_ERROR", message: `Settings: ${String(cause)}`, retryable: true });
  }
  try { await appController.refreshRuntime(); void appController.loadSources(); }
  catch (cause) { appController.diagnostics.push({ code: "ENGINE_NOT_READY", message: String(cause), retryable: true }); }
  if (window.__TAURI__) {
    let lastInteraction = Date.now();
    const active = () => { lastInteraction = Date.now(); appController.cancelBackgroundUpdates(); };
    const idle = () => document.visibilityState === 'visible' && Date.now() - lastInteraction >= 10_000;
    const check = async () => {
      if (!idle()) return;
      await appController.refreshOneSourceIfDue(idle);
      if (idle()) await appController.refreshOneMetadataIfDue(idle);
    };
    document.addEventListener('pointerdown', active, true);
    document.addEventListener('keydown', active, true);
    const initial = window.setTimeout(check, 15_000);
    const timer = window.setInterval(check, 60_000);
    window.addEventListener('pagehide', () => {
      clearTimeout(initial); clearInterval(timer);
      document.removeEventListener('pointerdown', active, true);
      document.removeEventListener('keydown', active, true);
      appController.cancelBackgroundUpdates();
    }, { once: true });
  }
}

const View = recovery ? (await import('./views/ResetRecoveryView.vue')).default : location.hash === "#/prototype"
  ? (await import("./views/PrototypeView.vue")).default
  : App;
if (!recovery) {
  setFontScale(appState.settings.font_scale);
  setAppFont(appState.settings.font_family);
}
createApp({
  render: () => h(UiProvider, null, { default: () => h(View) }),
}).mount("#app");
