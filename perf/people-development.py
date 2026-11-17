#!/usr/bin/env python3
"""Measure warm people RPC latency on a disposable development host only."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from http.client import HTTPConnection
import json
import math
from pathlib import Path
import subprocess
import tempfile
from threading import Barrier
from time import perf_counter_ns, sleep
from urllib.parse import urlsplit
from uuid import uuid4


def call(connection, credential, operation, data):
    body = json.dumps({"request_id": str(uuid4()), "operation": operation, "input": data})
    started = perf_counter_ns()
    connection.request("POST", "/api/v1/operations", body, {
        "Authorization": "Bearer " + credential,
        "Content-Type": "application/json",
    })
    reply = connection.getresponse()
    payload = json.loads(reply.read())
    elapsed = (perf_counter_ns() - started) / 1_000_000
    if reply.status != 200 or not payload["ok"]:
        raise RuntimeError(f"{operation} failed with HTTP {reply.status}")
    return payload["data"], elapsed


def measure(address, token, spaces, target, operation, concurrency, samples):
    ready = Barrier(concurrency, timeout=60)

    def caller(space):
        connection = HTTPConnection(address.hostname, address.port, timeout=60)
        try:
            # Warm the connection, authorization statements and the per-space
            # receipt store before starting the simultaneous sample.
            def request(sequence):
                data = {"space_id": space}
                if operation == "spaces.set_role":
                    data.update(account_id=target, role=("reader", "editor")[sequence % 2])
                return call(connection, token, operation, data)[1]

            request(0)
            ready.wait()
            return [request(n + 1) for n in range(samples)]
        finally:
            connection.close()

    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        elapsed = sorted(item for values in pool.map(caller, spaces[:concurrency]) for item in values)
    return {
        "operation": operation,
        "callers": concurrency,
        "samples": len(elapsed),
        **{f"p{q}_ms": round(elapsed[math.ceil(len(elapsed) * q / 100) - 1], 3) for q in (50, 95, 99)},
    }


def run(binary, samples):
    with tempfile.TemporaryDirectory(prefix="calternal-people-dev-") as temporary:
        root = Path(temporary)
        appdata = root / "appdata"
        with (root / "server.log").open("w+") as log:
            with subprocess.Popen([
                binary, "serve", "--development", "--create", "--port", "0",
                "--installation-id", str(uuid4()), "--appdata", str(appdata),
                "--userdata-ssd", str(root / "ssd"), "--userdata-hdd", str(root / "hdd"),
                "--plugin-root", "plugins",
            ], stdout=log, stderr=log) as server:
                try:
                    for _ in range(300):
                        if server.poll() is not None:
                            raise RuntimeError("disposable server exited during startup")
                        try:
                            startup = json.loads((appdata / "startup.json").read_text())
                            break
                        except (OSError, ValueError):
                            sleep(0.05)
                    else:
                        raise RuntimeError("disposable server did not become ready")
                    address = urlsplit(startup["base_url"])
                    owner = Path(startup["token_files"]["owner"]).read_text().strip()
                    guest = Path(startup["token_files"]["guest"]).read_text().strip()
                    connection = HTTPConnection(address.hostname, address.port, timeout=60)
                    connection.request("GET", "/api/v1/describe", headers={"Authorization": "Bearer " + guest})
                    target = json.loads(connection.getresponse().read())["principal"]["id"]
                    spaces = []
                    for n in range(32):
                        value, _ = call(connection, owner, "spaces.create", {"name": f"Latency fixture {n}"})
                        scope = value["space"]["id"]
                        call(connection, owner, "spaces.add_member", {"space_id": scope, "account_id": target, "role": "reader"})
                        spaces.append(scope)
                    connection.close()
                    return [
                        measure(address, owner, spaces, target, op, callers, samples)
                        for op in ("spaces.members", "spaces.set_role")
                        for callers in (1, 32)
                    ]
                finally:
                    server.terminate()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("server", type=lambda path: str(Path(path).resolve()))
    parser.add_argument("--samples-per-caller", type=int, default=20, choices=range(1, 1001), metavar="1..1000")
    args = parser.parse_args()
    print(json.dumps(run(args.server, args.samples_per_caller), indent=2))
