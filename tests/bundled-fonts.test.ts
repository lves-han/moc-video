import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { fontOptions, setAppFont, setFontScale } from '../src/ui/fonts.ts';

test('bundled fonts match their catalog and are referenced by local CSS', () => {
  const css = readFileSync(new URL('../src/style.css', import.meta.url), 'utf8');
  for (const font of fontOptions) {
    const bytes = readFileSync(new URL(`../public/fonts/${font.id}.ttf`, import.meta.url));
    assert.equal(bytes.length, font.bytes);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), font.sha256);
    assert.ok(css.includes(`/fonts/${font.id}.ttf`));
    assert.ok(css.includes(`font-family: "${font.family}"`));
  }
});

test('font and scale preferences apply locally with system fallback', t => {
  const original = Object.getOwnPropertyDescriptor(globalThis, 'document');
  const values = new Map<string, string>();
  const style = { zoom: '', setProperty: (name: string, value: string) => values.set(name, value) };
  Object.defineProperty(globalThis, 'document', { configurable: true, value: { documentElement: { style } } });
  t.after(() => original ? Object.defineProperty(globalThis, 'document', original) : Reflect.deleteProperty(globalThis, 'document'));
  setAppFont('noto-serif-sc');
  assert.equal(values.get('--app-font'), '"Moc Serif", system-ui, sans-serif');
  setAppFont('unknown');
  assert.equal(values.get('--app-font'), '"Moc Sans", system-ui, sans-serif');
  setFontScale(120); assert.equal(style.zoom, '1.2');
  setFontScale(999); assert.equal(style.zoom, '1');
});
