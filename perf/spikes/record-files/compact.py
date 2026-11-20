#!/usr/bin/env python3
"""Compact Markdown tables: per operation, p50 / p95 / p99 in ms by size and layout. Usage: compact.py timings.txt"""
import collections, sys
ops = collections.OrderedDict()
seen = set()
for line in open(sys.argv[1]):
    f = line.rstrip("\n").split("\t")
    if f[0] != "op":
        continue
    size, layout, op = f[1], f[2], f[3]
    if (op, size, layout) in seen and op == "append":
        op = "append_after_foreign_edit"
    seen.add((op, size, layout))
    d = dict(x.split("=", 1) for x in f[4:] if "=" in x)
    ops.setdefault(op, {})[(size, layout)] = d
def fmt(ms):
    ms = float(ms)
    return f"{ms:.2f}" if ms < 10 else (f"{ms:.1f}" if ms < 100 else f"{ms:,.0f}")
layouts = ["A", "A2", "B", "C"]
for op, cells in ops.items():
    present = [l for l in layouts if any(k[1] == l for k in cells)]
    n = next(iter(cells.values()))["n"]
    print(f"\n**{op}** (ms: p50 / p95 / p99; {n} samples per cell)\n")
    print("| Size | " + " | ".join(present) + " |")
    print("| --- | " + " | ".join("---:" for _ in present) + " |")
    for size in ["1MiB", "16MiB", "64MiB", "256MiB"]:
        row = []
        for l in present:
            d = cells.get((size, l))
            if not d:
                row.append("—")
                continue
            s = f"{fmt(d['p50'])} / {fmt(d['p95'])} / {fmt(d['p99'])}"
            if l == "B" and op.startswith(("edit", "redact")):
                s += f" ({d['fallback']} fallback)"
            row.append(s)
        print(f"| {size.replace('MiB', ' MiB')} | " + " | ".join(row) + " |")
    if op.startswith(("read_", "unread")):
        continue
    print()
    print("Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:\n")
    print("| Size | " + " | ".join(present) + " |")
    print("| --- | " + " | ".join("---:" for _ in present) + " |")
    for size in ["1MiB", "16MiB", "64MiB", "256MiB"]:
        row = []
        for l in present:
            d = cells.get((size, l))
            if not d:
                row.append("—")
                continue
            w = float(d["wchar"])
            ws = f"{w / 1024:.0f} KiB" if w < 1 << 20 else f"{w / (1 << 20):.1f} MiB"
            sc = f" + {fmt(d['scrub_ms'])}" if float(d["scrub_ms"]) else ""
            row.append(f"{fmt(d['db_ms'])} + {fmt(d['file_ms'])}{sc}; {ws}")
        print(f"| {size.replace('MiB', ' MiB')} | " + " | ".join(row) + " |")
