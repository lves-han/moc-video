// Imported only by Vite's t01 mode. The production artifact gate verifies removal.
import { capabilities, nativeInvoke, type PlaybackSource, type Player } from "../../../src/player";

declare global {
  interface Window { __T01_BOOTSTRAP__?: { base: string; cap: string } }
}
type Command = { id: number; op: string; kind?: "mp4" | "hls"; seconds?: number; value?: number };
type NativeVisibility = { visible: boolean; focused: boolean; minimized: boolean };
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
function assertCondition(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
async function until(predicate: () => boolean, message: string, timeout = 15000) {
  const end = performance.now() + timeout;
  while (!predicate()) {
    if (performance.now() >= end) throw new Error(message);
    await delay(50);
  }
}

export async function automate(video: HTMLVideoElement, player: Player, open: (source: PlaybackSource) => unknown) {
  const config = window.__T01_BOOTSTRAP__;
  assertCondition(config, "T01 automation requires the native test bootstrap");
  const { base, cap } = config;
  const url = (path: string) => `${base}${path}?cap=${cap}`;
  const post = async (path: string, value: unknown) => {
    const response = await fetch(url(path), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(value), credentials: "omit" });
    assertCondition(response.ok, "Fixture telemetry submission failed");
  };
  let running = true;
  let frames = 0;
  let frameId = 0;
  let raf = 0;
  let rafId = 0;
  let maxRafGap = 0;
  let lastRaf = performance.now();
  let lastFrame: { time: number; media_time: number; presented_frames: number } | null = null;
  const onFrame: VideoFrameRequestCallback = (time, metadata) => {
    frames++;
    lastFrame = { time, media_time: metadata.mediaTime, presented_frames: metadata.presentedFrames };
    if (running) frameId = video.requestVideoFrameCallback(onFrame);
  };
  assertCondition(typeof video.requestVideoFrameCallback === "function", "WebView frame presentation callback required");
  frameId = video.requestVideoFrameCallback(onFrame);
  const pulse = () => { const now = performance.now(); maxRafGap = Math.max(maxRafGap, now - lastRaf); lastRaf = now; raf++; if (running) rafId = requestAnimationFrame(pulse); };
  rafId = requestAnimationFrame(pulse);
  const audio = new AudioContext();
  const audioSource = audio.createMediaElementSource(video);
  const analyser = audio.createAnalyser();
  analyser.fftSize = 2048;
  audioSource.connect(analyser);
  analyser.connect(audio.destination);
  const bins = new Float32Array(analyser.fftSize);
  const spectrum = new Float32Array(analyser.frequencyBinCount);
  const errors: string[] = [];
  window.addEventListener("error", event => errors.push(event.message));
  window.addEventListener("unhandledrejection", event => errors.push(String(event.reason)));
  const tracing = new AbortController();
  const events: Record<string, unknown>[] = [];
  let foregroundInterruptions = 0;
  let lastTimeUpdate = 0;
  const trace = (event: Event) => {
    if ((event.type === "visibilitychange" || event.type === "blur") &&
        (document.visibilityState !== "visible" || !document.hasFocus())) foregroundInterruptions++;
    events.push({ type: event.type, time: performance.now(), position: video.currentTime,
      paused: video.paused, seeking: video.seeking, ready_state: video.readyState,
      network_state: video.networkState, visibility: document.visibilityState, focused: document.hasFocus() });
    if (events.length > 64) events.shift();
  };
  for (const type of ["play", "playing", "pause", "waiting", "stalled", "seeking", "seeked", "ended", "error", "emptied", "suspend", "abort"])
    video.addEventListener(type, trace, { signal: tracing.signal });
  video.addEventListener("timeupdate", () => { lastTimeUpdate = performance.now(); }, { signal: tracing.signal });
  document.addEventListener("visibilitychange", trace, { signal: tracing.signal });
  for (const type of ["focus", "blur", "pagehide", "pageshow"])
    window.addEventListener(type, trace, { signal: tracing.signal });
  const sample = () => {
    analyser.getFloatTimeDomainData(bins);
    const rms = Math.sqrt(bins.reduce((sum, x) => sum + x * x, 0) / bins.length);
    analyser.getFloatFrequencyData(spectrum);
    let peak = 0;
    for (let i = 1; i < spectrum.length; i++) if (spectrum[i] > spectrum[peak]) peak = i;
    const quality = video.getVideoPlaybackQuality();
    return { time: performance.now(), position: video.currentTime, duration: video.duration,
      paused: video.paused, ended: video.ended, ready_state: video.readyState,
      seeking: video.seeking, network_state: video.networkState, loop: video.loop,
      playback_rate: video.playbackRate, volume: video.volume, muted: video.muted,
      buffered: Array.from({ length: video.buffered.length }, (_, i) => [video.buffered.start(i), video.buffered.end(i)]),
      visibility: document.visibilityState, focused: document.hasFocus(), video_rect: video.getBoundingClientRect().toJSON(),
      ui_position: Number(document.querySelector<HTMLInputElement>("[aria-label='播放进度']")?.value ?? NaN),
      foreground_interruptions: foregroundInterruptions,
      last_frame: lastFrame, last_raf_ms: lastRaf, last_timeupdate_ms: lastTimeUpdate,
      width: video.videoWidth, height: video.videoHeight, frames, raf, max_raf_gap_ms: maxRafGap,
      total_frames: quality.totalVideoFrames, dropped_frames: quality.droppedVideoFrames,
      audio_rms: rms, audio_peak_hz: peak * audio.sampleRate / analyser.fftSize, audio_state: audio.state, audio_sample_rate: audio.sampleRate,
      output_channels: audio.destination.maxChannelCount, media_error: video.error?.code ?? null,
      diagnostics: player.diagnostics() };
  };
  const controls = () => [...document.querySelectorAll<HTMLElement>(".prototype-controls button, .prototype-controls input, .prototype-controls select")];
  const targets = () => [...document.querySelectorAll<HTMLElement>(".prototype-controls button, .prototype-controls label")];
  const rect = (el: HTMLElement) => {
    const r = el.getBoundingClientRect();
    return { label: el.getAttribute("aria-label") ?? el.textContent, left: r.left, right: r.right,
      top: r.top, bottom: r.bottom, width: r.width, height: r.height };
  };
  const layout = () => {
    return { inner_width: innerWidth, inner_height: innerHeight, dpr: devicePixelRatio,
      scroll_width: document.documentElement.scrollWidth,
      controls: controls().map(rect), targets: targets().map(rect),
      video_rect: video.getBoundingClientRect().toJSON() };
  };
  const usableLayout = async () => {
    const observed = layout();
    assertCondition(observed.scroll_width <= observed.inner_width + 1, "Horizontal overflow");
    assertCondition(observed.controls.every(c => c.left >= 0 && c.right <= innerWidth + 1 && c.width > 0 && c.height > 0), "Control clipped or empty");
    // Labels provide the clickable area for native checkboxes and selects.
    assertCondition(observed.targets.every(c => c.width >= 44 && c.height >= 44), "Control target too small");
    const reachable = [];
    for (const target of targets()) {
      target.scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" });
      await delay(50);
      const r = rect(target);
      const hit = document.elementFromPoint((r.left + r.right) / 2, (r.top + r.bottom) / 2);
      assertCondition(r.top >= 0 && r.bottom <= innerHeight + 1 && hit && target.contains(hit), "Control target is unreachable or obscured");
      reachable.push(r);
    }
    return { ...layout(), reachable_targets: reachable };
  };
  const click = async (action: string) => {
    const button = document.querySelector<HTMLButtonElement>(`[data-action="${action}"]`);
    assertCondition(button, `Missing ${action} control`);
    button.click();
    await delay(100);
  };
  let longObservation: unknown = null;
  const foreground = async () => {
    const requested = await nativeInvoke<NativeVisibility>("t01_control", { action: "foreground" });
    await until(() => document.visibilityState === "visible" && document.hasFocus(),
      "Foreground activation failed: WebView must be visible and focused");
    const confirmed = await nativeInvoke<NativeVisibility>("t01_control", { action: "identity" });
    assertCondition(confirmed.visible && confirmed.focused && !confirmed.minimized,
      "Foreground activation failed: native window must be visible, focused and restored");
    return { requested, confirmed, visibility: document.visibilityState, focused: document.hasFocus() };
  };
  async function execute(command: Command): Promise<unknown> {
    switch (command.op) {
      case "identity": return { foreground: await foreground(), origin: location.origin, secure_context: isSecureContext,
        user_agent: navigator.userAgent, capabilities: capabilities(video), native: await nativeInvoke("t01_control", { action: "identity" }), layout: layout() };
      case "open": {
        const activated = await foreground();
        const kind = command.kind ?? "mp4";
        const opened = open({ url: url(`media/${kind === "mp4" ? "clip.mp4" : "index.m3u8"}`), kind, subtitle: url("media/subtitles.vtt") });
        await until(() => video.readyState >= 2 || Boolean(video.error), "Media did not become ready");
        assertCondition(!video.error, `MediaError ${video.error?.code}`);
        await click("play");
        await until(() => !video.paused && video.currentTime > 0.2, "Playback requires a user gesture or failed to advance");
        await audio.resume();
        await delay(500);
        return { opened, foreground: activated, sample: sample(), native: await nativeInvoke("t01_control", { action: "identity" }) };
      }
      case "controls": {
        await click("play");
        assertCondition(video.paused, "Pause control failed");
        const paused = video.currentTime;
        await delay(350);
        assertCondition(Math.abs(video.currentTime - paused) < 0.1, "Paused media advanced");
        const seek = document.querySelector<HTMLInputElement>("[aria-label='播放进度']")!;
        seek.value = "2"; seek.dispatchEvent(new Event("input", { bubbles: true }));
        await until(() => !video.seeking && Math.abs(video.currentTime - 2) < 0.3, "Seek control failed");
        player.selectSubtitle(0);
        await until(() => Boolean(video.textTracks[0]?.activeCues?.length), "Subtitle cue was not rendered");
        const cue = (video.textTracks[0].activeCues![0] as VTTCue).text;
        assertCondition(cue.includes("映象"), "Unexpected subtitle cue");
        player.selectSubtitle(null);
        assertCondition(video.textTracks[0].mode === "disabled", "Subtitle disable failed");
        player.selectSubtitle(0);
        const volume = document.querySelector<HTMLInputElement>("[aria-label='音量']")!;
        volume.value = "0.5"; volume.dispatchEvent(new Event("input", { bubbles: true }));
        assertCondition(video.volume === 0.5, "Volume control failed");
        const speed = document.querySelector<HTMLSelectElement>("[aria-label='倍速']")!;
        speed.value = "1.5"; speed.dispatchEvent(new Event("change", { bubbles: true }));
        assertCondition(video.playbackRate === 1.5, "Speed control failed");
        player.setSpeed(1); player.setVolume(1);
        await click("play");
        await until(() => video.currentTime > 2.5, "Resume control failed");
        const before = await nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });
        await click("fullscreen");
        await delay(1500);
        const full = await nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });
        assertCondition(full.fullscreen !== before.fullscreen, "Native fullscreen control failed");
        await click("fullscreen"); await delay(1500);
        const restored = await nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });
        assertCondition(restored.fullscreen === before.fullscreen, "Native fullscreen exit failed");
        return { paused_position: paused, cue, fullscreen: full, restored, sample: sample() };
      }
      case "layout": {
        await nativeInvoke("t01_control", { action: "width", value: command.value });
        await delay(350);
        const observed = await usableLayout();
        assertCondition(Math.abs(observed.inner_width - command.value!) <= 2, "Native resize did not produce requested CSS width");
        return { native: await nativeInvoke("t01_control", { action: "identity" }), layout: observed };
      }
      case "zoom": {
        await nativeInvoke("t01_control", { action: "zoom", value: command.value });
        await delay(350);
        const observed = await usableLayout();
        assertCondition(Math.abs(observed.inner_width - 1200 / command.value!) <= 3, "Native zoom did not change effective CSS scale");
        return { requested_zoom: command.value, native: await nativeInvoke("t01_control", { action: "identity" }), layout: observed };
      }
      case "security": {
        const valid = await fetch(url("media/clip.mp4"), { headers: { Range: "bytes=0-15" } });
        assertCondition(valid.status === 206 && (await valid.arrayBuffer()).byteLength === 16, "Authenticated WebView Range failed");
        const invalid = await fetch(`${base}media/clip.mp4?cap=invalid`);
        assertCondition(invalid.status === 403, "Invalid capability accepted");
        let corsDenied = false;
        try { await fetch(url("probe/cors-denied")); } catch { corsDenied = true; }
        assertCondition(corsDenied, "WebView did not enforce CORS");
        const violations: string[] = [];
        const listener = (e: SecurityPolicyViolationEvent) => violations.push(e.effectiveDirective);
        document.addEventListener("securitypolicyviolation", listener);
        const script = document.createElement("script"); script.src = url("probe/script.js"); document.head.append(script);
        const inline = document.createElement("script"); inline.textContent = "window.__T01_INLINE_EXECUTED__=true"; document.head.append(inline);
        try { await fetch("https://example.invalid/t01"); } catch { /* CSP evidence collected below. */ }
        await delay(500);
        script.remove(); inline.remove(); document.removeEventListener("securitypolicyviolation", listener);
        assertCondition(violations.some(v => v.startsWith("script-src")) && violations.includes("connect-src"), "CSP did not block external script/connect");
        assertCondition(!("__T01_INLINE_EXECUTED__" in window), "Inline script executed");
        const denied: string[] = [];
        for (const command of ["execute_command", "plugin:fs|read_text_file", "plugin:shell|execute"]) {
          try { await nativeInvoke(command, { path: "/etc/hosts", program: "echo" }); }
          catch { denied.push(command); }
        }
        assertCondition(denied.length === 3, "Unexpected privileged IPC command available");
        return { cors_denied: corsDenied, csp_violations: violations, denied_commands: denied, range_status: valid.status };
      }
      case "long": {
        const seconds = command.seconds!;
        assertCondition(seconds === 1800, "Acceptance duration must be 1800 seconds per format");
        const activated = await foreground();
        video.loop = true; player.setSpeed(1); player.setVolume(1);
        const started = performance.now();
        const initial = sample();
        let previous = initial;
        let elapsedMedia = 0;
        let samples = 0;
        const signatures = new Set<string>();
        const canvas = document.createElement("canvas"); canvas.width = 64; canvas.height = 36;
        const context = canvas.getContext("2d", { willReadFrequently: true })!;
        let preview = "";
        while (performance.now() - started < seconds * 1000) {
          await delay(5000);
          const current = sample();
          // Capture before assertions: the failing interval never reaches control/sample.
          longObservation = { index: samples, elapsed_ms: performance.now() - started,
            previous, current, foreground: activated, events: [...events] };
          assertCondition(current.visibility === "visible" && current.focused &&
            current.foreground_interruptions === initial.foreground_interruptions,
            "Foreground condition lost during long playback");
          assertCondition(!current.paused && !current.media_error && current.frames > previous.frames && current.raf > previous.raf, "Playback or UI stalled");
          const native = await nativeInvoke<NativeVisibility>("t01_control", { action: "identity" });
          longObservation = { ...(longObservation as object), native };
          assertCondition(native.visible && native.focused && !native.minimized,
            "Foreground condition lost during long playback: native window");
          // The existing player throttles position publication to 250ms. Allow
          // one second for event delivery while checking the actual Vue control.
          assertCondition(Number.isFinite(current.ui_position) && current.ui_position !== previous.ui_position &&
            current.ui_position >= 0 && current.ui_position <= current.duration &&
            Math.min(Math.abs(current.ui_position - current.position),
              Math.abs(current.duration - Math.abs(current.ui_position - current.position))) <= 1,
            "Playback progress UI stalled or differs from media clock");
          assertCondition(current.width === 1920 && current.height === 1080, "Unexpected decoded resolution");
          assertCondition(current.audio_state === "running" && current.audio_rms > 0.001, "Decoded audio output is silent or suspended");
          assertCondition(Math.abs(current.audio_peak_hz - 440) < 30, "Decoded audio tone frequency differs");
          assertCondition(current.position !== previous.position, "Media clock stalled");
          elapsedMedia += current.position >= previous.position ? current.position - previous.position : current.duration - previous.position + current.position;
          context.drawImage(video, 0, 0, 64, 36);
          const pixels = context.getImageData(0, 0, 64, 36).data;
          assertCondition(pixels.some((v, i) => i % 4 !== 3 && v > 32), "Decoded frame is black");
          let signature = 2166136261;
          for (const byte of pixels) signature = Math.imul(signature ^ byte, 16777619);
          signatures.add(String(signature >>> 0));
          if (!preview) preview = canvas.toDataURL("image/png");
          await post("control/sample", { id: command.id, index: samples++, elapsed_ms: performance.now() - started, ...current, frame_signature: signature >>> 0,
            native });
          previous = current;
        }
        assertCondition(elapsedMedia >= seconds - 30 && signatures.size > 2, "Insufficient continuous playback or changing frames");
        return { duration_seconds: (performance.now() - started) / 1000, media_seconds: elapsedMedia,
          samples, unique_frames: signatures.size, foreground: activated, initial, final: sample(), decoded_preview: preview };
      }
      case "stop": {
        await click("stop");
        await delay(500);
        const before = frames;
        await delay(500);
        assertCondition(video.paused && !video.currentSrc && !video.hasAttribute("src") && video.querySelectorAll("track").length === 0, "Video resources remain after stop");
        assertCondition(!player.diagnostics().listening && player.diagnostics().hls_instances === 0 && frames <= before + 1, "Old playback still active");
        return { sample: sample(), source_removed: true, frames_after_stop: frames - before };
      }
      case "shutdown": {
        player.stop(); running = false;
        tracing.abort();
        video.cancelVideoFrameCallback(frameId); cancelAnimationFrame(rafId);
        audioSource.disconnect(); analyser.disconnect(); await audio.close();
        return { audio_state: audio.state, errors, diagnostics: player.diagnostics() };
      }
      default: throw new Error(`Unknown automation command ${command.op}`);
    }
  }
  await post("control/result", { id: 0, ok: true, value: { ready: true } });
  while (running) {
    const response = await fetch(url("control/next"), { cache: "no-store" });
    if (response.status === 204) continue;
    assertCondition(response.ok, "Fixture command transport failed");
    const command = await response.json() as Command;
    longObservation = null;
    try { await post("control/result", { id: command.id, ok: true, value: await execute(command) }); }
    catch (cause) {
      const diagnostics = { long: longObservation, current: sample(), events: [...events],
        native: null as unknown, native_error: null as string | null };
      try { diagnostics.native = await nativeInvoke("t01_control", { action: "identity" }); }
      catch (error) { diagnostics.native_error = String(error); }
      await post("control/result", { id: command.id, ok: false, error: String(cause), diagnostics });
    }
  }
  await nativeInvoke("t01_control", { action: "quit" });
}
