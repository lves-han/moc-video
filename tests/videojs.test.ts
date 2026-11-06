import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import ts from 'typescript';

const source = ts.transpileModule(readFileSync(new URL('../src/videojs.ts', import.meta.url), 'utf8'), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;

// The official feature boundary supplies notifications; native WKWebView A1
// separately verifies the shipped feature, skin, seek actions and lifecycle.
function fixture() {
  let now = 0;
  let selected: any[] = [];
  const attachments: any[] = [];
  const time = { name: 'time', state: () => ({ seek() {} }), attach(context: any) { attachments.push(context); } };
  const fullscreen = { name: 'fullscreen' };
  const other = { name: 'other' };
  runInNewContext(source, {
    exports: {}, window: {}, performance: { now: () => now },
    customElements: { get: () => undefined, define() {} },
    require(name: string) {
      if (name === '@videojs/html') return { createPlayer(options: any) { selected = options.features; return { PlayerElement: class {} }; } };
      if (name === '@videojs/html/video') return { videoFeatures: [time, fullscreen, other] };
      if (name === '@videojs/core/dom') return { timeFeature: time, fullscreenFeature: fullscreen, definePlayerFeature: (value: any) => value };
      if (name === './player.ts') return { nativeInvoke() { throw new Error('Unexpected native action'); } };
      if (name.includes('/skin') || name.includes('/compat-skin') || name.includes('/register')) return {};
      throw new Error(`Unexpected dependency: ${name}`);
    },
  });
  const feature = selected.find(value => value.name === 'time');
  const attach = () => {
    const signal = new AbortController().signal;
    const output: Record<string, unknown>[] = [];
    feature.attach({ signal, set(value: Record<string, unknown>) { output.push({ ...value }); } });
    return { signal, output, input: attachments.at(-1) };
  };
  return { time, fullscreen, other, selected, feature, attach, at(value: number) { now = value; } };
}

test('official progress notifications stay bounded while pause/seek/metadata remain immediate', () => {
  const f = fixture(), a = f.attach();
  for (let ms = 0; ms < 1000; ms += 10) { f.at(ms); a.input.set({ currentTime: ms / 1000 }); }
  assert.equal(a.output.length, 4);
  assert.deepEqual(a.output.map(value => value.currentTime), [0, .25, .5, .75]);
  for (const state of [
    { currentTime: .99, duration: 2, seeking: false }, // settled pause
    { currentTime: .5, duration: 2, seeking: true },
    { currentTime: .5, duration: 2, seeking: false },
    { currentTime: 0, duration: 0, seeking: false }, // emptied
  ]) {
    f.at(995); a.input.set(state);
    assert.deepEqual(a.output.at(-1), state);
  }
});

test('official state/actions and lifecycle ownership pass through unchanged', () => {
  const f = fixture(), a = f.attach();
  assert.equal(f.feature.state, f.time.state);
  assert.equal(f.selected[1], f.fullscreen);
  assert.equal(f.selected[2], f.other);
  assert.equal(a.input.signal, a.signal);
  a.input.set({ duration: 2 });
  assert.deepEqual(a.output, [{ duration: 2 }]);
});

test('a newly attached official media owner receives its first position immediately', () => {
  const f = fixture(), first = f.attach();
  f.at(100); first.input.set({ currentTime: .1 });
  const second = f.attach();
  f.at(101); second.input.set({ currentTime: 0 });
  assert.deepEqual(second.output, [{ currentTime: 0 }]);
});
