// Test-mode only. Runs the production App/NativeSourcesView/state/Api against a real Host.
// Every Cloudstream state and interaction below is synthetic: real ones are accepted in N02.
import { createApp, h, nextTick } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState } from "../../src/state.ts";
import type { Player } from "../../src/player.ts";
import { locale } from "../../src/i18n";

type Ready = { manifest: string; maccms: string; second: string; failing: string };
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
let describe = () => "";
async function until(condition: () => unknown, message: string, ms = 15000) {
  const end = performance.now() + ms;
  while (!condition()) { check(performance.now() < end, `Timed out: ${message}; state=${describe()}`); await delay(20); }
}
const one = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector);
const all = <T extends HTMLElement>(selector: string) => [...document.querySelectorAll<T>(selector)];
const set = (selector: string, value: string, event = "input") => { const element = one<HTMLInputElement>(selector)!; element.value = value; element.dispatchEvent(new Event(event, { bubbles: true })); };
const row = (name: string) => all(".sources-row").find(item => item.querySelector("h3")?.textContent === name)!;
const providers = () => appState.sources.data?.providers ?? [];

export async function sourcesScenario(ready: Ready, prototype: { player: Player; base: string; cap: string }) {
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
  const CS = "c0000000000000000000000000000000000000000000000000000000000000cs", SHA = "a".repeat(64);
  let synthetic: null | "ready" | "interaction" = null, interactionResult: "waiting" | "rejected" = "waiting";
  const csSource = (needed: boolean) => ({ id: CS, source_revision: "rev", name: "Synthetic provider", language: "en", content_types: ["series"], kind: "cloudstream", runtime_status: "ready",
    capabilities: { home: true, search: true, details: true, resolve: true, sequential_home: false }, origin: { plugin_internal_name: "SuperVod", plugin_sha256: SHA, registration_key: "k" },
    error: needed ? { code: "INTERACTION_REQUIRED", message: "Synthetic challenge", retryable: true, details: { source_id: CS, source_revision: "rev", operation: "home", reason: "challenge", host: "source.test" } } : null });
  const envelope = (args: Record<string, any> | undefined, data: unknown) => ({ request_id: args?.request_id, engine_epoch: appState.runtime.engine_epoch, data });
  window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
    if (name === "t01_control" || name === "player_fullscreen") return original.invoke<T>(name, args);
    calls.push({ command: name, input: args?.input });
    if (name === "start_interaction" && synthetic) return (interactionResult === "rejected"
      ? { request_id: args?.request_id, engine_epoch: appState.runtime.engine_epoch, error: { code: "ENGINE_NOT_READY", message: "Synthetic rejection", retryable: true } }
      : envelope(args, { id: "interaction-1", source_id: args?.input.source_id, status: "waiting", required_display_mode: "windowed", message: "Synthetic window open" })) as T;
    if (name === "finish_interaction" && synthetic) return envelope(args, { id: args?.input.interaction_id, source_id: CS, status: args?.input.request.action === "completed" ? "completed" : "failed", required_display_mode: "windowed", message: "done" }) as T;
    const response = await rpc({ command: name, args });
    if (synthetic && name === "get_sources") return { ...response, data: { providers: [...response.data.providers, csSource(synthetic === "interaction")], plugins: [{ internal_name: "SuperVod", version: 7, artifact_sha256: SHA, status: "loaded", provider_ids: [CS] }] } } as T;
    if (synthetic && (name === "runtime_status" || name === "runtime_prepare")) return { ...response, data: { ...response.data, cloudstream: { phase: "ready", engine_epoch: "22222222-2222-4222-8222-222222222222", display_mode: "headless", error: null,
      health: { api_version: "1", runtime_version: "synthetic-1.0", cloudstream_revision: "rev-abc", registry_sha256: SHA, status: "ready", loaded_plugin_count: 1, provider_count: 1, display_mode: "headless" } } } } as T;
    return response;
  } };
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  const count = (command: string) => calls.filter(call => call.command === command).length;
  const evidence: Record<string, any> = {};
  describe = () => JSON.stringify({ hash: location.hash, route: appState.route.name, failure: one(".sources-failure")?.textContent, errors_ui: one(".sources-errors")?.textContent, providers: providers().map(item => [item.name, item.runtime_status]), selected: [appState.selected_metadata_source?.source_id, appState.selected_playback_source?.source_id], calls: calls.slice(-5).map(call => call.command), errors });
  location.hash = "#/home";
  await appController.initialize();
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) });
  app.mount(container);
  const add = async (kind: string, name: string, urls: string) => {
    one<HTMLButtonElement>(".sources-add")!.click(); await until(() => one(".sources-form"), "Source form did not open");
    set(".sources-kind", kind, "change"); await nextTick(); set(".sources-name", name); set(".sources-urls", urls); await nextTick();
    one<HTMLButtonElement>(".sources-save")!.click();
  };
  const settled = (name: string) => until(() => row(name) && !one(".sources-form") && !all<HTMLButtonElement>(".sources-delete").some(button => button.disabled), `Source ${name} did not settle`);
  try {
    await until(() => one(".home-setup"), "First-run guidance missing on home");
    evidence.first_run = { guidance: one(".home-setup")!.textContent, link: one<HTMLAnchorElement>(".home-setup a")!.getAttribute("href") };
    one<HTMLAnchorElement>(".home-setup a")!.click();
    await until(() => one(".native-sources"), "Setup entry did not open the source manager");
    evidence.first_run.opened = { route: appState.route.name, empty: Boolean(one(".sources-empty")), guidance: all(".sources-guidance").length };

    const saves = count("save_settings"), invalid: Record<string, string> = {};
    for (const [name, kind, url] of [["credentials", "maccms", "https://user:hidden@example.test/api"], ["fragment", "maccms", "https://example.test/api#x"], ["scheme", "maccms", "ftp://example.test/api"], ["not-manifest", "stremio", "https://example.test/catalog.json"]] as const) {
      await add(kind, "Bad", url);
      await until(() => one(".sources-errors"), `Invalid ${name} was not rejected`);
      invalid[name] = one(".sources-errors")!.textContent!;
      one<HTMLButtonElement>(".sources-form-cancel")!.click(); await until(() => !one(".sources-form"), "Form did not cancel");
    }
    evidence.invalid = { saves: count("save_settings") - saves, messages: invalid };

    await add("stremio", "My metadata", ready.manifest); await settled("My metadata");
    await until(() => appState.selected_metadata_source, "Metadata source was not selected");
    await add("maccms", "My playback", ready.maccms); await settled("My playback");
    await until(() => appState.selected_playback_source, "Playback source was not selected");
    const metadataId = appState.selected_metadata_source!.source_id, firstPlayback = appState.selected_playback_source!.source_id;
    evidence.added = { kinds: appState.settings.source_configs.map(config => config.kind), state: row("My metadata").querySelector(".sources-state")!.textContent, last: row("My metadata").querySelector(".sources-last")!.textContent, shown_secret: one(".native-sources")!.textContent!.includes("T17SECRET") };

    location.hash = "#/home";
    await until(() => all(".home-card").length === 2, "Real home did not use the added source");
    evidence.home = { cards: all(".home-card").length, setup_gone: !one(".home-setup") };
    location.hash = "#/sources";
    await until(() => one(".native-sources"), "Source manager did not reopen");

    const id = appState.settings.source_configs.find(config => config.name === "My playback")!.config_id;
    row("My playback").querySelector<HTMLButtonElement>(".sources-edit")!.click();
    await until(() => one(".sources-form"), "Edit form did not open");
    evidence.edit = { kind_locked: one<HTMLSelectElement>(".sources-kind")!.disabled };
    set(".sources-name", "Renamed playback"); await nextTick();
    one<HTMLButtonElement>(".sources-save")!.click(); await settled("Renamed playback");
    Object.assign(evidence.edit, { same_id: appState.settings.source_configs.some(config => config.config_id === id && config.name === "Renamed playback"), count: appState.settings.source_configs.length, provider: providers().some(item => item.name === "Renamed playback") });

    await add("maccms", "Second playback", ready.second); await settled("Second playback");
    await until(() => providers().filter(item => item.kind === "maccms").length === 2, "Second playback source missing");
    const homeData = appState.home.data, second = providers().find(item => item.name === "Second playback")!.id;
    set(".sources-playback", second, "change");
    await until(() => appState.selected_playback_source?.source_id === second && appState.settings.last_playback_source_key === second, "Playback selection did not persist");
    evidence.independent = { metadata_kept: appState.selected_metadata_source?.source_id === metadataId, home_kept: appState.home.data === homeData, persisted: appState.settings.last_playback_source_key === second };

    row("Second playback").querySelector<HTMLButtonElement>(".sources-toggle")!.click();
    await until(() => !providers().some(item => item.id === second && item.runtime_status === "ready") && appState.selected_playback_source?.source_id !== second, "Disable did not withdraw the source");
    await settled("Second playback");
    evidence.disabled = { state: row("Second playback").querySelector(".sources-state")!.textContent, fallback: appState.selected_playback_source?.source_id === firstPlayback, option: [...one<HTMLSelectElement>(".sources-playback")!.options].some(option => option.value === second) };
    row("Second playback").querySelector<HTMLButtonElement>(".sources-toggle")!.click();
    await until(() => providers().some(item => item.name === "Second playback" && item.runtime_status === "ready"), "Enable did not restore the source");
    await settled("Second playback");
    evidence.enabled = true;

    row("Second playback").querySelector<HTMLButtonElement>(".sources-delete")!.click();
    await until(() => one(".sources-confirm-cancel"), "Delete confirmation missing");
    one<HTMLButtonElement>(".sources-confirm-cancel")!.click(); await until(() => !one(".sources-confirm-cancel"), "Delete cancel failed");
    const keptAfterCancel = Boolean(row("Second playback"));
    row("Second playback").querySelector<HTMLButtonElement>(".sources-delete")!.click();
    await until(() => one(".sources-confirm-delete"), "Delete confirmation missing");
    one<HTMLButtonElement>(".sources-confirm-delete")!.click();
    await until(() => !row("Second playback") && !providers().some(item => item.name === "Second playback"), "Delete did not remove the source");
    evidence.deleted = { kept_after_cancel: keptAfterCancel, configs: appState.settings.source_configs.length };

    await add("maccms", "Failing source", ready.failing);
    await until(() => row("Failing source") && one(".sources-failure"), "Failing source error missing");
    await settled("Failing source");
    evidence.failing = { message: one(".sources-failure")!.textContent, others_ready: providers().some(item => item.name === "My metadata" && item.runtime_status === "ready"), selection_kept: appState.selected_metadata_source?.source_id === metadataId };
    row("Failing source").querySelector<HTMLButtonElement>(".sources-delete")!.click();
    await until(() => one(".sources-confirm-delete"), "Delete confirmation missing");
    one<HTMLButtonElement>(".sources-confirm-delete")!.click();
    await until(() => !row("Failing source"), "Failing source not deleted");

    // Synthetic Cloudstream states, separate from the real host status.
    synthetic = "ready";
    const prepareCalls = count("runtime_prepare");
    one<HTMLButtonElement>(".sources-prepare")!.click();
    await until(() => count("runtime_prepare") > prepareCalls && one(".sources-plugin"), "Prepare did not show packages");
    await until(() => !one<HTMLButtonElement>(".sources-delete")?.disabled, "Prepare did not settle");
    evidence.cloudstream = { stimulus: "synthetic", target: calls.filter(call => call.command === "runtime_prepare").at(-1)!.input.target, plugin: one(".sources-plugin")!.textContent, guest: one(".sources-guest-state")!.textContent, host_phase: appState.runtime.phase, selectable: [...one<HTMLSelectElement>(".sources-playback")!.options].some(option => option.value === CS) };

    synthetic = "interaction";
    await appController.loadSources(); await nextTick();
    await until(() => one(".sources-interaction-open"), "Interaction entry missing");
    const starts = () => count("start_interaction"), startBefore = starts();
    one<HTMLButtonElement>(".sources-interaction-open")!.click();
    await until(() => one(".sources-interaction-reject"), "Interaction confirmation missing");
    one<HTMLButtonElement>(".sources-interaction-reject")!.click();
    await until(() => !one(".sources-interaction-reject"), "Interaction reject failed");
    const afterReject = starts() - startBefore;
    interactionResult = "rejected";
    one<HTMLButtonElement>(".sources-interaction-open")!.click();
    await until(() => one(".sources-interaction-confirm"), "Interaction confirmation missing");
    one<HTMLButtonElement>(".sources-interaction-confirm")!.click();
    await until(() => one(".sources-failure")?.textContent?.includes("Synthetic rejection"), "Rejected interaction error missing");
    const rejectedShown = !one(".sources-interaction");
    interactionResult = "waiting";
    one<HTMLButtonElement>(".sources-interaction-open")!.click();
    await until(() => one(".sources-interaction-confirm"), "Interaction confirmation missing");
    one<HTMLButtonElement>(".sources-interaction-confirm")!.click();
    await until(() => one(".sources-interaction"), "Interaction state missing");
    const startedText = one(".sources-interaction")!.textContent!, request = calls.filter(call => call.command === "start_interaction").at(-1)!.input;
    one<HTMLButtonElement>(".sources-interaction-cancel")!.click();
    await until(() => !one(".sources-interaction"), "Interaction cancel failed");
    const cancelled = calls.filter(call => call.command === "finish_interaction").at(-1)!.input.request.action;
    one<HTMLButtonElement>(".sources-interaction-open")!.click();
    await until(() => one(".sources-interaction-confirm"), "Interaction confirmation missing");
    one<HTMLButtonElement>(".sources-interaction-confirm")!.click();
    await until(() => one(".sources-interaction-done"), "Interaction state missing");
    synthetic = "ready";
    one<HTMLButtonElement>(".sources-interaction-done")!.click();
    await until(() => !one(".sources-interaction") && !one(".sources-interaction-open"), "Interaction completion failed");
    evidence.interaction = { stimulus: "synthetic", after_reject: afterReject, rejected_shown: rejectedShown, started: startedText, request, cancelled, completed: calls.filter(call => call.command === "finish_interaction").at(-1)!.input.request.action };

    const report = one(".sources-report")!.textContent!;
    let copied = "";
    const clipboard = navigator.clipboard;
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => { copied = text; } } });
    one<HTMLButtonElement>(".sources-copy")!.click();
    await until(() => one(".sources-copy-state"), "Copy state missing");
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: clipboard });
    const page = one(".native-sources")!.textContent!;
    evidence.diagnostics = { copied_equals_report: copied === report, platform: report.includes("platform: macos"), contract: report.includes("contract: 1.5.0"), engine: report.includes("synthetic-1.0"), package: report.includes(`package SuperVod v7 ${SHA.slice(0, 12)}`), full_digest_absent: !report.includes(SHA),
      correlation: /correlation=[0-9a-f-]{36}/.test(report), states_separate: report.includes("host phase: ready") && report.includes("cloudstream phase: ready"), secret: report.includes("T17SECRET") || page.includes("T17SECRET"), query: /[?&]token=/.test(report) };
    evidence.errors = errors;
    return evidence;
  } finally {
    app.unmount();
    window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
    window.__TAURI__!.core = original;
  }
}
