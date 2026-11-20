#!/usr/bin/env python3
"""Bench-only: turn results/*.jsonl into the Markdown tables of
docs/measurements/plugin-split-memory-2026-11-20.md and a JSON companion.
Usage: summarize.py <results dir> <json out>"""
import json
import statistics as st
import sys
from collections import defaultdict
from pathlib import Path

R = Path(sys.argv[1])
OUT = {}


def load(name):
    p = R / f'{name}.jsonl'
    return [json.loads(l) for l in p.read_text().splitlines()] if p.exists() else []


def q(xs, f):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, max(0, -(-int(f * len(xs) * 1000) // 1000) - 1))] if xs else None


def p95(xs):
    xs = sorted(xs)
    import math
    return xs[max(0, math.ceil(0.95 * len(xs)) - 1)]


def fmt(x, d=2):
    return '—' if x is None else f'{x:.{d}f}'


def tri(xs, d=2):
    return f'{fmt(st.median(xs), d)} / {fmt(p95(xs), d)} / {fmt(max(xs), d)}'


def table(head, rows):
    print('| ' + ' | '.join(head) + ' |')
    print('| ' + ' | '.join('---' for _ in head) + ' |')
    for r in rows:
        print('| ' + ' | '.join(str(c) for c in r) + ' |')
    print()


ev = load('layouts') + load('zero')
if ev:
    print('### Memory and cold start (layouts)\n')
    mem = defaultdict(list)
    cold = defaultdict(list)
    for e in ev:
        if e['kind'] == 'mem':
            mem[(e['mode'], e['n'], e['stage'])].append(e)
        if e['kind'] == 'cold_start':
            cold[(e['mode'], e['n'])].append(e)
    rows, js = [], []
    base = mem.get(('none', 0, 'after_calls'))
    base_used = st.median(e['guest_used_mib'] for e in base) if base else None
    base_host = st.median(e['host_pss_mib'] for e in base) if base else None
    for (mode, n, stage), es in sorted(mem.items(), key=lambda kv: (kv[0][0] != 'none', kv[0])):
        if stage != 'after_calls':
            continue
        anon = [sum(p['pss_anon'] for p in e['procs']) for e in es]
        used_m = st.median(e['guest_used_mib'] for e in es)
        added = used_m - base_used if base_used is not None else None
        pss = [e['plugin_pss_mib'] for e in es]
        rss = [e['plugin_rss_mib'] for e in es]
        used = [e['guest_used_mib'] for e in es]
        vmm = [e['vmm_pss_mib'] for e in es]
        host = [e['host_pss_mib'] for e in es]
        per = [p['pss'] for e in es for p in e['procs']]
        c = cold[(mode, n)]
        rows.append([mode, n, len(es), fmt(st.median(pss)), fmt(st.median(anon)), fmt(st.median(rss)),
                     fmt(st.median(host)), fmt(used_m), fmt(added), fmt(added / n) if n and added is not None else '—',
                     fmt(st.median(vmm)),
                     tri([x['ready_ms'] for x in c]) if c else '—', tri([x['boot_ms'] for x in c]) if c else '—'])
        js.append(dict(mode=mode, n=n, runs=len(es), plugin_pss_mib=pss, plugin_pss_anon_mib=anon, plugin_rss_mib=rss, guest_used_mib=used,
                       vmm_pss_mib=vmm, host_pss_mib=host, per_process_pss_mib=per,
                       ready_ms=[x['ready_ms'] for x in c], boot_ms=[x['boot_ms'] for x in c],
                       heaps=[e.get('heaps') for e in es]))
    table(['Layout', 'Plugins', 'Boots', 'Plugin PSS MiB', 'Plugin private (anon PSS) MiB', 'Plugin RSS MiB', 'Host PSS MiB',
           'Guest used MiB', 'Added over no plugins MiB', 'Added per plugin MiB', 'VMM PSS MiB', 'Start all p50/p95/max ms', 'Boot to host p50/p95/max ms'], rows)
    OUT['layouts'] = js
    print('### Per process PSS, 7 plugins, process layout (median of boots)\n')
    per = defaultdict(list)
    for e in ev:
        if e['kind'] == 'mem' and e['mode'] == 'process' and e['n'] == 7 and e['stage'] == 'after_calls':
            for p in e['procs']:
                per[p['name']].append((p['pss'], p['rss'], p['pss_anon'], p['pss_file']))
    table(['Plugin', 'PSS', 'RSS', 'PSS anon', 'PSS file'],
          [[k, *(fmt(st.median(x[i] for x in v)) for i in range(4))] for k, v in per.items()])
    heaps = [e['heaps'] for e in ev if e['kind'] == 'mem' and e['mode'] == 'shared' and e['n'] == 7 and e['stage'] == 'after_calls' and e.get('heaps')]
    if heaps:
        print('### Shared runtime, 7 plugins: V8 heap per worker thread (first boot)\n')
        table(['Thread', 'Used MiB', 'Total MiB', 'Limit MiB', 'External MiB'],
              [[h['p'], fmt(h['used_mib']), fmt(h['total_mib']), fmt(h['limit_mib']), fmt(h['external_mib'])] for h in heaps[0]['heaps']])
        print('Main thread:', json.dumps(heaps[0]['main']), '\n')
    print('### Call latency in the guest (1,000 calls after 50 warm-up)\n')
    rows = []
    for e in ev:
        if e['kind'] == 'latency' and e['n'] == 7:
            rows.append([e['mode'], e['plugin'], len(e['samples_us']), tri(e['samples_us'], 1)])
    table(['Layout', 'Plugin', 'Calls', 'p50 / p95 / max µs'], rows)
    OUT['latency'] = [dict(mode=e['mode'], n=e['n'], plugin=e['plugin'], p50=st.median(e['samples_us']), p95=p95(e['samples_us']), max=max(e['samples_us']), count=len(e['samples_us'])) for e in ev if e['kind'] == 'latency']

ev = load('wake')
if ev:
    print('### Wake from a full snapshot\n')
    w = defaultdict(list)
    snap = {}
    for e in ev:
        if e['kind'] == 'wake':
            w[(e['mode'], e['n'], e['cache'])].append(e)
        if e['kind'] == 'snapshot_mem':
            snap[(e['mode'], e['n'])] = e
    rows = []
    for (mode, n, cache), es in sorted(w.items()):
        s = snap.get((mode, n), {})
        rows.append([mode, n, cache, len(es), tri([e['wake_ms'] for e in es]), fmt(st.median(e['vmm_pss_mib'] for e in es)), fmt(s.get('guest_used_mib'))])
    table(['Layout', 'Plugins', 'Cache', 'Wakes', 'Wake p50 / p95 / max ms', 'VMM PSS after wake MiB', 'Guest used at snapshot MiB'], rows)
    OUT['wake'] = {f'{k[0]}-{k[1]}-{k[2]}': [e['wake_ms'] for e in v] for k, v in w.items()}

ev = load('lazy')
if ev:
    print('### First call after a stop\n')
    g = defaultdict(list)
    warmup = {}
    for e in ev:
        if e['kind'] == 'first_call':
            g[(e['runtime'], e['compile_cache'], e['plugin'], e['cache'])].append(e['first_ms'])
            if e['following_us']:
                warmup[(e['runtime'], e['compile_cache'], e['plugin'], e['cache'])] = e['following_us']
    rows = []
    for k, xs in sorted(g.items()):
        f = warmup.get(k, [])
        rows.append([k[0], 'on' if k[1] else 'off', k[2], k[3], len(xs), tri(xs),
                     ' / '.join(fmt(f[i] / 1e3, 2) for i in (0, 1, 4, 19)) if len(f) >= 20 else '—'])
    table(['Runtime', 'Compile cache', 'Plugin', 'Cache', 'Samples', 'First call p50 / p95 / max ms', 'Calls 2, 3, 6, 21 ms'], rows)
    OUT['first_call'] = {'-'.join(map(str, k)): v for k, v in g.items()}
    OUT['first_call_following_us'] = {'-'.join(map(str, k)): v for k, v in warmup.items()}

ev = load('timeline')
if ev:
    print('### Memory over time (all 7 used at 0 s, Threads every 2 s, Notes once at 30 s)\n')
    rows = []
    for e in ev:
        s = e['series']
        pick = lambda t: min(s, key=lambda x: abs(x['t_ms'] - t))
        rows.append([e['mode'], e['runtime'], e['idle_ms'], *(f"{fmt(pick(t)['plugin_pss_mib'], 1)} ({pick(t)['live_threads'] if pick(t)['live_threads'] is not None else pick(t)['live']})" for t in (1000, 10000, 20000, 31000, 40000, 50000, 59000)),
                     fmt(sum(x['plugin_pss_mib'] for x in s) / len(s), 1)])
    table(['Layout', 'Runtime', 'Idle stop ms', '1 s', '10 s', '20 s', '31 s', '40 s', '50 s', '59 s', 'Mean PSS MiB'], rows)
    OUT['timeline'] = [dict(mode=e['mode'], runtime=e['runtime'], idle_ms=e['idle_ms'], series=e['series']) for e in ev]

ev = load('faults')
if ev:
    print('### One misbehaving plugin\n')
    rows = []
    for e in ev:
        others = e['others']
        alive = all(o['alive'] for o in others.values())
        failed = sum(o['failed'] for o in others.values())
        calls = sum(o['calls'] for o in others.values())
        worst = max((o['p95_us'] or 0) for o in others.values())
        evs = '; '.join(f"{x['kind']} {x.get('plugin') or ''} {x.get('code') if x.get('code') is not None else ''}{x.get('signal') or ''}".strip() for x in e['events'][:4])
        rows.append([e['mode'], e['fault_kind'], 'yes' if alive else 'no', f'{calls} / {failed}', fmt(worst / 1e3), 'yes' if e['faults_alive'] else 'no',
                     json.dumps(e['stopped']) if e['stopped'] else '—', evs or '—', len(e['forged']) and ', '.join(f['plugin'] for f in e['forged']) or '—',
                     json.dumps(e['outcome'])[:80] if e['outcome'] else '—'])
    table(['Layout', 'Fault', 'Others alive', 'Other calls / failed', 'Others worst p95 ms', 'Fault plugin alive', 'Stop', 'Events', 'Forged frame attributed to', 'Fault call outcome'], rows)
    OUT['faults'] = ev

Path(sys.argv[2]).write_text(json.dumps(OUT, indent=1) + '\n')
