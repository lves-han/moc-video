void (async () => {
  const cases = [], screenshots = [], historyTrace = [];
  let historyDiagnostic = () => null;
  const native = (action, options = {}) => window.webkit.messageHandlers.t13.postMessage({ action, ...options });
  const assert = (value, message) => { if (!value) throw new Error(message); };
  const wait = async (condition, description, ms = 12000) => {
    const end = performance.now() + ms;
    while (performance.now() < end) { if (condition()) return; await new Promise(resolve => setTimeout(resolve, 30)); }
    throw new Error(`Timed out: ${description}`);
  };
  const frame = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  const settledColors = async () => {
    await frame();
    // NGlobalStyle and radio buttons animate colors for .3s. Measure the
    // rendered target theme after those real CSS transitions have completed.
    await wait(() => !document.getAnimations().some(animation => animation instanceof CSSTransition
      && animation.playState === "running"), "theme color transitions");
    await frame();
  };
  const failure = error => ({ name: error?.name, message: String(error?.message ?? error), stack: String(error?.stack ?? ""),
    measurement: error?.measurement ?? null, hash: location.hash, history: historyDiagnostic(),
    dialogs: Array.from(document.querySelectorAll('.n-modal[role], .history-confirmation')).map(element => ({
      role: element.getAttribute("role"), modal: element.getAttribute("aria-modal"), label: element.getAttribute("aria-label"),
      containsFocus: element.contains(document.activeElement), text: element.textContent.slice(0, 160) })) });
  const visible = element => element && element.getBoundingClientRect().width > 0 && getComputedStyle(element).visibility !== "hidden";
  const select = (selector, value) => { const element = document.querySelector(selector); assert(element, `Missing ${selector}`); element.value = value; element.dispatchEvent(new Event("change", { bubbles: true })); };
  const click = selector => { const element = document.querySelector(selector); assert(element && visible(element), `Missing/hidden ${selector}`); element.click(); };
  const shot = async name => { await frame(); const value = await native("screenshot", { name: `${name}.png` }); assert(value.bytes > 1000 && value.width > 0, "Empty native screenshot"); screenshots.push(value); return value; };
  const run = async (name, requirements, work) => {
    try { const actual = await work(); cases.push({ id: name, requirements, result: "PASS", actual }); }
    catch (error) { cases.push({ id: name, requirements, result: "FAIL", actual: failure(error) }); try { await shot(`failure-${name}`); } catch {} }
  };
  try {
    await wait(() => document.querySelector(".app-shell") && document.querySelector(".film-card"), "first meaningful screen");
    const { appController: app } = await import("/src/state.ts");
    const { locale } = await import("/src/i18n.ts");
    const { paletteOptions } = await import("/src/ui/theme.ts");
    const { historyRepository } = await import("/src/data/history.ts");
    const { historyContentId } = await import("/src/data/history-model.ts");
    const { previewSourceId } = await import("/src/data/preview.ts");
    const { historyCompletionReady } = await import("/tests/fixtures/t13/history-completion.js");
    let traceHistory = false, historyOrder = 0;
    const videoElements = new WeakMap();
    let videoSequence = 0, completionAttempt = null;
    const playbackSnapshot = () => {
      const state = app.state.playback;
      const element = document.querySelector("video");
      if (element && !videoElements.has(element)) videoElements.set(element, ++videoSequence);
      return { phase: state.phase, epoch: state.playback_epoch, engineEpoch: state.engine_epoch,
        element: element ? videoElements.get(element) : null, ended: element?.ended === true,
        session: state.session?.session_id ?? null, resolution: state.resolution?.id ?? null,
        match: state.context?.match_id ?? null, video: state.context?.metadata_video_id ?? null,
        metadata: state.context ? JSON.stringify(state.context.metadata_ref) : null,
        source: state.context?.episode_ref.source_id ?? null, position: state.position_ms, duration: state.duration_ms,
        route: app.state.route.name, hash: location.hash };
    };
    const recordHistory = (stage, detail = {}) => {
      if (!traceHistory) return;
      historyTrace.push({ order: ++historyOrder, at: performance.now(), stage, playback: playbackSnapshot(), ...detail });
      if (historyTrace.length > 100) historyTrace.shift();
    };
    historyDiagnostic = () => ({ current: playbackSnapshot(), completionAttempt, trace: historyTrace.slice() });
    // Copies keep later playback/navigation from rewriting the real call and
    // commit evidence used by the completion precondition.
    for (const method of ["save", "list"]) {
      const original = historyRepository[method];
      historyRepository[method] = async (...args) => {
        const input = JSON.parse(JSON.stringify(args));
        const captured = playbackSnapshot();
        recordHistory(`${method}-called`, { input });
        try {
          const result = await original(...args);
          if (method === "save" && completionAttempt) completionAttempt.committed = {
            at: performance.now(), playback: captured, entry: input[0] };
          recordHistory(`${method}-committed`, { input, result: result === undefined ? null : JSON.parse(JSON.stringify(result)) });
          return result;
        } catch (error) { recordHistory(`${method}-failed`, { input, error: String(error) }); throw error; }
      };
    }
    for (const type of ["loadedmetadata", "durationchange", "playing", "pause", "seeking", "seeked", "ended"]) {
      document.addEventListener(type, event => {
        const video = event.target;
        if (!(video instanceof HTMLVideoElement) || !traceHistory) return;
        const media = { trusted: event.isTrusted, timestamp: event.timeStamp, position: video.currentTime,
          duration: Number.isFinite(video.duration) ? video.duration : null, ended: video.ended, paused: video.paused };
        recordHistory(`native-${type}`, { media });
        if (type === "ended" && completionAttempt) completionAttempt.nativeEnded = {
          type, ...media, at: performance.now(), playback: {
            ...playbackSnapshot(), element: videoElements.get(video) ?? null } };
        queueMicrotask(() => recordHistory(`delivered-${type}`, { media }));
      }, true);
    }
    const navigate = async (hash, selector) => {
      location.hash = hash;
      const [page, id, episode = "1"] = hash.replace(/^#\//, "").split("?")[0].split("/");
      await wait(() => location.hash === hash && document.querySelector(selector)
        && (page === "watch" ? app.state.route.name === "playback" && app.state.playback.context?.metadata_video_id === `${id}:${episode}` && app.state.playback.resolution
          : app.state.route.name === (page === "film" ? "details" : page)), `${hash} ${selector}`);
      await frame();
    };
    const seekMedia = seconds => {
      const slider = document.querySelector('.playback-seek input');
      assert(slider && !slider.disabled, "Actual player seek control missing");
      slider.value = String(seconds); slider.dispatchEvent(new Event('input', { bubbles: true }));
    };
    const layout = () => {
      const width = innerWidth;
      assert(document.documentElement.scrollWidth <= width + 1, `Horizontal overflow ${document.documentElement.scrollWidth}/${width}`);
      const main = document.querySelector("#main-content"); assert(visible(main), "Main content is hidden");
      for (const element of document.querySelectorAll(".header-actions select, .palette-grid, .settings-panel, .playback-surface, .history-card")) {
        if (!visible(element)) continue;
        const box = element.getBoundingClientRect();
        assert(box.left >= -1 && box.right <= width + 1, `Clipped ${element.className}: ${box.left}/${box.right}/${width}`);
      }
      return { width, height: innerHeight, scrollWidth: document.documentElement.scrollWidth, devicePixelRatio };
    };
    const rgba = value => {
      const values = value.match(/[\d.]+/g)?.map(Number); assert(values?.length >= 3, `Unrecognized CSS color ${value}`);
      return [values[0] / 255, values[1] / 255, values[2] / 255, values[3] ?? 1];
    };
    const mix = (fore, back) => fore.slice(0, 3).map((value, index) => value * fore[3] + back[index] * (1 - fore[3])).concat(1);
    const background = element => {
      const chain = []; for (let current = element; current; current = current.parentElement) chain.unshift(current);
      return chain.reduce((color, node) => mix(rgba(getComputedStyle(node).backgroundColor), color), [1, 1, 1, 1]);
    };
    const luminance = color => color.slice(0, 3).map(value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4).reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
    const contrast = element => {
      const back = background(element), front = mix(rgba(getComputedStyle(element).color), back);
      const values = [luminance(back), luminance(front)].sort((a, b) => a - b);
      return (values[1] + 0.05) / (values[0] + 0.05);
    };
    const textContrast = selectors => {
      const measured = [];
      for (const element of document.querySelectorAll(selectors)) if (visible(element) && element.textContent.trim()) {
        const value = contrast(element), style = getComputedStyle(element);
        const measurement = { selectors, tag: element.tagName, class: element.className, text: element.textContent.trim().slice(0, 50),
          contrast: value, color: style.color, background: background(element), palette: document.documentElement.dataset.palette,
          scheme: document.documentElement.style.colorScheme, lang: document.documentElement.lang };
        if (!Number.isFinite(value) || value < 4.5) throw Object.assign(new Error(`Contrast ${value.toFixed(2)}: ${element.className} ${measurement.text}`), { measurement });
        measured.push(measurement);
      }
      assert(measured.length > 0, "No contrast samples"); return measured;
    };
    const language = async value => {
      select(".language-picker", value);
      await wait(() => locale.value === value && document.documentElement.lang === value, `language ${value}`);
      await settledColors();
    };
    await run("identity-first-frame", ["U07", "R08", "R31", "R62", "R67", "REQ-T13"], async () => {
      const first = window.__T13_FIRST_FRAME__;
      assert(first && first.lang === "en" && first.palette === "lime", "Default first frame must use English/lime");
      assert(first.scheme === (first.dark ? "dark" : "light") && first.accent, "First-frame system theme is missing");
      assert(document.title.includes("Moc Video"), "English document title missing");
      assert(document.querySelector(".language-picker").getAttribute("aria-label") === "App language", "English accessibility name missing");
      assert(!document.querySelector("vite-error-overlay"), "Vite error overlay");
      assert(document.body.innerText.includes("Interface preview"), "Preview provenance missing");
      assert(app.state.selected_metadata_source.source_id !== app.state.selected_playback_source.source_id, "Metadata and playback sources conflated");
      await shot("desktop-home"); return { first, title: document.title, native: await native("identity"), layout: layout() };
    });
    await run("return-and-language-filter", ["U07", "R01", "R36", "R62", "REQ-T13"], async () => {
      click('.genre-bar button:nth-child(2)');
      await frame();
      const selected = document.querySelector('.genre-bar [aria-pressed="true"]');
      assert(selected, "Genre selection missing");
      const index = Array.from(selected.parentElement.children).indexOf(selected);
      assert(index === 1, "Genre click did not select Drama after Vue rendered");
      const link = document.querySelector(".film-card .poster-link"); assert(link, "Filtered catalog empty"); const href = link.getAttribute("href");
      link.click(); await wait(() => app.state.route.name === "details" && app.state.details.phase === "ready", "film details");
      const stable = JSON.stringify(app.state.details.data.metadata_ref);
      await language("zh-CN"); assert(document.title.includes("墨客视频"), "Chinese document title missing");
      assert(JSON.stringify(app.state.details.data.metadata_ref) === stable, "Language changed metadata identity");
      click('.topbar a[aria-label="返回"]'); await wait(() => app.state.route.name === "home", "return home"); await frame();
      assert(Array.from(document.querySelector(".genre-bar").children).indexOf(document.querySelector('.genre-bar [aria-pressed="true"]')) === index, "Back lost genre filter");
      const before = location.hash; await language("en"); assert(location.hash === before, "Language changed route");
      click(".palette-shortcut"); await wait(() => document.querySelector('input[name="quick-color-palette"]'), "quick appearance");
      document.querySelector('input[name="quick-color-palette"][value="violet"]').click();
      await wait(() => app.state.settings.color_palette === "violet", "quick palette confirmation");
      click(".appearance-close"); await wait(() => !document.querySelector('[role="dialog"]'), "close appearance");
      assert(Array.from(document.querySelector(".genre-bar").children).indexOf(document.querySelector('.genre-bar [aria-pressed="true"]')) === index, "Palette lost genre filter");
      return { href, genreIndex: index, stableMetadata: stable };
    });
    await run("palette-language-system-matrix", ["U07", "R08", "R20", "R40", "R62", "R67", "REQ-T13"], async () => {
      await navigate("#/settings", ".palette-grid");
      await native("resize", { width: 390, height: 844, zoom: 1 }); await frame();
      const samples = [];
      for (const palette of paletteOptions) for (const dark of [false, true]) for (const lang of ["en", "zh-CN"]) {
        await native("appearance", { dark });
        await wait(() => matchMedia("(prefers-color-scheme: dark)").matches === dark && document.documentElement.style.colorScheme === (dark ? "dark" : "light"), "dynamic OS appearance");
        await language(lang);
        const radio = document.querySelector(`input[name="color-palette"][value="${palette.value}"]`);
        assert(radio, `Missing palette control ${palette.value}`);
        radio.click();
        await wait(() => app.state.settings.color_palette === palette.value && document.documentElement.dataset.palette === palette.value, "palette confirmation");
        await settledColors();
        const accent = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
        assert(accent === (dark ? palette.night : palette.day), "System and Naive palette disagree");
        const stored = JSON.parse(localStorage.getItem("moc-video.settings"));
        assert(stored.ui_locale === lang && stored.color_palette === palette.value, "Queued Settings lost a preference");
        assert(document.querySelector(".setting-select").value === lang, "Topbar and settings language disagree");
        const measured = textContrast(".setting-row h3, .setting-row p, .system-mode, .palette-name, .language-picker, .setting-select, .page-heading h1");
        const screenshot = await shot(`matrix-${palette.value}-${dark ? "dark" : "light"}-${lang}`);
        click(".palette-shortcut"); await wait(() => document.querySelector('[role="dialog"]'), "appearance popup"); await frame();
        const popup = textContrast('[role="dialog"] .n-card-header__main, [role="dialog"] p, [role="dialog"] button');
        click(".appearance-close"); await wait(() => !document.querySelector('[role="dialog"]'), "close appearance popup");
        await navigate("#/home", ".film-card");
        const catalog = textContrast(".poster-lettering strong, .card-title, .mobile-nav a, .primary-nav a.active");
        await navigate("#/watch/midnight/1", "video");
        await wait(() => document.querySelector("video")?.readyState >= 1, "matrix player metadata");
        document.querySelector("video").pause();
        const player = textContrast(".playback-controls label, .playback-controls select, .playback-controls button");
        document.querySelector("video").src = "/t13-intentionally-missing-media.mp4";
        await wait(() => document.querySelector(".playback-error"), "matrix media error");
        const errors = textContrast(".playback-error .n-alert-body__content");
        const errorScreenshot = await shot(`error-${palette.value}-${dark ? "dark" : "light"}-${lang}`);
        await navigate("#/settings", ".palette-grid");
        samples.push({ palette: palette.value, dark, lang, accent, layout: layout(), contrast: measured, popupContrast: popup, catalogContrast: catalog, playerContrast: player, errorContrast: errors, screenshot, errorScreenshot });
      }
      assert(samples.length === 20, "Incomplete theme matrix");
      return samples;
    });
    await run("minimum-window-and-dpi", ["U07", "R20", "R40", "R52", "REQ-T13"], async () => {
      await language("en");
      const samples = [];
      for (const zoom of [1, 1.25, 1.5, 2]) {
        const nativeState = await native("resize", { width: 960, height: 640, zoom }); await frame();
        samples.push({ native: nativeState, layout: layout(), screenshot: await shot(`dpi-${zoom}`) });
        assert(nativeState.width === 960 && nativeState.height === 640 && nativeState.zoom === zoom, "Native minimum/zoom differs");
      }
      await native("resize", { width: 1200, height: 800, zoom: 1 }); await frame();
      return samples;
    });
    await run("storage-failure-feedback", ["R01", "R31", "R62", "R67", "REQ-T13"], async () => {
      const previous = app.state.settings.ui_locale, previousPalette = app.state.settings.color_palette;
      const original = Storage.prototype.setItem;
      Storage.prototype.setItem = function () { throw new Error("T13 storage unavailable"); };
      try {
        select(".language-picker", previous === "en" ? "zh-CN" : "en");
        await wait(() => document.querySelector(".preference-alert"), "explicit preference failure"); await frame();
        assert(app.state.settings.ui_locale === previous && document.querySelector(".language-picker").value === previous, "Failed save changed confirmed locale/control");
        document.querySelector('input[name="color-palette"][value="lime"]').click();
        await wait(() => document.querySelector(".preference-alert")?.textContent.includes("T13 storage unavailable"), "palette save failure");
        assert(app.state.settings.color_palette === previousPalette, "Failed save changed confirmed palette");
        await shot("storage-failure");
      } finally { Storage.prototype.setItem = original; }
      click(".preference-alert button"); await wait(() => !document.querySelector(".preference-alert"), "preference recovery");
      await language("en");
      return { confirmedLocale: previous, confirmedPalette: previousPalette, recovered: true };
    });
    await run("playback-locale-palette-and-shortcuts", ["U07", "R01", "R31", "R36", "R40", "R62", "R67", "REQ-T13"], async () => {
      await navigate("#/watch/midnight/2", "video");
      await wait(() => document.querySelector("video")?.readyState >= 1, "real local video metadata");
      const video = document.querySelector("video"); video.pause(); seekMedia(6);
      await wait(() => Math.abs(video.currentTime - 6) < 0.5, "real media seek");
      const identity = JSON.stringify(app.state.playback.context), epoch = app.state.playback.playback_epoch;
      await language("zh-CN");
      assert(document.querySelector("video") === video && Math.abs(video.currentTime - 6) < 0.5, "Locale reset active media");
      assert(JSON.stringify(app.state.playback.context) === identity && app.state.playback.playback_epoch === epoch, "Locale changed playback identity/epoch");
      click(".palette-shortcut"); await wait(() => document.querySelector('input[name="quick-color-palette"]'), "appearance during playback");
      document.querySelector('input[name="quick-color-palette"][value="ocean"]').click();
      await wait(() => app.state.settings.color_palette === "ocean", "playback palette confirmation");
      document.querySelector("#main-content").dispatchEvent(new KeyboardEvent("keydown", { code: "Space", key: " ", bubbles: true, cancelable: true }));
      assert(video.paused, "Player shortcut acted through a modal");
      click(".appearance-close"); await wait(() => !document.querySelector('[role="dialog"]'), "close playback appearance");
      assert(document.querySelector("video") === video && Math.abs(video.currentTime - 6) < 0.5 && JSON.stringify(app.state.playback.context) === identity && app.state.playback.playback_epoch === epoch, "Palette reset playback state");
      await native("appearance", { dark: false }); await wait(() => document.documentElement.style.colorScheme === "light", "light playback");
      await native("appearance", { dark: true }); await wait(() => document.documentElement.style.colorScheme === "dark", "dark playback");
      assert(document.querySelector("video") === video && JSON.stringify(app.state.playback.context) === identity, "OS mode replaced playback");
      const input = document.querySelector('.playback-controls input[type="range"]'); input.focus();
      const before = video.currentTime;
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }));
      assert(Math.abs(video.currentTime - before) < 0.5, "Player shortcut stole focused form input");
      document.querySelector("#main-content").focus();
      document.querySelector("#main-content").dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }));
      await wait(() => video.currentTime >= before + 4, "contextual player seek");
      await native("resize", { width: 390, height: 844, zoom: 1 }); await frame();
      const mobile = layout(); await shot("mobile-player");
      assert(Array.from(document.querySelectorAll(".playback-controls button")).every(button => !!(button.getAttribute("aria-label") || button.textContent.trim())), "Unnamed playback controls");
      return { videoWidth: video.videoWidth, duration: video.duration, position: video.currentTime, identity, epoch, mobile };
    });
    await run("history-resume-merge-delete-clear-focus", ["U09", "R01", "R08", "R20", "R31", "R40", "REQ-T13"], async () => {
      traceHistory = true;
      recordHistory("history-flow-started");
      await language("en");
      const video = document.querySelector("video"); assert(video && video.currentTime > 0, "No actual playback progress"); video.pause();
      await navigate("#/history", ".history-card");
      assert(document.querySelectorAll(".history-card").length === 1, "History must merge one title");
      assert(document.body.innerText.includes("Preview history resets when you reload this page."), "Missing session-only history notice");
      assert(document.querySelector(".history-card").textContent.includes("Episode 2"), "Saved episode missing");
      assert(document.querySelector(".history-group h2").textContent === "Today", "Recent history group missing");
      click(".history-actions button:first-child"); await wait(() => document.querySelector("video")?.currentTime > 5, "resume actual saved progress");
      await wait(() => !document.querySelector(".resume-error"), "resume complete");
      const resumed = document.querySelector("video").currentTime; document.querySelector("video").pause();
      await navigate("#/watch/midnight/3", "video"); await wait(() => document.querySelector("video")?.readyState >= 1, "next episode metadata");
      const next = document.querySelector("video"); next.pause(); seekMedia(8); await wait(() => Math.abs(next.currentTime - 8) < 0.5, "next episode seek");
      await navigate("#/history", ".history-card");
      assert(document.querySelectorAll(".history-card").length === 1 && document.querySelector(".history-card").textContent.includes("Episode 3"), "Latest episode did not merge");
      await navigate("#/settings", ".settings-panel");
      const autoNext = document.querySelector('[role="switch"]');
      if (autoNext.getAttribute("aria-checked") === "true") autoNext.click();
      await wait(() => app.state.settings.auto_next === false, "disable automatic next episode");
      await navigate("#/watch/midnight/3", "video"); await wait(() => document.querySelector("video")?.readyState >= 1, "completion metadata");
      const finishing = document.querySelector("video");
      const context = app.state.playback.context;
      assert(context?.metadata_video_id === "midnight:3" && app.state.playback.session, "Completion requires the current episode/session");
      const started = performance.now();
      const expected = { ...playbackSnapshot(), contentId: historyContentId(context.metadata_ref.identity.type, context.metadata_ref.identity.meta_id),
        episode: Number(context.metadata_video_id.split(":").at(-1)), historySource: previewSourceId(app.state.selected_playback_source),
        started, deadline: started + 12000 };
      completionAttempt = { expected, nativeEnded: null, committed: null };
      let historyCompletion;
      seekMedia(finishing.duration - 0.2); await finishing.play();
      // WebKit may expose ended before dispatching the trusted event. Keep the
      // original single 12s budget through controller handling and save commit.
      await wait(() => {
        const current = playbackSnapshot(), observedAt = performance.now();
        if (!historyCompletionReady(expected, current, completionAttempt.nativeEnded, completionAttempt.committed, observedAt)) return false;
        historyCompletion = { expected, current, ...completionAttempt, observedAt };
        return true;
      }, "current trusted ended, controller completion and history commit", Math.max(0, expected.deadline - performance.now()));
      completionAttempt = null;
      recordHistory("completion-precondition-met", { completion: historyCompletion });
      await navigate("#/history", ".history-card");
      recordHistory("replay-action-read", { action: document.querySelector(".history-actions button").textContent.trim() });
      assert(document.querySelector(".history-actions button").textContent.trim() === "Play again", "Completed record has no replay action");
      click(".history-actions button:first-child");
      await wait(() => document.querySelector("video")?.readyState >= 1 && document.querySelector("video").currentTime < 2, "replay starts at zero");
      const replayed = document.querySelector("video").currentTime; document.querySelector("video").pause();
      await navigate("#/history", ".history-card");
      const remove = document.querySelector(".history-actions button:last-child"); remove.focus(); remove.click();
      await wait(() => document.querySelector('.history-confirmation'), "delete confirmation"); await frame();
      const modal = document.querySelector('.history-confirmation');
      assert(modal.getAttribute("role") === "dialog" && modal.getAttribute("aria-modal") === "true"
        && modal.getAttribute("aria-label") === "Remove from history", "History confirmation role/name/modal semantics differ");
      assert(modal.contains(document.activeElement), "Confirmation did not capture focus");
      assert(document.querySelectorAll(".history-card").length === 1, "History deleted before confirmation");
      textContrast('.history-confirmation .n-dialog__title, .history-confirmation .n-dialog__content');
      Array.from(modal.querySelectorAll("button")).find(button => button.textContent.trim() === "Cancel").click();
      await wait(() => !document.querySelector('.history-confirmation'), "cancel deletion"); await new Promise(resolve => setTimeout(resolve, 350));
      assert(document.activeElement === remove, "Confirmation did not restore trigger focus");
      remove.click(); await wait(() => document.querySelector('.history-confirmation[role="dialog"][aria-modal="true"]'), "repeat confirmation");
      Array.from(document.querySelectorAll('.history-confirmation button')).find(button => button.textContent.trim() === "Remove").click();
      await wait(() => !document.querySelector(".history-card"), "single-title removal");
      await navigate("#/watch/midnight/1", "video"); await wait(() => document.querySelector("video")?.readyState >= 1, "new playback");
      const again = document.querySelector("video"); again.pause(); seekMedia(4); await wait(() => Math.abs(again.currentTime - 4) < 0.5, "new progress");
      await navigate("#/history", ".history-card");
      Array.from(document.querySelectorAll("button")).find(button => button.textContent.trim() === "Clear history").click();
      await wait(() => document.querySelector('.history-confirmation[role="dialog"][aria-modal="true"]'), "clear confirmation");
      assert(document.querySelectorAll(".history-card").length === 1, "Clear ran before confirmation");
      await shot("history-clear-confirmation");
      Array.from(document.querySelectorAll('.history-confirmation button')).find(button => button.textContent.trim() === "Clear history").click();
      await wait(() => !document.querySelector(".history-card"), "clear history");
      traceHistory = false;
      return { resumedPosition: resumed, replayedPosition: replayed, mergedTitles: 1, latestEpisode: 3, cancelledDeletionPreservedData: true,
        dialogRole: modal.getAttribute("role"), focusRestored: true, removed: true, cleared: true, historyCompletion, history: historyDiagnostic() };
    });
    await run("error-and-poster-contrast", ["U07", "R20", "R40", "R67", "REQ-T13"], async () => {
      await navigate("#/watch/midnight/1", "video");
      document.querySelector("video").src = "/t13-intentionally-missing-media.mp4";
      await wait(() => document.querySelector(".playback-error"), "real media network error");
      const measured = textContrast(".playback-error .n-alert-body__content, .playback-controls label");
      await shot("media-error");
      await navigate("#/home", ".film-card");
      const posters = textContrast(".poster-lettering strong, .card-title");
      await shot("mobile-home");
      return { mediaError: measured, posters, layout: layout() };
    });
    await run("console-and-overlay-health", ["U07", "R08", "REQ-T13"], async () => {
      assert(!document.querySelector("vite-error-overlay"), "Framework overlay present");
      assert(window.__T13_LOGS__.length === 0, `Console: ${JSON.stringify(window.__T13_LOGS__)}`);
      return { logs: window.__T13_LOGS__, title: document.title, url: location.href };
    });
    await run("history-reload-precondition", ["U09", "R01", "REQ-T13"], async () => {
      await navigate("#/watch/midnight/2", "video"); await wait(() => document.querySelector("video")?.readyState >= 1, "reload-precondition media");
      const video = document.querySelector("video"); video.pause(); seekMedia(5);
      await wait(() => Math.abs(video.currentTime - 5) < 0.5, "reload-precondition seek");
      await navigate("#/history", ".history-card");
      assert(document.querySelectorAll(".history-card").length === 1, "Reload precondition requires saved history");
      return { visibleTitlesBeforeReload: 1 };
    });
    await native("reload-history", { report: { ok: cases.every(row => row.result === "PASS"), cases, screenshots,
      browser: "native macOS WKWebView", browserPlugin: "Browser plugin not available; reused native WebKit without adding browser dependencies", url: location.href } });
  } catch (error) {
    await native("finish", { report: { ok: false, error: failure(error), cases, screenshots } });
  }
})();
