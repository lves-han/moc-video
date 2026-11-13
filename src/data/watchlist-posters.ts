/** Load a small number of posters at once and stop scheduling after the view changes. */
export async function loadWatchlistPosters<T>(entries: readonly T[], active: () => boolean, load: (entry: T) => Promise<void>) {
  let index = 0;
  const worker = async () => {
    while (active() && index < entries.length) await load(entries[index++]!);
  };
  await Promise.all(Array.from({ length: Math.min(4, entries.length) }, worker));
}
