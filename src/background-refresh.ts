// Limit background metadata work to one request, with a pause between items.
export class BackgroundRefresh {
  private queue = new Map<string, () => Promise<void>>();
  private timer: ReturnType<typeof setTimeout> | undefined;
  private running = false;
  constructor(privateIdle: () => boolean, onError: (cause: unknown) => void = () => {}) { this.idle = privateIdle; this.onError = onError; }
  private idle: () => boolean;
  private onError: (cause: unknown) => void;

  enqueue(key: string, task: () => Promise<void>) {
    this.queue.set(key, task);
    if (this.queue.size > 50) this.queue.delete(this.queue.keys().next().value!);
    if (!this.running && !this.timer) this.timer = setTimeout(() => { void this.next(); }, 2000);
  }
  clear() { this.queue.clear(); clearTimeout(this.timer); this.timer = undefined; }
  private async next() {
    this.timer = undefined;
    if (this.running || !this.queue.size) return;
    if (!this.idle()) { this.timer = setTimeout(() => { void this.next(); }, 2000); return; }
    const entry = this.queue.entries().next().value;
    if (!entry) return;
    this.queue.delete(entry[0]); this.running = true;
    try { await entry[1](); } catch (cause) { this.onError(cause); } finally {
      this.running = false;
      if (this.queue.size) this.timer = setTimeout(() => { void this.next(); }, 3000);
    }
  }
}
