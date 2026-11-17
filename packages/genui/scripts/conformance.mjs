// Compare complete parser ASTs, compile diagnostics and evaluation snapshots for every fixture.
import { readFileSync,readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { strict as assert } from 'node:assert';
import { compile,evaluate,initialState } from '../dist/index.js';
const directory=new URL('../../../contract/genui-fixtures/',import.meta.url);
const fixtures=readdirSync(directory).filter(n=>n.endsWith('.json')).sort().flatMap(n=>JSON.parse(readFileSync(new URL(n,directory),'utf8')).cases);
const binary=process.argv[2]??new URL('../../../target/debug/examples/conformance',import.meta.url).pathname;
const rust=spawnSync(binary,[],{input:fixtures.map(f=>JSON.stringify(f)).join('\n')+'\n',encoding:'utf8',maxBuffer:16*1024*1024});
assert.equal(rust.status,0,rust.stderr||rust.error?.message);
const results=rust.stdout.trimEnd().split('\n').map(line=>JSON.parse(line));assert.equal(results.length,fixtures.length);
fixtures.forEach((f,i)=>{
  const c=compile(f.source,f.budgets);
  const evaluation=c.program&&(f.kind==='evaluate'||f.kind==='initialize')?(f.kind==='initialize'?initialState(c.program,f.evaluationBudgets):evaluate(c.program,f.state,f.evaluationBudgets)):null;
  assert.deepEqual({accepted:c.program!==null,statements:c.statements,compilationDiagnostics:c.diagnostics,evaluation},results[i],f.name);
});
console.log(`${fixtures.length} complete TypeScript/Rust AST, diagnostic and evaluation snapshots agree exactly`);
