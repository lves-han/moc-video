import { createApp, h } from "vue";
import App from "./App.vue";
import UiProvider from "./ui/UiProvider.vue";
import "./style.css";
import { appController } from "./state";
import { locale } from "./i18n";
import { palette, normalizePalette } from "./ui/theme";
import type { EngineStateChanged } from "./models";

if (location.hash !== "#/prototype") {
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
  try { await appController.refreshRuntime(); await appController.loadSources(); }
  catch (cause) { appController.diagnostics.push({ code: "ENGINE_NOT_READY", message: String(cause), retryable: true }); }
}

const View = location.hash === "#/prototype"
  ? (await import("./views/PrototypeView.vue")).default
  : App;
createApp({
  render: () => h(UiProvider, null, { default: () => h(View) }),
}).mount("#app");
