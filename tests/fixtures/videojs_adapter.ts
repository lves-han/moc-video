// A media-adapter boundary for unit tests; real Video.js components are exercised in packaged WebView acceptance.
export function videoJsAdapter(Hls: any, Target: typeof EventTarget = EventTarget) {
  return class HlsJsAdapter extends Target {
    target: any;
    engine: any = null;
    engineName: string | undefined;
    error: any = null;
    attach(target: any) { this.target = target; }
    set source(value: any) {
      this.engine?.destroy(); this.engine = null; this.error = null;
      if (!value) { this.engineName = undefined; return; }
      if (value.type.includes('mpegurl') && !this.target.canPlayType('application/vnd.apple.mpegurl')) {
        this.engine = new Hls(value.engine?.hlsJs ?? {}); this.engineName = 'hlsjs';
        this.engine.on('error', (_event: string, data: any) => {
          if (!data.fatal) return;
          this.error = { code: data.type === 'mediaError' ? 3 : 2, message: data.details, data };
          this.dispatchEvent(new Event('error'));
        });
        this.engine.attachMedia(this.target); this.engine.loadSource(value.src);
      } else { this.engineName = 'browser'; this.target.src = value.src; this.target.load(); }
    }
    destroy() { this.source = null; this.target = null; }
  };
}
