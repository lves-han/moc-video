import assert from 'node:assert/strict';
import test from 'node:test';
import { nextTick } from 'vue';
import { api } from '../src/api.ts';
import { appState, TMDB_CONFIG_ID } from '../src/state.ts';
import { observeAvailability } from '../src/card-availability.ts';

test('a visible matched film displays unavailable only after the service confirms it', async () => {
  const previousSettings = appState.settings, previousEpoch = appState.runtime.engine_epoch;
  const previousCall = api.call, previousTauri = (globalThis as any).__TAURI__;
  const ref = { source_id:'source', source_revision:'rev', data:'["stremio","addon","movie","tmdb:1"]' };
  const card = { id:'film', ref, title:'Film', kind:'movie' } as const;
  appState.runtime.engine_epoch = 'host';
  appState.settings = { ...previousSettings, ui_locale:'zh-CN', playback_service:{enabled:true,url:'https://service.test',token:'token',groups:[]} };
  (api as any).call = async () => ({ details:{id:'film', ref, status:'unknown', groups:[{episodes:[{ref:{...ref, data:'["stremio_video","addon","movie","tmdb:1","tmdb:1"]'}}]}]}, metadata_ref:{identity:{config_id:TMDB_CONFIG_ID,type:'movie',meta_id:'tmdb:1'},content_ref:ref} });
  await nextTick();
  let calls = 0;
  (globalThis as any).__TAURI__ = { core:{invoke:async () => { calls++; return 'unavailable_title'; }} };
  try {
    const result = await new Promise<string>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Availability check did not finish')), 1000);
      observeAvailability(card, status => { clearTimeout(timer); resolve(status); });
    });
    assert.equal(result, 'unavailable'); assert.equal(calls,1);
  } finally { (api as any).call=previousCall; (globalThis as any).__TAURI__=previousTauri; appState.settings=previousSettings; appState.runtime.engine_epoch=previousEpoch; }
});
