// Development helper: attach field-specific reference comments to inherited inline AST unions.
import { createRequire } from 'node:module';
import { readFileSync,writeFileSync,readdirSync } from 'node:fs';
const require=createRequire(new URL('../package.json',import.meta.url));
const ts=require('typescript');
const descriptions={type:'Syntax or interpreter-value discriminator; determines the other fields.',kind:'Statement or interpreter-value discriminator.',name:'Identifier, property, method or callee name; state names exclude the dollar prefix.',value:'Literal value or unevaluated expression belonging to this syntax item.',pos:'One-based UTF-16 source position used for diagnostics and component identity.',quasis:'Literal template segments; contains one more segment than parts.',parts:'Interpolated template expressions in source order.',items:'Ordered list-element expressions.',entries:'Object entries in source insertion order.',key:'Object key or stable source-derived native component identity.',op:'Whitelisted pure operator spelling.',operand:'Unevaluated prefix operand.',left:'Left operand; evaluated before the right operand.',right:'Right operand; logical operators evaluate this lazily.',test:'Conditional test expression.',then:'Branch evaluated for a truthy test.',else:'Branch evaluated for a false test.',object:'Receiver expression for a property, index or method access.',index:'Index expression; lists and strings use numbers, objects use text.',args:'Positional call arguments in source order.',named:'Named call arguments in source order.',params:'Lexical callback parameter names.',body:'Pure function body; never platform or JavaScript code.',t:'Lexer token category.',line:'One-based source line.',column:'One-based UTF-16 source column.',required:'Accessible argument names that must be present and resolve to nonblank text.',state:'Local state snapshot; names exclude the dollar prefix.',diagnostics:'Positioned errors collected in deterministic order.',states:'Local state initializers in source declaration order.',list:'Permitted pure list receiver methods.',string:'Permitted pure text receiver methods.',number:'Permitted pure numeric receiver methods.'};
for(const file of readdirSync(new URL('../src/',import.meta.url)).filter(n=>n.endsWith('.ts'))){
  const path=new URL('../src/'+file,import.meta.url),source=readFileSync(path,'utf8');
  const parsed=ts.createSourceFile(file,source,ts.ScriptTarget.Latest,true),edits=[];
  function walk(node){
    if((ts.isPropertySignature(node)||ts.isIndexSignatureDeclaration(node)||ts.isExportSpecifier(node))&&!ts.getJSDocCommentsAndTags(node).length){
      const name=node.name?.getText(parsed).replaceAll('"','').replaceAll("'",'');
      const comment=ts.isExportSpecifier(node)?`Public package export of ${name}; see its defining module for invariants, errors and costs.`:ts.isIndexSignatureDeclaration(node)?'JSON object entries keyed by literal property names.':descriptions[name]??`The ${name} value described by this contract.`;
      edits.push({at:node.getStart(parsed),text:'\n/** '+comment+' */\n'});
    }
    ts.forEachChild(node,walk);
  }walk(parsed);
  let result=source;for(const e of edits.sort((a,b)=>b.at-a.at))result=result.slice(0,e.at)+e.text+result.slice(e.at);writeFileSync(path,result);
}

for(const file of readdirSync(new URL('../src/',import.meta.url)).filter(n=>n.endsWith('.ts'))){
 const path=new URL('../src/'+file,import.meta.url);const source=ts.createSourceFile(file,readFileSync(path,'utf8'),ts.ScriptTarget.Latest,true);
 writeFileSync(path,ts.createPrinter({newLine:ts.NewLineKind.LineFeed}).printFile(source));
}
