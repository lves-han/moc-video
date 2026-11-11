// Packaged test mode only: production shell with the existing real Host fixture.
import { createApp, h, nextTick } from "vue";
import App from "../../src/App.vue";
import UiProvider from "../../src/ui/UiProvider.vue";
import { appController, appState } from "../../src/state";
import { nativeInvoke, type Player } from "../../src/player";
import { palette } from "../../src/ui/theme";

export const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
export function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
export async function until(condition: () => unknown, message: string) {
  const end = performance.now() + 20000;
  while (!condition()) { check(performance.now() < end, message); await delay(30); }
}
let mounted: ReturnType<typeof createApp> | null = null;
let fixture: ReturnType<typeof createApp> | null = null;
function active() {
  const element = document.activeElement as HTMLElement;
  const style = getComputedStyle(element), r = element.getBoundingClientRect();
  return { tag: element.tagName, class: element.className, href: element.getAttribute("href"), row: element.closest(".media-row")?.getAttribute("aria-label"), id: element.closest("[data-card-id]")?.getAttribute("data-card-id"),
    focused: element !== document.body && element !== document.documentElement, focus_visible: element.matches(":focus-visible"), hovered: element.matches(":hover"),
    x: r.x, y: r.y, width: r.width, height: r.height,
    transform: style.transform, shadow: style.boxShadow, outline: style.outline, outline_offset: style.outlineOffset,
    transition: style.transitionDuration, transition_property: style.transitionProperty,
    highlight: getComputedStyle(element, "::after").opacity, tilt_x: style.getPropertyValue("--tilt-x"), tilt_y: style.getPropertyValue("--tilt-y") };
}
export async function shellScenario(action: string, prototype: { player: Player; base: string; cap: string }) {
  if (!mounted) {
    prototype.player.stop();
    const container = document.getElementById("app") as HTMLElement & { __vue_app__?: { unmount(): void } };
    container.__vue_app__!.unmount();
    const original = window.__TAURI__!.core;
    const commands = new Set(["runtime_status", "runtime_prepare", "get_settings", "save_settings", "get_sources", "get_home", "get_details", "get_resource_status", "get_media_cache", "list_history"]);
    window.__TAURI__!.core = { ...original, invoke: async <T>(name: string, args?: Record<string, unknown>): Promise<T> => {
      if (!commands.has(name)) return original.invoke<T>(name, args);
      const response = await fetch(`${prototype.base}control/invoke?cap=${prototype.cap}`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ command: name, args }) });
      check(response.ok, "Real Host fixture transport failed");
      return response.json();
    } };
    location.hash = "#/home";
    await appController.initialize();
    mounted = createApp({ render: () => h(UiProvider, null, { default: () => h(App) }) });
    mounted.mount(container);
    await until(() => document.querySelector(".home-card"), "Real Host catalog missing");
  }
  if (action === "measure") {
    await nextTick(); await delay(400); scrollTo(0, 0);
    const styleKeys = ["display", "position", "width", "height", "padding", "margin", "gap", "fontSize", "lineHeight", "fontWeight", "borderRadius", "backgroundColor", "color", "gridTemplateColumns"] as const;
    const selectors = [".sidebar", ".brand", ".primary-nav", ".utility-nav", ".app-main", ".topbar", "#main-content", ".catalog-sources", ".home-section", ".home-grid", ".home-card", ".poster-link", ".card-title"];
    return { width: innerWidth, overflow: document.documentElement.scrollWidth > innerWidth + 1,
      nodes: Object.fromEntries(selectors.map(selector => {
        const element = document.querySelector<HTMLElement>(selector)!;
        check(element, `Missing baseline selector ${selector}`);
        const s = getComputedStyle(element), r = element.getBoundingClientRect();
        return [selector, { style: Object.fromEntries(styleKeys.map(key => [key, s[key]])), rect: { x: r.x, width: r.width, height: r.height } }];
      })), native: await nativeInvoke("t01_control", { action: "identity" }) };
  }
  if (action === "desktop") {
    await delay(400); scrollTo(0, 0);
    const sidebar = document.querySelector<HTMLElement>(".tv-sidebar")!, main = document.querySelector<HTMLElement>("main")!;
    const s = getComputedStyle(sidebar), m = getComputedStyle(main);
    const rect = (element: Element) => element.getBoundingClientRect().toJSON();
    return { width: innerWidth, height: innerHeight, sidebar: rect(sidebar), main: rect(main), drag: rect(document.querySelector(".tv-drag-region")!),
      padding_left: m.paddingLeft, padding_right: m.paddingRight, sidebar_background: s.backgroundColor, sidebar_radius: s.borderRadius,
      sidebar_border: s.borderTop, glass: s.backdropFilter, webkit_glass: s.getPropertyValue("-webkit-backdrop-filter"), sidebar_transition: s.transitionDuration,
      color_scheme: getComputedStyle(document.documentElement).colorScheme, background: getComputedStyle(document.body).backgroundColor,
      topbar: getComputedStyle(document.querySelector(".topbar")!).display, legacy: getComputedStyle(document.querySelector(".legacy-sidebar")!).display,
      navigation: [...sidebar.querySelectorAll("nav a")].map(element => ({ href: element.getAttribute("href"), title: element.getAttribute("title"), rect: rect(element), font: getComputedStyle(element).fontSize, weight: getComputedStyle(element).fontWeight, radius: getComputedStyle(element).borderRadius, icon: rect(element.querySelector("svg")!), stroke: getComputedStyle(element.querySelector("svg")!).strokeWidth, current: element.getAttribute("aria-current"), background: getComputedStyle(element).backgroundColor })),
      sources: [...sidebar.querySelectorAll("select")].map(element => ({ role: element.dataset.role, height: element.getBoundingClientRect().height, font: getComputedStyle(element).fontSize, value: element.value })),
      hidden_source_labels: [...document.querySelectorAll(".catalog-sources > label")].every(element => getComputedStyle(element).display === "none"),
      tokens: Object.fromEntries([...document.documentElement.style].filter(key => key.startsWith("--tv-") || key === "--accent").map(key => [key, document.documentElement.style.getPropertyValue(key)])),
      page_title: { font: getComputedStyle(document.querySelector(".tv-page-title")!).fontSize, line: getComputedStyle(document.querySelector(".tv-page-title")!).lineHeight, weight: getComputedStyle(document.querySelector(".tv-page-title")!).fontWeight },
      overflow: document.documentElement.scrollWidth > innerWidth + 1, reduced: matchMedia("(prefers-reduced-motion: reduce)").matches,
      native: await nativeInvoke("t01_control", { action: "identity" }) };
  }
  if (action === "fixture") {
    fixture?.unmount(); document.getElementById("tv1-rows")?.remove();
    const { default: MediaRow } = await import("../../src/components/MediaRow.vue");
    const host = document.createElement("div"); host.id = "tv1-rows";
    document.querySelector("main")!.prepend(host);
    const originals = appState.home.data!.page.sections[0]!.items;
    const cards = Array.from({ length: 14 }, (_, index) => ({ ...originals[index % originals.length]!, id: `tv1-${index}`, title: index === 13 ? "A very long missing poster title ".repeat(8) : `TV1 fixture ${index}`, ...(index === 13 ? { poster: null } : {}) }));
    fixture = createApp({ render: () => h("div", [h(MediaRow, { title: "TV1 portrait", items: cards, images: appState.home.data!.images, href: "#/home" }), h(MediaRow, { title: "TV1 landscape", items: cards, images: appState.home.data!.images, landscape: true }), h(MediaRow, { title: "TV1 loading", items: [], loading: true })]) });
    fixture.mount(host); await nextTick(); await delay(350);
    return { cards: document.querySelectorAll("#tv1-rows .media-card").length, expected_missing: cards.filter(card => !card.poster || !appState.home.data!.images[card.poster.resource_id]).length * 2 };
  }
  if (action === "focus-first" || action === "focus-last" || action === "focus-second-row") {
    const row = action === "focus-second-row" ? ".media-row-landscape" : ".media-row";
    const cards = [...document.querySelectorAll<HTMLElement>(`#tv1-rows ${row} .media-poster[href]`)];
    const card = action === "focus-last" ? cards[13]! : cards[0]!;
    card.scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" }); card.focus(); await delay(300);
    return active();
  }
  if (action === "active") { await delay(300); return active(); }
  if (action === "focus-source") { document.querySelector<HTMLSelectElement>('.tv-sidebar select[data-role="metadata"]')!.focus(); return active(); }
  if (action === "focus-real-card") { document.querySelector<HTMLAnchorElement>(".home-card .poster-link")!.focus(); return active(); }
  if (action === "home-ready") { await until(() => appState.route.name === "home" && document.querySelector(".home-card"), "Escape did not return to actual home"); await delay(300); return active(); }
  if (action === "toggle") { document.querySelector<HTMLButtonElement>(".tv-sidebar-toggle")!.click(); await delay(350); return active(); }
  if (action === "nav-history" || action === "nav-sources" || action === "nav-settings" || action === "nav-home") {
    fixture?.unmount(); fixture = null; document.getElementById("tv1-rows")?.remove();
    const page = action.slice(4);
    document.querySelector<HTMLAnchorElement>(`.tv-sidebar a[href="#/${page}"]`)!.click();
    await until(() => appState.route.name === page, `Route ${page} missing`); await delay(400);
    return { route: appState.route.name, main: document.querySelector("main")!.textContent!.slice(0, 120), active: active() };
  }
  if (action.startsWith("palette-")) {
    const id = action.slice(8) as typeof palette.value;
    check(["lime", "ocean", "violet", "rose", "amber"].includes(id), "Unknown test palette");
    palette.value = id; await nextTick(); await delay(100);
    return { accent: getComputedStyle(document.documentElement).getPropertyValue("--accent"), dark: getComputedStyle(document.documentElement).colorScheme };
  }
  if (action === "row-styles") {
    const root = document.querySelector<HTMLElement>("#tv1-rows")!;
    const measure = (selector: string) => {
      const e = root.querySelector<HTMLElement>(selector)!; check(e, `Missing row selector ${selector}`);
      const s = getComputedStyle(e), r = e.getBoundingClientRect();
      return { y: r.y, width: r.width, height: r.height, font: s.fontSize, line: s.lineHeight, weight: s.fontWeight, radius: s.borderRadius, gap: s.gap, padding: s.padding, margin: s.margin, snap: s.scrollSnapType, scrollbar: s.getPropertyValue("scrollbar-width"), animation: s.animationDuration, name: s.animationName, opacity: s.opacity, overflow: s.textOverflow, white_space: s.whiteSpace, transition: s.transitionDuration, filter: s.backdropFilter, display: s.display, scroll: e.scrollLeft, scroll_width: e.scrollWidth, client_width: e.clientWidth };
    };
    return Object.fromEntries([".media-row", ".media-row-heading", ".media-row h2", ".media-row-track", ".media-card", ".media-poster", ".media-card-title", ".media-card-detail", ".media-row-landscape .media-card", ".media-row-landscape .media-poster", ".media-poster-missing", ".media-loading .media-poster", ".media-row-next"].map(selector => [selector, measure(selector)]));
  }
  if (action === "images") {
    const images = [...document.querySelectorAll<HTMLImageElement>("#tv1-rows img.loaded")];
    return { loaded: images.length, valid: images.every(image => image.complete && image.naturalWidth > 0 && getComputedStyle(image).opacity === "1" && getComputedStyle(image).transitionDuration === "0.2s"), missing: document.querySelectorAll("#tv1-rows .media-poster-missing").length };
  }
  if (action === "hover-target") {
    const card = document.querySelector<HTMLElement>("#tv1-rows .media-poster[href]")!;
    document.querySelector<HTMLElement>("main")!.focus(); card.scrollIntoView({ block: "center", inline: "start", behavior: "instant" }); await delay(300);
    const r = card.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2, edge_x: r.right - 1, edge_y: r.top + 1 };
  }
  if (action === "hover-style") {
    const card = document.querySelector<HTMLElement>("#tv1-rows .media-poster[href]")!;
    await delay(300);
    const s = getComputedStyle(card);
    return { hovered: card.matches(":hover"), transform: s.transform, shadow: s.boxShadow, highlight: getComputedStyle(card,"::after").opacity, tilt_x: s.getPropertyValue("--tilt-x"), tilt_y: s.getPropertyValue("--tilt-y"), focus_visible: card.matches(":focus-visible"), animation: s.animationName, transition: s.transitionProperty };
  }
  if (action === "motion") return { reduced: matchMedia("(prefers-reduced-motion: reduce)").matches, animations: [...document.querySelectorAll<HTMLElement>(".media-poster,.feature-image")].map(e => ({ transform: getComputedStyle(e).transform, animation: getComputedStyle(e).animationName })) };
  if (action === "pager-next") {
    const button = document.querySelector<HTMLButtonElement>("#tv1-rows .media-row-next")!; check(button, "Next page arrow missing"); button.click(); await delay(800);
    return { active: active(), scroll: document.querySelector<HTMLElement>("#tv1-rows .media-row-track")!.scrollLeft };
  }
  throw new Error(`Unknown TV1 action: ${action}`);
}
