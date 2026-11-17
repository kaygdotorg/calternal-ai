#!/usr/bin/env python3
"""Probe the startup scheduling boundary on disposable roots under the perf lock.

Provisioning is untimed. Each real server receives up to ten seconds to answer;
logs and CPU/readiness observations remain, while large credential roots are removed.
This is a diagnostic reproduction, not a replacement for the full fixture matrix.
"""
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time

if os.environ.get("CALTERNAL_PERF_HELD") != "1":
    raise SystemExit("Run under /usr/local/bin/perf-lock.sh on perf-01.")
suite = Path(sys.argv[1]).resolve()
driver = Path(sys.argv[2]).resolve()
commit = sys.argv[3]
work = suite / f"startup-boundary-{os.getpid()}"
work.mkdir()
rows = []
for spaces in (128, 129):
    root = work / f"roots-{spaces}"
    subprocess.run([
        str(driver), "--bin", str(suite / "bin/calternal"),
        "--baseline", str(suite / "bin/calternal"), "--work", str(root),
        "--plugins", str(suite / "plugins"), "--budgets", str(suite / "budgets.json"),
        "--sizes", "1", "--spaces", str(spaces), "--prepare-only", "--json",
        "--commit", commit, "--baseline-commit", commit,
    ], check=True, stdout=subprocess.DEVNULL)
    (root / "appdata/startup.json").unlink(missing_ok=True)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    with (work / f"server-{spaces}.log").open("wb") as log:
        started = time.monotonic()
        child = subprocess.Popen([
            str(suite / "bin/calternal"), "serve", "--port", str(port),
            "--origin", "http://localhost:18080", "--create", "--installation-id",
            "00000000-0000-0000-0000-000000000001", "--appdata", str(root / "appdata"),
            "--userdata-ssd", str(root / "userdata-ssd"), "--userdata-hdd", str(root / "userdata-hdd"),
            "--plugin-root", str(suite / "plugins"),
        ], stdout=log, stderr=log)
        ready = False
        try:
            while time.monotonic() - started < 10 and child.poll() is None:
                conn = http.client.HTTPConnection("127.0.0.1", port, timeout=0.2)
                try:
                    conn.request("GET", "/api/v1/auth/status", headers={"Host": "localhost:18080"})
                    response = conn.getresponse()
                    response.read()
                    if response.status == 200:
                        ready = True
                        break
                except OSError:
                    pass
                finally:
                    conn.close()
                time.sleep(0.01)
            elapsed = time.monotonic() - started
            stat = Path(f"/proc/{child.pid}/stat").read_text().rsplit(")", 1)[1].split()
            cpu = (int(stat[11]) + int(stat[12])) / os.sysconf("SC_CLK_TCK")
            rows.append({"spaces": spaces, "notes_per_space": 1, "ready": ready,
                         "elapsed_seconds": elapsed, "cpu_seconds": cpu})
        finally:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
    shutil.rmtree(root)
report = {"commit": commit, "diagnostic": "startup cooperative-budget boundary", "observations": rows}
(work / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report))
