// Mirrors the Host's title normalization (fuzzy.rs) closely enough to tell whether
// any result really contains the typed text; otherwise the results are approximate.
export const normalizeTitle = (text: string) => text.normalize("NFKC").toLowerCase().replace(/[\s\p{P}\p{S}]+/gu, "");

export const approximateResults = (query: string, titles: string[]) => {
  const wanted = normalizeTitle(query);
  return Boolean(wanted) && titles.length > 0 && !titles.some(title => normalizeTitle(title).includes(wanted));
};
