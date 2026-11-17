#!/usr/bin/env python3
"""Select cases needing a repeat from the Rust runner's recorded noise and breaches.

Print only validated numeric space/note counts and a cold flag for the shell gate.
A noisy metric needs at least two breached head arms before it enters the plan.
"""
import json
import sys

report = json.load(open(sys.argv[1]))
plan = {}
for key, noise in report["noise_pct"].items():
    fixture, arm, cache, metric = key.split("/", 3)
    if arm != "head" or noise <= 25:
        continue
    spaces, notes = map(int, fixture.split("x"))
    if not (1 <= spaces <= 1000 and 1 <= notes <= 100000):
        raise ValueError("invalid fixture counts in report")
    cache_name = "disk-cold" if cache == "cold" else "warm"
    breaches = sum(
        run["arm"] == "head" and run["spaces"] == spaces and run["size"] == notes
        and run["cache"] == cache_name
        and any(row["metric"] == metric and row["breached"] for row in run["summary"])
        for run in report["runs"]
    )
    if breaches >= 2:
        plan[(spaces, notes)] = plan.get((spaces, notes), False) or cache == "cold"
for (spaces, notes), cold in sorted(plan.items()):
    print(spaces, notes, int(cold))
