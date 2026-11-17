"""Write language-neutral golden cases from upstream examples and explicit contract expectations.
This is fixture authoring, not an implementation oracle: expected values are specified below.
Run after intentional catalog changes and review the resulting JSON before accepting it.
"""
import json
from pathlib import Path
root=Path(__file__).parent
catalog=json.loads((root/'../../packages/genui/src/catalog-data.json').read_text())['components']
cases=[]
def diag(message,line=1,column=1):return {'severity':'error','line':line,'column':column,'message':message}
def node(name,props,key='1:1'):return {'kind':'node','type':name,'key':key,'props':props}
def add(name,kind,source,expected,**kw):cases.append(dict(name=name,kind=kind,source=source,expected=expected,**kw))
def ev(name,source,roots,state=None,diagnostics=None,**kw):add(name,'evaluate',source,{'state':state or {},'roots':roots,'diagnostics':diagnostics or []},**kw)
def expr(name,source,result):ev(name,'Text('+source+')',[node('Text',{'text':str(result) if not isinstance(result,bool) else str(result).lower()})])
def valid(name,source):add(name,'validate',source,{'accepted':True,'diagnostics':[]})
def invalid(name,source,messages,**kw):add(name,'validate',source,{'accepted':False,'diagnostics':messages},**kw)
# Upstream core/language.test.ts: arithmetic, named values, state, templates and pure functions.
for name,source,result in [
 ('arithmetic','1 + 2 * 3','7'),('power-right','2 ** 3 ** 2','512'),('power-prefix','-2 ** 2','-4'),
 ('parentheses','(1 + 2) * 3','9'),('nullish','1 / 0 ?? "inf"','inf'),('null-text','null + "x" + true','xtrue'),
 ('logical-short-circuit','false && (1 / 0)','false'),('conditional','3 > 2 ? "yes" : "no"','yes'),
 ('strict-number-string','1 == "1"','false'),('strict-scalars','true !== false','true'),
 ('template','`hello ${2 + 3}`','hello 5'),('escaped','"line\\nnext"','line\nnext'),
 ('unicode-braced','"\\u{1f680}"','🚀'),('unicode-length','"🚀".length','2'),('unicode-index','"🚀"[0]','�'),
 ('unicode-slice','"a🚀b".slice(1,3)','🚀'),('unicode-split','"🚀".split("").join("-")','�-�'),
 ('sum','sum([1,2,null,3])','6'),('min','min([7,5])','5'),('max','max(4,9,2)','9'),
 ('round-decimal','round(2.345,2)','2.35'),('round-away','round(-1.5)','-2'),('fmt','fmt(1234.5)','1,234.5'),
 ('fmt-digits','fmt(1234.5,0)','1,235'),('fmt-negative','fmt(-1234567.891)','-1,234,567.89'),
 ('fixed','3.14159.toFixed(2)','3.14'),('abs','abs(-4)','4'),('floor','floor(1.9)','1'),('ceil','ceil(1.1)','2'),
 ('sqrt','sqrt(9)','3'),('pow','pow(2,5)','32'),('trig','sin(0) + cos(0) + tan(0)','1'),
 ('inverse-trig','asin(0) + acos(1) + atan(0) + atan2(0,1)','0'),
 ('clamp','clamp(20,0,10)','10'),('lerp','lerp(2,6,0.5)','4'),('range','range(1,5).join(",")','1,2,3,4'),
 ('range-descending','range(3,0,-1).join(",")','3,2,1'),('range-empty','range(3,1).length','0'),
 ('methods','"a,b".split(",").join("+").toUpperCase()','A+B'),('trim','" A ".trim().toLowerCase()','a'),
 ('includes','"abc".includes("b") && "abc".startsWith("a") && "abc".endsWith("c")','true'),
 ('list-index','[1,2,3].indexOf(2)','1'),('list-includes','[1,2].includes(3)','false'),('slice-negative','[1,2,3].slice(-2).join()','2,3'),
 ('list-find','[1,2,3].find(x => x > 1)','2'),('list-some','[1,2].some(x=>x==2)','true'),('list-every','[1,2].every(x=>x>0)','true'),
 ('list-empty-every','[].every(x=>false)','true'),('object-member','{count:3}.count * 2 + ({count:3}.missing ?? 10)','16'),
 ('keys','keys({b:2,a:1}).join()','b,a'),('integer-keys','keys({"2":2,"1":1,z:0}).join()','1,2,z'),
 ('values','values({b:2,a:1}).join()','2,1'),('entries','entries({b:2})[0].join(":")','b:2'),
 ('scientific','1e21 + ""','1e+21'),('small-scientific','1e-7 + ""','1e-7'),('small-decimal','1e-6 + ""','0.000001')]:expr(name,source,result)
# Original upstream expectation: stable keys depend on source positions and iteration suffixes.
source='Stack([[1,2],[3]].map(row => Row(row.map(x => Badge(x)))))'
row=source.index('Row(')+1;badge=source.index('Badge(')+1
ev('nested-map',source,[node('Stack',{'children':[
 node('Row',{'children':[node('Badge',{'text':'1'},f'1:{badge}#0#0'),node('Badge',{'text':'2'},f'1:{badge}#0#1')]},f'1:{row}#0'),
 node('Row',{'children':[node('Badge',{'text':'3'},f'1:{badge}#1#0')]},f'1:{row}#1')
]})])
ev('named-root','root = Text("hi")',[node('Text',{'text':'hi'},'1:8')])
ev('named-value','greeting = "hi"\nText(greeting)',[node('Text',{'text':'hi'},'2:1')])
ev('state-initial',' $n = 2\nText($n)',[node('Text',{'text':'2'},'2:1')],{'n':2})
ev('state-override','$n = 2\nText($n)',[node('Text',{'text':'5'},'2:1')],{'n':5})
# Avoid duplicate Python keyword: override state supplied separately below.
cases[-1]['state']={'n':5}
ev('state-order','$a=2\n$b=$a+1\nText($b)',[node('Text',{'text':'3'},'3:1')],{'a':2,'b':3})
ev('named-function','greet = x => Text("Hi " + x)\ngreet("A")',[node('Text',{'text':'Hi A'},'1:14@2:1')])
ev('named-reference-identity','a = [1]\nText(a === a)',[node('Text',{'text':'true'},'2:1')])
expr('distinct-reference-identity','[1] === [1]','false')
ev('optional-null','Metric(value:"5",label:"Score",tone:null)',[node('Metric',{'label':'Score','value':'5'})])
ev('set-copy-actions','$n=2\nButton("Inc",[Set($n,$n+1),Copy("n="+$n)])',[node('Button',{'label':'Inc','action':[{'kind':'action','type':'set','name':'n','value':3},{'kind':'action','type':'copy','text':'n=2'}]},'2:1')],{'n':2})
ev('send-url-actions','Button("Go",[Send("hello"),OpenUrl("https://example.com")])',[node('Button',{'label':'Go','action':[{'kind':'action','type':'send','text':'hello'},{'kind':'action','type':'openUrl','url':'https://example.com'}]})])
valid('operator-continuation','label = true\n ? "yes"\n : "no"\nText(label +\n "!")')
valid('comments-and-semicolons','// comment\n$n=1; Text($n) // end')
# Validation diagnostics: portable rewrites of upstream unknown name, cycles, arguments and safety cases.
invalid('unknown-name','Text(missing)',[diag('unknown name: missing',1,6)])
invalid('unknown-state','Text($missing)',[diag('unknown state: $missing',1,6)])
invalid('unknown-call','Missing()',[diag('unknown call: Missing')])
invalid('unknown-method','Text([1].evil())',[diag('unknown method: evil',1,10)])
invalid('duplicate-state','$n=1\n$n=2\nText($n)',[diag('duplicate declaration: $n',2,1)])
invalid('duplicate-named','a=1\na=2\nText(a)',[diag('duplicate declaration: a',2,1)])
invalid('direct-cycle','a=a\nText(a)',[diag('cyclic named value: a')])
invalid('indirect-cycle','a=b\nb=a\nText(a)',[diag('cyclic named value: a')])
invalid('state-self','$n=$n\nText($n)',[diag('state initializer reads uninitialized state: $n')])
invalid('state-forward','$n=$m\n$m=1\nText($n)',[diag('state initializer reads uninitialized state: $m')])
invalid('state-transitive','$n=amount\n$m=1\namount=$m\nText($n)',[diag('state initializer reads uninitialized state: $m')])
invalid('prototype-read','Text({}.constructor)',[diag('`constructor` is not accessible',1,9)])
invalid('prototype-index','Text({}["__proto__"])',[diag('`__proto__` is not accessible',1,9)])
invalid('prototype-key','Text({prototype:1})',[diag('`prototype` is not accessible',1,7)])
invalid('wrong-arity','Text(pow(2))',[diag('wrong argument count: pow',1,6)])
invalid('missing-argument','Text()',[diag('missing argument: Text.text')])
invalid('unknown-argument','Text("a",extra:1)',[diag('unknown argument: Text.extra',1,10)])
invalid('duplicate-argument','Text("a",text:"b")',[diag('duplicate argument: text',1,10)])
invalid('binding-expression','$n=1\nSlider($n+1,0,2,label:"Count")',[diag('Slider.value must be a $state reference',2,10)])
invalid('arrow-root','x => x',[diag('arrow functions are only allowed in named functions and list methods')])
invalid('lambda-not-arrow','Text([1].map(2))',[diag('.map requires an arrow function',1,14)])
# Parser AST goldens are hand-shaped, independent of either parser.
pos=lambda column,line=1:{'line':line,'column':column}
lit=lambda v,c:{'type':'literal','value':v,'pos':pos(c)}
add('parse-literal','parse','Text("hi")',{'statements':[{'kind':'root','value':{'type':'call','name':'Text','args':[lit('hi',6)],'named':[],'pos':pos(1)}}],'diagnostics':[]})
add('parse-state','parse','$n=2\nText($n)',{'statements':[{'kind':'state','name':'n','value':lit(2,4),'pos':pos(1)},{'kind':'root','value':{'type':'call','name':'Text','args':[{'type':'state','name':'n','pos':pos(6,2)}],'named':[],'pos':pos(1,2)}}],'diagnostics':[]})
add('parse-precedence','parse','Text(1+2*3)',{'statements':[{'kind':'root','value':{'type':'call','name':'Text','args':[{'type':'binary','op':'+','left':lit(1,6),'right':{'type':'binary','op':'*','left':lit(2,8),'right':lit(3,10),'pos':pos(9)},'pos':pos(7)}],'named':[],'pos':pos(1)}}],'diagnostics':[]})
invalid('syntax-unclosed','Text("a"',[diag('expected `,` or `)` to close `(` from 1:5, found end of input',1,9)])
invalid('syntax-string','Text("x)',[diag('unterminated string: close it with " on the same line',1,6),diag('expected an expression, found end of input',1,9)])
invalid('syntax-invalid-number','Text(12abc)',[diag('invalid number `12a`',1,6),diag('expected `,` or `)` to close `(` from 1:5, found `abc`',1,8)])
invalid('syntax-unexpected','Text(@)',[diag('unexpected character `@`',1,6)])
invalid('syntax-unicode-column','Text("🚀"); @',[diag('unexpected character `@`',1,13)])
# Resource ceilings: validation and evaluation must both fail closed.
for key,source in [('nodes','Text("ok")'),('depth','Text("ok")'),('stateEntries','$a=1\n$b=2\nText($a)'),('listLength','Text([1,2])'),('serializedBytes','Text("🚀")')]:
 limit=1 if key in ['stateEntries','listLength'] else 0
 invalid('compile-budget-'+key,source,[diag('budget exceeded: '+key,1,6 if key=='listLength' else 1)],budgets={key:limit})
for name,source,limits,state,position in [
 ('steps','Text("ok")',{'steps':0},None,(1,1)),
 ('depth','Text("ok")',{'depth':1},None,(1,6)),
 ('listLength','Text(range(0,4))',{'listLength':3},None,(1,6)),
 ('stateEntries','Text("ok")',{'stateEntries':1},{'a':1,'b':2},(1,1)),
 ('state-depth','Text("ok")',{'depth':3},{'a':{'b':{'c':{'d':1}}}},(1,1)),
 ('serializedBytes','Text("ok")',{'serializedBytes':1000},{'a':'x'*1001},(1,1)),
 ('nodes','Stack(range(0,20).map(x=>Text(x)))',{'nodes':12},None,(1,26)),
]:
 ev('evaluate-budget-'+name,source,[],diagnostics=[diag('budget exceeded: '+('depth' if name=='state-depth' else name),*position)],evaluationBudgets=limits)
 if state is not None:cases[-1]['state']=state
# Static ceilings are reported at the first expression (source order) that crosses them,
# as the original GenUI reports its parser and evaluator limits at the offending expression.
invalid('compile-budget-depth-located','Text(1+(2+3))',[diag('budget exceeded: depth',1,9)],budgets={'depth':3})
invalid('compile-budget-nodes-located','Text("a")\nText("b")',[diag('budget exceeded: nodes',2,6)],budgets={'nodes':3})
invalid('compile-budget-listLength-located','Text([[1,2]])',[diag('budget exceeded: listLength',1,7)],budgets={'listLength':1})
# Non-budget runtime failures become local Error nodes; resource failures never do.
message='range step cannot be zero'
ev('runtime-component-error','Text(range(0,3,0))',[node('Error',{'message':message})],diagnostics=[diag(message,1,6)])
message='accessibility: Button.label must be nonblank text'
ev('runtime-accessibility','$label=""\nButton($label)',[node('Error',{'message':message},'2:1')],{'label':''},[diag(message,2,1)])
# One positive catalog case and one missing-field case per accessibility requirement.
for name,spec in catalog.items():
 args=[];props={};state={};prelude=''
 for p in spec['params']:
  if p.get('optional'):continue
  field=p['name'];kind=p['kind']
  if kind=='binding':
   val={'number':1,'boolean':True,'list':[],'text':''}.get(p.get('holds'),'')
   state['value']=val;prelude='$value='+json.dumps(val)+'\n';value='$value';expected={'kind':'binding','name':'value','value':val}
  elif kind in ['string','text']:value=json.dumps('Accessible' if field in ['label','alt','title'] else 'Description' if field=='description' else 'Value');expected=json.loads(value)
  elif kind=='number':value='1';expected=1
  elif kind=='boolean':value='true';expected=True
  elif kind in ['list','nodes','series']:value='[]';expected=[]
  elif kind=='enum':expected=p['values'][0];value=json.dumps(expected)
  elif kind=='actions':value='Copy("value")';expected=[{'kind':'action','type':'copy','text':'value'}]
  else:value='null';expected=None
  args.append((field,value));props[field]=expected
 source=prelude+name+'('+','.join(f'{f}:{v}' for f,v in args)+')'
 ev('catalog-'+name,source,[node(name,props,'2:1' if prelude else '1:1')],state)
 for field in spec['accessibility']['required']:
  source=prelude+name+'('+','.join(f'{f}:{v}' for f,v in args if f!=field)+')'
  invalid('accessibility-missing-'+name+'-'+field,source,[diag(f'accessibility: {name}.{field} is required',2 if prelude else 1,1)])
# Additional upstream composition/state/error-isolation cases.
expr('math-aliases','Math.round(Math.max(2.4,Math.floor(3.9)))+Math.ceil(0.2)+Math.pow(2,3)+Math.sqrt(9)','15')
expr('object-alias','Object.keys({react:"19.0.0",zod:"4.1.0"}).join(",")','react,zod')
expr('negative-power','2 ** -1','0.5')
expr('compound-interest','round(5000 * (1 + 0.005) ** 120,2)','9096.98')
expr('unicode-escaped-pair','"\\uD83D\\uDE80"','🚀')
expr('random-stable','random(0) === random(-0)','true')
ev('function-missing-arg','rate=2\nf=(a,b)=>(b??10)*a*rate\nText(fmt(f(3)))',[node('Text',{'text':'60'},'3:1')])
ev('multi-word-keys','KeyValue({Good for:"views",Day 1:"Lisbon",Area:"Alfama"})',[node('KeyValue',{'pairs':{'Good for':'views','Day 1':'Lisbon','Area':'Alfama'}})])
ev('steps-objects','Steps([{title:"Design",status:"done"},{title:"Build",detail:"API first",status:"current"}])',[node('Steps',{'items':[{'title':'Design','status':'done'},{'title':'Build','detail':'API first','status':'current'}]})])
ev('code-template','Code(`const x = 1\nexport default x`,"ts")',[node('Code',{'code':'const x = 1\nexport default x','language':'ts'})])
for n,source,message,column in [
 ('scalar-binding-list','$picked="a"\nChecklist($picked,["a","b"],label:"Checks")',None,0),
 ('scalar-binding-empty','$picked=""\nChecklist($picked,["a"],label:"Checks")',None,0),
 ('scalar-binding-number','$n="12"\nSlider($n,0,20,label:"Count")',None,0),
 ('scalar-binding-text','$q=0\nInput($q,label:"Query")',None,0),
]:
 state={'picked':'a'} if n=='scalar-binding-list' else {'picked':''} if n=='scalar-binding-empty' else {'n':'12'} if n=='scalar-binding-number' else {'q':0}
 if n.startswith('scalar-binding-list') or n.endswith('empty'):
  props={'value':{'kind':'binding','name':'picked','value':['a'] if n=='scalar-binding-list' else []},'options':['a','b'] if n=='scalar-binding-list' else ['a'],'label':'Checks'};component='Checklist'
 elif n.endswith('number'):props={'value':{'kind':'binding','name':'n','value':12},'min':0,'max':20,'label':'Count'};component='Slider'
 else:props={'value':{'kind':'binding','name':'q','value':'0'},'label':'Query'};component='Input'
 ev(n,source,[node(component,props,'2:1')],state)
source='items=[1,2]\nRow([Metric("Bad",items),Metric("Good","ok"),Text(items.length * {a:1})])'
m1='Metric.value needs string';m2='expected a number'
ev('sibling-error-isolation',source,[node('Row',{'children':[node('Error',{'message':m1},'2:6'),node('Metric',{'label':'Good','value':'ok'},'2:26'),node('Error',{'message':m2},'2:46')]},'2:1')],diagnostics=[diag(m1,2,19),diag(m2,2,64)])
source='Card(null,[{kind:"node",type:"Text",key:"x",props:{text:"fake"}}])';message='reserved interpreter value kind'
ev('forged-literal-node',source,[node('Error',{'message':message})],diagnostics=[diag(message,1,13)])
ev('forged-state-node','Text("safe")',[],diagnostics=[diag(message)])
cases[-1]['state']={'injected':{'kind':'node','type':'Button','key':'fake','props':{}}}
source='Text(range(0,20).map(a=>range(0,20).map(b=>a+b)).length)'
ev('evaluate-budget-loop-steps',source,[],diagnostics=[diag('budget exceeded: steps',1,source.index('a+b')+2)],evaluationBudgets={'steps':1000})
source='xs=['+','.join(map(str,range(900)))+']\nStack(xs.map(a=>Row([Badge(a),Badge(a)])))'
ev('evaluate-budget-produced-nodes',source,[],diagnostics=[diag('budget exceeded: nodes',2,17)])
invalid('nonfinite-literal','Text(1e309)',[diag('number must be finite',1,6)])
# Every accessible field must reject blank and null literal values, not only omission.
for c in list(cases):
 if not c['name'].startswith('catalog-'):continue
 component=c['name'][8:];spec=catalog[component];source=c['source'];line=source.count('\n')+1
 for field in spec['accessibility']['required']:
  import re
  for label,value in [('blank','"   "'),('null','null')]:
   pattern=r'('+re.escape(field)+r':)"[^"\n]*"'
   altered=re.sub(pattern,lambda m:m.group(1)+value,source,count=1)
   last=altered.split('\n')[-1];at=last.index(field+':')+len(field)+2
   invalid('accessibility-'+label+'-'+component+'-'+field,altered,[diag(f'accessibility: {component}.{field} must be nonblank text',line,at)])

# Repeated cached nodes count once per occurrence in the emitted tree.
source='a=Text("leaf")\n'+'\n'.join(chr(98+i)+'=Stack(['+chr(97+i)+','+chr(97+i)+'])' for i in range(10))+'\nk'
ev('evaluate-budget-reused-nodes',source,[],diagnostics=[diag('budget exceeded: nodes')])
add('initialize-only','initialize','$n=2\nText($n)',{'state':{'n':2},'diagnostics':[]})
add('initialize-validates-full-program','initialize','$n=2\nText($n)',{'state':{},'diagnostics':[diag('budget exceeded: nodes',2,6)]},evaluationBudgets={'nodes':2})
invalid('invalid-budget-ceiling','Text("ok")',[diag('invalid budget: steps')],budgets={'steps':200001})
ev('invalid-evaluation-budget','Text("ok")',[],diagnostics=[diag('invalid budget: steps')],evaluationBudgets={'steps':200001})

# Split by kind/category so fixture counts stay inspectable in review.
for family,selected in [('parsing',[c for c in cases if c['kind']=='parse' or c['name'].startswith('syntax-')]),('catalog',[c for c in cases if c['name'].startswith(('catalog-','accessibility-'))]),('language',[c for c in cases if c['kind']!='parse' and not c['name'].startswith(('syntax-','catalog-','accessibility-','compile-budget-','evaluate-budget-'))]),('budgets',[c for c in cases if c['name'].startswith(('compile-budget-','evaluate-budget-'))])]:
 (root/(family+'.json')).write_text(json.dumps({'version':1,'origin':'Adapted from bb-plugin-genui d8cac8d core/language.test.ts and core/components.test.ts; additional explicit accessibility and resource cases.','cases':selected},indent=2,ensure_ascii=False)+'\n')
