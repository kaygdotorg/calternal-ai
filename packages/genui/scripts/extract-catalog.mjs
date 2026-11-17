// One-time, development-host-only extraction from the credited upstream catalog.
import { pathToFileURL } from 'node:url';
const { COMPONENTS, ACTIONS, FUNCTIONS, METHODS, CONSTANTS, FORBIDDEN_PROPERTIES } = await import(pathToFileURL(process.argv[2]).href);
import { writeFileSync } from 'node:fs';
const dropped = ['File', 'Snippet', 'ImageSearch', 'Environments', 'Scene', 'Shader'];
const visual = new Set(['Icon','Chart','Sparkline','Progress','Gauge','CalendarHeatmap','SegmentedBar','Diagram','Gallery','Map','Hotspots','Compare','Transcript','Entity']);
const interactive = new Set(['Tabs','Carousel','Tree','Sources','List','Timer','Quiz','Flashcards','Walkthrough','Calendar','Board','Roadmap','Plan','Decision','Estimate','Acceptance']);
const components = Object.fromEntries(Object.entries(COMPONENTS).filter(([name,spec]) => !['Project','Dev'].includes(spec.group) && !dropped.includes(name)).map(([name,original]) => {
  const spec=structuredClone(original);
  if(spec.summary)spec.summary=spec.summary.replace(' or a Load()ed task/issue list','');
  spec.params=spec.params.filter(p=>!p.legacy);
  const required = name==='Image' ? ['alt'] : name==='Details' ? ['title'] : spec.group==='Inputs' || visual.has(name) || interactive.has(name) ? ['label'] : [];
  if (visual.has(name) && name!=='Icon') required.push('description');
  for(const field of required){
    let param=spec.params.find(p=>p.name===field);
    if (!param){param={name:field,kind:'string'};spec.params.push(param);}
    delete param.optional;delete param.nullable;
  }
  if(name==='Image')spec.summary='image source identifier, resolved by an authorized client adapter';
  spec.accessibility={required};
  return [name,spec];
}));
const functions={...FUNCTIONS};delete functions.Load;
const data={components,actions:ACTIONS,functions,methods:METHODS,constants:CONSTANTS,forbiddenProperties:FORBIDDEN_PROPERTIES};
writeFileSync(new URL('../src/catalog-data.json',import.meta.url),JSON.stringify(data,null,2)+'\n');
