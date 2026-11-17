import { readFileSync, readdirSync } from 'node:fs';
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { compile, evaluate, initialState, COMPONENTS } from '../dist/index.js';
const directory=new URL('../../../contract/genui-fixtures/',import.meta.url);
for(const name of readdirSync(directory).filter(n=>n.endsWith('.json')).sort()){
  const corpus=JSON.parse(readFileSync(new URL(name,directory),'utf8'));
  assert.equal(corpus.version,1);
  for(const fixture of corpus.cases)test(`${name}: ${fixture.name}`,()=>{
    const compiled=compile(fixture.source,fixture.budgets);
    let actual;
    if(fixture.kind==='parse')actual={statements:compiled.statements,diagnostics:compiled.diagnostics};
    else if(fixture.kind==='validate')actual={accepted:compiled.program!==null,diagnostics:compiled.diagnostics};
    else if(!compiled.program)actual={state:{},roots:[],diagnostics:compiled.diagnostics};
    else if(fixture.kind==='initialize')actual=initialState(compiled.program,fixture.evaluationBudgets);
    else actual=evaluate(compiled.program,fixture.state,fixture.evaluationBudgets);
    assert.deepEqual(actual,fixture.expected);
  });
}
test('interactive/media accessibility metadata names required string arguments',()=>{
  for(const [name,spec] of Object.entries(COMPONENTS))for(const field of spec.accessibility.required){
    const p=spec.params.find(p=>p.name===field);assert.ok(p,`${name}.${field}`);assert.equal(p.kind,'string');assert.ok(!p.optional&&!p.nullable);
  }
});
test('external cyclic/non-JSON state fails closed without running roots',()=>{
  const p=compile('Text("ok")').program;const cycle={};cycle.self=cycle;
  assert.match(evaluate(p,{cycle}).diagnostics[0].message,/acyclic/);
  assert.match(evaluate(p,{n:Infinity}).diagnostics[0].message,/finite JSON/);
});
