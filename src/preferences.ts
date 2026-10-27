import { Api, api, SettingsQueue } from "./api.ts";
import { defaultSettings } from "./state.ts";
import { getStremioLanguage, locale } from "./i18n.ts";
import { normalizePalette, palette } from "./ui/theme.ts";
import type { Settings } from "./models.ts";

// The browser preview uses the same full-object single-writer protocol as the host.
// Keep the legacy keys as first-paint mirrors; Settings remains the confirmed value.
const key = "yingxiang-video.settings";
const native = Boolean((globalThis as { __TAURI__?: unknown }).__TAURI__);
const preview = new Api(async (command, args) => {
  try {
    if (command === "cancel_request") return { request_id: args.request_id, engine_epoch: null,
      data: { target_request_id: (args.input as { target_request_id: string }).target_request_id, cancelled: false } };
    if (command === "get_settings") {
      const saved = localStorage.getItem(key);
      const data = saved ? JSON.parse(saved) as Settings : { ...defaultSettings(), ui_locale: locale.value, color_palette: palette.value };
      if (data.schema_version !== 2) throw new Error("Unsupported preview Settings version");
      return { request_id: args.request_id, engine_epoch: null, data };
    }
    if (command === "save_settings") {
      localStorage.setItem(key, JSON.stringify(args.input));
      return { request_id: args.request_id, engine_epoch: null, data: args.input };
    }
    throw new Error("Preview preferences command is unavailable");
  } catch (error) {
    return { request_id: args.request_id, engine_epoch: null,
      error: { code: "INTERNAL_ERROR", message: error instanceof Error ? error.message : String(error), retryable: true } };
  }
});
export const preferenceQueue = new SettingsQueue(native ? api : preview);
export function applyPreferences(value: Settings) {
  locale.value = getStremioLanguage(value.ui_locale ?? "en");
  palette.value = normalizePalette(value.color_palette);
  if (typeof document !== "undefined") {
    document.documentElement.dataset.palette = palette.value;
    document.documentElement.lang = locale.value;
  }
}
export async function restorePreferences() {
  const settings = await preferenceQueue.initialize();
  applyPreferences(settings);
  return settings;
}
