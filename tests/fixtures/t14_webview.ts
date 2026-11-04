// Test-mode only. Runs the production App/HomeView/state/Api against a real Host.
import { createApp, h, nextTick } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState } from "../../src/state.ts";
import { nativeInvoke, type Player } from "../../src/player.ts";
import { locale } from "../../src/i18n";

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
const check = (value: unknown, message: string) => { if (!value) throw new Error(message); };
async function until(condition: () => unknown, message: string, ms = 20000) {
  const end = performance.now() + ms;
  while (!condition()) { check(performance.now() < end, `Timed out: ${message}`); await delay(20); }
}
const all = <T extends Element>(selector: string, root: ParentNode = document) => [...root.querySelectorAll<T>(selector)];
const section = (index: number) => document.querySelector<HTMLElement>(`[data-section-index="${index}"]`);
const cards = (index: number) => section(index) ? all<HTMLElement>(".home-card", section(index)!).length : 0;

export async function homeScenario(prototype: { player: Player; base: string; cap: string }) {
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
  let gate: Promise<void> | null = null;
  // Host caches catalog pages by design, so empty/failed page states use an injected envelope.
  let inject: "empty" | "failed" | null = null;
  window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, any>): Promise<T> => {
    if (name === "t01_control" || name === "player_fullscreen") return original.invoke<T>(name, args);
    calls.push({ command: name, input: args?.input });
    if (name === "get_home" && gate) await gate;
    const response = await rpc({ command: name, args });
    if (name !== "get_home" || !inject) return response;
    return (inject === "empty" ? { ...response, data: { images: {}, page: { ...response.data.page, sections: response.data.page.sections.map((row: object) => ({ ...row, items: [], has_next: false, next_page: null })), failed_sections: [] } } }
      : { request_id: response.request_id, engine_epoch: response.engine_epoch, error: { code: "UPSTREAM_FAILED", message: "Injected source failure", retryable: true } }) as T;
  } };
  const onError = (event: ErrorEvent) => errors.push(event.message);
  const onRejection = (event: PromiseRejectionEvent) => errors.push(String(event.reason));
  window.addEventListener("error", onError); window.addEventListener("unhandledrejection", onRejection);
  const count = (command: string) => calls.filter(call => call.command === command).length;
  const hold = () => { let open!: () => void; gate = new Promise(resolve => { open = () => { gate = null; resolve(); }; }); return open; };
  const evidence: Record<string, any> = {};
  location.hash = "#/home";
  let release = hold();
  await appController.initialize();
  const app = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) });
  app.mount(container);
  try {
    await until(() => all(".skeleton-card").length, "Skeleton did not render");
    evidence.skeleton = all(".skeleton-card").length;
    release();
    await until(() => cards(0) && !all(".skeleton-card").length, "Real catalog did not render");
    evidence.restored = { metadata: appState.sources.data!.providers.find(s => s.id === appState.selected_metadata_source?.source_id)?.kind,
      playback: appState.sources.data!.providers.find(s => s.id === appState.selected_playback_source?.source_id)?.kind };

    const order = all<HTMLElement>(".home-section").map(row => ({ index: Number(row.dataset.sectionIndex), title: row.querySelector("h2")!.textContent, cards: all(".home-card", row).length, error: Boolean(row.querySelector(".home-section-error")) }));
    evidence.partial = { order, full_page_error: Boolean(document.querySelector(".home-failed")) };

    const loadMore = async () => {
      const before = cards(0), homes = count("get_home"), open = hold();
      section(0)!.querySelector<HTMLButtonElement>(".home-load-more")!.click();
      await until(() => section(0)!.querySelector(".home-load-more.n-button--loading"), "Tail loading state missing");
      const kept = cards(0);
      open();
      await until(() => cards(0) > before, "Next page did not append");
      return { before, kept, after: cards(0), requests: count("get_home") - homes };
    };
    const extrasBefore = cards(2);
    evidence.pages = [await loadMore(), await loadMore()];
    await until(() => !section(0)!.querySelector(".home-load-more.n-button--loading"), "Pagination did not settle");
    const ids = all<HTMLElement>(".home-card", section(0)!).map(card => card.dataset.cardId!);
    evidence.pagination = { cards: ids.length, unique: new Set(ids).size, peers_unchanged: cards(2) === extrasBefore };

    const posters = all<HTMLImageElement>(".home-card img");
    evidence.images = { lazy: posters.every(image => image.loading === "lazy"), posters: posters.length, unique_urls: new Set(posters.map(image => image.src)).size, placeholders: all(".home-poster-missing").length };
    const long = all<HTMLElement>(".home-card-title").find(title => title.textContent!.length > 100)!;
    check(long, "Long title fixture missing");
    evidence.long_title = { title_chars: long.textContent!.length, width: long.getBoundingClientRect().width, card_width: long.closest(".home-card")!.getBoundingClientRect().width, height: long.getBoundingClientRect().height };

    const layouts: Record<string, unknown>[] = [];
    for (const width of [1200, 960, 768, 390]) {
      await nativeInvoke("t01_control", { action: "width", value: width }); await delay(250);
      const rows = all<HTMLElement>(".home-card", section(0)!), top = rows[0]!.getBoundingClientRect().top;
      const link = rows[0]!.querySelector(".poster-link")!.getBoundingClientRect();
      layouts.push({ width: innerWidth, columns: rows.filter(card => Math.abs(card.getBoundingClientRect().top - top) < 2).length, poster_width: link.width, ratio: link.height / link.width, overflow: document.documentElement.scrollWidth > innerWidth + 1 });
    }
    evidence.layouts = layouts;
    await nativeInvoke("t01_control", { action: "width", value: 960 }); await delay(250);
    const minimum = section(0)!.getBoundingClientRect().width;
    await nativeInvoke("t01_control", { action: "width", value: 1200 }); await delay(250);
    // Window sizes the automation cannot select (for example 1000px) are measured by their real content width.
    const sweep: Record<string, unknown>[] = [];
    for (const width of [minimum, 700, 764, 800, 830, 900, 1000, 1100, 1250, 1400, 1800]) {
      section(0)!.style.width = `${width}px`; await delay(30);
      const rows = all<HTMLElement>(".home-card", section(0)!), top = rows[0]!.getBoundingClientRect().top;
      sweep.push({ content_width: width, columns: rows.filter(card => Math.abs(card.getBoundingClientRect().top - top) < 2).length, poster_width: rows[0]!.querySelector(".poster-link")!.getBoundingClientRect().width });
    }
    section(0)!.style.width = "";
    evidence.column_sweep = sweep;

    await rpc({ operation: "mode", value: 1 });
    const beforeRetry = { featured: cards(0), homes: count("get_home") };
    section(1)!.querySelector<HTMLButtonElement>(".home-section-retry")!.click();
    await until(() => cards(1), "Failed section retry did not recover in place");
    evidence.retry = { recovered: cards(1), still_second: Number(all<HTMLElement>(".home-section")[1]!.dataset.sectionIndex) === 1, featured_kept: cards(0) === beforeRetry.featured, requests: count("get_home") - beforeRetry.homes, error_cleared: !section(1)!.querySelector(".home-section-error") };

    scrollTo({ top: 700, behavior: "instant" }); await delay(100);
    const scrolled = scrollY, homesBeforeReturn = count("get_home"), cardCount = cards(0);
    section(0)!.querySelector<HTMLAnchorElement>(".poster-link")!.click();
    await until(() => appState.route.name === "details" && appState.details.phase === "ready" && !document.querySelector(".home-section"), "Details did not open");
    location.hash = "#/home";
    await until(() => cards(0) && Math.abs(scrollY - scrolled) < 8, "Return did not restore scroll", 5000);
    evidence.return = { scrolled, restored: scrollY, cards_kept: cards(0) === cardCount, requests: count("get_home") - homesBeforeReturn };

    const homesBeforeSource = count("get_home"), dataBefore = appState.home.data;
    const picker = document.querySelector<HTMLSelectElement>(".home-playback-source")!;
    const other = [...picker.options].find(option => option.value && option.value !== picker.value)!;
    check(other, "Second playback source missing");
    picker.value = other.value; picker.dispatchEvent(new Event("change", { bubbles: true }));
    await until(() => appState.selected_playback_source?.source_id === other.value, "Playback source did not change");
    await nextTick();
    evidence.playback_source = { kept_home: appState.home.data === dataBefore && appState.route.name === "home" && cards(0) === cardCount, requests: count("get_home") - homesBeforeSource, persisted: appState.settings.last_playback_source_key === other.value };

    scrollTo({ top: 0, behavior: "instant" }); await delay(100);
    const target = all<HTMLImageElement>(".home-card img", section(0)!)[0]!;
    const pendingId = target.closest<HTMLElement>(".home-card")!.dataset.cardId!, statusBefore = count("get_resource_status"), homesBeforeExpiry = count("get_home");
    await rpc({ operation: "evict-images" });
    // A decoded poster would be served from its 60s cache; a new query forces the real gateway 410.
    target.src = `${target.src}&t14=expired`;
    await until(() => count("get_resource_status") > statusBefore && count("get_home") > homesBeforeExpiry, "Expired poster did not refresh its DTO");
    await until(() => { const image = document.querySelector<HTMLImageElement>(`[data-card-id="${pendingId}"] img`); return image?.complete && image.naturalWidth > 0 && !image.src.includes("t14="); }, "Refreshed poster did not decode");
    await delay(300);
    const refresh = calls.filter(call => call.command === "get_home").slice(homesBeforeExpiry);
    evidence.expiry = { status_queries: count("get_resource_status") - statusBefore, home_requests: refresh.length, page_one_snapshot: refresh.every(call => call.input.page === 1 && call.input.section_ref === null), cards_after: cards(0), error: Boolean(document.querySelector(".home-image-error")) };

    const homeRefresh = () => document.querySelector<HTMLButtonElement>(".home-refresh")!.click();
    inject = "empty"; homeRefresh();
    await until(() => document.querySelector(".home-empty"), "Empty source state missing");
    evidence.empty = { stimulus: "injected", text: document.querySelector(".home-empty")!.textContent, retry: Boolean(document.querySelector(".home-empty .home-retry")), selector: Boolean(document.querySelector(".home-metadata-source")) };
    inject = "failed"; document.querySelector<HTMLButtonElement>(".home-empty .home-retry")!.click();
    await until(() => document.querySelector(".home-failed"), "Failed source state missing");
    evidence.failed = { stimulus: "injected", retry: Boolean(document.querySelector(".home-failed .home-retry")), selector: Boolean(document.querySelector(".home-metadata-source")) };
    inject = null; document.querySelector<HTMLButtonElement>(".home-failed .home-retry")!.click();
    await until(() => cards(0) && !document.querySelector(".home-failed"), "Retry did not recover the source");
    evidence.recovered = cards(0);
    evidence.errors = errors;
    return evidence;
  } finally {
    app.unmount();
    window.removeEventListener("error", onError); window.removeEventListener("unhandledrejection", onRejection);
    window.__TAURI__!.core = original;
  }
}
