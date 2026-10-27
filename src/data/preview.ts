import { Api } from "../api.ts";
import type { AppSource, Bridge_MediaRef, DetailsViewData, PlaybackContext, RuntimeStatus, SourceKey } from "../models.ts";
import { films, initialSources } from "./demo.ts";

// Explicit, session-only interface preview. These handles never enter native media IPC.
export const previewEpoch = "11111111-1111-4111-8111-111111111111";
const config = "22222222-2222-4222-8222-222222222222";
const revision = "d".repeat(64), addon = "e".repeat(64);
export const previewSources: AppSource[] = initialSources.map((source, index) => ({
  id: (index + 10).toString(16).repeat(64), source_revision: revision, name: source.name,
  language: "en", content_types: ["movie", "series", "anime"], runtime_status: "ready", kind: source.kind,
  capabilities: { home: true, search: false, details: true, resolve: source.kind !== "stremio", sequential_home: source.kind === "cloudstream" },
  origin: source.kind === "cloudstream" ? { plugin_internal_name: "InterfacePreview", plugin_sha256: addon, registration_key: addon }
    : source.kind === "stremio" ? { config_id: config, manifest_urls: ["https://preview.example/manifest.json"] }
    : { config_id: "33333333-3333-4333-8333-333333333333", base_url: "https://preview.example/api.php" },
} as AppSource));
export const previewSourceKey = (id: string): SourceKey => {
  const source = previewSources[initialSources.findIndex(source => source.id === id)];
  if (!source) throw new Error("Unknown preview source");
  return { source_id: source.id, source_revision: source.source_revision };
};
export const previewSourceId = (key: SourceKey | null) => initialSources[previewSources.findIndex(source => source.id === key?.source_id)]?.id ?? "demo-maccms";
const metadataSource = previewSourceKey("demo-stremio");
export const previewRef = (id: string): Bridge_MediaRef => ({ ...metadataSource, data: JSON.stringify({ id }) });
export function previewDetails(id: string): DetailsViewData {
  const film = films.find(film => film.id === id);
  if (!film) throw new Error("Preview title is unavailable");
  const ref = previewRef(id);
  return { images: {}, metadata_ref: { content_ref: ref, identity: { config_id: config, addon_key: addon,
    namespace: `addon:${addon}`, type: film.kind, meta_id: film.id, canonical_id: film.id } },
    details: { id: film.id, ref, title: film.title, plot: film.plot, kind: film.kind, tags: film.genres, related: [],
      groups: [{ id: `preview:${id}`, title: "Episodes", kind: "season", dub_status: "unspecified",
        episodes: Array.from({ length: film.episodes }, (_, index) => ({ id: `preview:${id}:${index + 1}`, label: String(index + 1), index,
          ref: { ...metadataSource, data: JSON.stringify({ id, video_id: `${id}:${index + 1}` }) } })) }] } };
}
export const previewRuntime = (): RuntimeStatus => ({ platform: "macos", phase: "ready", engine_epoch: previewEpoch,
  health: { status: "ready", contract_version: "1.5.0", media_gateway_ready: true }, error: null,
  cloudstream: { phase: "stopped", engine_epoch: null, display_mode: "none", health: null, error: null } });
export const previewApi = new Api(async (command, args) => {
  let data: unknown;
  const input = args.input as Record<string, any>;
  switch (command) {
    case "runtime_status": data = previewRuntime(); break;
    case "get_sources": data = { providers: previewSources, plugins: [] }; break;
    case "get_home": data = { page: { ...metadataSource, sections: [{ id: "preview", title: "Discover", section_ref: "preview:home", index: 0,
      page: 1, has_next: false, next_page: null, items: films.map(film => ({ id: film.id, title: film.title, kind: film.kind, ref: previewRef(film.id) })) }], failed_sections: [] }, images: {} }; break;
    case "get_details": data = previewDetails(JSON.parse(input.ref.data).id); break;
    case "match_playback": {
      const content = JSON.parse(input.metadata_ref.content_ref.data);
      const context: PlaybackContext = { metadata_ref: input.metadata_ref, metadata_video_id: input.video_id,
        match_id: `preview:${crypto.randomUUID()}`, content_ref: { ...input.playback_source, data: JSON.stringify(content) },
        group_id: `preview:${content.id}`, episode_id: `preview:${input.video_id}`,
        episode_ref: { ...input.playback_source, data: JSON.stringify({ id: content.id, video_id: input.video_id }) }, selected_stream_id: null };
      data = { status: "matched", metadata_ref: context.metadata_ref, video_id: context.metadata_video_id,
        candidates: [{ content_ref: context.content_ref, title: previewDetails(content.id).details.title, year: null }], context }; break;
    }
    case "resolve_episode": data = { id: `preview:${crypto.randomUUID()}`, source_id: input.episode_ref.source_id,
      source_revision: input.episode_ref.source_revision, episode_ref: input.episode_ref, expires_at: "2099-01-01T00:00:00Z",
      streams: [0, 1].map(index => ({ id: `preview:stream:${index}`, label: index ? "Alternative stream" : "Default stream", kind: "progressive", playable: true, has_interceptor: false })), subtitles: [] }; break;
    // The preview's media element owns its local clip; no native Session is claimed here.
    case "close_playback": case "release_resolution": data = { ok: true }; break;
    case "cancel_request": data = { target_request_id: input.target_request_id, cancelled: false }; break;
    default: throw new Error(`Unavailable interface-preview command: ${command}`);
  }
  return { request_id: args.request_id, engine_epoch: previewEpoch, data };
});
