// Imported only by Vite's t01 mode. The production artifact gate verifies removal.
import { capabilities, nativeInvoke, type PlaybackSource, type Player } from "../../../src/player";

declare global {
  interface Window { __T01_BOOTSTRAP__?: { base: string; cap: string } }
}
type Command = { id: number; op: string; kind?: "mp4" | "hls"; seconds?: number; value?: number };
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
function require(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
async function until(predicate: () => boolean, message: string, timeout = 15000) {
  const end = performance.now() + timeout;
  while (!predicate()) {
    if (performance.now() >= end) throw new Error(message);
    await delay(50);
  }
}

export async function automate(video: HTMLVideoElement, player: Player, open: (source: PlaybackSource) => unknown) {
  const config = window.__T01_BOOTSTRAP__;
  require(config, "T01 automation requires the native test bootstrap");
  const { base, cap } = config;
  const url = (path: string) => `${base}${path}?cap=${cap}`;
  const post = async (path: string, value: unknown) => {
    const response = await fetch(url(path), { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(value), credentials: "omit" });
    require(response.ok, "Fixture telemetry submission failed");
  };
  let running = true;
  let frames = 0;
  let frameId = 0;
  let raf = 0;
  let rafId = 0;
  let maxRafGap = 0;
  let lastRaf = performance.now();
  const onFrame = () => { frames++; if (running) frameId = video.requestVideoFrameCallback(onFrame); };
  require(typeof video.requestVideoFrameCallback === "function", "WebView frame presentation callback required");
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
  const sample = () => {
    analyser.getFloatTimeDomainData(bins);
    const rms = Math.sqrt(bins.reduce((sum, x) => sum + x * x, 0) / bins.length);
    analyser.getFloatFrequencyData(spectrum);
    let peak = 0;
    for (let i = 1; i < spectrum.length; i++) if (spectrum[i] > spectrum[peak]) peak = i;
    const quality = video.getVideoPlaybackQuality();
    return { time: performance.now(), position: video.currentTime, duration: video.duration,
      paused: video.paused, ended: video.ended, ready_state: video.readyState,
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
    require(observed.scroll_width <= observed.inner_width + 1, "Horizontal overflow");
    require(observed.controls.every(c => c.left >= 0 && c.right <= innerWidth + 1 && c.width > 0 && c.height > 0), "Control clipped or empty");
    // Labels provide the clickable area for native checkboxes and selects.
    require(observed.targets.every(c => c.width >= 44 && c.height >= 44), "Control target too small");
    const reachable = [];
    for (const target of targets()) {
      target.scrollIntoView({ block: "center", inline: "nearest", behavior: "instant" });
      await delay(50);
      const r = rect(target);
      const hit = document.elementFromPoint((r.left + r.right) / 2, (r.top + r.bottom) / 2);
      require(r.top >= 0 && r.bottom <= innerHeight + 1 && hit && target.contains(hit), "Control target is unreachable or obscured");
      reachable.push(r);
    }
    return { ...layout(), reachable_targets: reachable };
  };
  const click = async (action: string) => {
    const button = document.querySelector<HTMLButtonElement>(`[data-action="${action}"]`);
    require(button, `Missing ${action} control`);
    button.click();
    await delay(100);
  };
  async function execute(command: Command): Promise<unknown> {
    switch (command.op) {
      case "identity": return { origin: location.origin, secure_context: isSecureContext,
        user_agent: navigator.userAgent, capabilities: capabilities(video), native: await nativeInvoke("t01_control", { action: "identity" }), layout: layout() };
      case "open": {
        const kind = command.kind ?? "mp4";
        const opened = open({ url: url(`media/${kind === "mp4" ? "clip.mp4" : "index.m3u8"}`), kind, subtitle: url("media/subtitles.vtt") });
        await until(() => video.readyState >= 2 || Boolean(video.error), "Media did not become ready");
        require(!video.error, `MediaError ${video.error?.code}`);
        await click("play");
        await until(() => !video.paused && video.currentTime > 0.2, "Playback requires a user gesture or failed to advance");
        await audio.resume();
        await delay(500);
        return { opened, sample: sample(), native: await nativeInvoke("t01_control", { action: "identity" }) };
      }
      case "controls": {
        await click("play");
        require(video.paused, "Pause control failed");
        const paused = video.currentTime;
        await delay(350);
        require(Math.abs(video.currentTime - paused) < 0.1, "Paused media advanced");
        const seek = document.querySelector<HTMLInputElement>("[aria-label='播放进度']")!;
        seek.value = "2"; seek.dispatchEvent(new Event("input", { bubbles: true }));
        await until(() => !video.seeking && Math.abs(video.currentTime - 2) < 0.3, "Seek control failed");
        player.selectSubtitle(0);
        await until(() => Boolean(video.textTracks[0]?.activeCues?.length), "Subtitle cue was not rendered");
        const cue = (video.textTracks[0].activeCues![0] as VTTCue).text;
        require(cue.includes("映象"), "Unexpected subtitle cue");
        player.selectSubtitle(null);
        require(video.textTracks[0].mode === "disabled", "Subtitle disable failed");
        player.selectSubtitle(0);
        const volume = document.querySelector<HTMLInputElement>("[aria-label='音量']")!;
        volume.value = "0.5"; volume.dispatchEvent(new Event("input", { bubbles: true }));
        require(video.volume === 0.5, "Volume control failed");
        const speed = document.querySelector<HTMLSelectElement>("[aria-label='倍速']")!;
        speed.value = "1.5"; speed.dispatchEvent(new Event("change", { bubbles: true }));
        require(video.playbackRate === 1.5, "Speed control failed");
        player.setSpeed(1); player.setVolume(1);
        await click("play");
        await until(() => video.currentTime > 2.5, "Resume control failed");
        const before = await nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });
        await click("fullscreen");
        await delay(1500);
        const full = await nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });
        require(full.fullscreen !== before.fullscreen, "Native fullscreen control failed");
        await click("fullscreen"); await delay(1500);
        const restored = await nativeInvoke<{ fullscreen: boolean }>("t01_control", { action: "identity" });
        require(restored.fullscreen === before.fullscreen, "Native fullscreen exit failed");
        return { paused_position: paused, cue, fullscreen: full, restored, sample: sample() };
      }
      case "layout": {
        await nativeInvoke("t01_control", { action: "width", value: command.value });
        await delay(350);
        const observed = await usableLayout();
        require(Math.abs(observed.inner_width - command.value!) <= 2, "Native resize did not produce requested CSS width");
        return { native: await nativeInvoke("t01_control", { action: "identity" }), layout: observed };
      }
      case "zoom": {
        await nativeInvoke("t01_control", { action: "zoom", value: command.value });
        await delay(350);
        const observed = await usableLayout();
        require(Math.abs(observed.inner_width - 1200 / command.value!) <= 3, "Native zoom did not change effective CSS scale");
        return { requested_zoom: command.value, native: await nativeInvoke("t01_control", { action: "identity" }), layout: observed };
      }
      case "security": {
        const valid = await fetch(url("media/clip.mp4"), { headers: { Range: "bytes=0-15" } });
        require(valid.status === 206 && (await valid.arrayBuffer()).byteLength === 16, "Authenticated WebView Range failed");
        const invalid = await fetch(`${base}media/clip.mp4?cap=invalid`);
        require(invalid.status === 403, "Invalid capability accepted");
        let corsDenied = false;
        try { await fetch(url("probe/cors-denied")); } catch { corsDenied = true; }
        require(corsDenied, "WebView did not enforce CORS");
        const violations: string[] = [];
        const listener = (e: SecurityPolicyViolationEvent) => violations.push(e.effectiveDirective);
        document.addEventListener("securitypolicyviolation", listener);
        const script = document.createElement("script"); script.src = url("probe/script.js"); document.head.append(script);
        const inline = document.createElement("script"); inline.textContent = "window.__T01_INLINE_EXECUTED__=true"; document.head.append(inline);
        try { await fetch("https://example.invalid/t01"); } catch { /* CSP evidence collected below. */ }
        await delay(500);
        script.remove(); inline.remove(); document.removeEventListener("securitypolicyviolation", listener);
        require(violations.some(v => v.startsWith("script-src")) && violations.includes("connect-src"), "CSP did not block external script/connect");
        require(!("__T01_INLINE_EXECUTED__" in window), "Inline script executed");
        const denied: string[] = [];
        for (const command of ["execute_command", "plugin:fs|read_text_file", "plugin:shell|execute"]) {
          try { await nativeInvoke(command, { path: "/etc/hosts", program: "echo" }); }
          catch { denied.push(command); }
        }
        require(denied.length === 3, "Unexpected privileged IPC command available");
        return { cors_denied: corsDenied, csp_violations: violations, denied_commands: denied, range_status: valid.status };
      }
      case "long": {
        const seconds = command.seconds!;
        require(seconds === 1800, "Acceptance duration must be 1800 seconds per format");
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
          require(!current.paused && !current.media_error && current.frames > previous.frames && current.raf > previous.raf, "Playback or UI stalled");
          require(current.width === 1920 && current.height === 1080, "Unexpected decoded resolution");
          require(current.audio_state === "running" && current.audio_rms > 0.001, "Decoded audio output is silent or suspended");
          require(Math.abs(current.audio_peak_hz - 440) < 30, "Decoded audio tone frequency differs");
          require(current.position !== previous.position, "Media clock stalled");
          elapsedMedia += current.position >= previous.position ? current.position - previous.position : current.duration - previous.position + current.position;
          context.drawImage(video, 0, 0, 64, 36);
          const pixels = context.getImageData(0, 0, 64, 36).data;
          require(pixels.some((v, i) => i % 4 !== 3 && v > 32), "Decoded frame is black");
          let signature = 2166136261;
          for (const byte of pixels) signature = Math.imul(signature ^ byte, 16777619);
          signatures.add(String(signature >>> 0));
          if (!preview) preview = canvas.toDataURL("image/png");
          await post("control/sample", { id: command.id, index: samples++, elapsed_ms: performance.now() - started, ...current, frame_signature: signature >>> 0,
            native: await nativeInvoke("t01_control", { action: "identity" }) });
          previous = current;
        }
        require(elapsedMedia >= seconds - 30 && signatures.size > 2, "Insufficient continuous playback or changing frames");
        return { duration_seconds: (performance.now() - started) / 1000, media_seconds: elapsedMedia,
          samples, unique_frames: signatures.size, initial, final: sample(), decoded_preview: preview };
      }
      case "stop": {
        await click("stop");
        await delay(500);
        const before = frames;
        await delay(500);
        require(video.paused && !video.currentSrc && !video.hasAttribute("src") && video.querySelectorAll("track").length === 0, "Video resources remain after stop");
        require(!player.diagnostics().listening && player.diagnostics().hls_instances === 0 && frames <= before + 1, "Old playback still active");
        return { sample: sample(), source_removed: true, frames_after_stop: frames - before };
      }
      case "shutdown": {
        player.stop(); running = false;
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
    require(response.ok, "Fixture command transport failed");
    const command = await response.json() as Command;
    try { await post("control/result", { id: command.id, ok: true, value: await execute(command) }); }
    catch (cause) { await post("control/result", { id: command.id, ok: false, error: String(cause) }); }
  }
  await nativeInvoke("t01_control", { action: "quit" });
}
