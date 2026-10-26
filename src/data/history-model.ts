export interface HistoryEntry {
  content_id: string;
  title: string;
  poster: string;
  kind: 'movie' | 'series' | 'anime';
  episode: number;
  episode_total: number;
  source_id: string;
  source_name: string;
  position_ms: number;
  duration_ms: number | null;
  completed: boolean;
  updated_at: number;
}

const columns = ['content_id','title','poster','kind','episode','episode_total','source_id','source_name','position_ms','duration_ms','completed','updated_at'] as const;

export function parseHistoryEntry(value: unknown): HistoryEntry {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('Invalid history record');
  const row = value as Record<string, unknown>;
  if (Object.keys(row).some(key => !columns.includes(key as typeof columns[number]))) throw new TypeError('Unknown history field');
  for (const [key, max] of [['content_id',512],['title',512],['poster',2048],['source_id',256],['source_name',256]] as const) {
    if (typeof row[key] !== 'string' || !row[key].trim().length || row[key].length > max) throw new TypeError(`Invalid history ${key}`);
  }
  if (!['movie','series','anime'].includes(row.kind as string)) throw new TypeError('Invalid history kind');
  for (const key of ['episode','episode_total','position_ms','updated_at'] as const) {
    if (!Number.isSafeInteger(row[key])) throw new TypeError(`Invalid history ${key}`);
  }
  if (row.duration_ms !== null && (!Number.isSafeInteger(row.duration_ms) || (row.duration_ms as number) <= 0)) throw new TypeError('Invalid history duration');
  if ((row.episode as number) < 1 || (row.episode_total as number) < (row.episode as number) || (row.episode_total as number) > 100000 || (row.position_ms as number) < 0 || (row.updated_at as number) <= 0 || !Number.isFinite(new Date(row.updated_at as number).getTime()) || (row.duration_ms !== null && (row.position_ms as number) > (row.duration_ms as number))) throw new TypeError('Invalid history progress');
  if (typeof row.completed !== 'boolean') throw new TypeError('Invalid history completion');
  return { ...row } as unknown as HistoryEntry;
}
export function historyProgress(entry: HistoryEntry): number {
  return entry.duration_ms ? Math.min(100, Math.max(0, Math.round(entry.position_ms / entry.duration_ms * 100))) : 0;
}
export function historyTime(milliseconds: number | null): string {
  if (milliseconds === null) return '--:--';
  const seconds = Math.floor(milliseconds / 1000);
  const minutes = Math.floor(seconds / 60);
  return minutes >= 60 ? `${Math.floor(minutes/60)}:${String(minutes%60).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}` : `${String(minutes).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}`;
}
export function historyContentId(kind: string, id: string): string {
  return `demo:${kind}:${id}`;
}
