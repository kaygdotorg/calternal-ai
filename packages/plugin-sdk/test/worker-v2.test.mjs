import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { validateManifestV1, WorkerRuntimeV2, encodeWorkerFrameV2, parseWorkerFrameV2, MAX_V2_FRAME_BYTES } from '../dist/index.js';
const manifest = JSON.parse(await readFile(new URL('../../../plugins/calternal-notes-insights/manifest.json', import.meta.url)));
const authority = { account_id:'01932910-0000-7000-8000-000000000001', space_id:'01932910-0000-7000-8000-000000000002', grant:'host-reference' };
const frame = (id,message) => ({protocol_version:2,invocation_id:id,message});
const invoke = id => frame(id,{type:'invoke',operation:'notes-insights.summarize',input:{id:'x'},authority});
const tick = () => new Promise(resolve => setImmediate(resolve));
test('manifest strict nested rejection and closed authority vocabulary', () => {
  assert.equal(validateManifestV1(manifest),manifest);
  const { kinds: _kinds, ...stateless } = manifest;
  assert.deepEqual(validateManifestV1(stateless).kinds, []);
  assert.throws(() => validateManifestV1({ ...stateless, kinds: null }));
  for (const mutate of [m=>m.extra=true,m=>m.operations[0].extra=true,m=>m.host_calls.push('shell'),m=>m.operations[0].required_capability='admin',m=>m.operations[0].mutates=true,m=>m.operations.push(m.operations[0]),m=>m.hooks.push({id:'h',mode:'rewrite',patterns:['notes.*'],operation:'notes-insights.summarize'}),m=>m.themes.push({id:'t',stylesheet:'https://x/a.css'}),m=>m.settings.push({id:'s',value_type:'boolean',default:0})]) { const m=structuredClone(manifest); mutate(m); assert.throws(()=>validateManifestV1(m)); }
});
test('protocol round trips reject versions, unknown keys, sizes and invalid UTF8', () => {
  assert.deepEqual(parseWorkerFrameV2(encodeWorkerFrameV2(invoke('i1'))),invoke('i1'));
  for (const bad of [{...invoke('i1'),protocol_version:1},{...invoke('i1'),extra:true},frame('i1',{type:'failed',extra:1})]) assert.throws(()=>parseWorkerFrameV2(JSON.stringify(bad)));
  assert.throws(()=>parseWorkerFrameV2(new Uint8Array(MAX_V2_FRAME_BYTES+1)));
  assert.throws(()=>parseWorkerFrameV2(new Uint8Array([255])));
});
test('long lived worker uses declared broker, correlated replies and multiple invocations',async()=>{
  const sent=[]; const runtime=new WorkerRuntimeV2(manifest,{'notes-insights.summarize':async(_,ctx)=>ctx.call('read_item',{item_id:'x'})},async f=>{sent.push(f)});
  for(const id of ['i1','i2']) { await runtime.receive(invoke(id)); await tick(); const call=sent.at(-1); assert.equal(call.message.type,'host_call'); assert.deepEqual(call.message.authority,authority); await runtime.receive(frame(id,{type:'host_reply',call_id:call.message.call_id,ok:true,data:{markdown:'hello'}})); await tick(); assert.equal(sent.at(-1).message.type,'complete'); }
});
test('undeclared calls fail before transport and cancellation suppresses late completion',async()=>{
 const sent=[]; let finish;
 const runtime=new WorkerRuntimeV2(manifest,{'notes-insights.summarize':async(_,ctx)=>ctx.call('write_item',{})},async f=>{sent.push(f)});
 await runtime.receive(invoke('i1')); await tick(); assert.deepEqual(sent.map(f=>f.message.type),['failed']);
 const cancelled=new WorkerRuntimeV2(manifest,{'notes-insights.summarize':async()=>new Promise(r=>finish=r)},async f=>{sent.push(f)});
 await cancelled.receive(invoke('i2')); await cancelled.receive(frame('i2',{type:'cancel'})); finish({late:true}); await tick(); assert.equal(sent.length,1);
});
test('slow host bounds pending callbacks and invocation admission',async()=>{
 const sent=[]; const runtime=new WorkerRuntimeV2(manifest,{'notes-insights.summarize':async(_,ctx)=>ctx.call('read_item',{})},async f=>{sent.push(f)});
 for(let i=0;i<32;i++) await runtime.receive(invoke(`i${i}`));
 assert.equal(sent.length,32); await assert.rejects(runtime.receive(invoke('overflow')));
 for(let i=0;i<32;i++) await runtime.receive(frame(`i${i}`,{type:'cancel'})); await tick(); assert.equal(sent.length,32);
});
test('shared worker wire vectors match Rust',async()=>{
 const cases=JSON.parse(await readFile(new URL('./fixtures/worker-v2-cases.json',import.meta.url)));
 for(const c of cases) { if(c.valid) assert.doesNotThrow(()=>parseWorkerFrameV2(JSON.stringify(c.value)),c.name); else assert.throws(()=>parseWorkerFrameV2(JSON.stringify(c.value)),c.name); }
});
test('job chunks require admitted handles and remain ordered under transport back pressure',async()=>{
 const m=structuredClone(manifest);m.host_calls.push('start_job');const sent=[];let release;
 const runtime=new WorkerRuntimeV2(m,{'notes-insights.summarize':async(_,ctx)=>{
  await assert.rejects(ctx.chunk('forged',{}));const job=await ctx.call('start_job',{});
  const first=ctx.chunk(job.job_id,'a');await assert.rejects(ctx.chunk(job.job_id,'overlap'));await first;await ctx.chunk(job.job_id,'b');return {stats:{}};
 }},async f=>{sent.push(f);if(f.message.type==='chunk' && f.message.sequence===0) await new Promise(r=>release=r)});
 await runtime.receive(invoke('i1'));await tick();const c=sent[0];await runtime.receive(frame('i1',{type:'host_reply',call_id:c.message.call_id,ok:true,data:{job_id:'job-1'}}));await tick();assert.equal(sent[1].message.sequence,0);release();await tick();assert.deepEqual(sent.filter(f=>f.message.type==='chunk').map(f=>[f.message.sequence,f.message.data]),[[0,'a'],[1,'b']]);assert.equal(sent.at(-1).message.type,'complete');
});
test('kind storage placement enforces hidden and visible NFC components',()=>{
 const m=structuredClone(manifest);m.kinds=[{id:'notes',schema:true,storage:{folder:'Notes'},claimed_text:true}];assert.doesNotThrow(()=>validateManifestV1(m));
 for(const folder of ['', '.calternal','.private','Trash','TRASH','../Notes','a/b','a\\b','a:b','a\0b',' Notes','Notes ','Cafe\u0301']) { const bad=structuredClone(m);bad.kinds[0].storage={folder};assert.throws(()=>validateManifestV1(bad),folder); }
 for(const storage of [{hidden:false},{folder:'Notes',hidden:true},{hidden:true,extra:true}]) {const bad=structuredClone(m);bad.kinds[0].storage=storage;assert.throws(()=>validateManifestV1(bad));}
 m.kinds[0].storage={hidden:true};m.kinds[0].claimed_text=false;assert.throws(()=>validateManifestV1(m),'non-text kinds name an extension');
 for(const extension of ['','../x','PNG','a'.repeat(17)]) {const bad=structuredClone(m);bad.kinds[0].extension=extension;assert.throws(()=>validateManifestV1(bad),extension);}
 m.kinds[0].extension='png';assert.doesNotThrow(()=>validateManifestV1(m));
 const text=structuredClone(m);text.kinds[0].claimed_text=true;text.kinds[0].extension='txt';assert.throws(()=>validateManifestV1(text));text.kinds[0].extension='md';assert.doesNotThrow(()=>validateManifestV1(text));
});
test('operation risk is required, closed, and consistent with mutations',()=>{
  for(const change of [m=>delete m.operations[0].risk,m=>m.operations[0].risk='unknown',m=>m.operations[0].risk='external']){
    const value=structuredClone(manifest);change(value);assert.throws(()=>validateManifestV1(value));
  }
});

test('visible subtrees share the host bounded component rules',()=>{
 const m=structuredClone(manifest);m.kinds=[{id:'threads',schema:true,storage:{folder:'AI',subdirectories:['Chats']},claimed_text:true}];
 assert.doesNotThrow(()=>validateManifestV1(m));
 for(const subdirectories of [[],Array(9).fill('Chats'),['..'],['a/b'],['.hidden'],['Trash'],['a\\b'],['Cafe\u0301'],[1]]) {
  const bad=structuredClone(m);bad.kinds[0].storage.subdirectories=subdirectories;assert.throws(()=>validateManifestV1(bad));
 }
});

test('standing instruction files cannot become kind directories',()=>{
 const m=structuredClone(manifest);m.kinds=[{id:'threads',schema:true,storage:{folder:'AI',subdirectories:['AGENTS.md']},claimed_text:true}];
 assert.throws(()=>validateManifestV1(m));
 m.kinds[0].storage={folder:'ai',subdirectories:['agents.md','child']};assert.throws(()=>validateManifestV1(m));
});

test("owned slot declarations and qualified targets are closed and bounded", () => {
  const candidate = structuredClone(manifest);
  candidate.owned_slots = [{ id: "stats", required_capability: "space.read", accepts: ["generated"], max_contributions: 2 }];
  candidate.slots[0].slot = { plugin_id: candidate.id, id: "stats" };
  assert.equal(validateManifestV1(candidate), candidate);
  for (const change of [
    m => m.owned_slots[0].required_capability = "spaces.list",
    m => m.owned_slots[0].max_contributions = 0,
    m => m.owned_slots[0].max_contributions = 33,
    m => m.owned_slots[0].accepts.push("generated"),
    m => m.owned_slots[0].accepts = ["sealed"],
    m => m.slots[0].slot = "approvals",
    m => m.slots[0].slot.plugin_id = "../host",
    m => m.slots[0].slot = { plugin_id: "host", id: "approvals", foundation: true },
    m => m.slots[0].renderer = { type: "sealed", asset: "panel.html" },
  ]) {
    const invalid = structuredClone(candidate);
    change(invalid);
    assert.throws(() => validateManifestV1(invalid));
  }
});

test('content proxy requires an explicit profile and a stored source declaration', () => {
  const source = structuredClone(manifest);
  source.kinds = [{ id: 'mail', schema: true, claimed_text: true, storage: { hidden: true } }];
  source.network = { content_fetch: { profile: 'images_fonts', sources: [{ kind: 'mail', field: 'text' }] } };
  source.host_calls.push('content_prepare_v1');
  assert.doesNotThrow(() => validateManifestV1(source));
  for (const modify of [
    value => delete value.network,
    value => value.network.content_fetch.profile = 'arbitrary',
    value => value.network.content_fetch.sources[0].kind = 'undeclared',
    value => value.network.content_fetch.sources.push(value.network.content_fetch.sources[0]),
    value => value.network.content_fetch.sources[0].field = 'url',
    value => value.network.content_fetch.hosts = ['example.com'],
  ]) {
    const invalid = structuredClone(source);
    modify(invalid);
    assert.throws(() => validateManifestV1(invalid));
  }
});
