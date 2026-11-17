#!/usr/bin/env python3
"""Summarize the dated control's qualified arms; never export raw vectors."""
import json
import math
import pathlib
import statistics
import sys

root = pathlib.Path(sys.argv[1])

def load(name):
    return json.loads((root / name).read_text())

def quantiles(values):
    values = sorted(values)
    return {name: values[max(0, math.ceil(len(values) * p) - 1)]
            for name, p in [('p50', .5), ('p95', .95), ('p99', .99), ('max', 1)]}

warm = load('verified-warm/summary.json')
runs = list(warm['runs'])
# Initial suite has no final summary after the old runner stopped at 1000 spaces.
for path in sorted((root / 'full').glob('*-*-*/raw.json')):
    fixture, pair, arm = path.parent.name.split('-')
    if int(pair) >= 3:
        continue  # Warm copies were not synced: exclude writeback contamination.
    spaces, notes = map(int, fixture.split('x'))
    summaries = []
    for metric, values in json.loads(path.read_text()).items():
        summaries.append(dict(metric=metric, samples=len(values), **quantiles(values)))
    runs.append(dict(arm=arm, cache='cold', pair=int(pair), spaces=spaces,
                     size=notes, summary=summaries))

groups = []
for spaces, notes, cache, arm in sorted({(r['spaces'], r['size'], r['cache'], r['arm']) for r in runs}):
    chosen = [r for r in runs if (r['spaces'], r['size'], r['cache'], r['arm']) == (spaces, notes, cache, arm)]
    metrics = {}
    for name in sorted({s['metric'] for r in chosen for s in r['summary']}):
        ss = [s for r in chosen for s in r['summary'] if s['metric'] == name]
        med = statistics.median(s['p50'] for s in ss)
        if all(s['samples'] == 1 for s in ss):
            aggregate = quantiles([s['p50'] for s in ss])
            method = 'nearest rank across singleton arms'
        else:
            aggregate = {q: statistics.median(s[q] for s in ss) for q in ['p50', 'p95', 'p99']}
            aggregate['max'] = max(s['max'] for s in ss)
            method = 'median of per-arm quantiles; maximum of arm maxima'
        metrics[name] = dict(**aggregate, aggregation=method,
            max_arm_p95=max(s['p95'] for s in ss), samples=sum(s['samples'] for s in ss),
            p50_noise_pct=(max(s['p50'] for s in ss)-min(s['p50'] for s in ss))/med*100 if med else 0)
    groups.append(dict(spaces=spaces, notes_per_space=notes, cache=cache, arm=arm,
                       arms=len(chosen), metrics=metrics))
idle = load('idle-measurable/idle-summary.json')
memory = []
for spaces in idle['space_cases']:
    obs = [o for o in idle['observations'] if o['spaces'] == spaces]
    memory.append(dict(spaces=spaces, replicas=len(obs), unit='MiB',
        **quantiles([o['after_mib'] for o in obs]), all_writers_retired=all(o['writers_after'] == 0 for o in obs),
        minimum_idle_seconds=min(o['idle_seconds'] for o in obs), budget=64))
jobs = [load(f'jobs-fixed-{i}/jobs-summary.json') for i in range(3)]
repeat = load('repeat-100spaces/summary.json')
output = dict(schema_version=1, date='2026-11-17', qualification='blocked: reproduced breaches and missing mandatory instrumentation',
    product_commit=warm['commit'], reference_commit='ea57b84b9c9e40e8b67b7d1e1f1805915123b040',
    reference_budgets_blob='aaf25a669ca2b362ed9d0cb1921bc702cfb8cd12',
    control='identical baseline/head binaries; not an old-app comparison or regression estimate',
    binary_sha256=warm['binary_sha256'], machine=warm['machine'], seed=warm['seed'],
    operation_concurrency=1, subscriptions=2, valid_arms=len(runs),
    sources=['full: cold pairs 0..2 only', 'verified-warm: all five warm pairs'],
    exclusions=['original full warm arms and unsynced repeat warm arms: copy writeback',
        'smoke: preliminary/mislabelled metadata', 'instrumented timings', 'failed or cancelled idle attempts',
        'jobs before per-client request nonce fix'],
    groups=groups, idle_released_memory=memory,
    jobs=dict(provider='none; existing jobs-demo fixture', replicas=3,
        cold_first=quantiles([j['cold_first_ms'] for j in jobs]),
        warm_median_quantiles={q: statistics.median(j['summary'][q] for j in jobs) for q in ['p50','p95','p99']},
        warm_max=max(j['summary']['max'] for j in jobs), warm_samples=297),
    startup_boundary=load('startup-boundary-3111966/summary.json'),
    startup_1000=dict(ready=False, observation_windows_seconds=[30,300], idle_memory='blocked; no value'),
    noise_rule='range of arm p50 / median >25% triggers bounded repeat; two breached head arms fail regardless',
    cold_100spaces_repeat_noise={k:v for k,v in repeat['noise_pct'].items() if '/cold/' in k and 'startup.' in k},
    profiles=dict(startup='startup-profile-20261117T165246Z', startup_wake_park_cpu_pct=88.36,
        writes='profile-20261117T175337Z', selected_fsync_calls=1207,
        selected_fsync_wall_seconds=7.027989, selected_fsync_wall_fraction_pct=95.22,
        qualification='syscall fraction is selected syscall wall time, not total CPU or wall time'),
    raw_location=str(root), raw_retention_days=30,
    limitations=['not genuine cold-space isolation: eager startup may populate caches',
      'no admission-only or receipt-only timing', 'no exact post-commit-to-client marker',
      'no three-host-call fixture', '1000-space startup and released memory blocked',
      'logical roots share SSD; no attachment or tier workload', 'allocator tuning differs from reference'])
json.dump(output, sys.stdout, indent=2)
print()
