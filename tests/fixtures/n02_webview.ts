// Test-mode only. Runs the production App against a real Host: public presets first, then the controlled flow.
import { createApp, h, nextTick } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState } from "../../src/state.ts";
import type { Player } from "../../src/player.ts";
import { locale } from "../../src/i18n";

type Ready = { manifest: string; maccms: string; second: string; failing: string; refused: string; stale_preference: string };
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
let describe = () => "";
async function until(condition: () => unknown, message: string, ms = 20000) {
  const end = performance.now() + ms;
  while (!condition()) { check(performance.now() < end, `Timed out: ${message}; state=${describe()}`); await delay(20); }
}
const one = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);
const all = <T extends HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];
const set = (selector: string, value: string, event = "input") => { const element = one<HTMLInputElement>(selector)!; element.value = value; element.dispatchEvent(new Event(event, { bubbles: true })); };
const row = (name: string) => all(".sources-row").find(item => item.querySelector("h3")?.textContent === name)!;
const preset = (name: string) => one(`.sources-preset[data-preset="${name}"]`)!.querySelector<HTMLButtonElement>(".sources-preset-add")!;
const providers = () => appState.sources.data?.providers ?? [];
const quiet = () => !all<HTMLButtonElement>(".sources-delete").some(button => button.disabled);

export async function sourcesToPlaybackScenario(ready: Ready, prototype: { player: Player; base: string; cap: string }) {
  prototype.player.stop();
  locale.value = "zh-CN";
  const container = document.getElementById("app") as HTMLElement & { __vue_app__?: { unmount(): void } };
  check(container.__vue_app__, "Original native fixture root is missing");
  container.__vue_app__!.unmount();
  const rpc = async (value: unknown) => {
    const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(value) });
    check(response.ok, "Real Host test transport failed");
    return response.json();
  };
  const original = window.__TAURI__!.core, calls: { command: string; input: any }[] = [], errors: string[] = [];
  window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
    if (name === "t01_control" || name === "player_fullscreen") return original.invoke<T>(name, args);
    calls.push({ command: name, input: args?.input });
    return await rpc({ command: name, args });
  } };
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  const count = (command: string) => calls.filter(call => call.command === command).length;
  const evidence: Record<string, any> = {};
  describe = () => JSON.stringify({ hash: location.hash, route: appState.route.name, failure: one(".sources-failure")?.textContent, errors_ui: one(".sources-errors")?.textContent, playback: appState.playback.phase, dialog: Boolean(one('[role="dialog"]')), match: appState.playback.error?.message, details: appState.details.phase, home: appState.home.phase, video: one<HTMLVideoElement>("video")?.readyState, errors });
  location.hash = "#/sources";
  await appController.initialize();
  await appController.updateSettings(settings => ({ ...settings, ui_locale: "zh-CN" }));
  locale.value = "zh-CN";
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) });
  app.mount(container);
  const add = async (kind: string, name: string, urls: string) => {
    one<HTMLButtonElement>(".sources-add")!.click(); await until(() => one(".sources-form"), "Source form did not open");
    set(".sources-kind", kind, "change"); await nextTick(); set(".sources-name", name); set(".sources-urls", urls); await nextTick();
    one<HTMLButtonElement>(".sources-save")!.click();
  };
  const settled = (name: string) => until(() => row(name) && !one(".sources-form") && quiet(), `Source ${name} did not settle`, 90000);
  const remove = async (name: string) => {
    row(name).querySelector<HTMLButtonElement>(".sources-delete")!.click();
    await until(() => one(".sources-confirm-delete"), "Delete confirmation missing");
    one<HTMLButtonElement>(".sources-confirm-delete")!.click();
    await until(() => !row(name) && quiet(), `Source ${name} was not deleted`);
  };
  try {
    await until(() => one(".native-sources") && appState.sources.data, "Source manager did not open");
    const page = () => one(".native-sources")!.textContent!;
    evidence.first_run = { presets: all(".sources-preset").map(item => item.dataset.preset), configs: appState.settings.source_configs.length,
      cloudstream_section: Boolean(one(".sources-cloudstream, .sources-prepare, .sources-interaction-open")), android_text: /Android|AVD|ADB|APK|准备引擎/.test(page()),
      legacy: Boolean(one(".sources-legacy")), preference_kept: appState.settings.last_playback_source_key === ready.stale_preference };

    const saves = count("save_settings"), invalid: Record<string, string> = {};
    for (const [name, kind, url] of [["credentials", "maccms", "https://user:hidden@example.test/api"], ["scheme", "maccms", "ftp://example.test/api"], ["not-manifest", "stremio", "https://example.test/catalog.json"]] as const) {
      await add(kind, "Bad", url);
      await until(() => one(".sources-errors"), `Invalid ${name} was not rejected`);
      invalid[name] = one(".sources-errors")!.textContent!;
      one<HTMLButtonElement>(".sources-form-cancel")!.click(); await until(() => !one(".sources-form"), "Form did not cancel");
    }
    evidence.invalid = { saves: count("save_settings") - saves, messages: invalid };

    // Public measurement: every preset through the production button, each result recorded as the source's own state.
    evidence.public = [];
    for (const name of evidence.first_run.presets as string[]) {
      preset(name).click();
      await settled(name);
      const source = providers().find(item => item.name === name);
      evidence.public.push({ name, kind: source?.kind, registration: source?.runtime_status ?? "not synced", capabilities: source?.capabilities ?? null,
        last: row(name).querySelector(".sources-last")!.textContent, duplicate_blocked: preset(name).disabled });
      one(".sources-failure") && (evidence.public.at(-1).failure = one(".sources-failure")!.textContent);
    }
    const publicConfigs = appState.settings.source_configs.length;
    for (const name of evidence.first_run.presets as string[]) await remove(name);
    evidence.public_cleanup = { before: publicConfigs, after: appState.settings.source_configs.length };

    // Controlled flow from here on.
    await add("stremio", "我的元数据", ready.manifest); await settled("我的元数据");
    await until(() => appState.selected_metadata_source, "Metadata source was not selected");
    await add("maccms", "第一播放源", ready.maccms); await settled("第一播放源");
    await until(() => appState.selected_playback_source, "Playback source was not selected");
    const metadataId = appState.selected_metadata_source!.source_id;
    evidence.legacy_after_fallback = { note: Boolean(one(".sources-legacy")), key: appState.settings.last_playback_source_key };
    await add("maccms", "第二播放源", ready.second); await settled("第二播放源");
    const second = providers().find(item => item.name === "第二播放源")!.id, first = providers().find(item => item.name === "第一播放源")!.id;
    set(".sources-playback", first, "change");
    await until(() => appState.settings.last_playback_source_key === first, "Playback selection did not persist");
    await nextTick();
    evidence.selection = { metadata_kept: appState.selected_metadata_source?.source_id === metadataId, legacy_cleared: !one(".sources-legacy") };

    const id = appState.settings.source_configs.find(config => config.name === "第二播放源")!.config_id;
    row("第二播放源").querySelector<HTMLButtonElement>(".sources-edit")!.click();
    await until(() => one(".sources-form"), "Edit form did not open");
    set(".sources-name", "改名播放源"); await nextTick();
    one<HTMLButtonElement>(".sources-save")!.click(); await settled("改名播放源");
    evidence.edit = { same_id: appState.settings.source_configs.some(config => config.config_id === id && config.name === "改名播放源"), same_source: providers().some(item => item.id === second && item.name === "改名播放源") };
    row("改名播放源").querySelector<HTMLButtonElement>(".sources-toggle")!.click();
    await until(() => !providers().some(item => item.id === second && item.runtime_status === "ready"), "Disable did not withdraw the source");
    await settled("改名播放源");
    evidence.disabled = { option: [...one<HTMLSelectElement>(".sources-playback")!.options].some(option => option.value === second), selected_kept: appState.selected_playback_source?.source_id === first };
    row("改名播放源").querySelector<HTMLButtonElement>(".sources-toggle")!.click();
    await until(() => providers().some(item => item.id === second && item.runtime_status === "ready"), "Enable did not restore the source");
    await settled("改名播放源");
    await remove("改名播放源");

    await add("maccms", "失败来源", ready.failing);
    await until(() => row("失败来源") && one(".sources-failure"), "Failing source error missing");
    await settled("失败来源");
    const failedSource = providers().find(item => item.name === "失败来源");
    evidence.failing = { message: one(".sources-failure")!.textContent, others_ready: providers().some(item => item.name === "第一播放源" && item.runtime_status === "ready"), selection_kept: appState.selected_playback_source?.source_id === first, registration: failedSource?.runtime_status };
    await remove("失败来源");
    await add("maccms", "拒绝来源", ready.refused);
    await until(() => row("拒绝来源") && one(".sources-failure")?.textContent?.includes("拒绝访问"), "Refused source error missing");
    await settled("拒绝来源");
    evidence.refused = { message: one(".sources-failure")!.textContent, others_ready: providers().some(item => item.name === "第一播放源" && item.runtime_status === "ready") };
    const report = one(".sources-report")!.textContent!;
    evidence.diagnostics = { failure_listed: /failure sync_catalog UPSTREAM_FAILED correlation=[0-9a-f-]{36}/.test(report), secret: report.includes("N02SECRET") || page().includes("N02SECRET"), old_engine: /cloudstream phase|engine:|package /.test(report) };
    await remove("拒绝来源");

    // Limit: the Host keeps 32 configurations and the preset button says so instead of saving.
    const seeded = await appController.updateSettings(settings => ({ ...settings, source_configs: [...settings.source_configs,
      ...Array.from({ length: 32 - settings.source_configs.length }, (_, index) => ({ kind: "maccms" as const, config_id: crypto.randomUUID(), name: `占位${index}`, enabled: false, api_url: `https://example.test/${index}` }))] }));
    await nextTick();
    const limitSaves = count("save_settings");
    preset(evidence.first_run.presets[0]).click();
    await until(() => one(".sources-failure")?.textContent?.includes("32"), "Limit message missing");
    evidence.limit = { configs: seeded.source_configs.length, saves: count("save_settings") - limitSaves, message: one(".sources-failure")!.textContent };
    await appController.updateSettings(settings => ({ ...settings, source_configs: settings.source_configs.filter(config => !config.name.startsWith("占位")) }));
    await appController.loadSources();
    await nextTick();

    // Restart: a new Host reads the saved settings, and the user's sources and choices come back.
    const before = { configs: appState.settings.source_configs.map(config => config.config_id), playback: appState.settings.last_playback_source_key, metadata: appState.settings.last_metadata_config_id };
    const epoch = appState.runtime.engine_epoch;
    await rpc({ operation: "restart" });
    await appController.initialize();
    await until(() => appState.runtime.engine_epoch && appState.runtime.engine_epoch !== epoch, "Restart did not reach the page");
    for (const config of appState.settings.source_configs) await rpc({ command: "sync_catalog", args: { request_id: crypto.randomUUID(), input: { kind: config.kind, config_id: config.config_id } } });
    await appController.loadSources();
    await until(() => appState.selected_metadata_source && appState.selected_playback_source, "Restart did not restore the selection");
    evidence.restart = { new_epoch: appState.runtime.engine_epoch !== epoch, configs_kept: JSON.stringify(appState.settings.source_configs.map(config => config.config_id)) === JSON.stringify(before.configs),
      playback_kept: appState.settings.last_playback_source_key === before.playback && appState.selected_playback_source?.source_id === before.playback, metadata_kept: appState.settings.last_metadata_config_id === before.metadata };

    // Recommendation -> details -> chosen video -> manual confirmation -> resolve -> playback -> next episode.
    location.hash = "#/home";
    await until(() => one(".home-card"), "Recommendation did not render");
    one<HTMLElement>(".home-card")!.click();
    await until(() => appState.details.phase === "ready" && one(".details-episode"), "Details did not open");
    const video = () => one<HTMLVideoElement>("video")!, session = () => appState.playback.session?.session_id;
    const confirmEpisode = async (index: number) => {
      await until(() => one(".details-candidate"), "Manual confirmation did not appear");
      const opened = count("open_playback");
      one<HTMLButtonElement>(".details-candidate")!.click();
      await until(() => all(".details-candidate-episode").length === 3, "Candidate episodes did not render");
      const shown = all(".details-candidate-episode").map(button => button.textContent?.trim());
      all<HTMLButtonElement>(".details-candidate-episode")[index]!.click();
      return { opened, shown };
    };
    const chosen = all<HTMLElement>(".details-episode")[0]!, videoId = chosen.dataset.videoId!, opensBeforeConfirm = count("open_playback");
    chosen.click();
    const confirmed = await confirmEpisode(0);
    await until(() => appState.route.name === "playback" && session() && video()?.readyState >= 2, "Confirmed episode did not play");
    await video().play();
    const start = video().currentTime;
    await until(() => video().currentTime > start + .3, "Playback did not advance");
    evidence.flow = { video_id: videoId, context_video: appState.playback.context!.metadata_video_id, opens_before_confirm: confirmed.opened - opensBeforeConfirm, candidates: confirmed.shown, source: appState.playback.context!.episode_ref.source_id === first, advanced: true, gateway: video().currentSrc === appState.playback.session?.media_url };

    const firstSession = session();
    one<HTMLButtonElement>('[data-action="next"]')!.click();
    const nextConfirmed = await confirmEpisode(1);
    await until(() => session() && session() !== firstSession && video()?.readyState >= 2, "Next episode did not play");
    evidence.switched = { asked_again: nextConfirmed.shown.length === 3, video_id: appState.playback.context!.metadata_video_id, different: appState.playback.context!.metadata_video_id !== videoId };

    // Unattended next stops at the same confirmation with nothing playing.
    await appController.updateSettings(settings => ({ ...settings, auto_next: true }));
    const opensBefore = count("open_playback");
    await video().play();
    video().currentTime = Math.max(0, video().duration - .2);
    await until(() => one('[role="dialog"]'), "Unattended next did not stop for confirmation", 30000);
    await nextTick();
    evidence.ambiguous = { dialog: true, opens: count("open_playback") - opensBefore, session: session() ?? null };
    one<HTMLButtonElement>(".details-confirm-cancel")!.click();
    await until(() => !one('[role="dialog"]'), "Cancel did not close confirmation");
    evidence.cleanup = { session_after_cancel: session() ?? null };
    evidence.errors = errors;
    return evidence;
  } finally {
    app.unmount();
    window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
    window.__TAURI__!.core = original;
  }
}
