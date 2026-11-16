export type Route =
  | { name: 'activity' }
  | { name: 'notes'; space: string; id?: string; view?: 'history' | 'new' }
  | { name: 'plugins'; space: string; plugin?: string }
  | { name: 'home' };

const seg = (s: string) => encodeURIComponent(s);

export function parsePath(path: string): Route {
  const parts = path.replace(/^\/+/, '').split('/').filter(Boolean).map((p) => { try { return decodeURIComponent(p); } catch { return p; } });
  if (parts[0] === 'activity') return { name: 'activity' };
  if (parts[0] === 's' && parts[1]) {
    const space = parts[1];
    if (parts[2] === 'plugins') return { name: 'plugins', space, plugin: parts[3] };
    if (parts[2] === 'notes' || parts.length === 2) {
      if (parts[3] === 'new') return { name: 'notes', space, view: 'new' };
      return { name: 'notes', space, id: parts[3], view: parts[4] === 'history' ? 'history' : undefined };
    }
  }
  return { name: 'home' };
}

export function hrefFor(r: Route): string {
  switch (r.name) {
    case 'activity': return '/activity';
    case 'notes':
      if (r.view === 'new') return `/s/${seg(r.space)}/notes/new`;
      return `/s/${seg(r.space)}/notes` + (r.id ? `/${seg(r.id)}` + (r.view === 'history' ? '/history' : '') : '');
    case 'plugins': return `/s/${seg(r.space)}/plugins` + (r.plugin ? `/${seg(r.plugin)}` : '');
    default: return '/';
  }
}
