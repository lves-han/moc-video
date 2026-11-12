<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton, NModal, NRadioGroup, NRadioButton, useMessage } from "naive-ui";
import SettingsView from "./views/SettingsView.vue";
import HistoryView from "./views/HistoryView.vue";
import LocalView from "./views/LocalView.vue";
import LiveView from "./views/LiveView.vue";
import PlaybackView from "./views/PlaybackView.vue";
import HomeView from "./views/HomeView.vue";
import BrowseView from "./views/BrowseView.vue";
import DetailsView from "./views/DetailsView.vue";
import SearchView from "./views/SearchView.vue";
import Icon from "./components/Icon.vue";
import FilmCard from "./components/FilmCard.vue";
import { appState, appController, metadataVideoId, liveAvailable } from "./state.ts";
import { finishBeforeClose, nativeWindowBounds, settingsQueue } from "./api";
import { historyRefused, historyRepository, historyWriteDue } from "./data/history";
import { historyContentId, type HistoryEntry } from "./data/history-model";
import { films as baseFilms, localizeFilm, sourceText, initialSources, kindLabels, type FilmKind, type Source } from "./data/demo";
import { previewRef, previewSourceId, previewSourceKey } from "./data/preview";
import { locale, t, type Message } from "./i18n";
import { palette, paletteOptions, normalizePalette } from "./ui/theme";
import { installFocusNavigation } from "./ui/focus";
import type { AppSource, Bridge_MediaRef, PlaybackContext, ResumeSnapshot, Settings, WatchHistoryRecord } from "./models";

const native = Boolean(window.__TAURI__);
const desktopQuery = matchMedia("(min-width: 960px)");
const expandedQuery = matchMedia("(min-width: 1200px)");
const sidebarCollapsed = ref(!expandedQuery.matches);
const sidebarToggle = ref<HTMLButtonElement>();
const shell = ref<HTMLElement>();
const sidebarSources = ref<HTMLElement>();
const desktopNavigation = computed(() => [
  { page: "home", href: "#/home", icon: "compass", label: t("Discover") },
  { page: "search", href: "#/search", icon: "search", label: locale.value === 'zh-CN' ? '搜索' : 'Search' },
  { page: 'browse', href: '#/browse/movie', icon: 'film', label: t('Movies') },
  { page: 'browse', href: '#/browse/series', icon: 'tv', label: t('Series') },
  { page: 'browse', href: '#/browse/anime', icon: 'sparkles', label: t('Animation') },
  { page: 'browse', href: '#/browse/documentary', icon: 'library', label: t('Documentary') },
  ...(native && appState.home.data?.page.sections.some(section => section.kind === 'series' && /^(variety|综艺)$/i.test(section.title)) ? [{ page: 'browse', href: '#/browse/variety', icon: 'tv', label: t('Variety') }] : []),
  ...(['variety', 'short', 'sports'] as const).filter(category => !native && appState.sources.data?.providers.some(item => item.kind === 'maccms'
    && item.runtime_status === 'ready' && item.id === appState.selected_playback_source?.source_id && item.content_types.includes(category)))
    .map(category => ({ page: 'browse', href: `#/browse/${category}`, icon: category === 'sports' ? 'play' : 'tv', label: t({ variety: 'Variety', short: 'Short dramas', sports: 'Sports' }[category] as Message) })),
  { page: "local", href: "#/local", icon: "folder", label: t("Local video") },
  ...(liveAvailable(appState.settings) ? [{ page: "live", href: "#/live", icon: "broadcast", label: t("Live") }] : []),
  { page: "history", href: "#/history", icon: "library", label: locale.value === 'zh-CN' ? '资料库' : 'Library' },
  { page: "settings", href: "#/settings", icon: "settings", label: t("Settings") },
]);
const metadataProviders = computed(() => (appState.sources.data?.providers ?? []).filter(item => item.kind === "stremio" && item.runtime_status === "ready" && item.capabilities.home && item.capabilities.details));
function resetSidebar() { sidebarCollapsed.value = !expandedQuery.matches; }
async function expandSource(role: "metadata" | "playback") {
  sidebarCollapsed.value = false;
  await nextTick();
  sidebarSources.value?.querySelector<HTMLSelectElement>(`[data-role="${role}"]`)?.focus();
}
async function dragWindow(event: MouseEvent) {
  if (event.button !== 0 || (event.target as HTMLElement).closest("a,button,input,select,textarea,[role=button]")) return;
  try { await window.__TAURI__?.core.invoke("plugin:window|start_dragging"); }
  catch (error) { preferenceError.value = String(error); }
}
function escapeShell() {
  if (appearanceOpen.value) { appearanceOpen.value = false; return true; }
  if (aboutOpen.value) { aboutOpen.value = false; return true; }
  if (route.value.page !== "home") { location.hash = "#/home"; return true; }
  const active = document.activeElement;
  if (!active?.closest(".tv-sidebar")) {
    shell.value?.querySelector<HTMLAnchorElement>('.tv-sidebar [aria-current="page"]')?.focus();
  } else if (!sidebarCollapsed.value) {
    sidebarCollapsed.value = true;
    sidebarToggle.value?.focus();
  } else sidebarToggle.value?.focus();
  return true;
}
let removeFocus: (() => void) | undefined;
watch(shell, root => { removeFocus?.(); removeFocus = root ? installFocusNavigation(root, escapeShell) : undefined; });
const routeFocus = new Map<string, string>();
function rememberFocus(event: FocusEvent) {
  const target = event.target as HTMLElement;
  const link = target.closest<HTMLAnchorElement>("a[href]");
  if (link?.closest("main")) routeFocus.set(location.hash, link.getAttribute("href")!);
}
const messages = useMessage();
const preferenceError = ref("");
const historySaveError = ref<string | null>(null);
const hash = ref(location.hash);
const languageOptions = [{ label: "EN", value: "en" }, { label: "中文", value: "zh-CN" }];
const films = computed(() => baseFilms.map(localizeFilm));
function readHash() {
  const [path = "", query = ""] = hash.value.replace(/^#\/?/, "").split("?");
  const [name = "home", id = "", episode = "1"] = path.split("/");
  return { name, id, episode: Math.max(1, Math.floor(Number(episode) || 1)), resume: new URLSearchParams(query).get("resume") === "1" };
}
type Page = "home" | "browse" | "search" | "local" | "live" | "film" | "watch" | "settings" | "history";
const route = computed<{ page: Page; id: string; kind: FilmKind | "" }>(() => {
  const stored = appState.route, view = readHash();
  // The source manager now lives in Settings; the contract still lists its old route names.
  const page: Page = stored.name === 'home' && ['browse','search','local','live'].includes(view.name) ? view.name as 'browse' | 'search' | 'local' | 'live' : stored.name === "details" ? "film" : stored.name === "playback" ? "watch"
    : stored.name === "sources" || stored.name === "setup" ? "settings" : stored.name;
  return { page,
    id: view.id, kind: view.name === "kind" && ["movie", "series", "anime"].includes(view.id) ? view.id as FilmKind : "" };
});
// A rematch that needs the user keeps hash #/playback; its confirmation/result UI lives in CatalogView.
const matchNeedsUser = computed(() => ["needs_confirmation", "not_found"].includes(appController.matching.result?.status ?? ""));
const formalPlayback = computed(() => appState.route.name === "playback" || (readHash().name === "playback" && !matchNeedsUser.value && !appController.serviceRound.open));
const playbackContext = computed(() => appState.route.name === "playback" ? appState.route.context : null);
const playbackView = ref<InstanceType<typeof PlaybackView>>();
defineExpose({ playbackController: () => playbackView.value?.playback });
const previewSources = ref<Source[]>(initialSources.map(source => ({ ...source })));
const sources = computed<Source[]>(() => native ? (appState.sources.data?.providers ?? []).map(source => ({
  id: source.id, name: source.name, kind: source.kind, description: source.error?.message ?? "", enabled: source.runtime_status === "ready", demo: false,
})) : previewSources.value);
const sourceId = computed(() => native ? appState.selected_playback_source?.source_id ?? "" : previewSourceId(appState.selected_playback_source));
const source = computed(() => sources.value.find(item => item.id === sourceId.value) ?? sources.value[0] ?? initialSources[0]!);
const maccmsDetailsSource = computed(() => appState.details.data?.metadata_ref?.identity.namespace === 'maccms'
  ? appState.sources.data?.providers.find(item => item.kind === 'maccms' && item.id === appState.details.data?.details.ref.source_id) : undefined);
const maccmsDetailsStyle = computed(() => {
  const data = appState.details.data, id = data?.details.poster?.resource_id, url = id ? data?.images[id]?.url : null;
  return maccmsDetailsSource.value ? { '--maccms-poster': url ? `url(${JSON.stringify(url)})` : 'none', '--maccms-source': JSON.stringify(maccmsDetailsSource.value.name) } : {};
});
const selectedFilm = computed(() => films.value.find(film => film.id === route.value.id) ?? films.value[0]!);
const activeGenre = ref<Message>("All"), expanded = ref(false), heroIndex = ref(0);
const homeSnapshots = new Map<string, { genre: Message; expanded: boolean; hero: number; scroll: number }>();
const browseScroll = new Map<string, number>();
let lastHash = location.hash;
function loadPageHome() { return native && desktopQuery.matches ? appController.loadCatalog() : appController.loadHome(); }
watch(() => appState.selected_metadata_source, source => {
  if (source && appState.home.phase === 'idle' && ['home', 'browse', 'kind'].includes(readHash().name)) void loadPageHome();
});
const homeKey = () => `${appState.selected_metadata_source?.source_id}:${appState.selected_metadata_source?.source_revision}:${route.value.kind}`;
function rememberHome() { if (route.value.page === "home") homeSnapshots.set(homeKey(), { genre: activeGenre.value, expanded: expanded.value, hero: heroIndex.value, scroll: scrollY }); }
const heroFilms = computed(() => route.value.kind ? films.value.filter(film => film.kind === route.value.kind) : [films.value[0]!, films.value[2]!, films.value[3]!]);
const hero = computed(() => heroFilms.value[heroIndex.value % heroFilms.value.length]!);
const title = computed(() => route.value.kind ? t(kindLabels[route.value.kind as FilmKind]) : t(({
  home: "Discover", browse: 'Browse by genre', search:'Search', local: "Local video", live: "Live", film: "Film details", watch: "Now playing", settings: "Settings", history: "Watch history",
} as const)[route.value.page]));
const filteredFilms = computed(() => films.value.filter(film => (!route.value.kind || film.kind === route.value.kind) && (activeGenre.value === "All" || film.genres.includes(activeGenre.value))));
const visibleFilms = computed(() => expanded.value ? filteredFilms.value : filteredFilms.value.slice(0, 6));
const relatedFilms = computed(() => films.value.filter(film => film.id !== selectedFilm.value.id).slice(0, 4));
const genres: Message[] = ["All", "Drama", "Adventure", "Mystery", "Sci-fi", "Feel-good", "Documentary"];
const loading = computed(() => appState.home.phase === "loading");
const nav = computed(() => [
  { label: t("Discover"), icon: "compass", href: "#/home", kind: "" },
  { label: t("Movies"), icon: "film", href: "#/kind/movie", kind: "movie" },
  { label: t("Series"), icon: "tv", href: "#/kind/series", kind: "series" },
  { label: t("Animation"), icon: "sparkles", href: "#/kind/anime", kind: "anime" },
]);
const aboutOpen = ref(false), appearanceOpen = ref(false);
const aboutTrigger = ref<HTMLButtonElement>(), paletteTrigger = ref<HTMLButtonElement>();
const speed = computed({ get: () => appState.playback.speed, set: value => { appState.playback.speed = value; } });
const autoNext = computed({ get: () => appState.settings.auto_next, set: value => { void savePreference(settings => ({ ...settings, auto_next: value })); } });
function notify(text: string) { messages.info(text, { duration: 3200 }); }
watch(() => appState.settings, settings => {
  locale.value = settings.ui_locale ?? "en";
  palette.value = normalizePalette(settings.color_palette);
  document.documentElement.dataset.palette = palette.value;
}, { immediate: true, flush: "sync" });
watch([locale, title, () => appState.details.data?.details.title], () => {
  document.documentElement.lang = locale.value;
  document.title = `${formalPlayback.value ? t("Now playing") : route.value.page === "film" ? native ? appState.details.data?.details.title ?? title.value : selectedFilm.value.title : title.value} · ${t("Moc Video")}`;
}, { immediate: true });
async function savePreference(change: (value: Settings) => Settings) {
  preferenceError.value = "";
  try {
    const confirmed = await appController.updateSettings(change);
    // Legacy keys are first-paint mirrors. The full Settings queue owns commits.
    localStorage.setItem("moc-video.locale", confirmed.ui_locale ?? "en");
    localStorage.setItem("moc-video.palette", confirmed.color_palette ?? "lime");
  } catch (error) { preferenceError.value = error instanceof Error ? error.message : String(error); }
}
function changeLanguage(value: string) { if (value === "en" || value === "zh-CN") return savePreference(settings => ({ ...settings, ui_locale: value })); }
function changePalette(value: string) { if (paletteOptions.some(item => item.value === value)) return savePreference(settings => ({ ...settings, color_palette: value as NonNullable<Settings["color_palette"]> })); }
async function retryPreferences() {
  try {
    if (appController.awaitingCleanup) await appController.retryCleanup();
    await appController.restoreSettings(); appController.diagnostics.splice(0); preferenceError.value = "";
  }
  catch (error) { preferenceError.value = String(error); }
}
async function selectNativeSource(id: string, role: "metadata" | "playback") {
  try {
    await flushHistory();
    rememberHome();
    pendingResume.value = null;
    await appController.selectSource(id, role);
    if (role === "metadata") {
      if (location.hash === "#/home" && appState.home.phase !== 'loading') await loadPageHome();
      else location.hash = "#/home";
    }
  } catch (error) { preferenceError.value = String(error); }
}
async function changeSource(id: string) {
  try { await selectNativeSource(native ? id : previewSourceKey(id).source_id, "playback"); }
  catch (cause) { preferenceError.value = String(cause); }
}
function openDetails(ref: Bridge_MediaRef) {
  const target = `#/details/${encodeURIComponent(JSON.stringify(ref))}`;
  if (target === location.hash) return;
  // Each visit owns its predecessor, including a repeated title. History state
  // survives reload and lets the browser keep Back/Forward in the same order.
  history.pushState({ detailsOrigin: location.hash }, '', target);
  void onHashChange();
}
function backFromDetails() {
  if (typeof history.state?.detailsOrigin === 'string') history.back();
  else location.hash = '#/home';
}
function markTransitionPoster(target: string) {
  const candidates = [...document.querySelectorAll<HTMLElement>('.poster-link[href]')].filter(element => element.getAttribute('href') === target);
  const element = candidates.find(element => { const r = element.getBoundingClientRect(); return r.bottom > 0 && r.top < innerHeight; }) ?? candidates[0];
  if (element) element.style.viewTransitionName = 'details-art';
  return element;
}
async function onHashChange() {
  const leaving = lastHash, target = location.hash;
  const detailsChange = native && desktopQuery.matches && (leaving.startsWith('#/details/') || target.startsWith('#/details/'))
    && !leaving.startsWith('#/playback') && !target.startsWith('#/playback');
  if (!detailsChange || matchMedia('(prefers-reduced-motion: reduce)').matches) { await updateRoute(); return; }
  const poster = target.startsWith('#/details/') ? markTransitionPoster(target) : null;
  const update = async () => {
    await updateRoute();
    if (leaving.startsWith('#/details/') && location.hash === target) markTransitionPoster(leaving);
  };
  try {
    if (document.startViewTransition) {
      const transition = document.startViewTransition(update);
      // The update still completes when the platform skips an interrupted transition.
      await Promise.allSettled([transition.ready, transition.finished, transition.updateCallbackDone]);
    } else {
      const main = document.getElementById('main-content'), old = main?.cloneNode(true) as HTMLElement | undefined;
      if (main && old) {
        const rect = main.getBoundingClientRect(); old.removeAttribute('id'); old.querySelectorAll('[id]').forEach(element => element.removeAttribute('id'));
        old.classList.add('page-transition-old'); old.inert = true; old.setAttribute('aria-hidden', 'true');
        Object.assign(old.style, { position: 'fixed', left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`, margin: '0', pointerEvents: 'none', zIndex: '10' });
        main.after(old);
      }
      try {
        await update();
        await Promise.all([old?.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 300, fill: 'forwards' }).finished,
          main?.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 300 }).finished]);
      } finally { old?.remove(); }
    }
  } catch (error) { preferenceError.value = String(error); }
  finally { poster?.style.removeProperty('view-transition-name'); document.querySelectorAll<HTMLElement>('.poster-link').forEach(element => element.style.removeProperty('view-transition-name')); }
}
async function playEpisode(videoId: string, explicitLine = false) {
  try {
    await flushHistory();
    if (native && appState.settings.playback_service?.enabled) {
      if (appState.playback.context?.metadata_video_id === videoId && appState.playback.position_ms > 0) wantedResume = { video_id: videoId, position_ms: appState.playback.position_ms };
      await appController.startServiceRound(videoId, false, !explicitLine);
      return;
    }
    if (native) throw new Error('请先连接播放服务。 / Connect the playback service first.');
    const result = await appController.match(videoId, null, !explicitLine);
    if (result?.status === "matched") location.hash = "#/playback";
  } catch (error) { preferenceError.value = String(error); }
}
let switchingServiceLine = false;
async function switchFailedServiceLine(context: PlaybackContext) {
  if (switchingServiceLine) return;
  switchingServiceLine = true;
  try {
  const snapshot = appState.playback, epoch = snapshot.playback_epoch, session = snapshot.session?.session_id;
  const position = snapshot.position_ms;
  await flushHistory();
  if (snapshot.playback_epoch !== epoch || snapshot.session?.session_id !== session || snapshot.context?.match_id !== context.match_id) return;
  wantedResume = { video_id: context.metadata_video_id, position_ms: position };
  if (!await appController.tryNextServiceLine(context)) wantedResume = null;
  } finally { switchingServiceLine = false; }
}
const pendingResume = ref<ResumeSnapshot | null>(null);
let sequence = 0;
function focusMain() {
  if (route.value.page === 'search') { searchView.value?.focus(); return; }
  const main = document.getElementById("main-content");
  if (desktopQuery.matches) {
    const href = routeFocus.get(location.hash);
    const previous = href && [...main?.querySelectorAll<HTMLAnchorElement>("a[href]") ?? []].find(link => link.getAttribute("href") === href);
    if (previous) { previous.focus({ preventScroll: true }); return; }
  }
  main?.focus({ preventScroll: true });
}
async function updateRoute() {
  const current = ++sequence;
  await flushHistory();
  if (current !== sequence) return;
  if (lastHash.startsWith('#/browse/')) browseScroll.set(lastHash, scrollY);
  lastHash = location.hash;
  rememberHome(); hash.value = location.hash;
  const next = readHash();
  if (next.name === "playback") return;
  pendingResume.value = null;
  try {
    if (next.name === "film" || next.name === "watch" || next.name === "details") {
      const film = baseFilms.find(film => film.id === next.id);
      if (!native && !film) throw new Error("Preview title is unavailable");
      const ref: Bridge_MediaRef = next.name === "details" ? JSON.parse(decodeURIComponent(next.id)) : previewRef(next.id);
      appController.navigate({ name: "details", ref });
      await appController.loadDetails(ref);
      if (current !== sequence || appState.details.phase !== "ready") return;
      if (wantedResume) void playEpisode(wantedResume.video_id);
      if (next.name === "watch") {
        const episode = Math.min(next.episode, film!.episodes);
        let saved: HistoryEntry | null = null;
        if (next.resume) {
          saved = await historyRepository.get(historyContentId(film!.kind, film!.id));
          if (current !== sequence) return;
          if (!saved || saved.episode !== episode) throw new Error(t("No saved progress was found for this episode."));
          const original = sources.value.find(source => source.id === saved!.source_id && source.enabled);
          if (!original) throw new Error(t("The saved playback source is unavailable. Enable it in Sources before continuing."));
          if (original.id !== sourceId.value) await appController.selectSource(previewSourceKey(original.id).source_id, "playback");
        }
        if (current !== sequence) return;
        const result = await appController.match(`${film!.id}:${episode}`);
        if (current !== sequence) return;
        if (result?.context) pendingResume.value = { context: result.context, position_ms: saved && !saved.completed ? saved.position_ms : 0, was_playing: true };
      }
    } else {
      const name = ["settings", "history"].includes(next.name) ? next.name as "settings" | "history" : "home";
      appController.navigate({ name });
      if (name === "home" && !['search', 'local', 'live'].includes(next.name) && (!appState.home.data || appController.homeCatalogOnly !== (native && desktopQuery.matches))
        && appState.home.phase !== 'loading' && appState.selected_metadata_source) await loadPageHome();
    }
    if (current !== sequence) return;
    const home = route.value.page === "home" ? homeSnapshots.get(homeKey()) : undefined;
    if (route.value.page === "home") { activeGenre.value = home?.genre ?? "All"; expanded.value = home?.expanded ?? false; heroIndex.value = home?.hero ?? 0; }
    await nextTick();
    if (current !== sequence) return;
    // A returning MacCMS wall remounts and fetches its first page before it has
    // enough height to restore scroll. Wait for that existing load to settle.
    if (route.value.page === 'browse' && browseScroll.has(location.hash)) {
      while (current === sequence && document.querySelector('.browse-catalog .catalog-status')) await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
      if (current !== sequence) return;
      await nextTick();
    }
    scrollTo({ top: route.value.page === 'browse' ? browseScroll.get(location.hash) ?? 0 : home?.scroll ?? 0, behavior: "instant" }); focusMain();
  } catch (error) { preferenceError.value = error instanceof Error ? error.message : String(error); }
}
let lastWrite = 0;
let lastHistory: HistoryEntry | null = null;
function nativeHistorySnapshot(finished: boolean): HistoryEntry | null {
  const state = appState.playback, context = state.context, data = appState.details.data;
  if (!context || !data || state.position_ms <= 0) return null;
  const episodes = data.details.groups.flatMap(group => group.episodes);
  const index = episodes.findIndex(episode => metadataVideoId(episode.ref) === context.metadata_video_id);
  const provider = appState.sources.data?.providers.find(item => item.id === context.episode_ref.source_id);
  let serviceLine = '';
  try { const ref = JSON.parse(context.episode_ref.data); if (ref[0] === 'service_line') serviceLine = ref[2]; } catch { /* legacy reference */ }
  if (index < 0 || (!provider && !serviceLine)) return null;
  const sourceId = context.episode_ref.source_id;
  const sourceName = provider?.name ?? appController.serviceRound.snapshot?.lines.find(line => line.line_id === serviceLine)?.source_name ?? 'Playback service';
  const session = state.session;
  if (!session) return null;
  const kind = data.details.kind === "other" ? "series" : data.details.kind;
  const poster = data.details.poster ? data.images[data.details.poster.resource_id]?.url : undefined;
  const episode = episodes[index]!, position_ms = Math.round(state.duration_ms ? Math.min(state.duration_ms, state.position_ms) : state.position_ms);
  const duration_ms = state.duration_ms ? Math.max(1, Math.round(state.duration_ms)) : null;
  const completed = finished || state.phase === "ended", updated_at = Date.now();
  return { content_id: data.details.id, title: data.details.title, poster: poster ?? "/mark.svg", kind,
    episode: index + 1, episode_total: episodes.length, source_id: sourceId, source_name: sourceName,
    position_ms, duration_ms, completed, updated_at,
    record: { content_id: data.details.id, metadata_identity: context.metadata_ref.identity, video_id: context.metadata_video_id, season_number: episode.season ?? null,
      episode_number: episode.number ?? index + 1, title: data.details.title, kind: data.details.kind, poster_cache_key: null, source_id: sourceId, source_name: sourceName,
      position_ms, duration_ms, completed, updated_at },
    capture: { session_id: session.session_id, match_id: context.match_id, playback_epoch: state.playback_epoch } };
}
function historySnapshot(finished = false): HistoryEntry | null {
  if (native) return nativeHistorySnapshot(finished);
  const state = appState.playback, context = state.context;
  const film = baseFilms.find(film => film.id === context?.metadata_ref.identity.meta_id);
  if (!context || !film || state.position_ms <= 0 || state.phase === "failed") return null;
  const episode = Number(context.metadata_video_id.split(":").at(-1));
  if (!Number.isInteger(episode) || episode < 1 || episode > film.episodes) return null;
  return { content_id: historyContentId(film.kind, film.id), title: film.title, poster: film.artwork, kind: film.kind,
    episode, episode_total: film.episodes, source_id: sourceId.value, source_name: source.value.name,
    position_ms: state.duration_ms ? Math.min(state.duration_ms, state.position_ms) : state.position_ms,
    duration_ms: state.duration_ms, completed: finished || state.phase === "ended", updated_at: Date.now() };
}
let failedHistory: HistoryEntry | null = null;
async function persistHistory(snapshot: HistoryEntry) {
  try { await historyRepository.save(snapshot); historySaveError.value = null; }
  catch (error) {
    // A refused snapshot only matters if it is the one awaiting retry; an earlier real failure keeps its notice.
    if (!historyRefused(error)) { failedHistory = snapshot; historySaveError.value = error instanceof Error ? error.message : String(error); }
    else if (failedHistory === snapshot) { failedHistory = null; historySaveError.value = null; }
    return;
  }
  failedHistory = null;
}
async function flushHistory(finished = false) {
  const snapshot = historySnapshot(finished);
  if (snapshot) { lastWrite = Date.now(); lastHistory = snapshot; await persistHistory(snapshot); }
}
function retryHistorySave() { const snapshot = failedHistory ?? lastHistory; if (snapshot) void persistHistory(snapshot); }
function discardPendingHistory(id: string | null) {
  if (id === null || lastHistory?.content_id === id) lastHistory = null;
  if (id === null || failedHistory?.content_id === id) { failedHistory = null; historySaveError.value = null; }
  if (id === null || latestHistory?.content_id === id) latestHistory = null;
}
// Resuming goes back through the source and match checks; the position waits for the confirmed episode.
let wantedResume: { video_id: string; position_ms: number } | null = null;
async function continueNativeHistory(record: WatchHistoryRecord) {
  const providers = appState.sources.data?.providers ?? [], identity = record.metadata_identity;
  const ready = (source: AppSource) => source.runtime_status === "ready";
  const direct = identity.namespace === 'maccms';
  const metadata = providers.find(source => source.kind === (direct ? 'maccms' : 'stremio') && ready(source) && source.origin.config_id === identity.config_id);
  if (!metadata) { messages.error(t("This title is unavailable in the current catalog.")); return; }
  const service = appState.settings.playback_service?.enabled === true;
  const playback = providers.find(source => source.id === record.source_id && ready(source));
  if (!service && !playback) { messages.error(t("The saved playback source is unavailable. Enable it in Sources before continuing.")); return; }
  try {
    if (!direct) await appController.selectSource(metadata.id, "metadata");
    if (!service && playback) await appController.selectSource(playback.id, "playback");
    if (direct && metadata.id !== playback?.id) throw new Error(t('This title is unavailable in the current catalog.'));
    const ref: Bridge_MediaRef = { source_id: metadata.id, source_revision: metadata.source_revision, data: JSON.stringify(direct ? ['maccms', identity.meta_id] : ["stremio", identity.addon_key, identity.type, identity.meta_id]) };
    wantedResume = record.video_id ? { video_id: record.video_id, position_ms: record.completed ? 0 : record.position_ms } : null;
    const target = `#/details/${encodeURIComponent(JSON.stringify(ref))}`;
    if (location.hash === target && appState.details.phase === 'ready') {
      if (wantedResume) await playEpisode(wantedResume.video_id);
    } else location.hash = target;
  } catch (error) { preferenceError.value = String(error); }
}
watch(() => appState.route, route => {
  if (route.name === "playback" && wantedResume && route.context.metadata_video_id === wantedResume.video_id) {
    pendingResume.value = { context: route.context, position_ms: wantedResume.position_ms, was_playing: true };
    wantedResume = null;
  }
}, { flush: "sync" });
function continueHistory(entry: HistoryEntry) {
  // Local files resume on the Local video page, which owns their playback session.
  if (native && entry.record?.metadata_identity.namespace === "local") { location.hash = `#/local?play=${entry.record.video_id}`; return; }
  if (native && entry.record) { void continueNativeHistory(entry.record); return; }
  const film = baseFilms.find(film => historyContentId(film.kind, film.id) === entry.content_id);
  if (film) location.hash = `#/watch/${film.id}/${entry.episode}?resume=1`;
  else messages.error(t("This title is unavailable in the current catalog."));
}
function onEnded(context: PlaybackContext, epoch: number) {
  if (epoch !== appState.playback.playback_epoch) return;
  void flushHistory(true);
  latestHistory = null;
  // PlaybackView owns the cancellable EOS countdown; playEpisode saves before rematching.
}
// load/stop reset position with the epoch, so the old episode's last snapshot is kept to commit on switch.
let latestHistory: HistoryEntry | null = null;
watch(() => [appState.playback.position_ms, appState.playback.phase], () => {
  latestHistory = historySnapshot() ?? latestHistory;
  if (historyWriteDue(appState.playback.phase, lastWrite, Date.now())) void flushHistory();
});
watch(() => appState.playback.playback_epoch, () => {
  const snapshot = latestHistory;
  latestHistory = null;
  if (snapshot && snapshot !== lastHistory) { lastWrite = Date.now(); lastHistory = snapshot; void persistHistory(snapshot); }
});
function onPageHide() { void flushHistory(); }
// Audio saved on a short timer and the window's last size go in with everything else still queued.
async function saveLastSettings() {
  const bounds = await nativeWindowBounds().catch(() => null);
  const { volume, muted } = appState.playback, saved = appState.settings;
  if (volume !== saved.volume || muted !== saved.muted || (bounds && JSON.stringify(bounds) !== JSON.stringify(saved.window_bounds)))
    await appController.updateSettings(settings => ({ ...settings, volume, muted, window_bounds: bounds ?? settings.window_bounds }));
}
let boundsTimer: ReturnType<typeof setTimeout> | undefined;
function onResize() {
  clearTimeout(boundsTimer);
  boundsTimer = setTimeout(async () => {
    const bounds = await nativeWindowBounds().catch(() => null);
    if (bounds && JSON.stringify(bounds) !== JSON.stringify(appState.settings.window_bounds))
      void appController.updateSettings(settings => ({ ...settings, window_bounds: bounds })).catch(error => { preferenceError.value = String(error); });
  }, 500);
}
async function onClosing() {
  const core = window.__TAURI__?.core;
  // From here the Host waits for this page, however long the saves take.
  await core?.invoke("app_close_started", {}).catch(() => {});
  try {
    await finishBeforeClose({ saveHistory: async () => { await flushHistory(); await historyRepository.drain(); }, saveSettings: saveLastSettings,
      settings: settingsQueue, release: async () => core?.invoke("app_close_ready", {}), abort: async () => core?.invoke("app_close_aborted", {}) });
    closeFailed.value = false;
  } catch (error) { closeFailed.value = true; preferenceError.value = error instanceof Error ? error.message : String(error); }
}
// The user's explicit choice to leave without the unsaved settings.
const closeFailed = ref(false);
function quitWithoutSaving() { void window.__TAURI__?.core.invoke("app_close_ready", {});
}
function onVisibilityChange() { if (document.visibilityState === "hidden") void flushHistory(); }
const searchView = ref<InstanceType<typeof SearchView>>();
async function searchShortcut(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'f' && desktopQuery.matches) {
    event.preventDefault(); location.hash = '#/search';
    await nextTick(); searchView.value?.focus();
  }
}
watch(searchView, view => { if (view) view.focus(); });
onMounted(() => {
  window.addEventListener('keydown', searchShortcut, true);
  expandedQuery.addEventListener("change", resetSidebar);
  document.addEventListener("focusin", rememberFocus);
  // T12's formal harness supplies a Host-confirmed context before mounting App.
  if (readHash().name !== "playback") void onHashChange();
  window.addEventListener("hashchange", onHashChange);
  window.addEventListener("pagehide", onPageHide);
  window.addEventListener("moc-closing", onClosing);
  if (native) window.addEventListener("resize", onResize);
  document.addEventListener("visibilitychange", onVisibilityChange);
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', searchShortcut, true);
  expandedQuery.removeEventListener("change", resetSidebar);
  removeFocus?.();
  document.removeEventListener("focusin", rememberFocus);
  void flushHistory(); ++sequence;
  window.removeEventListener("hashchange", onHashChange);
  window.removeEventListener("pagehide", onPageHide);
  window.removeEventListener("moc-closing", onClosing);
  window.removeEventListener("resize", onResize);
  clearTimeout(boundsTimer);
  document.removeEventListener("visibilitychange", onVisibilityChange);
});
</script>

<template>
  <PlaybackView v-if="formalPlayback" ref="playbackView" :context="playbackContext" :resume="pendingResume" :auto-next="appState.settings.auto_next" @ended="onEnded" @play="playEpisode" @service-failed="switchFailedServiceLine" @seek="flushHistory()" @update:auto-next="autoNext = $event">
  <template #preferences><div class="playback-preferences">
    <a href="#/history">{{ t('History') }}</a>
    <button ref="paletteTrigger" class="icon-button palette-shortcut" :aria-label="t('Customize colors')" @click="appearanceOpen = true"><Icon name="palette" /></button>
    <select class="language-picker compact-select" :value="locale" :aria-label="t('App language')" @change="changeLanguage(($event.target as HTMLSelectElement).value)"><option v-for="item in languageOptions" :key="item.value" :value="item.value">{{ item.label }}</option></select>
    <select v-if="!native" class="compact-select playback-source" :value="sourceId" :aria-label="t('Choose a video source')" @change="changeSource(($event.target as HTMLSelectElement).value)"><option v-for="item in sources.filter(item => item.enabled && item.kind !== 'stremio')" :key="item.id" :value="item.id">{{ sourceText(item).name }}</option></select>
  </div></template>
  </PlaybackView>
  <template v-else>
  <a class="skip-link" href="#main-content" @click.prevent="focusMain">{{
    t("Skip to content")
  }}</a>
  <div ref="shell" class="app-shell" :class="{ 'tv-sidebar-collapsed': sidebarCollapsed }">
    <div class="tv-drag-region" @mousedown="dragWindow" />
    <aside class="tv-sidebar" :aria-label="t('Main navigation')">
      <button ref="sidebarToggle" class="tv-sidebar-toggle" :aria-expanded="!sidebarCollapsed" :aria-label="t(sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar')" :title="t(sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar')" @click="sidebarCollapsed = !sidebarCollapsed"><Icon name="sidebar" /></button>
      <nav class="tv-navigation" data-focus-row>
        <a v-for="item in desktopNavigation" :key="item.href" :href="item.href" :aria-label="item.label" :title="item.label" :aria-current="item.page === 'browse' ? hash.split('?')[0] === item.href ? 'page' : undefined : route.page === item.page ? 'page' : undefined"><Icon :name="item.icon" /><span>{{ item.label }}</span></a>
      </nav>
      <div ref="sidebarSources" class="tv-sidebar-sources" data-focus-row>
        <template v-if="sidebarCollapsed">
          <button :aria-label="t('Choose a metadata source')" :title="t('Choose a metadata source')" @click="expandSource('metadata')"><Icon name="library" /></button>
          <button v-if="!native" :aria-label="t('Choose a video source')" :title="t('Choose a video source')" @click="expandSource('playback')"><Icon name="layers" /></button>
        </template>
        <template v-else>
          <label :title="t('Metadata provider')"><Icon name="library" :size="16" /><select data-role="metadata" :aria-label="t('Choose a metadata source')" :value="appState.selected_metadata_source?.source_id ?? ''" @change="selectNativeSource(($event.target as HTMLSelectElement).value, 'metadata')"><option value="" disabled>{{ t('Choose a metadata source') }}</option><option v-for="item in metadataProviders" :key="item.id" :value="item.id">{{ item.name }}</option></select></label>
          <label v-if="!native" :title="t('Playback source')"><Icon name="layers" :size="16" /><select data-role="playback" :aria-label="t('Choose a video source')" :value="sourceId" @change="changeSource(($event.target as HTMLSelectElement).value)"><option v-for="item in sources.filter(item => item.enabled && item.kind !== 'stremio')" :key="item.id" :value="item.id">{{ sourceText(item).name }}</option></select></label>
        </template>
      </div>
    </aside>
    <aside class="sidebar legacy-sidebar" :aria-label="t('Main navigation')">
      <a class="brand" href="#/home" :aria-label="t('Moc Video home')"
        ><img src="/mark.svg" alt="" width="38" height="38" /><span
          >{{ locale === "en" ? "Moc" : t("Moc Video")
          }}<small>{{
            locale === "en" ? "VIDEO" : "MOC VIDEO"
          }}</small></span
        ></a
      >
      <p class="nav-eyebrow">{{ t("DISCOVER GOOD STORIES") }}</p>
      <nav class="primary-nav">
        <a
          v-for="item in nav"
          :key="item.href"
          :href="item.href"
          :class="{ active: route.page === 'home' && route.kind === item.kind }"
          :aria-current="
            route.page === 'home' && route.kind === item.kind
              ? 'page'
              : undefined
          "
          :title="item.label"
          ><Icon :name="item.icon" /><span>{{ item.label }}</span
          ><i v-if="route.page === 'home' && route.kind === item.kind"
        /></a>
      </nav>
      <div class="nav-divider" />
      <p class="nav-eyebrow">{{ t("YOUR SCREENING ROOM") }}</p>
      <nav class="utility-nav">
        <a href="#/history" :class="{ active: route.page === 'history' }" :aria-current="route.page === 'history' ? 'page' : undefined" :title="t('Watch history')"><Icon name="history" /><span>{{ t('Watch history') }}</span></a>
        <a
          href="#/settings"
          :class="{ active: route.page === 'settings' }"
          :aria-current="route.page === 'settings' ? 'page' : undefined"
          :title="t('Settings')"
          ><Icon name="settings" /><span>{{ t("Settings") }}</span></a
        >
      </nav>
      <div class="sidebar-bottom">
        <div class="sidebar-note">
          <span class="little-spark">✳</span>
          <p>{{ t("Make good stories") }}<br />{{ t("part of every day.") }}</p>
        </div>
        <button
          ref="aboutTrigger"
          class="preview-button"
          @click="aboutOpen = true"
        >
          <span class="status-dot" /><span>{{ t(native ? "About Moc Video" : "Interface preview") }}</span
          ><Icon name="info" :size="14" />
        </button>
        <p class="sidebar-version">{{ t("Moc Video · 0.1.0") }}</p>
      </div>
    </aside>

    <div class="app-main" :class="{ 'watch-layout': route.page === 'watch' }">
      <header class="topbar">
        <div class="page-location">
          <img
            class="mobile-mark"
            src="/mark.svg"
            width="30"
            height="30"
            :alt="t('Moc Video')"
          /><a
            v-if="route.page === 'film' || route.page === 'watch'"
            :href="
              route.page === 'watch' ? `#/film/${selectedFilm.id}` : '#/home'
            "
            class="icon-button"
            :aria-label="t('Back')"
            ><Icon name="back" /></a
          ><span>{{ title }}</span
          ><span class="topbar-divider" /><span class="topbar-note">{{
            t("Make time for a good story.")
          }}</span>
        </div>
        <div class="header-actions">
          <button
            ref="paletteTrigger"
            class="icon-button palette-shortcut"
            @click="appearanceOpen = true"
            :aria-label="t('Customize colors')"
            :title="t('Customize colors')"
          >
            <Icon name="palette" :size="19" />
          </button>
          <select
            class="language-picker compact-select"
            :value="locale"
            :aria-label="t('App language')"
            @change="changeLanguage(($event.target as HTMLSelectElement).value)"
          >
            <option
              v-for="item in languageOptions"
              :key="item.value"
              :value="item.value"
            >
              {{ item.label }}
            </option>
          </select>
          <label v-if="!native" class="source-select"
            ><span class="status-dot" /><select
              class="compact-select"
              :value="sourceId"
              :aria-label="t('Choose a video source')"
              @change="changeSource(($event.target as HTMLSelectElement).value)"
            >
              <option
                v-for="item in sources.filter((item) => item.enabled && item.kind !== 'stremio')"
                :key="item.id"
                :value="item.id"
              >
                {{ sourceText(item).name }}
              </option>
            </select></label
          >
          <a
            class="icon-button settings-shortcut"
            href="#/settings"
            :aria-label="t('Open settings')"
            ><Icon name="settings" :size="19"
          /></a>
        </div>
      </header>

      <main id="main-content" tabindex="-1" :class="{ 'tv-manage-main': route.page === 'settings', 'tv-library-main':['history','local','live'].includes(route.page), 'tv-search-main':route.page === 'search' }">
        <SearchView v-if="route.page === 'search'" ref="searchView" @open="openDetails" />
        <LocalView v-else-if="route.page === 'local'" />
        <LiveView v-else-if="route.page === 'live'" />
        <HomeView v-else-if="native && route.page === 'home'" @open="openDetails" @select="selectNativeSource" @resume="continueHistory" />
        <BrowseView v-else-if="native && route.page === 'browse'" :category="readHash().id" :query="hash.split('?')[1] ?? ''" @open="openDetails" />
        <div v-else-if="native && route.page === 'film'" :class="{ 'maccms-details': maccmsDetailsSource }" :style="maccmsDetailsStyle">
          <DetailsView @open="openDetails" @play="playEpisode" @resume="continueNativeHistory" @back="backFromDetails" />
          <span v-if="maccmsDetailsSource" class="maccms-accessible-source">{{ maccmsDetailsSource.name }}</span>
        </div>
        <template v-else-if="route.page === 'home'">
          <div class="page-heading">
            <div>
              <p class="eyebrow">A LITTLE ESCAPE, EVERY DAY</p>
              <h1>
                {{
                  route.kind
                    ? t("Take your time with good {kind}.", {
                        kind: t(kindLabels[route.kind]),
                      })
                    : t("What will you watch tonight?")
                }}
              </h1>
            </div>
            <span class="heading-caption">{{
              t("Find a story for the way you feel.")
            }}</span>
          </div>
          <section class="feature" :aria-label="t('Editor\'s pick')">
            <img
              :key="hero.id"
              class="feature-image"
              :src="hero.artwork"
              alt=""
              fetchpriority="high"
              :style="{ objectPosition: hero.position || 'center' }"
              width="1659"
              height="948"
            />
            <div class="feature-gradient" />
            <div class="feature-content">
              <span class="feature-kicker"
                ><span />{{ t("Editor's pick")
                }}<template v-if="locale === 'zh-CN'"
                  ><i /> EDITOR'S PICK</template
                ></span
              >
              <p v-if="locale === 'zh-CN'" class="feature-english">
                {{ hero.english }}
              </p>
              <h2>{{ hero.title }}</h2>
              <div class="film-meta">
                <span>{{ hero.year }}</span
                ><span>{{
                  hero.genres.map((genre) => t(genre as Message)).join(" / ")
                }}</span
                ><span>{{ hero.runtime }}</span>
              </div>
              <p class="feature-note">{{ hero.note }}</p>
              <div class="feature-actions">
                <a :href="`#/watch/${hero.id}`" class="button button-primary"
                  ><Icon name="play" :size="17" />{{ t("Play now") }}</a
                ><a :href="`#/film/${hero.id}`" class="button button-glass"
                  >{{ t("Film details") }}<Icon name="chevron" :size="15"
                /></a>
              </div>
            </div>
            <div class="feature-footer">
              <span class="feature-tag">{{
                t("A journey of departures and new beginnings")
              }}</span>
              <div class="feature-pagination">
                <span
                  ><b>{{
                    String((heroIndex % heroFilms.length) + 1).padStart(2, "0")
                  }}</b>
                  / {{ String(heroFilms.length).padStart(2, "0") }}</span
                ><button
                  class="icon-button"
                  :disabled="heroFilms.length < 2"
                  :aria-label="t('Previous feature')"
                  @click="
                    heroIndex =
                      (heroIndex + heroFilms.length - 1) % heroFilms.length
                  "
                >
                  <Icon name="chevron" class="flipped" :size="17" /></button
                ><button
                  class="icon-button"
                  :disabled="heroFilms.length < 2"
                  :aria-label="t('Next feature')"
                  @click="heroIndex = (heroIndex + 1) % heroFilms.length"
                >
                  <Icon name="chevron" :size="17" />
                </button>
              </div>
            </div>
          </section>

          <section class="catalog-section" aria-labelledby="catalog-title">
            <div class="section-heading">
              <div>
                <h2 id="catalog-title">
                  {{
                    route.kind
                      ? t("The {kind} room", {
                          kind: t(kindLabels[route.kind]),
                        })
                      : t("Selected for you")
                  }}<span class="heading-dot" />
                </h2>
                <p>
                  {{
                    t("Start your next screening with a story worth seeing.")
                  }}
                </p>
              </div>
              <button
                v-if="filteredFilms.length > 6"
                class="text-button"
                @click="expanded = !expanded"
              >
                {{ expanded ? t("Show fewer") : t("All titles")
                }}<Icon :name="expanded ? 'down' : 'arrow'" :size="16" />
              </button>
            </div>
            <div class="genre-bar" :aria-label="t('Filter by genre')">
              <button
                v-for="genre in genres"
                :key="genre"
                :class="{ selected: activeGenre === genre }"
                :aria-pressed="activeGenre === genre"
                @click="
                  activeGenre = genre;
                  expanded = false;
                "
              >
                {{ t(genre as Message) }}</button
              ><span class="catalog-count">{{
                t("{count} titles", { count: filteredFilms.length })
              }}</span>
            </div>
            <div
              v-if="loading"
              class="film-grid"
              aria-busy="true"
              :aria-label="t('Switching demo catalog')"
            >
              <div v-for="n in 6" :key="n" class="skeleton-card">
                <div />
                <span />
              </div>
            </div>
            <div v-else-if="visibleFilms.length" class="film-grid">
              <FilmCard
                v-for="(film, index) in visibleFilms"
                :key="film.id"
                :film="film"
                :index="index"
              />
            </div>
            <div v-else class="empty-state">
              <Icon name="film" :size="34" />
              <h3>{{ t("A new story is on its way") }}</h3>
              <p>
                {{
                  t(
                    "There are no {genre} titles in this demo catalog yet. Try another genre.",
                    { genre: t(activeGenre) },
                  )
                }}
              </p>
              <button
                class="button button-secondary"
                @click="activeGenre = 'All'"
              >
                {{ t("View all titles") }}
              </button>
            </div>
          </section>
          <section
            v-if="!route.kind && activeGenre === 'All'"
            class="mood-banner"
          >
            <div>
              <span class="eyebrow">A DIFFERENT PERSPECTIVE</span>
              <h2>{{ t("See the world a little differently.") }}</h2>
              <p>{{ t("Step outside the everyday and into a new story.") }}</p>
            </div>
            <a href="#/kind/anime" class="button button-secondary"
              >{{ t("Explore animation") }}<Icon name="arrow" :size="17"
            /></a>
            <div class="mood-orbits" aria-hidden="true">
              <i /><i /><i /><span>✳</span>
            </div>
          </section>
        </template>

        <template v-else-if="route.page === 'film'">
          <section class="detail-hero">
            <img class="detail-background" :src="selectedFilm.artwork" alt="" />
            <div class="detail-shade" />
            <div class="detail-poster">
              <img
                :src="selectedFilm.artwork"
                :alt="
                  t('Illustrative poster for {title}', {
                    title: selectedFilm.title,
                  })
                "
                :style="{ objectPosition: selectedFilm.position || 'center' }"
              /><span>{{ selectedFilm.title }}</span>
            </div>
            <div class="detail-copy">
              <p v-if="locale === 'zh-CN'" class="eyebrow">
                {{ selectedFilm.english }}
              </p>
              <h1>{{ selectedFilm.title }}</h1>
              <div class="film-meta">
                <span>{{ selectedFilm.year }}</span
                ><span>{{ t(kindLabels[selectedFilm.kind]) }}</span
                ><span>{{ selectedFilm.runtime }}</span>
              </div>
              <div class="detail-tags">
                <span v-for="genre in selectedFilm.genres" :key="genre">{{
                  t(genre as Message)
                }}</span
                ><span v-if="selectedFilm.episodes > 1">{{
                  t("{count} episodes", { count: selectedFilm.episodes })
                }}</span>
              </div>
              <p class="detail-plot">{{ selectedFilm.plot }}</p>
              <dl class="credits">
                <div>
                  <dt>{{ t("Director") }}</dt>
                  <dd>{{ selectedFilm.director }}</dd>
                </div>
                <div>
                  <dt>{{ t("Cast") }}</dt>
                  <dd>{{ selectedFilm.cast.join(" / ") }}</dd>
                </div>
              </dl>
              <a
                :href="`#/watch/${selectedFilm.id}`"
                class="button button-primary"
                ><Icon name="play" :size="17" />{{
                  selectedFilm.episodes > 1
                    ? t("Start episode 1")
                    : t("Play now")
                }}</a
              >
            </div>
          </section>
          <section class="episodes-section">
            <div class="section-heading">
              <div>
                <h2>
                  {{
                    t("Choose {kind}", {
                      kind: t(
                        selectedFilm.episodes > 1 ? "episodes" : "a video",
                      ),
                    })
                  }}
                </h2>
                <p>
                  {{
                    t("Source: {name} · Local demo clip", {
                      name: sourceText(source).name,
                    })
                  }}
                </p>
              </div>
              <span class="subtle-label">{{
                selectedFilm.episodes > 1
                  ? t("{count} episodes in total", {
                      count: selectedFilm.episodes,
                    })
                  : t("1 feature")
              }}</span>
            </div>
            <div class="episode-grid">
              <a
                v-for="n in selectedFilm.episodes"
                :key="n"
                :href="`#/watch/${selectedFilm.id}/${n}`"
                class="episode-link"
                ><Icon
                  v-if="selectedFilm.episodes === 1"
                  name="play"
                  :size="15"
                />{{
                  selectedFilm.episodes > 1
                    ? String(n).padStart(2, "0")
                    : t("Feature")
                }}</a
              >
            </div>
          </section>
          <section class="related-section">
            <div class="section-heading">
              <div>
                <h2>{{ t("You might also like") }}</h2>
                <p>{{ t("Something to look forward to next.") }}</p>
              </div>
            </div>
            <div class="film-grid related-grid">
              <FilmCard
                v-for="film in relatedFilms"
                :key="film.id"
                :film="film"
              />
            </div>
          </section>
        </template>

        <HistoryView v-else-if="route.page === 'history'" :save-error="historySaveError" @open="openDetails" @resume="continueHistory" @retry-save="retryHistorySave" @removed="discardPendingHistory" />
        <SettingsView
          v-else-if="route.page === 'settings'"
          v-model:speed="speed"
          v-model:auto-next="autoNext"
          :preview="!native"
          @notice="notify"
          @language="changeLanguage"
          @palette="changePalette"
          @select="selectNativeSource"
        />
        <footer class="page-footer">
          <span
            >{{ t("Moc Video") }}<span class="footer-cross">✳</span
            >{{ t("Let stories happen") }}</span
          ><span>{{
            t(native ? "A place for good stories." : "Interface preview · Original fictional content")
          }}</span>
        </footer>
      </main>
    </div>
    <nav class="mobile-nav" :aria-label="t('Mobile navigation')">
      <a
        v-for="item in nav"
        :key="item.href"
        :href="item.href"
        :class="{ active: route.page === 'home' && route.kind === item.kind }"
        :aria-current="
          route.page === 'home' && route.kind === item.kind ? 'page' : undefined
        "
        ><Icon :name="item.icon" :size="20" /><span>{{ item.label }}</span></a
      ><a href="#/local" :class="{ active: route.page === 'local' }" :aria-current="route.page === 'local' ? 'page' : undefined"><Icon name="folder" :size="20" /><span>{{ t('Local video') }}</span></a
      ><a v-if="liveAvailable(appState.settings)" href="#/live" :class="{ active: route.page === 'live' }" :aria-current="route.page === 'live' ? 'page' : undefined"><Icon name="broadcast" :size="20" /><span>{{ t('Live') }}</span></a
      ><a href="#/history" :class="{ active: route.page === 'history' }" :aria-current="route.page === 'history' ? 'page' : undefined"><Icon name="history" :size="20" /><span>{{ t('History') }}</span></a><a href="#/settings" :class="{ active: route.page === 'settings' }" :aria-current="route.page === 'settings' ? 'page' : undefined"
        ><Icon name="settings" :size="20" /><span>{{ t("Settings") }}</span></a
      >
    </nav>
    <NModal
      v-model:show="aboutOpen"
      class="tv-dialog"
      preset="card"
      role="dialog"
      :aria-label="t('Welcome to Moc Video')"
      :title="t('Welcome to Moc Video')"
      transform-origin="center"
      :style="{ width: 'min(480px, calc(100vw - 32px))' }"
      @after-leave="aboutTrigger?.focus()"
    >
      <p class="dialog-description">
        {{
          t(
            native ? "A place for good stories." : "This is an interactive interface preview. Films, credits and synopses are fictional, artwork is AI-generated, and the player uses a local 30-second test clip. Explore details, episodes, subtitles, playback preferences and source management.",
          )
        }}
      </p>
      <NButton type="primary" @click="aboutOpen = false">{{
        t("Start exploring")
      }}</NButton>
    </NModal>
  </div>
  </template>
  <NAlert v-if="preferenceError" type="error" class="history-alert preference-alert" :title="t('Preference could not be saved')">{{ preferenceError }}<NButton @click="retryPreferences">{{ t('Retry') }}</NButton><NButton v-if="closeFailed" class="quit-without-saving" @click="quitWithoutSaving">{{ locale === 'zh-CN' ? '不保存并退出' : 'Quit without saving' }}</NButton></NAlert>
  <NAlert v-for="(error, index) in appController.diagnostics" :key="index" type="error" class="history-alert" :title="t('Action failed')">{{ error.message }}<NButton @click="retryPreferences">{{ t('Retry') }}</NButton></NAlert>
  <NModal v-model:show="appearanceOpen" class="tv-dialog" preset="card" role="dialog" :aria-label="t('Appearance')" :title="t('Appearance')" :style="{ width: 'min(480px, calc(100vw - 32px))' }" @after-leave="paletteTrigger?.focus()">
    <p>{{ t('Choose your colors. Each palette adapts to light and dark mode.') }}</p>
    <NRadioGroup :value="palette" name="quick-color-palette" :aria-label="t('Color palette')" class="quick-palette" @update:value="value => changePalette(String(value))"><NRadioButton v-for="item in paletteOptions" :key="item.value" :value="item.value">{{ t(item.label) }}</NRadioButton></NRadioGroup>
    <NButton class="appearance-close" @click="appearanceOpen = false">{{ t('Close') }}</NButton>
  </NModal>
</template>
