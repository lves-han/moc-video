// Test-mode only. Production App and HistoryView against a real Host and real media; the database file is real.
import { createApp, h, nextTick } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState } from "../../src/state.ts";
import type { Player } from "../../src/player.ts";
import { historyRepository } from "../../src/data/history";
import { locale } from "../../src/i18n";

type Ready = { manifest: string; maccms: string; second: string };
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
let describe = () => "";
async function until(condition: () => unknown, message: string, ms = 20000) {
  const end = performance.now() + ms;
  while (!condition()) { check(performance.now() < end, `Timed out: ${message}; state=${describe()}`); await delay(20); }
}
const one = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);
const all = <T extends HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];

export async function historyScenario(ready: Ready, prototype: { player: Player; base: string; cap: string }) {
  prototype.player.stop();
  locale.value = "en";
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
    if (["app_close_started", "app_close_aborted", "app_close_ready", "window_bounds_now"].includes(name)) return original.invoke<T>(name, args);
    if (name === "save_settings" && slowSaves > 0) { slowSaves--; await delay(3500); }
    return await rpc({ command: name, args });
  } };
  let keepOpen = false, slowSaves = 0, seekedAt: number | null = null;
  const onSeeked = (event: Event) => { seekedAt = (event.target as HTMLVideoElement).currentTime; };
  document.addEventListener("seeked", onSeeked, true);
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  const count = (command: string) => calls.filter(call => call.command === command).length;
  const evidence: Record<string, any> = {};
  const list = async () => (await rpc({ command: "list_history", args: { request_id: crypto.randomUUID(), input: {} } })).data as { entries: any[]; write_sequence: number };
  describe = () => JSON.stringify({ done: Object.keys(evidence), hash: location.hash, route: appState.route.name, playback: appState.playback.phase, position: appState.playback.position_ms, details: appState.details.phase, dialog: Boolean(one('[role="dialog"]')), cards: all(".history-card").length, error: appState.playback.error?.message, errors });
  location.hash = "#/home";
  await appController.initialize();
  await appController.saveSourceConfig({ config_id: null, kind: "stremio", name: "Metadata", urls: ready.manifest, enabled: true });
  await appController.saveSourceConfig({ config_id: null, kind: "maccms", name: "Playback", urls: ready.maccms, enabled: true });
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) });
  app.mount(container);
  const video = () => one<HTMLVideoElement>("video"), session = () => appState.playback.session?.session_id;
  const confirmFirst = async () => {
    await until(() => one(".details-candidate"), "Confirmation did not appear");
    one<HTMLButtonElement>(".details-candidate")!.click();
    await until(() => all(".details-candidate-episode").length === 3, "Candidate episodes did not render");
    all<HTMLButtonElement>(".details-candidate-episode")[0]!.click();
    await until(() => session() && (video()?.readyState ?? 0) >= 2, "Episode did not play");
  };
  const playFromHome = async () => {
    location.hash = "#/home";
    await until(() => one(".home-card-title"), "Recommendation did not render");
    one<HTMLElement>(".home-card-title")!.click();
    await until(() => appState.details.phase === "ready" && one(".details-episode"), "Details did not open");
    all<HTMLElement>(".details-episode")[0]!.click();
    await confirmFirst();
  };
  const pauseAndSave = async () => {
    await video()!.play().catch(() => {});
    await until(() => video()!.currentTime > .5, "Playback did not advance");
    const saves = count("save_history");
    video()!.pause();
    await until(() => count("save_history") > saves, "Pause did not save a snapshot");
  };
  try {
    await playFromHome();
    evidence.empty_before = (await list()).entries.length;
    await pauseAndSave();
    const first = await (async () => { for (let i = 0; i < 100; i++) { const value = await list(); if (value.entries.length) return value; await delay(50); } throw new Error("No row was stored"); })();
    const stored = first.entries[0], context = appState.playback.context!;
    evidence.saved = { rows: first.entries.length, write_sequence: first.write_sequence, video: stored.video_id === context.metadata_video_id, source: stored.source_id === context.episode_ref.source_id, identity: JSON.stringify(stored.metadata_identity) === JSON.stringify(context.metadata_ref.identity), position_ms: stored.position_ms, content: stored.content_id === appState.details.data?.details.id,
      runtime_fields: /https?:|capability|session_id|match_id/.test(JSON.stringify(stored).replace(/"title":"[^"]*"/, "")) };

    // A failed write is reported, keeps the row, and the retry button repeats it.
    await rpc({ operation: "lock_history" });
    const beforeFail = count("save_history");
    await video()!.play().catch(() => {});
    await delay(300);
    video()!.pause();
    await until(() => count("save_history") > beforeFail, "The failing save was not attempted");
    location.hash = "#/history";
    await until(() => all(".history-card").length === 1, "History view did not list the row");
    await until(() => one(".history-alert"), "Unsaved history notice missing");
    const notice = one(".history-alert")!.textContent!, rowText = one(".history-card")!.textContent!;
    await rpc({ operation: "unlock_history" });
    one<HTMLButtonElement>(".history-alert button")!.click();
    await until(() => !one(".history-alert"), "Retry did not clear the notice");
    evidence.failed_write = { notice: /could not be saved/.test(notice), row_kept: rowText.includes("Same title"), same_sequence: (() => { const sequences = calls.filter(call => call.command === "save_history").map(call => call.input.write_sequence); return sequences.slice(0, -1).includes(sequences.at(-1)); })(), preview_note: Boolean(one(".history-preview-note")) };

    // Removing asks first; afterwards the row stays gone even after a later list.
    await until(() => one(".history-actions") && !one(".history-loading"), "History did not reload after the retry");
    const removeButton = one(".history-actions")!.querySelectorAll<HTMLButtonElement>("button");
    removeButton[removeButton.length - 1]!.click();
    await until(() => one(".history-confirmation"), "Remove confirmation missing");
    const kept = (await list()).entries.length;
    const confirm = one(".history-confirmation")!.querySelectorAll<HTMLButtonElement>("button");
    confirm[confirm.length - 1]!.click();
    await until(() => !all(".history-card").length, "Row was not removed from the view");
    await delay(300);
    evidence.removed = { kept_until_confirmed: kept, rows_after: (await list()).entries.length };

    // Stored progress survives a restart and resume goes back through the match.
    await playFromHome();
    await pauseAndSave();
    let saved = (await list()).entries[0];
    for (let i = 0; i < 100 && !saved; i++) { await delay(50); saved = (await list()).entries[0]; }
    check(saved, "Second row was not stored");
    // Only the currently bound video is completed by a real EOS.
    await appController.updateSettings(settings => ({ ...settings, auto_next: false }));
    const eosVideo = appState.playback.context!.metadata_video_id;
    await video()!.play(); video()!.currentTime = Math.max(0, video()!.duration - .15);
    await until(() => appState.playback.phase === "ended", "Current video EOS did not arrive");
    await until(() => count("save_history") > 0, "EOS did not save");
    await delay(100); await historyRepository.drain();
    const eosRow = (await list()).entries[0];
    check(eosRow.video_id === eosVideo && eosRow.completed, "EOS marked the wrong video");
    evidence.eos = { current_video: true, completed: true };

    // A second season reuses episode 1 but must retain its different metadata video.
    const metadataRef = appState.details.data!.details.ref;
    location.hash = `#/details/${encodeURIComponent(JSON.stringify(metadataRef))}`;
    await until(() => appState.route.name === "details" && one(".details-group"), "Season controls missing");
    const secondSeason = appState.details.data!.details.groups.find(group => group.season_number === 2)!;
    check(secondSeason, "Cross-season fixture missing");
    all<HTMLButtonElement>(".details-group").find(button => button.dataset.groupId === secondSeason.id)!.click(); await nextTick();
    one<HTMLButtonElement>(".details-episode")!.click(); await confirmFirst(); await pauseAndSave();
    await historyRepository.drain(); saved = (await list()).entries[0];
    check(saved.season_number === 2 && saved.episode_number === 1 && saved.video_id !== eosVideo && !saved.completed, "Cross-season history lost its video binding");
    evidence.cross_season = { season: saved.season_number, episode: saved.episode_number, distinct_video: saved.video_id !== eosVideo };
    const epoch = appState.runtime.engine_epoch;
    appController.invalidatePlayback();
    await until(() => !session(), "Playback did not stop before the restart");
    await delay(500);
    await rpc({ operation: "restart" });
    await appController.initialize();
    for (const config of appState.settings.source_configs) await rpc({ command: "sync_catalog", args: { request_id: crypto.randomUUID(), input: { kind: config.kind, config_id: config.config_id } } });
    await appController.loadSources();
    await until(() => appState.selected_playback_source, "Sources did not return after restart");
    location.hash = "#/history";
    await until(() => all(".history-card").length === 1, "Row did not survive the restart");
    evidence.restart = { new_epoch: appState.runtime.engine_epoch !== epoch, rows: all(".history-card").length, write_sequence: (await list()).write_sequence };
    await until(() => one(".history-actions") && !one(".history-loading"), "History did not finish loading");
    seekedAt = null;
    one(".history-actions")!.querySelector<HTMLButtonElement>("button")!.click();
    await confirmFirst();
    // Only a real seek counts; playing up from zero would pass through the same time.
    await until(() => seekedAt !== null, "Resume did not seek to the saved position");
    evidence.resumed = { saved_ms: saved.position_ms, at: seekedAt, video: appState.playback.context!.metadata_video_id === saved.video_id };

    // Changing the source closes the prior Session and rematches before any open.
    const originalRow = (await list()).entries[0];
    await appController.saveSourceConfig({ config_id: null, kind: "maccms", name: "Second playback", urls: ready.second, enabled: true });
    const secondSource = appState.sources.data!.providers.find(source => source.name === "Second playback")!;
    const openedBeforeSwitch = count("open_playback");
    await appController.selectSource(secondSource.id, "playback");
    await until(() => !session(), "Old source Session remained active");
    location.hash = `#/details/${encodeURIComponent(JSON.stringify(metadataRef))}`;
    await until(() => appState.route.name === "details" && appState.details.phase === "ready", "Metadata missing after source switch");
    const rematched = await appController.match(saved.video_id); await nextTick();
    check(rematched?.status === "needs_confirmation" && rematched.candidates.every(candidate => candidate.content_ref.source_id === secondSource.id), "Source switch reused an old confirmation");
    check(count("open_playback") === openedBeforeSwitch, "Source switch opened before confirmation");
    appController.invalidatePlayback(); await delay(100); await historyRepository.drain();
    const retained = (await list()).entries[0];
    check(retained.video_id === originalRow.video_id && retained.source_id === originalRow.source_id && retained.position_ms > 0, "Source switch lost previous history");
    evidence.source_switch = { rematched: true, confirmed_before_open: true, history_kept: true };
    appController.invalidatePlayback();
    await nextTick();
    location.hash = "#/history";
    await until(() => one(".history-card"), "History did not reopen");
    await until(() => one<HTMLButtonElement>(".page-heading button") && !one<HTMLButtonElement>(".page-heading button")!.disabled && !one(".history-loading"), "Clear button missing");
    one<HTMLButtonElement>(".page-heading button")!.click();
    await until(() => one(".history-confirmation"), "Clear confirmation missing");
    const clearConfirm = one(".history-confirmation")!.querySelectorAll<HTMLButtonElement>("button");
    clearConfirm[clearConfirm.length - 1]!.click();
    await until(() => !all(".history-card").length, "Clear did not empty the view");
    await delay(200);
    evidence.cleared = { rows: (await list()).entries.length };

    window.dispatchEvent(new Event("resize"));
    await until(() => appState.settings.window_bounds, "Window bounds were not saved");
    const native = await original.invoke<{ physical_width: number; physical_height: number; scale_factor: number }>("t01_control", { action: "identity" });
    evidence.bounds = { saved: appState.settings.window_bounds, width: Math.round(native.physical_width / native.scale_factor), height: Math.round(native.physical_height / native.scale_factor), webview_height: window.innerHeight };

    // Left playing with no stored row and a settings change on its way: the real window close must keep both.
    await appController.updateSettings(settings => ({ ...settings, auto_next: false }));
    await playFromHome();
    await video()!.play().catch(() => {});
    await until(() => appState.playback.position_ms > 0, "Playback did not advance before closing");
    await delay(300);
    await historyRepository.clear();
    evidence.closing = { rows_before: (await list()).entries.length, video: appState.playback.context!.metadata_video_id };
    // Longer in total than the unanswered-page timer: two slow saves queued, then audio still on its short timer.
    slowSaves = 2;
    void appController.updateSettings(settings => ({ ...settings, volume: .42 }));
    void appController.updateSettings(settings => ({ ...settings, auto_next: true }));
    appState.playback.volume = .37;
    // A second close request lands while the slow saves are still running; it must wait with the first.
    setTimeout(() => { void original.invoke("t01_control", { action: "quit" }); }, 1500);
    evidence.errors = errors;
    keepOpen = true;
    return evidence;
  } finally {
    document.removeEventListener("seeked", onSeeked, true);
    window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
    if (!keepOpen) { app.unmount(); window.__TAURI__!.core = original; }
  }
}
