import assert from 'node:assert/strict';
import test from 'node:test';
import { Api } from '../src/api.ts';
import { AppController, createAppState } from '../src/state.ts';
import type { Bridge_MediaCard } from '../src/models.ts';

const host = '11111111-1111-4111-8111-111111111111';
const source = { source_id:'catalog', source_revision:'revision' };
const card = (title:string):Bridge_MediaCard => ({ id:title, title, kind:'movie', ref:{ ...source, data:title }, poster:{ resource_id:'poster',path:'/images/poster',capability:'a'.repeat(43),expires_at:'2030-01-01T00:00:00Z' } });
function fixture() {
  const state = createAppState(); state.runtime.engine_epoch = host;
  state.selected_metadata_source = source;
  state.sources.data = { plugins:[], providers:[{ id:source.source_id, source_revision:source.source_revision, name:'Catalog', kind:'stremio', language:'en', content_types:['movie'],
    runtime_status:'ready', capabilities:{ home:true, details:true, search:true, resolve:false, sequential_home:false }, origin:{ config_id:host, manifest_urls:['https://example.com/manifest.json'] } }] };
  const calls:{ command:string; input:any; id:string; finish:(value:any)=>void }[] = [];
  const api = new Api((command,args) => new Promise(resolve => {
    const row = { command,input:args.input,id:args.request_id,finish:(data:any)=>resolve({ request_id:args.request_id,engine_epoch:host,data }) }; calls.push(row);
    if (command === 'cancel_request') row.finish({ cancelled:true });
  }));
  const app = new AppController(api, {} as never, state);
  return { state,app,calls,api };
}

test('TV7 search keeps previous results while loading, cancels the old IPC and discards its late success',async()=>{
  const f=fixture(); const initial=f.app.searchCatalog('initial',source); f.calls[0]!.finish({items:[card('initial')]}); await initial;
  const old=f.app.searchCatalog('old',source); assert.equal(f.app.search.phase,'loading'); assert.equal(f.app.search.items[0]!.title,'initial');
  const latest=f.app.searchCatalog('latest',source);
  assert.ok(f.calls.some(c=>c.command==='cancel_request' && c.input.target_request_id===f.calls[1]!.id));
  f.calls.find(c=>c.input.query==='latest')!.finish({items:[card('latest')]}); await latest;
  f.calls.find(c=>c.input.query==='old')!.finish({items:[card('old')]}); await old;
  assert.equal(f.app.search.items[0]!.title,'latest'); assert.equal(f.app.search.phase,'ready');
});

test('TV7 search clear/leave cancels pending work and rejects a late response',async()=>{
  const f=fixture(); const pending=f.app.searchCatalog('query',source); f.app.cancelSearch(true);
  f.calls[0]!.finish({items:[card('late')]}); await pending;
  assert.deepEqual(f.app.search.items,[]); assert.equal(f.app.search.phase,'idle');
  assert.equal(f.calls.filter(c=>c.command==='cancel_request').length,1);
});

test('TV7 search rejects foreign source bindings and exposes the error',async()=>{
  const f=fixture(); const pending=f.app.searchCatalog('query',source); const foreign=card('foreign'); foreign.ref.source_revision='old';
  f.calls[0]!.finish({items:[foreign]}); await pending;
  assert.equal(f.app.search.phase,'failed'); assert.ok(f.app.search.error?.message.includes('source')); assert.deepEqual(f.app.search.items,[]);
});

test('TV7 search posters use the owning details DTO and reject mismatched identity',async()=>{
  const f=fixture(); const pending=f.app.searchCatalog('query',source); f.calls[0]!.finish({items:[card('film')]}); await pending;
  const poster=f.app.searchPoster(f.app.search.items[0]!);
  f.calls.find(c=>c.command==='get_details')!.finish({details:{id:'foreign',ref:{...source,data:'film'}},images:{poster:{url:'http://127.0.0.1/foreign'}}}); await poster;
  assert.deepEqual(f.app.search.images,{}); assert.ok(f.app.search.error?.message.includes('identity'));
});

test('TV7 search has the default metadata budget and source/epoch invalidation drops completed work',async()=>{
  const f=fixture(); assert.equal(f.api.budget('search_catalog',{...source,query:'x'}),30000);
  const pending=f.app.searchCatalog('query',source); f.state.sources.data!.providers[0]!.source_revision='new';
  f.calls[0]!.finish({items:[card('late')]}); await pending;
  assert.deepEqual(f.app.search.items,[]);
  f.app.cancelSearch(true); assert.equal(f.app.search.phase,'idle');
});
