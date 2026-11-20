#!/usr/bin/env python3
"""Markdown tables from the spike's output. Usage: tables.py timings.txt [counts.txt]"""
import collections, sys

def kv(fields):
    return dict(f.split("=", 1) for f in fields if "=" in f)

ops = collections.OrderedDict()
cases, hist, verify = [], [], []
for line in open(sys.argv[1]):
    f = line.rstrip("\n").split("\t")
    if f[0] == "op":
        size, layout, op = f[1], f[2], f[3]
        d = kv(f[4:])
        key = (op, size, layout)
        # The second `append` block (after foreign edits) is reported separately.
        if key in ops and op == "append":
            key = ("append_after_foreign_edit", size, layout)
        ops[key] = d
    elif f[0] == "case":
        cases.append(f)
    elif f[0] == "history":
        hist.append(f)
    elif f[0] == "verify":
        verify.append(f)

def mib(s):
    return int(s.replace("MiB", ""))

def kb(x):
    x = float(x)
    return f"{x / 1024:.0f} KiB" if x < 1 << 20 else f"{x / (1 << 20):.1f} MiB"

names = []
for (op, _, _) in ops:
    if op not in names:
        names.append(op)
print("### Corpus\n")
print("| Size | Layout | Records | Files | File bytes | Ledger bytes | Index bytes | Build s |")
print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |")
for c in cases:
    d = kv(c[3:])
    print(f"| {c[1]} | {c[2]} | {int(d['records']):,} | {d['files']} | {kb(d['file_bytes'])} | {kb(d['ledger_bytes'])} | {kb(d['index_bytes'])} | {d['build_s']} |")
for op in names:
    rows = sorted([k for k in ops if k[0] == op], key=lambda k: (mib(k[1]), k[2]))
    print(f"\n### {op}\n")
    print("| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for k in rows:
        d = ops[k]
        print(f"| {k[1]} | {k[2]} | {d['n']} | {d['p50']} | {d['p95']} | {d['p99']} | {d['max']} | {d['db_ms']} | {d['file_ms']} | {d['scrub_ms']} | {kb(d['wchar'])} | {kb(d['write_bytes'])} | {d['sql_reads']} | {d['fallback'] if k[2] == 'B' else '-'} | {d['changed_rows']} |")
if hist:
    print("\n### History after 1,000 edits\n")
    for h in hist:
        print(f"- {h[1]} {h[2]}: " + "; ".join(h[3:]))
if verify:
    print("\n### Final check (file equals ledger)\n")
    for v in verify:
        print(f"- {v[1]} {v[2]}: {' '.join(v[3:])}")
if len(sys.argv) > 2:
    print("\n### Syscalls per operation (strace, 20 samples each)\n")
    print("| Case | Operation | flushes (fsync+fdatasync) | reads | writes | detail |")
    print("| --- | --- | ---: | ---: | ---: | --- |")
    for line in open(sys.argv[2]):
        f = line.rstrip("\n").split("\t")
        print(f"| {f[0]} | {f[1]} | {f[3].split('=')[1]} | {f[4].split('=')[1]} | {f[5].split('=')[1]} | {f[6] if len(f) > 6 else ''} |")
