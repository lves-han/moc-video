// Test-mode only. Runs the production App/PlaybackView/state/Api/player against a real Host and real media.
import { createApp, h, nextTick, ref } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState, navigatePlayback } from "../../src/state.ts";
import { nativeInvoke, type Player } from "../../src/player.ts";
import { locale } from "../../src/i18n";
import { historyRepository } from "../../src/data/history.ts";
import type { AppSourceList, DetailsViewData, PlaybackContext, PlaybackMatchInput } from "../../src/models.ts";

type Bootstrap = { initial_context: PlaybackContext; match_inputs: PlaybackMatchInput[]; details: DetailsViewData; sources: AppSourceList };
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
let describe = () => "";
async function until(condition: () => unknown, message: string, ms = 15000) {
  const end = performance.now() + ms;
  while (!condition()) { check(performance.now() < end, `Timed out: ${message}; state=${describe()}`); await delay(20); }
}
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const one = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);
const all = <T extends HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];
const control = (action: string) => one<HTMLButtonElement>(`[data-action="${action}"]`)!;
const labelled = <T extends HTMLElement>(label: string) => one<T>(`[aria-label="${label}"]`)!;
const select = (element: HTMLSelectElement, value: string) => { element.value = value; element.dispatchEvent(new Event("change", { bubbles: true })); };
const slide = (element: HTMLInputElement, value: number) => { element.value = String(value); element.dispatchEvent(new Event("input", { bubbles: true })); };
const key = (name: string, code = name) => one(".playback-view")!.dispatchEvent(new KeyboardEvent("keydown", { key: name, code, bubbles: true, cancelable: true }));
const identity = () => nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });

export async function playbackUiScenario(payload: Bootstrap, prototype: { player: Player; base: string; cap: string }) {
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
  // MacCMS has no verified identity, so an unattended match needs the Host-confirmed selection supplied here.
  let confirmed = true;
  const inputs = payload.match_inputs.slice(1, 5), videos = inputs.map(input => input.video_id);
  window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
    if (name === "t01_control" || name === "player_fullscreen") return original.invoke<T>(name, args);
    calls.push({ command: name, input: args?.input });
    const input = name === "match_playback" && confirmed && !args?.input?.selection ? { ...args, input: inputs.find(row => row.video_id === args?.input?.video_id) ?? args?.input } : args;
    return rpc({ command: name, args: input });
  } };
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  const saves: { episode: number; position_ms: number; completed: boolean }[] = [], save = historyRepository.save;
  historyRepository.save = async entry => { saves.push({ episode: entry.episode, position_ms: entry.position_ms, completed: entry.completed }); return save(entry); };
  const count = (command: string) => calls.filter(call => call.command === command).length;
  // A first runtime report clears every page and source, so it precedes the page state below.
  await appController.restoreSettings(); await appController.refreshRuntime();
  const first = inputs[0]!, metadata = clone(first.metadata_ref), page = clone(payload.details);
  appState.sources.data = payload.sources;
  appState.selected_metadata_source = { source_id: metadata.content_ref.source_id, source_revision: metadata.content_ref.source_revision };
  appState.selected_playback_source = clone(first.playback_source);
  page.metadata_ref = metadata; page.details.ref = metadata.content_ref;
  page.details.groups = [{ ...page.details.groups[0]!, title: "Season 1", episodes: videos.map((video, index) => ({ ...page.details.groups[0]!.episodes[0]!, id: `episode-${index}`, label: `Episode ${index + 1}`, index,
    ref: { ...metadata.content_ref, data: JSON.stringify(["stremio_video", "addon", "series", "meta", video]) } })) }];
  appState.details.data = page; appState.details.phase = "ready";
  await appController.updateSettings(settings => ({ ...settings, auto_next: false, volume: .5, muted: false }));
  const context = (await rpc({ command: "match_playback", args: { request_id: crypto.randomUUID(), input: first } })).data.context;
  appState.playback.context = context;
  navigatePlayback(context);
  const component = ref<InstanceType<typeof App>>();
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App, { ref: component }) }) });
  app.mount(container); await nextTick();
  const video = () => one<HTMLVideoElement>("video")!;
  const evidence: Record<string, any> = {};
  describe = () => JSON.stringify({ phase: appState.playback.phase, route: appState.route.name, video: appState.playback.context?.metadata_video_id, position: video()?.currentTime, paused: video()?.paused, ready: video()?.readyState, session: Boolean(appState.playback.session), error: appState.playback.error, diagnostic: document.querySelector('[role="alert"]')?.textContent, pending: appController.matching.pending, match: appController.matching.result?.status, calls: calls.slice(-5).map(call => call.command), errors });
  const session = () => appState.playback.session?.session_id;
  const startPlaying = async () => {
    await until(() => session() && video().readyState >= 2, "Media did not become ready");
    if (video().paused) control("play").click();
    await until(() => !video().paused && video().currentTime > .05 && appState.playback.phase === "playing", "Playback did not start");
  };
  const pause = async () => { control("play").click(); await until(() => video().paused && appState.playback.phase === "paused", "Pause did not reach the pipeline"); };
  try {
    // The owned media lasts 2s; only the EOS steps below let it really end.
    video().loop = true;
    await startPlaying();
    evidence.title = one(".playback-now")!.textContent;
    await pause(); await nextTick();
    const pausedLabel = control("play").textContent;
    control("play").click(); await until(() => !video().paused && appState.playback.phase === "playing", "Play did not resume");
    await nextTick();
    const playingLabel = control("play").textContent;
    await pause();
    evidence.play_pause = { paused_label: pausedLabel, playing_label: playingLabel };

    slide(labelled<HTMLInputElement>("Playback position"), 1.2);
    await until(() => Math.abs(video().currentTime - 1.2) < .05 && !video().seeking, "Seek did not reach the media");
    evidence.seek = { media: video().currentTime, state_ms: appState.playback.position_ms };

    slide(labelled<HTMLInputElement>("Volume"), .37); await nextTick();
    evidence.volume = { media: video().volume, label: one(".playback-controls")!.textContent!.includes("37%") };
    await until(() => appState.settings.volume === .37, "Volume did not persist through Settings");
    evidence.volume.persisted = appState.settings.volume;

    const rates: Record<string, number> = {}, speed = [...one(".playback-controls")!.querySelectorAll("label")].find(label => label.textContent!.startsWith("Speed"))!.querySelector("select")!;
    evidence.speed_options = [...speed.options].map(option => Number(option.value));
    for (const rate of evidence.speed_options) { select(speed, String(rate)); await nextTick(); rates[rate] = video().playbackRate; }
    select(speed, "1"); evidence.speed = rates;

    const keys: Record<string, unknown> = {};
    const at = video().currentTime; slide(labelled<HTMLInputElement>("Playback position"), .2); await until(() => Math.abs(video().currentTime - .2) < .05 && !video().seeking, "Seek back failed");
    key(" ", "Space"); await until(() => !video().paused && appState.playback.phase === "playing", "Space did not play"); keys.space_plays = true;
    key(" ", "Space"); await until(() => video().paused && appState.playback.phase === "paused", "Space did not pause"); keys.space_pauses = true;
    // Five seconds past this 2s clip clamps at its end, which looping would wrap around.
    video().loop = false;
    const before = video().currentTime; key("ArrowRight"); await until(() => !video().seeking && video().currentTime !== before, "ArrowRight did not seek"); keys.right_delta = video().currentTime - before;
    await until(() => Math.abs(appState.playback.position_ms / 1000 - video().currentTime) < .3, "Position state did not follow ArrowRight");
    const end = video().currentTime; key("ArrowLeft"); await until(() => !video().seeking && video().currentTime < end, "ArrowLeft did not seek"); keys.left_delta = video().currentTime - end;
    video().loop = true;
    const volume = video().volume; key("ArrowUp"); await nextTick(); keys.up_delta = video().volume - volume; key("ArrowDown"); await nextTick(); keys.down_delta = video().volume - volume;
    key("m"); await nextTick(); keys.muted = video().muted; key("m"); await nextTick(); keys.unmuted = !video().muted;
    labelled<HTMLInputElement>("Volume").focus(); const focused = labelled<HTMLInputElement>("Volume"), volumeBefore = video().volume;
    focused.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true, cancelable: true })); await nextTick(); keys.input_keeps_keys = video().volume === volumeBefore;
    keys.at = at; evidence.keys = keys;

    const fullscreen = { on: false, off: false, esc_off: false };
    control("fullscreen").click(); await delay(1200); fullscreen.on = (await identity()).fullscreen;
    control("fullscreen").click(); await delay(1200); fullscreen.off = !(await identity()).fullscreen;
    control("fullscreen").click(); await delay(1200);
    one<HTMLElement>(".playback-view")!.focus(); key("Escape"); await delay(1200); fullscreen.esc_off = !(await identity()).fullscreen;
    evidence.fullscreen = fullscreen;

    const subtitles = [...one(".playback-controls")!.querySelectorAll("label")].find(label => label.textContent!.startsWith("Subtitles"))!.querySelector("select")!;
    const subtitle = [...subtitles.options].find(option => option.value)!;
    select(subtitles, subtitle.value); await until(() => video().querySelector("track")?.track.mode === "showing", "Subtitle did not show");
    const shown = video().querySelector("track")!.track.mode;
    select(subtitles, ""); await until(() => [...video().textTracks].every(track => track.mode !== "showing"), "Subtitle did not turn off");
    evidence.subtitles = { shown, off: true };

    const streams = [...one(".playback-controls")!.querySelectorAll("label")].find(label => label.textContent!.startsWith("Stream"))!.querySelector("select")!;
    const oldSession = session(), resolution = appState.playback.resolution!.id, otherStream = [...streams.options].find(option => option.value !== streams.value)!.value;
    select(streams, otherStream); await until(() => session() && session() !== oldSession && video().readyState >= 2, "Stream switch did not open");
    evidence.stream = { session_changed: true, same_resolution: appState.playback.resolution!.id === resolution, selected: appState.playback.session!.stream_id === otherStream };

    await startPlaying();
    (document.activeElement as HTMLElement | null)?.blur();
    one(".playback-view")!.dispatchEvent(new PointerEvent("pointermove", { bubbles: true }));
    await delay(3400);
    const concealed = one(".playback-controls")!.classList.contains("concealed");
    one(".playback-view")!.dispatchEvent(new PointerEvent("pointermove", { bubbles: true })); await nextTick();
    const restored = !one(".playback-controls")!.classList.contains("concealed");
    await delay(3400);
    const concealedAgain = one(".playback-controls")!.classList.contains("concealed");
    labelled<HTMLInputElement>("Volume").focus(); await nextTick();
    evidence.hide = { concealed: concealed && concealedAgain, restored, restored_by_focus: !one(".playback-controls")!.classList.contains("concealed") };
    (document.activeElement as HTMLElement | null)?.blur();
    await pause();

    // Resume waits for loadedmetadata: nothing seeks before the new media reports its duration.
    const resumed = component.value!.playbackController()!;
    const before2 = session();
    void resumed.load(clone(appState.playback.context!), { resume: { context: clone(appState.playback.context!), position_ms: 1300, was_playing: false } });
    await until(() => session() && session() !== before2 && video().readyState >= 1 && Math.abs(video().currentTime - 1.3) < .1, "Resume did not restore its position");
    evidence.resume = { position: video().currentTime, paused: appState.playback.phase === "paused" || video().paused };

    saves.length = 0;
    await appController.updateSettings(settings => ({ ...settings, auto_next: false }));
    await nextTick();
    slide(labelled<HTMLInputElement>("Playback position"), .4); await until(() => Math.abs(video().currentTime - .4) < .1, "Seek before history failed");
    await delay(100);
    evidence.seek_saved = saves.some(row => row.episode === 1 && Math.abs(row.position_ms - 400) < 150);

    control("episodes").click(); await nextTick();
    const panelOpen = Boolean(one(".playback-episodes")), current = one(".playback-episode[aria-current]")!.dataset.videoId;
    key("Escape"); await nextTick();
    const panelClosed = !one(".playback-episodes");
    control("episodes").click(); await nextTick();
    slide(labelled<HTMLInputElement>("Playback position"), 1.5); await until(() => Math.abs(video().currentTime - 1.5) < .1 && !video().seeking, "Seek before switch failed");
    const opensBefore = count("open_playback"), positionBefore = appState.playback.position_ms;
    one<HTMLButtonElement>(`.playback-episode[data-video-id="${videos[2]}"]`)!.click();
    await until(() => appState.playback.context?.metadata_video_id === videos[2] && session() && video().readyState >= 2, "Episode selection did not open");
    evidence.select_episode = { panelOpen, panelClosed, current, video: appState.playback.context!.metadata_video_id, expected: videos[2], opens: count("open_playback") - opensBefore, title: one(".playback-now")!.textContent, old_saved: saves.some(row => row.episode === 1 && Math.abs(row.position_ms - positionBefore) < 600), old_position: positionBefore, new_rows_at_open: saves.filter(row => row.episode === 3).map(row => row.position_ms) };
    control("episodes").click();

    // EOS with auto_next off keeps this episode.
    const toEnd = async () => { video().loop = false; if (video().paused) control("play").click(); await until(() => !video().paused, "Play before EOS failed"); slide(labelled<HTMLInputElement>("Playback position"), Math.max(0, video().duration - .25)); };
    const matchesBefore = count("match_playback");
    await toEnd(); await until(() => appState.playback.phase === "ended", "EOS did not arrive"); await delay(500);
    evidence.auto_off = { video: appState.playback.context!.metadata_video_id, expected: videos[2], matches: count("match_playback") - matchesBefore, completed: saves.some(row => row.episode === 3 && row.completed) };

    // A manual stop never advances, even with auto_next on.
    one<HTMLInputElement>('[data-action="auto-next"]')!.click();
    await until(() => appState.settings.auto_next === true, "Auto-next toggle did not persist");
    const stopMatches = count("match_playback");
    control("play").click(); await delay(200);
    control("stop").click(); await delay(500);
    evidence.stop = { matches: count("match_playback") - stopMatches, context: appState.playback.context };

    // EOS with auto_next on rematches the next metadata video and really plays it.
    video().loop = true;
    check(await appController.match(videos[2]!), "Rematch before EOS failed");
    await until(() => session() && video().readyState >= 2, "Rematched episode did not open");
    await startPlaying();
    const nextOld = session();
    await toEnd();
    await until(() => appState.playback.context?.metadata_video_id === videos[3] && session() && session() !== nextOld && video().readyState >= 2, "Auto next did not open the next video");
    video().loop = true;
    await startPlaying();
    evidence.auto_next = { video: appState.playback.context!.metadata_video_id, expected: videos[3], playing: !video().paused, session_bound: video().currentSrc === appState.playback.session!.media_url, title: one(".playback-now")!.textContent };

    // The last video has no next: control disabled and EOS stays.
    const lastMatches = count("match_playback");
    evidence.last = { next_disabled: control("next").disabled };
    await toEnd(); await until(() => appState.playback.phase === "ended", "Last EOS did not arrive"); await delay(500);
    evidence.last.matches = count("match_playback") - lastMatches; evidence.last.video = appState.playback.context!.metadata_video_id; evidence.last.expected = videos[3];

    // An ambiguous next video waits for the user, with nothing playing.
    video().loop = true;
    check(await appController.match(videos[0]!), "Rematch before ambiguity failed");
    await until(() => session() && video().readyState >= 2, "First episode did not reopen");
    await startPlaying();
    confirmed = false;
    const opensAmbiguous = count("open_playback");
    await toEnd();
    await until(() => one('[role="dialog"]'), "Ambiguous next did not ask for confirmation");
    evidence.ambiguous = { dialog: true, opens: count("open_playback") - opensAmbiguous, session: session() ?? null, video_elements: all("video").length };
    one<HTMLButtonElement>(".details-confirm-cancel")!.click();
    await until(() => !one('[role="dialog"]'), "Confirmation did not close");
    await delay(600);
    // Poster URLs carry capabilities, so only identity and progress enter the report.
    evidence.history = { saved: (await historyRepository.list()).map(({ poster: _poster, ...entry }) => entry) };
    evidence.errors = errors;
    return evidence;
  } finally {
    historyRepository.save = save;
    app.unmount();
    window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
    window.__TAURI__!.core = original;
  }
}
