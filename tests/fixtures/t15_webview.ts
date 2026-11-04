// Test-mode only. Runs the production App/DetailsView/state/Api against a real Host.
import { createApp, h, nextTick } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState } from "../../src/state.ts";
import { nativeInvoke, type Player } from "../../src/player.ts";
import { locale } from "../../src/i18n";
import type { Bridge_MediaRef } from "../../src/models.ts";

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
let describe = () => "";
async function until(condition: () => unknown, message: string, ms = 20000) {
  const end = performance.now() + ms;
  while (!condition()) { check(performance.now() < end, `Timed out: ${message}; state=${describe()}`); await delay(20); }
}
const all = <T extends HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];
const one = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);

export async function detailsScenario(prototype: { player: Player; base: string; cap: string }) {
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
  const gates = new Map<string, Promise<void>>();
  let related: { id: string; title: string; kind: string; ref: Bridge_MediaRef } | null = null;
  window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
    if (name === "t01_control" || name === "player_fullscreen") return original.invoke<T>(name, args);
    calls.push({ command: name, input: args?.input });
    const key = name === "get_details" ? JSON.stringify(args?.input?.ref) : name;
    const response = await rpc({ command: name, args });
    const gate = gates.get(key);
    if (gate) await gate;
    // Host's Stremio adapter returns no related titles; render them from an injected card.
    if (name === "get_details" && related && response.data?.metadata_ref) response.data.details.related = [related];
    return response;
  } };
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  const hold = (key: string) => { let open!: () => void; gates.set(key, new Promise(resolve => { open = () => { gates.delete(key); resolve(); }; })); return open; };
  const evidence: Record<string, any> = {};
  describe = () => JSON.stringify({ hash: location.hash, route: appState.route.name, h1: one("h1")?.textContent, phase: appState.details.phase, error: appState.details.error,
    title: appState.details.data?.details.title, pending: appController.matching.pending, match: appController.matching.result?.status, playback_error: appState.playback.error,
    source: appState.selected_playback_source?.source_id, sources: appState.sources.phase, calls: calls.slice(-6).map(call => call.command), errors });
  location.hash = "#/home";
  await appController.initialize();
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) });
  app.mount(container);
  const refOf = (title: string) => appState.home.data!.page.sections.flatMap(section => section.items).find(card => card.title === title)!.ref;
  const open = async (title: string) => {
    location.hash = `#/details/${encodeURIComponent(JSON.stringify(refOf(title)))}`;
    await until(() => one("h1")?.textContent === title && appState.details.phase === "ready", `Details ${title} did not open`);
    await nextTick();
  };
  try {
    await until(() => appState.home.data?.page.sections.length === 2, "Real catalog did not load");
    await open("Long movie");
    const collapsed = one(".details-plot")!.textContent!;
    one<HTMLButtonElement>(".details-plot-toggle")!.click(); await nextTick();
    const full = one(".details-plot")!.textContent!;
    evidence.movie = { kind: appState.details.data!.details.kind, meta: one(".details-meta")!.textContent, play: Boolean(one(".details-play")), episode_grid: Boolean(one(".details-episodes")),
      collapsed_chars: collapsed.length, full_chars: full.length, plain_text: !one(".details-plot")!.querySelector("*") && !full.includes("<") && !full.includes("secret"), poster: Boolean(one<HTMLImageElement>(".details-poster img")) };

    await open("Two hundred episodes");
    const buttons = all(".details-episode");
    evidence.series = { kind: appState.details.data!.details.kind, buttons: buttons.length, unique: new Set(buttons.map(button => button.dataset.episodeId)).size, videos: new Set(buttons.map(button => button.dataset.videoId)).size, duplicate_titles: new Set(appState.details.data!.details.groups[0]!.episodes.map(episode => episode.title)).size === 1 };

    const layouts: Record<string, unknown>[] = [];
    for (const width of [960, 390]) {
      await nativeInvoke("t01_control", { action: "width", value: width }); await delay(250);
      layouts.push({ width: innerWidth, overflow: document.documentElement.scrollWidth > innerWidth + 1 });
    }
    evidence.layouts = layouts;
    await nativeInvoke("t01_control", { action: "width", value: 1200 }); await delay(250);

    await open("Sparse series");
    const groups = all<HTMLElement>(".details-group"), switched: Record<string, unknown>[] = [];
    for (const group of groups) {
      group.click(); await nextTick();
      const source = appState.details.data!.details.groups.find(row => row.id === group.dataset.groupId)!;
      switched.push({ pressed: group.getAttribute("aria-pressed") === "true", others: groups.filter(other => other.getAttribute("aria-pressed") === "true").length, shown: all(".details-episode").map(button => button.dataset.episodeId), expected: source.episodes.map(episode => episode.id) });
    }
    evidence.sparse = { groups: groups.length, switched, poster_placeholder: Boolean(one(".details-poster-missing")), plot: Boolean(one(".details-plot")), meta: one(".details-meta")?.textContent ?? null };

    await open("Nothing to watch");
    evidence.empty = { message: Boolean(one(".details-no-episodes")), play: Boolean(one(".details-play")), buttons: all(".details-episode").length };

    // A slower first page must not overwrite the page opened after it.
    const slow = refOf("Long movie"), release = hold(JSON.stringify(slow));
    location.hash = `#/details/${encodeURIComponent(JSON.stringify(slow))}`;
    await until(() => calls.filter(call => call.command === "get_details").at(-1)?.input.ref.data === slow.data, "Slow details request not sent");
    await delay(50);
    await open("Sparse series");
    release(); await delay(300);
    evidence.stale = { title: one("h1")!.textContent, data_title: appState.details.data!.details.title };

    related = { id: "related-card", title: "Related title", kind: "series", ref: refOf("Sparse series") };
    await open("Long movie");
    await until(() => one(".details-related-card"), "Related card did not render");
    evidence.related = { cards: all(".details-related-card").length, text: one(".details-related-card")!.textContent };
    one<HTMLAnchorElement>(".details-related-card")!.click();
    await until(() => one("h1")?.textContent === "Sparse series", "Related card did not open its details");
    evidence.related.opened = one("h1")!.textContent;
    related = null;

    await open("Two hundred episodes");
    const chosen = all(".details-episode")[6]!, video = chosen.dataset.videoId!, matchGate = hold("match_playback"), matchesBefore = calls.filter(call => call.command === "match_playback").length;
    chosen.click();
    await until(() => chosen.classList.contains("n-button--loading"), "Pending match state missing");
    const pending = { loading: true, disabled: all<HTMLButtonElement>(".details-episode").filter(button => button !== chosen).every(button => button.disabled) };
    matchGate();
    await until(() => one('[role="dialog"]'), "Confirmation did not render");
    const firstMatch = calls.filter(call => call.command === "match_playback")[matchesBefore]!;

    one<HTMLButtonElement>(".details-confirm-cancel")!.click();
    await until(() => !one('[role="dialog"]'), "Cancel did not close confirmation");
    const cancelled = { route: appState.route.name, title: one("h1")!.textContent, context: appState.playback.context };

    all(".details-episode")[6]!.click();
    await until(() => one(".details-candidate"), "Candidates did not render");
    one<HTMLButtonElement>(".details-candidate")!.click();
    await until(() => all(".details-candidate-group").length > 1, "Candidate groups did not render");
    const candidate = appState.playback.context === null ? appController.matching.details!.details : null;
    const groupElement = all<HTMLElement>(".details-candidate-group")[1]!, button = groupElement.querySelector<HTMLElement>(".details-candidate-episode:last-of-type")!;
    const group = candidate!.groups.find(row => row.id === groupElement.dataset.groupId)!, episode = group.episodes.find(row => row.id === button.dataset.episodeId)!;
    const expected = JSON.parse(JSON.stringify({ ref: episode.ref, group: group.id, id: episode.id, content: candidate!.ref }));
    button.click();
    await until(() => appState.route.name === "playback" && appState.playback.context, "Confirmed episode did not reach playback");
    const context = appState.route.name === "playback" ? appState.route.context : null, confirmedMatch = calls.filter(call => call.command === "match_playback").at(-1)!;
    evidence.flow = { video, sent_video: firstMatch.input.video_id, first_selection: firstMatch.input.selection ?? null, pending, cancelled, expected,
      selection: confirmedMatch.input.selection, confirmed_video: confirmedMatch.input.video_id, context_episode_ref: context!.episode_ref, context_group: context!.group_id, context_episode: context!.episode_id, context_video: context!.metadata_video_id, hash: location.hash };
    appController.invalidatePlayback();
    await delay(200);
    evidence.errors = errors;
    return evidence;
  } finally {
    app.unmount();
    window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
    window.__TAURI__!.core = original;
  }
}
