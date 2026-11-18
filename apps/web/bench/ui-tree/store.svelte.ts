import type { Node, Patch } from "./protocol";
/** The renderer's state: nodes by id (props), child id lists, per-list counts and fetched rows. Patches are applied here; components read it. */
class Store {
  nodes = $state<Record<string, Node>>({});
  kids = $state<Record<string, string[]>>({});
  counts = $state<Record<string, number>>({});
  rows = $state<Record<string, Record<number, { title: string; sub: string }>>>({});
  range: (id: string, a: number, b: number) => void = () => {};
  private add(n: Node, parent?: string, index?: number) {
    this.nodes[n.id] = { id: n.id, type: n.type, props: n.props };
    this.kids[n.id] = [];
    for (const c of n.children ?? []) this.add(c, n.id);
    if (parent) { const k = this.kids[parent]!; if (index === undefined) k.push(n.id); else k.splice(index, 0, n.id); }
  }
  private drop(id: string) { for (const c of this.kids[id] ?? []) this.drop(c); delete this.nodes[id]; delete this.kids[id]; }
  apply(p: Patch) {
    if (p.op === "tree") this.add(p.root);
    else if (p.op === "set") Object.assign(this.nodes[p.id]!.props, p.props);
    else if (p.op === "append") this.nodes[p.id]!.props.text = String(this.nodes[p.id]!.props.text) + p.text;
    else if (p.op === "insert") this.add(p.node, p.parent, p.index);
    else if (p.op === "remove") { for (const k of Object.values(this.kids)) { const i = k.indexOf(p.id); if (i >= 0) k.splice(i, 1); } this.drop(p.id); }
    else if (p.op === "count") this.counts[p.id] = p.n;
    else if (p.op === "rows") { const t = (this.rows[p.id] ??= {}); p.items.forEach((it, k) => (t[p.start + k] = it)); }
  }
}
export const store = new Store();
