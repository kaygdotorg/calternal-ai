// Static publishing configuration. Appearance stays with Starlight defaults.
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightLlmsTxt from 'starlight-llms-txt';

export default defineConfig({
  site: process.env.DOCS_SITE_URL || 'https://preview.calternal.cloud',
  base: process.env.DOCS_BASE || '/docs/',
  output: 'static',
  trailingSlash: 'always',
  integrations: [starlight({
    title: 'Calternal documentation',
    description: 'Generated reference for the Calternal development preview. The app is not released.',
    plugins: [starlightLlmsTxt({ rawContent: true })],
    sidebar: [
      { label: 'User', items: [{ autogenerate: { directory: 'user' } }] },
      { label: 'Administration', items: [{ autogenerate: { directory: 'admin' } }] },
      { label: 'Development', items: [{ autogenerate: { directory: 'dev' } }] },
    ],
  })],
});
