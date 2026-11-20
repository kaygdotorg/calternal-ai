#!/usr/bin/env python3
"""Tallies syscalls per operation from `strace -f -y -o LOG record-files-spike bench --markers`.

Usage: count.py LOG. Prints, per case and operation, the mean number of each
syscall per sample, split by the file it touched (chat file, ledger.db,
ledger.db-wal, index.db, index.db-wal, other).
"""
import collections, re, sys

mark = re.compile(r'write\(\d+</dev/null>, "@@([BE]) (\S+) (\S+) (\d+)"')
call = re.compile(r'^(?:\d+\s+)?([a-z0-9_]+)\((?:(\d+)<([^>]*)>)?')
WANT = {"read", "pread64", "write", "pwrite64", "fsync", "fdatasync", "sync_file_range",
        "openat", "newfstatat", "statx", "fstat", "rename", "renameat", "renameat2", "ftruncate", "fadvise64"}

def role(path):
    """Which file a descriptor is; A descriptor with no extension (SQLite temporary files, or a folder being flushed) shows as `temp`."""
    if not path:
        return "-"
    base = path.rsplit("/", 1)[-1]
    if base.endswith(".md"):
        return "chat"
    if base.startswith(("ledger.db", "index.db")):
        return base
    if "/proc/" in path or base == "null":
        return None
    return "temp" if "." not in base else "other"

cur = None
tally = collections.defaultdict(collections.Counter)
samples = collections.Counter()
for line in open(sys.argv[1], errors="replace"):
    m = mark.search(line)
    if m:
        kind, case, op, _ = m.groups()
        if kind == "B":
            cur = (case, op)
            samples[cur] += 1
        else:
            cur = None
        continue
    if cur is None or "resumed>" in line:
        continue
    c = call.match(line)
    if not c or c.group(1) not in WANT:
        continue
    r = role(c.group(3))
    if r is None:
        continue
    name = c.group(1)
    if name in ("newfstatat", "statx") and "/proc/" in line:
        continue
    tally[cur][(name, r)] += 1

for key in sorted(tally, key=lambda k: (int(k[0].split("-")[0]), k[0], k[1])):
    n = samples[key]
    t = tally[key]
    syncs = sum(v for (s, _), v in t.items() if s in ("fsync", "fdatasync"))
    reads = sum(v for (s, _), v in t.items() if s in ("read", "pread64"))
    writes = sum(v for (s, _), v in t.items() if s in ("write", "pwrite64"))
    detail = ", ".join(f"{s}[{r}]={v / n:.2f}" for (s, r), v in sorted(t.items()))
    print(f"{key[0]}\t{key[1]}\tn={n}\tflushes={syncs / n:.2f}\treads={reads / n:.2f}\twrites={writes / n:.2f}\t{detail}")
