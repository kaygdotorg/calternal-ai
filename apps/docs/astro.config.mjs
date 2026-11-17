// Static publishing configuration. The site wears the design system: its fonts and colours (src/styles), a theme picker offering the app's presets
// (src/components/ThemeSelect.astro), and a small inline script that applies the chosen preset before the first paint so there is no flash.
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLlmsTxt from 'starlight-llms-txt';
import { readFileSync } from 'node:fs';

// Written by scripts/prepare.mjs; an empty list before it has run, so `astro` alone still starts.
let presets = [];
try { presets = JSON.parse(readFileSync(new URL('./src/presets.generated.json', import.meta.url), 'utf8')); } catch { /* not prepared yet */ }
const early = `(function(){try{var id=localStorage.getItem('cal-docs-preset');if(!id)return;var P=${JSON.stringify(presets)};var p=P.find(function(x){return x.id===id});if(!p)return;var t=localStorage.getItem('starlight-theme');var dark=t?t==='dark':matchMedia('(prefers-color-scheme: dark)').matches;var pal=dark?p.dark:p.light;if(!pal)return;var s=document.documentElement.style;s.setProperty('--cal-paper',pal.paper);s.setProperty('--cal-ink',pal.ink);s.setProperty('--cal-accent',pal.accent);s.setProperty('--cal-wall','none')}catch(e){}})()`;

export default defineConfig({
  site: process.env.DOCS_SITE_URL || 'https://preview.calternal.cloud',
  base: process.env.DOCS_BASE || '/docs/',
  output: 'static',
  trailingSlash: 'always',
  integrations: [starlight({
    title: 'Calternal documentation',
    description: 'Generated reference for the Calternal development preview. The app is not released.',
    plugins: [starlightLlmsTxt({ rawContent: true })],
    customCss: ['./src/styles/tokens.generated.css', './src/styles/calternal.css'],
    components: { ThemeSelect: './src/components/ThemeSelect.astro' },
    head: [{ tag: 'script', content: early }],
    sidebar: [
      { label: 'User', items: [{ autogenerate: { directory: 'user' } }] },
      { label: 'Administration', items: [{ autogenerate: { directory: 'admin' } }] },
      { label: 'Development', items: [{ autogenerate: { directory: 'dev' } }] },
    ],
  })],
});
