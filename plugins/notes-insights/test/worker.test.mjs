import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import test from 'node:test';
const path = new URL('../dist/worker.js', import.meta.url);
const authority={account_id:'01932910-0000-7000-8000-000000000001',space_id:'01932910-0000-7000-8000-000000000002',grant:'host-ref'};
const frame=(invocation_id,message)=>({protocol_version:2,invocation_id,message});
test('one worker handles multiple authorized broker-backed invocations',async()=>{
 const child=spawn(process.execPath,[path.pathname],{stdio:['pipe','pipe','pipe']});
 const lines=createInterface({input:child.stdout})[Symbol.asyncIterator]();
 const timer=setTimeout(()=>child.kill('SIGKILL'),5000);
 try {
  for(const id of ['i1','i2']) {
   child.stdin.write(JSON.stringify(frame(id,{type:'invoke',operation:'notes-insights.summarize',input:{note_id:'01932910-0000-7000-8000-000000000003'},authority}))+'\n');
   const call=JSON.parse((await lines.next()).value); assert.equal(call.message.type,'host_call'); assert.equal(call.message.method,'read_item'); assert.deepEqual(call.message.authority,authority);
   child.stdin.write(JSON.stringify(frame(id,{type:'host_reply',call_id:call.message.call_id,ok:true,data:{text:'hello world\n'}}))+'\n');
   const result=JSON.parse((await lines.next()).value); assert.equal(result.message.type,'complete'); assert.equal(result.invocation_id,id); assert.deepEqual(result.message.data.stats,{words:2,characters:12,lines:1}); assert.deepEqual(result.message.data.view.root.children.map(n=>n.value),['2','12','1']);
  }
  child.stdin.end(); const [code]=await once(child,'exit'); assert.equal(code,0);
 } finally { clearTimeout(timer); child.kill(); }
});
test('malformed and oversized frames fail with safe diagnostics',async()=>{
 for(const input of ['{"protocol_version":1}\n','x'.repeat(256*1024+1)]) {
  const child=spawn(process.execPath,[path.pathname],{stdio:['pipe','pipe','pipe']}); let stdout='',stderr=''; child.stdout.on('data',b=>stdout+=b);child.stderr.on('data',b=>stderr+=b);child.stdin.on('error',()=>{}); const done=once(child,'exit'); child.stdin.end(input); const [code]=await done; assert.notEqual(code,0);assert.equal(stdout,'');assert.equal(stderr,'notes-insights worker protocol failure\n');
 }
});
