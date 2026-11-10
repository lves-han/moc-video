import type { Bridge_Episode, Bridge_EpisodeGroup, DetailsViewData, MetadataRef, PlaybackContext, SourceKey } from './models.ts';

// This shares the app's existing localStorage persistence. Only digests and
// source identities cross this boundary; media references stay in live DTOs.
export const LINE_PREFERENCE_KEY = 'yingxiang-video.lines.v1';
type Details = DetailsViewData['details'];
export type LinePreference = { film: string; metadata: string; source: SourceKey; content: string; evidence: string; layout: string; line: string };
type StoragePort = Pick<Storage, 'getItem' | 'setItem'>;
const digest = async (value: unknown) => [...new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify(value))))].map(n => n.toString(16).padStart(2, '0')).join('');
const hash = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const fields = ['film', 'metadata', 'content', 'evidence', 'layout', 'line'] as const;

export function filmKey(metadata: MetadataRef) { return digest([metadata.identity, metadata.content_ref.source_id, metadata.content_ref.source_revision]); }
export async function preferredContent(preference: LinePreference, reference: Details['ref']) { return preference.content === await digest(reference); }
function stable(details: Details) {
  return [details.title, details.kind, details.year ?? null, (details.actors ?? []).map(person => person.name),
    (details.directors ?? []).map(person => person.name), details.regions ?? [], details.languages ?? []];
}
function auditable(details: Details) {
  return Boolean(details.title.trim() && (details.year != null || details.actors?.length || details.directors?.length));
}
const line = (group: Bridge_EpisodeGroup) => [group.title, group.kind, group.dub_status, group.season_number ?? null];
const layout = (details: Details) => details.groups.map(group => [line(group), group.episodes.map(episode => [episode.label, episode.season ?? null, episode.number ?? null])]);

// Anchors deliberately reject trailers, ranges and provider-created ordinal
// labels. MacCMS synthesizes plain "1" for an unlabeled URL, so digits alone
// carry no episode evidence.
export function episodeCoordinates(episode: Bridge_Episode): { season: number | null; number: number } | null {
  const label = episode.label.trim();
  const qualified = /^(?:S(\d+)E(\d+)|第(\d+)季第(\d+)集)$/i.exec(label);
  const plain = /^(?:第(\d+)集|(?:EP?|Episode)\s*(\d+))$/i.exec(label);
  const parsed = qualified ? { season: Number(qualified[1] ?? qualified[3]), number: Number(qualified[2] ?? qualified[4]) }
    : plain ? { season: null, number: Number(plain[1] ?? plain[2]) } : null;
  const structured = episode.number != null && Number.isFinite(episode.number) && episode.number >= 0
    && episode.season != null && Number.isSafeInteger(episode.season) && episode.season >= 0
    ? { season: episode.season, number: episode.number } : null;
  if (structured && parsed && (structured.number !== parsed.number || parsed.season !== null && structured.season !== parsed.season)) return null;
  return structured ?? (parsed && parsed.number > 0 && (parsed.season === null || parsed.season > 0) ? parsed : null);
}

export function correspondingEpisode(metadata: Details, target: Bridge_Episode, group: Bridge_EpisodeGroup): Bridge_Episode | null {
  const requested = episodeCoordinates(target);
  if (metadata.kind === 'movie') return group.episodes.length === 1 ? group.episodes[0]! : null;
  if (!requested) return null;
  const seasons = new Set<number>();
  for (const other of metadata.groups) {
    if (other.season_number != null) seasons.add(other.season_number);
    for (const episode of other.episodes) {
      const position = episodeCoordinates(episode);
      if (!position) return null;
      if (position.season !== null) seasons.add(position.season);
    }
  }
  // Unqualified labels are useful only in an unambiguous single-season view.
  if (requested.season === null && seasons.size > 1) return null;
  const singleSeason = seasons.size === 1 ? [...seasons][0]! : null;
  const season = requested.season ?? singleSeason;
  const positions = group.episodes.map(episodeCoordinates);
  if (positions.some(position => !position)) return null;
  const keys = positions.map(position => {
    const explicit = position!.season ?? group.season_number ?? null;
    if (position!.season !== null && group.season_number != null && position!.season !== group.season_number) return null;
    if (explicit === null && seasons.size > 1) return null;
    return [explicit ?? singleSeason, position!.number];
  });
  if (keys.some(key => key === null) || new Set(keys.map(key => JSON.stringify(key))).size !== keys.length) return null;
  const matches = group.episodes.filter((_, index) => keys[index]![0] === season && keys[index]![1] === requested.number);
  return matches.length === 1 ? matches[0]! : null;
}

export async function captureLine(metadata: DetailsViewData, details: Details, context: PlaybackContext): Promise<LinePreference | null> {
  const group = details.groups.find(group => group.id === context.group_id);
  if (!metadata.metadata_ref || !group || !auditable(details) || !auditable(metadata.details)
    || details.groups.filter(other => JSON.stringify(line(other)) === JSON.stringify(line(group))).length !== 1) return null;
  return { film: await filmKey(metadata.metadata_ref), metadata: await digest(stable(metadata.details)),
    source: { source_id: context.episode_ref.source_id, source_revision: context.episode_ref.source_revision },
    content: await digest(details.ref), evidence: await digest(stable(details)), layout: await digest(layout(details)), line: await digest(line(group)) };
}

export async function preferredSelection(preference: LinePreference, metadata: DetailsViewData, target: Bridge_Episode, details: Details) {
  if (!metadata.metadata_ref || preference.film !== await filmKey(metadata.metadata_ref)
    || preference.metadata !== await digest(stable(metadata.details)) || preference.content !== await digest(details.ref)
    || preference.evidence !== await digest(stable(details)) || preference.layout !== await digest(layout(details))) return null;
  const groups = [];
  for (const group of details.groups) if (await digest(line(group)) === preference.line) groups.push(group);
  if (groups.length !== 1) return null;
  const group = groups[0]!, episode = correspondingEpisode(metadata.details, target, group);
  return episode ? { content_ref: details.ref, group_id: group.id, episode_id: episode.id } : null;
}

export function createLinePreferences(storage: () => StoragePort = () => localStorage) {
  // Hashing is asynchronous, storage replacement is synchronous. A caller's
  // identity guard is checked immediately before replacement, with no await.
  function read(): LinePreference[] {
    let raw: string | null;
    try { raw = storage().getItem(LINE_PREFERENCE_KEY); }
    catch { throw new Error('Line preference read failed (storage unavailable)'); }
    if (raw === null) return [];
    try {
      const rows: unknown = JSON.parse(raw);
      if (!Array.isArray(rows) || rows.some(row => !row || typeof row !== 'object'
        || Object.keys(row).sort().join() !== [...fields, 'source'].sort().join()
        || fields.some(key => !hash(row[key])) || !row.source || Object.keys(row.source).sort().join() !== 'source_id,source_revision'
        || !hash(row.source.source_id) || !hash(row.source.source_revision)) || new Set(rows.map(row => row.film)).size !== rows.length) throw new Error();
      return rows;
    } catch { throw new Error('Line preference read failed (damaged records; original storage preserved)'); }
  }
  return {
    async get(metadata: MetadataRef) { const key = await filmKey(metadata); return read().find(row => row.film === key) ?? null; },
    save(preference: LinePreference, current: () => boolean) {
      if (!current()) return false;
      const rows = read().filter(row => row.film !== preference.film);
      rows.push(preference);
      if (!current()) return false;
      try { storage().setItem(LINE_PREFERENCE_KEY, JSON.stringify(rows)); }
      catch { throw new Error('Line preference save failed (permission or quota; confirmed records preserved)'); }
      return true;
    },
  };
}
