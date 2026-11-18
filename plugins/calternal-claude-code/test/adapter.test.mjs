import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {Adapter,lines} from '../dist/adapter.js';
const binary=resolve('test/fake-claude.mjs');
function fixture(){
 const events=[];const a=new Adapter(binary,async line=>events.push(JSON.parse(line)));let seq=0;
 return {a,events,command:command=>a.command(JSON.stringify({version:1,seq:++seq,command}))};
}
async function run(prompt){const f=fixture();await f.command({type:'start',options:{},model:'sonnet',cwd:process.cwd(),instructions:'Core instructions',resume:null});await f.command({type:'send',prompt,context:[]});await f.a.finish();return f.events;}
test('official CLI stream maps text, tools, session and usage without raw diagnostics',async()=>{
 const events=await run('hello');
 assert.deepEqual(events.map(e=>e.seq),events.map((_,i)=>i+1));
 assert.deepEqual(events.map(e=>e.event.type),['check','session','text_delta','tool_started','tool_output','tool_finished','usage','done']);
 assert.equal(events.at(-1).event.outcome,'done');
 assert.equal(events[2].event.text,'Hello 🌏\n');
 assert.ok(!JSON.stringify(events).includes('PRIVATE_LOGIN_SENTINEL'));
});
test('scripted steps keep their name, target and bounded output metadata',async()=>{
 const events=(await run('scripted-tool-steps long-output')).map(e=>e.event);
 const steps=events.filter(e=>e.type.startsWith('tool_'));
 assert.deepEqual(steps.map(e=>[e.type,e.name,e.id]),[
  ['tool_started','Bash','step-1'],['tool_output','Bash','step-1'],['tool_finished','Bash','step-1'],
  ['tool_started','Read','step-2'],['tool_output','Read','step-2'],['tool_finished','Read','step-2'],
 ]);
 assert.equal(steps[0].summary,'Run ls');
 assert.equal(steps[1].summary,'Run ls');
 assert.ok(steps[1].bytes>=96*1024);
 assert.equal(steps[2].detail,'Run ls completed');
 assert.equal(steps[5].failed,true);
 assert.ok(!JSON.stringify(steps).includes('PRIVATE_LOGIN_SENTINEL'));
});
test('questions and failures are normalized',async()=>{
 assert.ok((await run('question')).some(e=>e.event.type==='question'));
 assert.equal((await run('fail')).at(-1).event.outcome,'failed');
});
test('crash after tool start has unknown outcome, never success',async()=>{
 const events=await run('crash');assert.equal(events.at(-1).event.outcome,'outcome_unknown');assert.ok(events.some(e=>e.event.code==='cli_exited'));
});
test('stop ends a hung official CLI',async()=>{
 const f=fixture();await f.command({type:'start',options:{},model:'sonnet',cwd:process.cwd(),instructions:'Core',resume:null});
 await f.command({type:'send',prompt:'hang',context:[]});
 while(!f.events.some(e=>e.event.type==='tool_started'))await new Promise(r=>setTimeout(r,5));
 await f.command({type:'stop'});assert.equal(f.events.at(-1).event.outcome,'cancelled');
});
test('check uses exit status and never forwards provider status',async()=>{
 const home=await mkdtemp(join(tmpdir(),'calternal-login-'));const old=process.env.HOME;process.env.HOME=home;
 try{const f=fixture();await f.command({type:'check'});assert.equal(f.events[0].event.signed_in,false);await writeFile(join(home,'signed-in'),'disposable fixture');await f.command({type:'check'});assert.equal(f.events[1].event.signed_in,true);assert.ok(!JSON.stringify(f.events).includes('PRIVATE_LOGIN_SENTINEL'));}
 finally{process.env.HOME=old;await rm(home,{recursive:true,force:true});}
});
test('bounded input, event numbers and partial frames fail closed',async()=>{
 const f=fixture();await assert.rejects(f.a.command(JSON.stringify({version:2,seq:1,command:{type:'check'}})));
 await assert.rejects(f.a.command(JSON.stringify({version:1,seq:4,command:{type:'check'}})));
 await assert.rejects(f.a.command(' '.repeat(65536)));
 await assert.rejects(async()=>{for await(const _ of lines((async function*(){yield Buffer.from('partial');})(),32)) {}});
 assert.equal((await run('oversized')).at(-1).event.outcome,'outcome_unknown');
});
test('real signed-in Claude opt-in', {skip:process.env.CALTERNAL_TEST_REAL_CLAUDE!=='1'},async()=>{
 const events=[];const a=new Adapter('claude',async line=>events.push(JSON.parse(line)));
 await a.command(JSON.stringify({version:1,seq:1,command:{type:'start',options:{},model:'sonnet',cwd:process.cwd(),instructions:'Reply concisely. Use no tools.',resume:null}}));
 await a.command(JSON.stringify({version:1,seq:2,command:{type:'send',prompt:'Say hello.',context:[]}}));await a.finish();assert.equal(events.at(-1).event.outcome,'done');
});
