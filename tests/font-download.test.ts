import test from 'node:test';
import assert from 'node:assert/strict';
import { fontDownloader } from '../src/ui/fontDownload.ts';
import { fontOptions } from '../src/ui/fontCache.ts';
test('native font downloads resolve only a fixed ID and fetch a literal loopback gateway', async () => {
  const ids: string[] = [], urls: string[] = [];
  const local = 'http://127.0.0.1:1234/fonts/noto-sans-sc?cap=test';
  const download = fontDownloader(async id => { ids.push(id); return local; }, (async url => { urls.push(String(url)); return new Response('font'); }) as typeof fetch);
  await download(fontOptions[0]!.url, { credentials: 'omit' });
  assert.deepEqual(ids, ['noto-sans-sc']); assert.deepEqual(urls, [local]);
  await assert.rejects(download('https://example.com/font'), /Unknown packaged/);
  for (const url of ['http://example.com/fonts/noto-sans-sc','http://127.0.0.1:1234/settings','https://127.0.0.1/fonts/noto-sans-sc']) {
    await assert.rejects(fontDownloader(async () => url)(fontOptions[0]!.url), /Invalid native/);
  }
});
test('only browser previews download directly; native resolver failures do not fall back to WebView DNS', async () => {
  let fetched = 0;
  const fetcher = (async () => { fetched++; return new Response('font'); }) as typeof fetch;
  await fontDownloader(undefined, fetcher)(fontOptions[0]!.url);
  assert.equal(fetched, 1);
  await assert.rejects(fontDownloader(async () => { throw Error('gateway unavailable'); }, fetcher)(fontOptions[0]!.url), /gateway unavailable/);
  assert.equal(fetched, 1);
});
