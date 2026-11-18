#!/usr/bin/env node
// Account-free fixture of the documented stream-json output, selected by prompt.
import { existsSync,readFileSync,writeFileSync,mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
const args=process.argv.slice(2);
const loginFile = join(process.env.HOME, '.claude', '.credentials.json');
if (args.join(' ') === 'auth login') {
  console.log('Open https://claude.ai/oauth/authorize?state=FIXTURE_BROWSER_LINK');
  console.error('PRIVATE_LOGIN_SENTINEL');
  let code = '';
  for await (const data of process.stdin) {
    code += data;
    if (code.includes('\n')) break;
  }
  if (code.trim() !== 'FIXTURE_BROWSER_ANSWER') process.exit(1);
  mkdirSync(join(process.env.HOME, '.claude'), { recursive: true });
  writeFileSync(loginFile, 'FIXTURE_MACHINE_ONLY_LOGIN', { mode: 0o600 });
  process.exit(0);
}
if(args.join(' ')==='auth status'){
  // Raw status and stderr must never reach the adapter's caller.
  console.log(JSON.stringify({authMethod:existsSync(join(process.env.HOME,'subscription'))?'claude.ai':'api_key',subscriptionType:existsSync(join(process.env.HOME,'managed'))?'team':'max',private:'PRIVATE_LOGIN_SENTINEL'},null,2)); console.error('PRIVATE_LOGIN_SENTINEL');
  process.exit(existsSync(join(process.env.HOME,'signed-in')) || existsSync(loginFile)?0:1);
}
// The machine supplies the same operation CLI inside both execution backends.
if(process.env.CALTERNAL_AGENT_TOKEN) execFileSync('calternal',['--help'],{stdio:'ignore'});
let prompt='';for await(const data of process.stdin)prompt+=data;
const emit=m=>process.stdout.write(JSON.stringify(m)+'\n');
if(args.includes('--safe-mode')){
  if(!['--disable-slash-commands','--no-session-persistence','--include-hook-events','--system-prompt'].every(arg=>args.includes(arg))||
     args[args.indexOf('--tools')+1]!==''||args[args.indexOf('--effort')+1]!=='low'||
     args[args.indexOf('--model')+1]!=='claude-haiku-5-5'||process.env.CALTERNAL_AGENT_TOKEN||
     !prompt.includes('balcony'))process.exit(95);
  emit({type:'system',subtype:'init',model:'claude-haiku-5-5',tools:[],skills:[],mcp_servers:[],slash_commands:[]});
  emit({type:'result',subtype:'success',result:'Three-Pot Balcony Garden'});
  process.exit(0);
}
if(prompt.includes('require-resume')&&!args.includes('--resume'))process.exit(88);
if(prompt.includes('require-fresh')&&args.includes('--resume'))process.exit(89);
if(!args.includes('--append-system-prompt')||!args.includes('--disallowedTools')||process.env.ANTHROPIC_API_KEY||process.env.CLAUDE_CODE_OAUTH_TOKEN)process.exit(93);
if(args.includes('--resume')&&!existsSync(join(process.cwd(),'.fake-session')))process.exit(87);
writeFileSync(join(process.cwd(),'.fake-session'),'fixture session');
emit({type:'system',subtype:'init',session_id:'11111111-1111-4111-8111-111111111111'});
if (prompt.includes('scripted-reasoning')) {
  if (args[args.indexOf('--effort') + 1] !== 'xhigh' ||
      args[args.indexOf('--model') + 1] !== 'claude-sonnet-5-5') process.exit(94);
  const event = event => emit({type:'stream_event',event});
  event({type:'content_block_start',index:0,content_block:{type:'thinking',thinking:''}});
  const token = process.env.CALTERNAL_AGENT_TOKEN ?? 'fixture-thinking';
  const cut = Math.floor(token.length / 2);
  event({type:'content_block_delta',index:0,delta:{type:'thinking_delta',thinking:'Inspecting ' + token.slice(0,cut)}});
  await new Promise(resolve => setTimeout(resolve, 30));
  event({type:'content_block_delta',index:0,delta:{type:'thinking_delta',thinking:token.slice(cut)}});
  event({type:'content_block_delta',index:0,delta:{type:'signature_delta',signature:'PRIVATE_LOGIN_SENTINEL'}});
  event({type:'content_block_stop',index:0});
}
emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text:'Hello 🌏\n'}}});
const scenarioFile=join(process.cwd(),'.fake-scenario');
if(/transient-|usage-limit|auth-failure/.test(prompt))writeFileSync(scenarioFile,prompt);
const scenario=existsSync(scenarioFile)?readFileSync(scenarioFile,'utf8'):prompt;
const counter=join(process.cwd(),'.fake-attempt');
const attempt=existsSync(counter)?Number(readFileSync(counter,'utf8'))+1:1;
if(/transient-|usage-limit|auth-failure/.test(scenario))writeFileSync(counter,String(attempt));
if(scenario.includes('transient-always')||(scenario.includes('transient-once')&&attempt===1)||scenario.includes('usage-limit')||scenario.includes('auth-failure')){
 if(scenario.includes('usage-limit'))emit({type:'rate_limit_event',rate_limit_info:{status:'rejected',resetsAt:Math.floor(Date.now()/1000)+3600}});
 emit({type:'assistant',error:scenario.includes('usage-limit')?'rate_limit':scenario.includes('auth-failure')?'authentication_failed':'server_error',message:{content:[]}});
 emit({type:'result',subtype:'error_during_execution',is_error:true,usage:{input_tokens:0,output_tokens:0}});process.exit(0);
}
if (prompt.includes('thread-size-limit')) {
  for (let i = 0; i < 257; i++) {
    const text = `overflow-segment-${i}:
` + 'x'.repeat(64 * 1024);
    emit({type:'stream_event',event:{type:'content_block_delta',delta:{type:'text_delta',text}}});
  }
} else {
  const scripted = prompt.includes('scripted-tool-steps');
  const step = (id, name, input) =>
    emit({type:'assistant',message:{content:[{type:'tool_use',id,name,input}]}});
  const result = (id, content, is_error) =>
    emit({type:'user',message:{content:[{type:'tool_result',tool_use_id:id,content,is_error}]}});
  step('step-1', scripted ? 'Bash' : prompt.includes('question') ? 'AskUserQuestion' : 'Read',
    scripted ? {command:'ls -la'} : {file_path:'README.md',secret:'PRIVATE_LOGIN_SENTINEL'});
  if(prompt.includes('crash'))process.exit(71);
  if(!scripted && prompt.includes('hang'))await new Promise(()=>setInterval(()=>{},1000));
  result('step-1', scripted && prompt.includes('long-output')
    ? 'PRIVATE_LOGIN_SENTINEL' + 'x'.repeat(96 * 1024)
    : 'PRIVATE_LOGIN_SENTINEL', false);
  if (scripted) {
    const token = process.env.CALTERNAL_AGENT_TOKEN ?? 'private-path';
    step('step-2', 'Read', {file_path:`./missing/${token}`});
    if(prompt.includes('hang'))await new Promise(()=>setInterval(()=>{},1000));
    result('step-2', 'File unavailable', true);
  }
}
if (prompt.includes('scripted-person-wait')) {
  const form = {form:{genui_version:2,source:'$release=""\nForm([Input($release,"Which release?")], "Release selection")'}};
  execFileSync('calternal', ['invoke', 'requests.ask', '--input', JSON.stringify(form)], {stdio:'ignore'});
}
if(prompt.includes('oversized'))process.stdout.write('x'.repeat(1024*1024+1));
emit({type:'result',subtype:prompt.includes('fail')?'error_during_execution':'success',is_error:prompt.includes('fail'),usage:{input_tokens:10,output_tokens:3}});
