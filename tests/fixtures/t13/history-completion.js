// Shared by the native fixture and its negative controls. Element tokens come
// from a WeakMap of actual video objects; no production state is changed here.
export function historyCompletionReady(expected, current, nativeEnded, committed, now) {
  const same = observation => observation && ["element", "epoch", "engineEpoch", "session", "resolution", "match", "video", "source", "metadata"]
    .every(key => observation[key] === expected[key]);
  return Boolean(expected.element > 0 && Number.isSafeInteger(expected.epoch) && expected.epoch >= 0
    && ["engineEpoch", "session", "resolution", "match", "video", "source", "metadata", "contentId", "historySource"]
      .every(key => typeof expected[key] === "string" && expected[key].length > 0)
    && Number.isInteger(expected.episode) && expected.episode > 0
    && Number.isFinite(expected.started) && expected.deadline === expected.started + 12000
    && Number.isFinite(now) && now >= expected.started && now <= expected.deadline
    && same(current) && current.route === "playback" && current.phase === "ended" && current.ended === true
    && nativeEnded?.type === "ended" && nativeEnded.trusted === true && nativeEnded.ended === true
    && same(nativeEnded.playback) && Number.isFinite(nativeEnded.timestamp)
    && nativeEnded.timestamp >= expected.started && nativeEnded.timestamp <= nativeEnded.at
    && nativeEnded.at >= expected.started && nativeEnded.at <= now
    && same(committed?.playback) && committed.playback.phase === "ended"
    && committed.at >= nativeEnded.at && committed.at <= now
    && committed.entry?.completed === true && committed.entry.content_id === expected.contentId
    && committed.entry.episode === expected.episode && committed.entry.source_id === expected.historySource);
}
