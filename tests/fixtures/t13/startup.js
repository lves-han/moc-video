void (async () => {
  const native = (action, options = {}) => window.webkit.messageHandlers.t13.postMessage({ action, ...options });
  try {
    const end = performance.now() + 15000;
    while (!window.__T13_FIRST_FRAME__ && performance.now() < end) await new Promise(resolve => setTimeout(resolve, 30));
    const reloaded = !window.__T13_SEED_EXPECT__;
    const expected = window.__T13_SEED_EXPECT__ || JSON.parse(localStorage.getItem("yingxiang-video.settings")), first = window.__T13_FIRST_FRAME__;
    if (!first || first.palette !== expected.color_palette || first.lang !== expected.ui_locale || first.scheme !== (first.dark ? "dark" : "light") || !first.accent) throw new Error(`First-frame mismatch: ${JSON.stringify({ expected, first })}`);
    const { paletteOptions } = await import("/src/ui/theme.ts");
    const brand = paletteOptions.find(item => item.value === expected.color_palette);
    if (first.accent !== (first.dark ? brand.night : brand.day)) throw new Error("First frame uses stale palette colors");
    const { historyRepository } = await import("/src/data/history.ts");
    if ((await historyRepository.list()).length !== 0) throw new Error("Session-only history survived a fresh page");
    if (reloaded && location.hash !== "#/history") throw new Error("Reload changed the history route");
    const screenshot = await native("screenshot", { name: "startup.png" });
    await native("finish", { report: { ok: true, cases: [{ id: reloaded ? "history-refresh-reset-and-preferences" : `first-frame-${expected.color_palette}`, requirements: ["U09", "R01", "R62", "R67", "REQ-T13"], result: "PASS", actual: { first, expected, emptyHistory: true, samePageReload: reloaded, native: await native("identity") } }], screenshots: [screenshot] } });
  } catch (error) { await native("finish", { report: { ok: false, error: { message: String(error?.message ?? error), stack: String(error?.stack ?? "") }, cases: [] } }); }
})();
