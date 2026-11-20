import json, sys
for l in open(sys.argv[1]):
    try: d = json.loads(l)
    except Exception: print('RAW', l[:300]); continue
    if 'error' in d: print('ERR', d['error'][:600]); continue
    r = d['result']
    if 'samples' in r: r = {'p50': sorted(r['samples'])[len(r['samples']) // 2]}
    if 'procs' in r: r = {'pss': r['plugin_pss_mib'], 'procs': [(p['name'], round(p['pss'], 1)) for p in r['procs']], 'heaps': bool(r.get('heaps'))}
    if 'others' in r: r = {'kind': r['kind'], 'outcome': r['outcome'], 'stopped': r['stopped'], 'faults_alive': r['faults_alive'], 'others': {k: (v['alive'], v['calls'], v['failed']) for k, v in r['others'].items()}, 'ev': r['events'][:3], 'forged': r['forged']}
    print(json.dumps(r)[:500])
