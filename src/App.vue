<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { NAlert, NButton, NModal, NRadioGroup, NRadioButton, useMessage } from "naive-ui";
import SettingsView from "./views/SettingsView.vue";
import HistoryView from "./views/HistoryView.vue";
import SourcesView from "./views/SourcesView.vue";
import PlaybackView from "./views/PlaybackView.vue";
import NativeSourcesView from "./views/NativeSourcesView.vue";
import HomeView from "./views/HomeView.vue";
import DetailsView from "./views/DetailsView.vue";
import Icon from "./components/Icon.vue";
import FilmCard from "./components/FilmCard.vue";
import { appState, appController, metadataVideoId } from "./state.ts";
import { historyRepository, historyWriteDue } from "./data/history";
import { historyContentId, type HistoryEntry } from "./data/history-model";
import { films as baseFilms, localizeFilm, sourceText, initialSources, kindLabels, type FilmKind, type Source } from "./data/demo";
import { previewRef, previewSourceId, previewSourceKey } from "./data/preview";
import { locale, t, type Message } from "./i18n";
import { palette, paletteOptions, normalizePalette } from "./ui/theme";
import type { AppSource, Bridge_MediaRef, PlaybackContext, ResumeSnapshot, Settings, WatchHistoryRecord } from "./models";

const native = Boolean(window.__TAURI__);
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
type Page = "home" | "film" | "watch" | "sources" | "setup" | "settings" | "history";
const route = computed<{ page: Page; id: string; kind: FilmKind | "" }>(() => {
  const stored = appState.route, view = readHash();
  const page: Page = stored.name === "details" ? "film" : stored.name === "playback" ? "watch" : stored.name;
  return { page,
    id: view.id, kind: view.name === "kind" && ["movie", "series", "anime"].includes(view.id) ? view.id as FilmKind : "" };
});
// A rematch that needs the user keeps hash #/playback; its confirmation/result UI lives in CatalogView.
const matchNeedsUser = computed(() => ["needs_confirmation", "not_found"].includes(appController.matching.result?.status ?? ""));
const formalPlayback = computed(() => appState.route.name === "playback" || (readHash().name === "playback" && !matchNeedsUser.value));
const playbackContext = computed(() => appState.route.name === "playback" ? appState.route.context : null);
const playbackView = ref<InstanceType<typeof PlaybackView>>();
defineExpose({ playbackController: () => playbackView.value?.playback });
const previewSources = ref<Source[]>(initialSources.map(source => ({ ...source })));
const sources = computed<Source[]>(() => native ? (appState.sources.data?.providers ?? []).map(source => ({
  id: source.id, name: source.name, kind: source.kind, description: source.error?.message ?? "", enabled: source.runtime_status === "ready", demo: false,
})) : previewSources.value);
const sourceId = computed(() => native ? appState.selected_playback_source?.source_id ?? "" : previewSourceId(appState.selected_playback_source));
const source = computed(() => sources.value.find(item => item.id === sourceId.value) ?? sources.value[0] ?? initialSources[0]!);
const selectedFilm = computed(() => films.value.find(film => film.id === route.value.id) ?? films.value[0]!);
const activeGenre = ref<Message>("All"), expanded = ref(false), heroIndex = ref(0);
const homeSnapshots = new Map<string, { genre: Message; expanded: boolean; hero: number; scroll: number }>();
const homeKey = () => `${appState.selected_metadata_source?.source_id}:${appState.selected_metadata_source?.source_revision}:${route.value.kind}`;
function rememberHome() { if (route.value.page === "home") homeSnapshots.set(homeKey(), { genre: activeGenre.value, expanded: expanded.value, hero: heroIndex.value, scroll: scrollY }); }
const heroFilms = computed(() => route.value.kind ? films.value.filter(film => film.kind === route.value.kind) : [films.value[0]!, films.value[2]!, films.value[3]!]);
const hero = computed(() => heroFilms.value[heroIndex.value % heroFilms.value.length]!);
const title = computed(() => route.value.kind ? t(kindLabels[route.value.kind as FilmKind]) : t(({
  home: "Discover", film: "Film details", watch: "Now playing", sources: "Source manager", setup: "Source manager", settings: "Settings", history: "Watch history",
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
  document.title = `${formalPlayback.value ? t("Now playing") : route.value.page === "film" ? native ? appState.details.data?.details.title ?? title.value : selectedFilm.value.title : title.value} · ${t("Yingxiang Video")}`;
}, { immediate: true });
async function savePreference(change: (value: Settings) => Settings) {
  preferenceError.value = "";
  try {
    const confirmed = await appController.updateSettings(change);
    // Legacy keys are first-paint mirrors. The full Settings queue owns commits.
    localStorage.setItem("yingxiang-video.locale", confirmed.ui_locale ?? "en");
    localStorage.setItem("yingxiang-video.palette", confirmed.color_palette ?? "lime");
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
      if (location.hash === "#/home") await appController.loadHome();
      else location.hash = "#/home";
    }
  } catch (error) { preferenceError.value = String(error); }
}
async function changeSource(id: string) {
  try { await selectNativeSource(native ? id : previewSourceKey(id).source_id, "playback"); }
  catch (cause) { preferenceError.value = String(cause); }
}
function addSource(value: Source) { previewSources.value.push(value); notify(t("Source added to this preview. Live connections will follow.")); }
function toggleSource(item: Source) { if (item.id === sourceId.value) notify(t("Switch to another source before disabling the current one.")); else item.enabled = !item.enabled; }
function openDetails(ref: Bridge_MediaRef) { location.hash = `#/details/${encodeURIComponent(JSON.stringify(ref))}`; }
async function playEpisode(videoId: string) {
  try {
    await flushHistory();
    const result = await appController.match(videoId);
    if (result?.status === "matched") location.hash = "#/playback";
  } catch (error) { preferenceError.value = String(error); }
}
const pendingResume = ref<ResumeSnapshot | null>(null);
let sequence = 0;
function focusMain() { document.getElementById("main-content")?.focus({ preventScroll: true }); }
async function onHashChange() {
  const current = ++sequence;
  await flushHistory();
  if (current !== sequence) return;
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
      // The first-run setup entry and the source manager are one native page.
      const name = next.name === "setup" ? "sources" : ["settings", "history", "sources"].includes(next.name) ? next.name as "settings" | "history" | "sources" : "home";
      appController.navigate({ name });
      if (name === "home" && !appState.home.data && appState.selected_metadata_source) await appController.loadHome();
    }
    if (current !== sequence) return;
    const home = route.value.page === "home" ? homeSnapshots.get(homeKey()) : undefined;
    if (route.value.page === "home") { activeGenre.value = home?.genre ?? "All"; expanded.value = home?.expanded ?? false; heroIndex.value = home?.hero ?? 0; }
    await nextTick();
    if (current !== sequence) return;
    scrollTo({ top: home?.scroll ?? 0, behavior: "instant" }); focusMain();
  } catch (error) { preferenceError.value = error instanceof Error ? error.message : String(error); }
}
let lastWrite = 0;
let lastHistory: HistoryEntry | null = null;
function nativeHistorySnapshot(finished: boolean): HistoryEntry | null {
  const state = appState.playback, context = state.context, data = appState.details.data;
  if (!context || !data || state.position_ms <= 0 || state.phase === "failed") return null;
  const episodes = data.details.groups.flatMap(group => group.episodes);
  const index = episodes.findIndex(episode => metadataVideoId(episode.ref) === context.metadata_video_id);
  const provider = appState.sources.data?.providers.find(item => item.id === context.episode_ref.source_id);
  if (index < 0 || !provider) return null;
  const session = state.session;
  if (!session) return null;
  const kind = data.details.kind === "other" ? "series" : data.details.kind;
  const poster = data.details.poster ? data.images[data.details.poster.resource_id]?.url : undefined;
  const episode = episodes[index]!, position_ms = Math.round(state.duration_ms ? Math.min(state.duration_ms, state.position_ms) : state.position_ms);
  const duration_ms = state.duration_ms ? Math.max(1, Math.round(state.duration_ms)) : null;
  const completed = finished || state.phase === "ended", updated_at = Date.now();
  return { content_id: data.details.id, title: data.details.title, poster: poster ?? "/mark.svg", kind,
    episode: index + 1, episode_total: episodes.length, source_id: provider.id, source_name: provider.name,
    position_ms, duration_ms, completed, updated_at,
    record: { content_id: data.details.id, metadata_identity: context.metadata_ref.identity, video_id: context.metadata_video_id, season_number: episode.season ?? null,
      episode_number: episode.number ?? index + 1, title: data.details.title, kind: data.details.kind, poster_cache_key: null, source_id: provider.id, source_name: provider.name,
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
async function persistHistory(snapshot: HistoryEntry) {
  try { await historyRepository.save(snapshot); historySaveError.value = null; }
  catch (error) { historySaveError.value = String(error); }
}
async function flushHistory(finished = false) {
  const snapshot = historySnapshot(finished);
  if (snapshot) { lastWrite = Date.now(); lastHistory = snapshot; await persistHistory(snapshot); }
}
function retryHistorySave() { if (lastHistory) void persistHistory(lastHistory); }
function discardPendingHistory(id: string | null) {
  if (id === null || lastHistory?.content_id === id) { lastHistory = null; historySaveError.value = null; }
  if (id === null || latestHistory?.content_id === id) latestHistory = null;
}
// Resuming goes back through the source and match checks; the position waits for the confirmed episode.
let wantedResume: { video_id: string; position_ms: number } | null = null;
async function continueNativeHistory(record: WatchHistoryRecord) {
  const providers = appState.sources.data?.providers ?? [], identity = record.metadata_identity;
  const ready = (source: AppSource) => source.runtime_status === "ready";
  const metadata = providers.find(source => source.kind === "stremio" && ready(source) && source.origin.config_id === identity.config_id);
  if (!metadata) { messages.error(t("This title is unavailable in the current catalog.")); return; }
  const playback = providers.find(source => source.id === record.source_id && ready(source));
  if (!playback) { messages.error(t("The saved playback source is unavailable. Enable it in Sources before continuing.")); return; }
  try {
    await appController.selectSource(metadata.id, "metadata");
    await appController.selectSource(playback.id, "playback");
    const ref: Bridge_MediaRef = { source_id: metadata.id, source_revision: metadata.source_revision, data: JSON.stringify(["stremio", identity.addon_key, identity.type, identity.meta_id]) };
    wantedResume = record.video_id ? { video_id: record.video_id, position_ms: record.completed ? 0 : record.position_ms } : null;
    location.hash = `#/details/${encodeURIComponent(JSON.stringify(ref))}`;
  } catch (error) { preferenceError.value = String(error); }
}
watch(() => appState.route, route => {
  if (route.name === "playback" && wantedResume && route.context.metadata_video_id === wantedResume.video_id) {
    pendingResume.value = { context: route.context, position_ms: wantedResume.position_ms, was_playing: true };
    wantedResume = null;
  }
}, { flush: "sync" });
function continueHistory(entry: HistoryEntry) {
  if (native && entry.record) { void continueNativeHistory(entry.record); return; }
  const film = baseFilms.find(film => historyContentId(film.kind, film.id) === entry.content_id);
  if (film) location.hash = `#/watch/${film.id}/${entry.episode}?resume=1`;
  else messages.error(t("This title is unavailable in the current catalog."));
}
function onEnded(context: PlaybackContext, epoch: number) {
  if (epoch !== appState.playback.playback_epoch) return;
  const saved = flushHistory(true);
  latestHistory = null;
  // Only a real EOS of the current session reaches here, so errors and manual stops keep the episode.
  if (native && appState.settings.auto_next) {
    const next = appController.nextVideo(context.metadata_video_id);
    if (next) void saved.then(() => playEpisode(next));
  }
  if (!native && appState.settings.auto_next) {
    const film = baseFilms.find(film => film.id === context.metadata_ref.identity.meta_id);
    const episode = Number(context.metadata_video_id.split(":").at(-1));
    if (film && episode < film.episodes) location.hash = `#/watch/${film.id}/${episode + 1}`;
  }
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
// The Host waits briefly while the window closes; saved or failed, the page then lets it go.
async function onClosing() {
  try { await flushHistory(); await historyRepository.drain(); }
  finally { await window.__TAURI__?.core.invoke("app_close_ready", {}).catch(() => {}); }
}
function onVisibilityChange() { if (document.visibilityState === "hidden") void flushHistory(); }
onMounted(() => {
  // T12's formal harness supplies a Host-confirmed context before mounting App.
  if (readHash().name !== "playback") void onHashChange();
  window.addEventListener("hashchange", onHashChange);
  window.addEventListener("pagehide", onPageHide);
  window.addEventListener("yingxiang-closing", onClosing);
  document.addEventListener("visibilitychange", onVisibilityChange);
});
onBeforeUnmount(() => {
  void flushHistory(); ++sequence;
  window.removeEventListener("hashchange", onHashChange);
  window.removeEventListener("pagehide", onPageHide);
  window.removeEventListener("yingxiang-closing", onClosing);
  document.removeEventListener("visibilitychange", onVisibilityChange);
});
</script>

<template>
  <div v-if="formalPlayback" class="playback-preferences">
    <a href="#/history">{{ t('History') }}</a>
    <button ref="paletteTrigger" class="icon-button palette-shortcut" :aria-label="t('Customize colors')" @click="appearanceOpen = true"><Icon name="palette" /></button>
    <select class="language-picker compact-select" :value="locale" :aria-label="t('App language')" @change="changeLanguage(($event.target as HTMLSelectElement).value)"><option v-for="item in languageOptions" :key="item.value" :value="item.value">{{ item.label }}</option></select>
    <select class="compact-select playback-source" :value="sourceId" :aria-label="t('Choose a video source')" @change="changeSource(($event.target as HTMLSelectElement).value)"><option v-for="item in sources.filter(item => item.enabled && item.kind !== 'stremio')" :key="item.id" :value="item.id">{{ sourceText(item).name }}</option></select>
  </div>
  <PlaybackView v-if="formalPlayback" ref="playbackView" :context="playbackContext" :resume="pendingResume" :auto-next="appState.settings.auto_next" @ended="onEnded" @play="playEpisode" @seek="flushHistory()" @update:auto-next="autoNext = $event" />
  <template v-else>
  <a class="skip-link" href="#main-content" @click.prevent="focusMain">{{
    t("Skip to content")
  }}</a>
  <div class="app-shell">
    <aside class="sidebar" :aria-label="t('Main navigation')">
      <a class="brand" href="#/home" :aria-label="t('Yingxiang Video home')"
        ><img src="/mark.svg" alt="" width="38" height="38" /><span
          >{{ locale === "en" ? "Yingxiang" : t("Yingxiang Video")
          }}<small>{{
            locale === "en" ? "VIDEO" : "YINGXIANG VIDEO"
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
          href="#/sources"
          :class="{ active: route.page === 'sources' }"
          :aria-current="route.page === 'sources' ? 'page' : undefined"
          :title="t('Source manager')"
          ><Icon name="layers" /><span>{{ t("Source manager") }}</span></a
        >
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
          <span class="status-dot" /><span>{{ t(native ? "About Yingxiang Video" : "Interface preview") }}</span
          ><Icon name="info" :size="14" />
        </button>
        <p class="sidebar-version">{{ t("Yingxiang Video · 0.1.0") }}</p>
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
            :alt="t('Yingxiang Video')"
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
          <label class="source-select"
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

      <main id="main-content" tabindex="-1">
        <HomeView v-if="native && route.page === 'home'" @open="openDetails" @select="selectNativeSource" />
        <DetailsView v-else-if="native && route.page === 'film'" @open="openDetails" @play="playEpisode" />
        <NativeSourcesView v-else-if="native && route.page === 'sources'" @select="selectNativeSource" />
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

        <HistoryView v-else-if="route.page === 'history'" :save-error="historySaveError" @resume="continueHistory" @retry-save="retryHistorySave" @removed="discardPendingHistory" />
        <SourcesView
          v-else-if="route.page === 'sources'"
          :sources="sources"
          :selected-source-id="sourceId"
          @select="changeSource"
          @toggle="toggleSource"
          @add="addSource"
        />
        <SettingsView
          v-else-if="route.page === 'settings'"
          v-model:speed="speed"
          v-model:auto-next="autoNext"
          :preview="!native"
          @notice="notify"
          @language="changeLanguage"
          @palette="changePalette"
        />
        <footer class="page-footer">
          <span
            >{{ t("Yingxiang Video") }}<span class="footer-cross">✳</span
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
      ><a href="#/history" :class="{ active: route.page === 'history' }" :aria-current="route.page === 'history' ? 'page' : undefined"><Icon name="history" :size="20" /><span>{{ t('History') }}</span></a><a href="#/sources" :class="{ active: route.page === 'sources' }"
        ><Icon name="layers" :size="20" /><span>{{ t("Sources") }}</span></a
      >
    </nav>
    <NModal
      v-model:show="aboutOpen"
      preset="card"
      role="dialog"
      :aria-label="t('Welcome to Yingxiang Video')"
      :title="t('Welcome to Yingxiang Video')"
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
  <NAlert v-if="preferenceError" type="error" class="history-alert preference-alert" :title="t('Preference could not be saved')">{{ preferenceError }}<NButton @click="retryPreferences">{{ t('Retry') }}</NButton></NAlert>
  <NAlert v-for="(error, index) in appController.diagnostics" :key="index" type="error" class="history-alert" :title="t('Action failed')">{{ error.message }}<NButton @click="retryPreferences">{{ t('Retry') }}</NButton></NAlert>
  <NModal v-model:show="appearanceOpen" preset="card" role="dialog" :aria-label="t('Appearance')" :title="t('Appearance')" :style="{ width: 'min(480px, calc(100vw - 32px))' }" @after-leave="paletteTrigger?.focus()">
    <p>{{ t('Choose your colors. Each palette adapts to light and dark mode.') }}</p>
    <NRadioGroup :value="palette" name="quick-color-palette" :aria-label="t('Color palette')" class="quick-palette" @update:value="value => changePalette(String(value))"><NRadioButton v-for="item in paletteOptions" :key="item.value" :value="item.value">{{ t(item.label) }}</NRadioButton></NRadioGroup>
    <NButton class="appearance-close" @click="appearanceOpen = false">{{ t('Close') }}</NButton>
  </NModal>
</template>
