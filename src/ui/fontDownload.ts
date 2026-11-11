import { fontOptions } from './fontCache.ts';

// Native receives a fixed ID, never a URL. Browser-only previews use their own fetch.
export function fontDownloader(resolve: ((id: string) => Promise<string>) | undefined, fetcher: typeof fetch = fetch): typeof fetch {
  return async (input, init) => {
    const requested = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
    const font = fontOptions.find(item => item.url === requested);
    if (!font) throw new Error('Unknown packaged font');
    const url = resolve ? await resolve(font.id) : font.url;
    if (resolve) {
      const target = new URL(url);
      if (target.protocol !== 'http:' || target.hostname !== '127.0.0.1' || target.pathname !== `/fonts/${font.id}`) throw new Error('Invalid native font gateway URL');
    }
    return fetcher(url, init);
  };
}
