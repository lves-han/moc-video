<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { NAlert, NButton, NModal, NSwitch, useMessage } from "naive-ui";
import SettingsView from "./views/SettingsView.vue";
import HistoryView from "./views/HistoryView.vue";
import { historyRepository } from "./data/history";
import { historyContentId, type HistoryEntry } from "./data/history-model";
import SourcesView from "./views/SourcesView.vue";
import Icon from "./components/Icon.vue";
import FilmCard from "./components/FilmCard.vue";
import {
  films as baseFilms,
  localizeFilm,
  sourceText,
  initialSources,
  kindLabels,
  type FilmKind,
  type Source,
} from "./data/demo";
import { locale, setLocale, t, type Message } from "./i18n";

const films = computed(() => baseFilms.map(localizeFilm));
const messages = useMessage();
const languageOptions = [
  { label: "EN", value: "en" },
  { label: "中文", value: "zh-CN" },
];

type Page = "home" | "film" | "watch" | "sources" | "settings" | "history";
function readRoute(): {
  page: Page;
  id: string;
  kind: FilmKind | "";
  episode: number;
  resume: boolean;
} {
  const [path, query = ""] = window.location.hash.replace(/^#\/?/, "").split("?");
  const [name = "home", id = "", episodeValue = "1"] = path!.split("/");
  const resume = name === "watch" && new URLSearchParams(query).get("resume") === "1";
  const episode = Math.max(1, Math.floor(Number(episodeValue) || 1));
  if (name === "kind" && ["movie", "series", "anime"].includes(id))
    return { page: "home", id: "", kind: id as FilmKind, episode: 1, resume: false };
  if (["film", "watch", "sources", "settings", "history"].includes(name))
    return { page: name as Page, id, kind: "", episode, resume };
  return { page: "home", id: "", kind: "", episode: 1, resume: false };
}
const route = ref(readRoute());
const sources = ref<Source[]>(initialSources.map((source) => ({ ...source })));
const sourceId = ref(sources.value[0]!.id);
const source = computed(
  () =>
    sources.value.find((item) => item.id === sourceId.value) ||
    sources.value[0]!,
);
const activeGenre = ref<Message>("All");
const expanded = ref(false);
const heroIndex = ref(0);
const homeSnapshots = new Map<
  string,
  { genre: Message; expanded: boolean; hero: number; scroll: number }
>();
const loading = ref(false);
let loadingTimer: ReturnType<typeof setTimeout> | undefined;
const heroFilms = computed(() =>
  route.value.kind
    ? films.value.filter((film) => film.kind === route.value.kind)
    : [films.value[0]!, films.value[2]!, films.value[3]!],
);
const hero = computed(
  () => heroFilms.value[heroIndex.value % heroFilms.value.length]!,
);
const selectedFilm = computed(
  () =>
    films.value.find((film) => film.id === route.value.id) || films.value[0]!,
);
const title = computed(() =>
  route.value.kind
    ? t(kindLabels[route.value.kind])
    : {
        home: t("Discover"),
        film: t("Film details"),
        watch: t("Now playing"),
        sources: t("Source manager"),
        settings: t("Settings"),
        history: t("Watch history"),
      }[route.value.page],
);
const filteredFilms = computed(() =>
  films.value.filter(
    (film) =>
      (!route.value.kind || film.kind === route.value.kind) &&
      (activeGenre.value === "All" || film.genres.includes(activeGenre.value)),
  ),
);
const visibleFilms = computed(() =>
  expanded.value ? filteredFilms.value : filteredFilms.value.slice(0, 6),
);
const relatedFilms = computed(() =>
  films.value.filter((film) => film.id !== selectedFilm.value.id).slice(0, 4),
);
const genres: Message[] = [
  "All",
  "Drama",
  "Adventure",
  "Mystery",
  "Sci-fi",
  "Feel-good",
  "Documentary",
];

function rememberHome() {
  if (route.value.page === "home")
    homeSnapshots.set(`${sourceId.value}:${route.value.kind}`, {
      genre: activeGenre.value,
      expanded: expanded.value,
      hero: heroIndex.value,
      scroll: window.scrollY,
    });
}
const nav = computed(() => [
  { label: t("Discover"), icon: "compass", href: "#/home", kind: "" },
  { label: t("Movies"), icon: "film", href: "#/kind/movie", kind: "movie" },
  { label: t("Series"), icon: "tv", href: "#/kind/series", kind: "series" },
  {
    label: t("Animation"),
    icon: "sparkles",
    href: "#/kind/anime",
    kind: "anime",
  },
]);

function notify(text: string) {
  messages.info(text, { duration: 3200 });
}
function changeSource(id: string) {
  void flushHistory();
  historyContext = null;
  playbackSequence++;
  rememberHome();
  sourceId.value = id;
  activeGenre.value = "All";
  expanded.value = false;
  heroIndex.value = 0;
  if (route.value.page === "watch")
    window.location.hash = `#/film/${selectedFilm.value.id}`;
  loading.value = true;
  clearTimeout(loadingTimer);
  loadingTimer = setTimeout(() => {
    loading.value = false;
  }, 350);
  notify(
    t("Selected {name} · Using the demo catalog", {
      name: sourceText(source.value).name,
    }),
  );
}

function changeLanguage(value: string) {
  if (value !== "en" && value !== "zh-CN") return;
  messages.destroyAll();
  if (!setLocale(value))
    notify(
      t(
        "Language changed, but the preference could not be saved on this device. Reloading will restore English.",
      ),
    );
}
watch(
  locale,
  () => {
    document.documentElement.lang = locale.value;
    updateDocumentTitle();
  },
  { immediate: true },
);

function focusMain() {
  document.getElementById("main-content")?.focus({ preventScroll: true });
}
function updateDocumentTitle() {
  document.title = `${route.value.page === "film" || route.value.page === "watch" ? selectedFilm.value.title : title.value} · ${t("Yingxiang Video")}`;
}

const aboutOpen = ref(false);
const aboutTrigger = ref<HTMLButtonElement>();
function addSource(value: Source) {
  sources.value.push(value);
  notify(t("Source added to this preview. Live connections will follow."));
}
function toggleSource(item: Source) {
  if (item.id === sourceId.value) {
    notify(t("Switch to another source before disabling the current one."));
    return;
  }
  item.enabled = !item.enabled;
  notify(item.enabled ? t("Source enabled") : t("Source disabled"));
}

const video = ref<HTMLVideoElement>();
const videoFrame = ref<HTMLDivElement>();
const isPlaying = ref(false);
const mediaBusy = ref(false);
const mediaError = ref(false);
const position = ref(0);
const duration = ref(0);
const volume = ref(0.65);
const muted = ref(false);
const speed = ref(1);
const subtitles = ref(false);
const episode = ref(Math.min(route.value.episode, selectedFilm.value.episodes));
const autoNext = ref(true);
const activeLine = ref(0);
const historySaveError = ref<string | null>(null);
const resumeError = ref<string | null>(null);
let historyContext: Omit<HistoryEntry, 'position_ms' | 'duration_ms' | 'completed' | 'updated_at'> | null = null;
let lastHistorySnapshot: HistoryEntry | null = null;
let lastHistoryWrite = 0;
let playbackSequence = 0;
let preparingResume = false;
let pendingSeek: number | null = null;

function historySnapshot(finished = false): HistoryEntry | null {
  const element = video.value;
  if (!element || !historyContext || route.value.page !== 'watch' || preparingResume || pendingSeek !== null || mediaError.value || resumeError.value || !Number.isFinite(element.currentTime) || element.currentTime <= 0) return null;
  const total = Number.isFinite(element.duration) && element.duration > 0 ? Math.round(element.duration * 1000) : null;
  const current = Math.max(0, Math.round(element.currentTime * 1000));
  return { ...historyContext, position_ms: total ? Math.min(total,current) : current, duration_ms: total, completed: finished || (total !== null && current >= total), updated_at: Date.now() };
}
async function persistHistory(snapshot: HistoryEntry, exit = false) {
  try {
    if (exit) await historyRepository.saveOnExit(snapshot); else await historyRepository.save(snapshot);
    historySaveError.value = null;
  } catch (failure) {
    const firstFailure = !historySaveError.value;
    historySaveError.value = failure instanceof Error ? failure.message : String(failure);
    if (firstFailure && document.visibilityState === 'visible') messages.error(t('Watch history could not be saved'));
  }
}
function flushHistory(finished = false, exit = false): Promise<void> {
  const snapshot = historySnapshot(finished);
  if (!snapshot) return Promise.resolve();
  lastHistoryWrite = Date.now();
  lastHistorySnapshot = snapshot;
  return persistHistory(snapshot,exit);
}
function retryHistorySave() {
  return lastHistorySnapshot ? persistHistory(lastHistorySnapshot) : Promise.resolve();
}
function discardPendingHistory(id: string | null) {
  if (id === null || lastHistorySnapshot?.content_id === id) {
    lastHistorySnapshot = null;
    historySaveError.value = null;
  }
}
function onPause() {
  if (video.value?.paused) {
    isPlaying.value = false;
    void flushHistory();
  }
}
function onPlaying() {
  mediaBusy.value = false;
  void flushHistory();
}
function applyPendingSeek() {
  if (pendingSeek !== null && video.value && video.value.readyState >= 1) {
    const total = Number.isFinite(video.value.duration) ? video.value.duration : Infinity;
    video.value.currentTime = Math.max(0,Math.min(total,pendingSeek));
    position.value = video.value.currentTime;
    pendingSeek = null;
  }
}
async function preparePlayback() {
  const sequence = ++playbackSequence;
  historyContext = null;
  preparingResume = true;
  resumeError.value = null;
  pendingSeek = 0;
  let saved: HistoryEntry | null = null;
  try {
    if (route.value.resume) {
      saved = await historyRepository.get(historyContentId(selectedFilm.value.kind,selectedFilm.value.id));
      if (sequence !== playbackSequence || route.value.page !== 'watch') return;
      if (!saved || saved.episode !== episode.value) throw new Error(t('No saved progress was found for this episode.'));
      const savedSource = sources.value.find(item=>item.id===saved!.source_id && item.enabled);
      if (!savedSource) throw new Error(t('The saved playback source is unavailable. Enable it in Sources before continuing.'));
      sourceId.value = savedSource.id;
    }
    if (sequence !== playbackSequence || route.value.page !== 'watch') return;
    const film = baseFilms.find(item=>item.id===selectedFilm.value.id)!;
    historyContext = { content_id: historyContentId(film.kind,film.id), title: film.title, poster: film.artwork, kind: film.kind, episode: episode.value, episode_total: film.episodes, source_id: source.value.id, source_name: source.value.name };
    pendingSeek = saved && !saved.completed ? saved.position_ms / 1000 : 0;
    lastHistoryWrite = 0;
    preparingResume = false;
    setDuration();
    await startMedia();
  } catch (failure) {
    if (sequence !== playbackSequence) return;
    preparingResume = false;
    resumeError.value = failure instanceof Error ? failure.message : String(failure);
  }
}
function continueHistory(entry: HistoryEntry) {
  const film = baseFilms.find(item=>historyContentId(item.kind,item.id)===entry.content_id);
  if (!film) { messages.error(t('This title is unavailable in the current catalog.')); return; }
  if (entry.episode > film.episodes) { messages.error(t('The saved episode is unavailable. Open the film details to choose an episode.')); return; }
  window.location.hash = `#/watch/${film.id}/${entry.episode}?resume=1`;
}
function onPageHide() { void flushHistory(false,true); }
function onVisibilityChange() { if (document.visibilityState === 'hidden') void flushHistory(false,true); }

const caption = computed(() =>
  selectedFilm.value.episodes > 1
    ? t("Episode {number}", { number: episode.value })
    : t("Feature"),
);
const progress = computed(() =>
  duration.value ? (position.value / duration.value) * 100 : 0,
);
function formatTime(seconds: number) {
  if (!Number.isFinite(seconds)) return "00:00";
  return `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, "0")}:${Math.floor(seconds % 60)
    .toString()
    .padStart(2, "0")}`;
}
async function startMedia() {
  if (!video.value || preparingResume || resumeError.value) return;
  video.value.volume = volume.value;
  video.value.muted = muted.value;
  video.value.playbackRate = speed.value;
  mediaError.value = false;
  try {
    await video.value.play();
  } catch {
    isPlaying.value = false;
  }
}
function togglePlayback() {
  if (!video.value) return;
  if (video.value.paused) void startMedia();
  else video.value.pause();
}
function updateTime() {
  position.value = video.value?.currentTime || 0;
  if (isPlaying.value && Date.now() - lastHistoryWrite >= 5000) void flushHistory();
}
function setDuration() {
  duration.value = Number.isFinite(video.value?.duration)
    ? video.value!.duration
    : 0;
  applyPendingSeek();
}
function seek(value: number) {
  if (video.value && duration.value) {
    video.value.currentTime = Math.max(0, Math.min(duration.value, value));
    updateTime();
    void flushHistory();
  }
}
function setVolume(value: number) {
  volume.value = value;
  muted.value = value === 0;
  if (video.value) {
    video.value.volume = value;
    video.value.muted = muted.value;
  }
}
function toggleMute() {
  muted.value = !muted.value;
  if (video.value) video.value.muted = muted.value;
}
function setSpeed(value: number) {
  speed.value = value;
  if (video.value) video.value.playbackRate = value;
}
function toggleSubtitles() {
  subtitles.value = !subtitles.value;
  syncSubtitles();
}
function syncSubtitles() {
  for (const track of Array.from(video.value?.textTracks || [])) {
    track.mode =
      subtitles.value && track.language === locale.value
        ? "showing"
        : "disabled";
  }
}
watch(locale, () => nextTick(syncSubtitles));
async function fullscreen() {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await videoFrame.value?.requestFullscreen();
  } catch {
    notify(t("Fullscreen is unavailable here. Try maximizing the window."));
  }
}
function selectEpisode(value: number) {
  void flushHistory();
  historyContext = null;
  playbackSequence++;
  video.value?.pause();
  episode.value = value;
  window.history.replaceState(null,"",`#/watch/${selectedFilm.value.id}/${value}`);
  route.value = { ...route.value, episode: value, resume: false };
  position.value = 0;
  void preparePlayback();
}
function onEnded() {
  isPlaying.value = false;
  void flushHistory(true);
  if (autoNext.value && episode.value < selectedFilm.value.episodes)
    selectEpisode(episode.value + 1);
}
function changeLine(value: number) {
  activeLine.value = value;
  notify(
    t("Selected {name} · Both streams use the same local demo clip", {
      name: t(value ? "Alternative stream" : "Default stream"),
    }),
  );
}
function reloadVideo() {
  mediaError.value = false;
  pendingSeek = position.value;
  video.value?.load();
  void startMedia();
}
function onHashChange() {
  void flushHistory();
  historyContext = null;
  playbackSequence++;
  pendingSeek = null;
  preparingResume = false;
  resumeError.value = null;
  rememberHome();
  video.value?.pause();
  route.value = readRoute();
  const home =
    route.value.page === "home"
      ? homeSnapshots.get(`${sourceId.value}:${route.value.kind}`)
      : undefined;
  activeGenre.value = home?.genre || "All";
  expanded.value = home?.expanded || false;
  heroIndex.value = home?.hero || 0;
  episode.value = Math.min(route.value.episode, selectedFilm.value.episodes);
  position.value = 0;
  duration.value = 0;
  isPlaying.value = false;
  mediaError.value = false;
  subtitles.value = false;
  activeLine.value = 0;
  updateDocumentTitle();
  nextTick(() => {
    window.scrollTo({ top: home?.scroll || 0, behavior: "instant" });
    if (route.value.page === "watch") void preparePlayback();
  });
}
function onKeydown(event: KeyboardEvent) {
  if (aboutOpen.value) return;
  if (
    route.value.page !== "watch" ||
    (event.target as HTMLElement).closest(
      'input,select,button,a,textarea,[role="switch"],[role="combobox"],[role="radio"],[role="slider"],[contenteditable="true"]',
    ) ||
    event.ctrlKey ||
    event.metaKey ||
    event.altKey
  )
    return;
  if (event.code === "Space") {
    event.preventDefault();
    togglePlayback();
  }
  if (event.key === "ArrowRight") {
    event.preventDefault();
    seek(position.value + 5);
  }
  if (event.key === "ArrowLeft") {
    event.preventDefault();
    seek(position.value - 5);
  }
  if (event.key.toLowerCase() === "m") toggleMute();
  if (event.key.toLowerCase() === "f") void fullscreen();
}
watch(volume, (value) => {
  if (video.value) video.value.volume = value;
});
onMounted(() => {
  updateDocumentTitle();
  window.addEventListener("hashchange", onHashChange);
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("pagehide", onPageHide);
  document.addEventListener("visibilitychange", onVisibilityChange);
  if (route.value.page === "watch") void preparePlayback();
});
onBeforeUnmount(() => {
  void flushHistory(false,true);
  historyContext = null;
  playbackSequence++;
  clearTimeout(loadingTimer);
  window.removeEventListener("hashchange", onHashChange);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("pagehide", onPageHide);
  document.removeEventListener("visibilitychange", onVisibilityChange);
});
</script>

<template>
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
          <span class="status-dot" /><span>{{ t("Interface preview") }}</span
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
          <a
            class="icon-button palette-shortcut"
            href="#/settings"
            :aria-label="t('Customize colors')"
            :title="t('Customize colors')"
          >
            <Icon name="palette" :size="19" />
          </a>
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
                v-for="item in sources.filter((item) => item.enabled)"
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
        <template v-if="route.page === 'home'">
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

        <template v-else-if="route.page === 'watch'">
          <NAlert v-if="historySaveError" type="error" class="history-alert" :title="t('Watch history could not be saved')">{{ historySaveError }}<NButton size="small" @click="retryHistorySave">{{ t('Retry') }}</NButton></NAlert>
          <NAlert v-if="resumeError" type="error" class="history-alert" :title="t('Saved progress could not be restored')">{{ resumeError }}<NButton size="small" @click="preparePlayback">{{ t('Retry') }}</NButton></NAlert>
          <div class="watch-heading">
            <div>
              <h1>
                {{ selectedFilm.title }}<span>{{ caption }}</span>
              </h1>
              <p>
                {{ sourceText(source).name }}<span>·</span
                >{{ t("Local demo clip · 00:30") }}
              </p>
            </div>
            <a :href="`#/film/${selectedFilm.id}`" class="text-button"
              >{{ t("Film details") }}<Icon name="arrow" :size="16"
            /></a>
          </div>
          <div ref="videoFrame" class="video-frame">
            <video
              ref="video"
              src="/demo.mp4"
              :poster="selectedFilm.artwork"
              playsinline
              preload="metadata"
              @click="togglePlayback"
              @play="isPlaying = true"
              @pause="onPause"
              @timeupdate="updateTime"
              @loadedmetadata="setDuration"
              @ended="onEnded"
              @waiting="mediaBusy = true"
              @playing="onPlaying"
              @error="
                mediaError = true;
                mediaBusy = false;
              "
              @canplay="mediaBusy = false"
            >
              <track
                src="/demo.vtt"
                kind="subtitles"
                srclang="zh-CN"
                label="中文"
              />
              <track
                src="/demo.en.vtt"
                kind="subtitles"
                srclang="en"
                label="English"
              />
            </video>
            <div class="video-watermark">
              <img src="/mark.svg" width="22" height="22" alt="" /><span>{{
                t("Yingxiang Video")
              }}</span>
            </div>
            <button
              v-if="!isPlaying && !mediaError"
              class="big-play"
              :aria-label="
                position >= duration && duration ? t('Replay') : t('Play')
              "
              @click="togglePlayback"
            >
              <Icon
                :name="position >= duration && duration ? 'repeat' : 'play'"
                :size="31"
              />
            </button>
            <div
              v-if="mediaBusy && !mediaError"
              class="media-loading"
              role="status"
            >
              {{ t("Preparing video…") }}
            </div>
            <div v-if="mediaError" class="media-error" role="alert">
              <Icon name="info" :size="30" />
              <h2>{{ t("The clip could not be loaded") }}</h2>
              <p>
                {{
                  t(
                    "Check that the local preview server is running, then try again.",
                  )
                }}
              </p>
              <button class="button button-primary" @click="reloadVideo">
                {{ t("Reload") }}
              </button>
            </div>
            <div class="player-controls">
              <label class="timeline"
                ><span class="sr-only">{{ t("Playback progress") }}</span
                ><input
                  type="range"
                  min="0"
                  :max="duration || 30"
                  step="0.1"
                  :value="position"
                  :style="{ '--progress': `${progress}%` }"
                  :disabled="!duration"
                  @input="
                    seek(Number(($event.target as HTMLInputElement).value))
                  "
              /></label>
              <div class="player-control-row">
                <button
                  class="icon-button"
                  :aria-label="isPlaying ? t('Pause') : t('Play')"
                  @click="togglePlayback"
                >
                  <Icon
                    :name="isPlaying ? 'pause' : 'play'"
                    :size="20"
                  /></button
                ><button
                  class="icon-button"
                  :aria-label="t('Next episode')"
                  :disabled="episode >= selectedFilm.episodes"
                  @click="selectEpisode(episode + 1)"
                >
                  <Icon name="next" :size="21" /></button
                ><span class="play-time"
                  >{{ formatTime(position) }}<span>/</span
                  >{{ formatTime(duration) }}</span
                >
                <div class="player-spacer" />
                <div class="volume-group">
                  <button
                    class="icon-button"
                    :aria-label="muted ? t('Unmute') : t('Mute')"
                    @click="toggleMute"
                  >
                    <Icon
                      :name="muted ? 'muted' : 'volume'"
                      :size="19"
                    /></button
                  ><input
                    :aria-label="t('Volume')"
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    :value="muted ? 0 : volume"
                    :aria-valuetext="`${Math.round((muted ? 0 : volume) * 100)}%`"
                    @input="
                      setVolume(
                        Number(($event.target as HTMLInputElement).value),
                      )
                    "
                  />
                  <output class="volume-value"
                    >{{ Math.round((muted ? 0 : volume) * 100) }}%</output
                  >
                </div>
                <label class="speed-select"
                  ><span class="sr-only">{{ t("Playback speed") }}</span
                  ><select
                    :value="speed"
                    @change="
                      setSpeed(
                        Number(($event.target as HTMLSelectElement).value),
                      )
                    "
                  >
                    <option
                      v-for="rate in [0.5, 1, 1.25, 1.5, 2]"
                      :key="rate"
                      :value="rate"
                    >
                      {{ rate }}×
                    </option>
                  </select></label
                ><button
                  class="icon-button"
                  :class="{ 'control-active': subtitles }"
                  :aria-pressed="subtitles"
                  :aria-label="t('Toggle subtitles')"
                  @click="toggleSubtitles"
                >
                  <Icon name="captions" :size="20" /></button
                ><button
                  class="icon-button"
                  :aria-label="t('Fullscreen')"
                  @click="fullscreen"
                >
                  <Icon name="fullscreen" :size="19" />
                </button>
              </div>
            </div>
          </div>
          <div class="watch-bottom">
            <p>
              <span class="status-dot" />{{
                t(
                  "Playing an original test clip. All film and credit information is fictional.",
                )
              }}
            </p>
            <label class="switch-label"
              ><NSwitch
                v-model:value="autoNext"
                :aria-label="t('Auto-play next')"
              />{{ t("Auto-play next") }}</label
            >
          </div>
          <section class="watch-options">
            <div class="line-tabs" :aria-label="t('Playback streams')">
              <button
                :class="{ selected: activeLine === 0 }"
                @click="changeLine(0)"
              >
                <span class="status-dot" />{{ t("Default stream") }}</button
              ><button
                :class="{ selected: activeLine === 1 }"
                @click="changeLine(1)"
              >
                {{ t("Alternative stream") }}
              </button>
            </div>
            <div class="section-heading">
              <h2>
                {{ t("Episodes")
                }}<span class="subtle-label">{{
                  selectedFilm.episodes > 1
                    ? t("{count} episodes", { count: selectedFilm.episodes })
                    : t("1 feature")
                }}</span>
              </h2>
              <span class="subtle-label">{{ caption }}</span>
            </div>
            <div class="episode-grid">
              <button
                v-for="n in selectedFilm.episodes"
                :key="n"
                :class="['episode-link', { selected: episode === n }]"
                :aria-pressed="episode === n"
                @click="selectEpisode(n)"
              >
                {{
                  selectedFilm.episodes > 1
                    ? String(n).padStart(2, "0")
                    : t("Feature")
                }}
              </button>
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
          @notice="notify"
        />
        <footer class="page-footer">
          <span
            >{{ t("Yingxiang Video") }}<span class="footer-cross">✳</span
            >{{ t("Let stories happen") }}</span
          ><span>{{
            t("Interface preview · Original fictional content")
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
            "This is an interactive interface preview. Films, credits and synopses are fictional, artwork is AI-generated, and the player uses a local 30-second test clip. Explore details, episodes, subtitles, playback preferences and source management.",
          )
        }}
      </p>
      <NButton type="primary" @click="aboutOpen = false">{{
        t("Start exploring")
      }}</NButton>
    </NModal>
  </div>
</template>
