#!/usr/bin/env python3
"""Compare reload topologies with the real plugin worker and Podman profile.

Run under the performance lock with a prebuilt worker.js, manifest.json and
worker-seccomp.json in the current directory. The image must already be cached.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import select
import subprocess
import time

PODMAN = [os.environ.get("PLUGIN_BENCH_PODMAN", "podman")]
if os.environ.get("PLUGIN_BENCH_STORAGE"):
    root = Path(os.environ["PLUGIN_BENCH_STORAGE"])
    PODMAN += ["--root", str(root / "storage"), "--runroot", str(root / "runtime")]
IMAGE = os.environ.get("PLUGIN_BENCH_IMAGE", "docker.io/library/node:24-alpine")
NODE = os.environ.get("PLUGIN_BENCH_NODE", "/usr/local/bin/node")
SAMPLES = int(os.environ.get("PLUGIN_BENCH_SAMPLES", "30"))
ROOT = Path.cwd()
SOURCE = (ROOT / "worker.js").read_bytes()
UPDATED = SOURCE.replace(b'Metric("Words"', b'Metric("Updated words"', 1)
if UPDATED == SOURCE:
    raise RuntimeError("could not mark the second fixture revision")
AUTHORITY = {
    "account_id": "01900000-0000-7000-8000-000000000001",
    "space_id": "01900000-0000-7000-8000-000000000002",
    "grant": "fixture",
}


def podman(*args):
    return subprocess.check_output([*PODMAN, *args], text=True, timeout=20).strip()


def create(name, image_path):
    flags = [
        "run", "-d", "--pull=never", "--name", name, "--init", "--network=none",
        "--read-only", "--read-only-tmpfs=false", "--cap-drop=all",
        "--security-opt=no-new-privileges",
        f"--security-opt=seccomp={ROOT / 'worker-seccomp.json'}",
        "--log-driver=none", "--memory", "268435456", "--memory-swap", "268435456",
        "--pids-limit", "64", "--cpus", "1.000", "--volume", f"{image_path}:/plugin:ro",
        "--workdir", "/plugin", IMAGE, "sleep", "2147483647",
    ]
    return subprocess.Popen([*PODMAN, *flags], stdout=subprocess.PIPE, text=True)


def ready(container):
    container_id = container.communicate(timeout=15)[0].strip()
    if container.returncode != 0 or len(container_id) != 64:
        raise RuntimeError(f"container start failed: {container.returncode}")
    return container_id


def verify(path, checksum):
    manifest = json.loads((ROOT / "manifest.json").read_bytes())
    if manifest["operations"][0]["id"] != "notes-insights.summarize":
        raise RuntimeError("wrong manifest")
    if hashlib.sha256(path.read_bytes()).hexdigest() != checksum:
        raise RuntimeError("worker checksum changed")


def start(container, code):
    return subprocess.Popen([
        *PODMAN, "exec", "-i", "--workdir", "/plugin", container,
        "/usr/bin/env", "-i", "LANG=C.UTF-8", NODE, "--permission",
        "--allow-fs-read=.", "--disallow-code-generation-from-strings",
        "--max-old-space-size=128", code,
    ], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)


def invoke(process, updated):
    def send(message):
        process.stdin.write(json.dumps({
            "protocol_version": 2, "invocation_id": "fixture-1", "message": message,
        }) + "\n")
        process.stdin.flush()

    def receive():
        if not select.select([process.stdout], [], [], 10)[0]:
            raise TimeoutError("worker did not answer")
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("worker exited without a result")
        frame = json.loads(line)
        if frame["invocation_id"] != "fixture-1":
            raise RuntimeError("wrong invocation")
        return frame["message"]

    send({"type": "invoke", "operation": "notes-insights.summarize",
          "input": {"id": "01900000-0000-7000-8000-000000000003"}, "authority": AUTHORITY})
    call = receive()
    if call["type"] != "host_call" or call["method"] != "read_item":
        raise RuntimeError("worker used an unexpected broker call")
    send({"type": "host_reply", "call_id": call["call_id"], "ok": True,
          "data": {"text": "Disposable Markdown\n"}})
    result = receive()
    if result["type"] != "complete" or result["data"]["stats"]["words"] != 2:
        raise RuntimeError("invalid worker result")
    source = result["data"]["view"]["source"]
    if ('Metric("Updated words"' in source) != updated:
        raise RuntimeError("wrong worker revision served the request")


def stop(process):
    try:
        process.stdin.close()
        if process.wait(timeout=5) != 0:
            raise RuntimeError("worker did not stop cleanly")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def single(container, code, updated):
    proc = start(container, code)
    try:
        invoke(proc, updated)
        return time.perf_counter_ns()
    finally:
        stop(proc)


def percentile(data, q):
    return sorted(data)[math.ceil(len(data) * q) - 1]


def summary(data):
    return {
        "samples": len(data), "min_ms": min(data),
        **{f"p{int(q * 100)}_ms": percentile(data, q) for q in (0.5, 0.95, 0.99)},
        "max_ms": max(data),
    }


def elapsed(start_time, end_time):
    return (end_time - start_time) / 1_000_000


def remove(name):
    subprocess.run([*PODMAN, "rm", "-f", name], stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL, timeout=15, check=False)


def main():
    for directory, files in [
        ("fresh", {"worker.js": UPDATED}),
        ("reused", {"worker.js": SOURCE, "worker-next.js": UPDATED}),
    ]:
        (ROOT / directory).mkdir(mode=0o700)
        for name, source in files.items():
            (ROOT / directory / name).write_bytes(source)

    vals = {key: [] for key in ["fresh_container", "fresh_process_reused_sandbox",
                                "standby_started_at_validation", "standby_ready_to_result",
                                "verification", "container_ready"]}
    checksum = hashlib.sha256(UPDATED).hexdigest()
    shared_name = f"calternal-reload-{os.getpid()}-reused"
    shared = ready(create(shared_name, ROOT / "reused"))
    try:
        for i in range(SAMPLES + 2):
            old = start(shared, "worker.js")
            invoke(old, False)
            begun = time.perf_counter_ns()
            verify(ROOT / "reused/worker-next.js", checksum)
            stop(old)
            same = elapsed(begun, single(shared, "worker-next.js", True))
            iteration = {"fresh_process_reused_sandbox": same}
            for standby in (False, True):
                name = f"calternal-reload-{os.getpid()}-{i}-{int(standby)}"
                try:
                    begun = time.perf_counter_ns()
                    if standby:
                        launch = create(name, ROOT / "fresh")
                    before_verify = time.perf_counter_ns()
                    verify(ROOT / "fresh/worker.js", checksum)
                    verified = time.perf_counter_ns()
                    if not standby:
                        launch = create(name, ROOT / "fresh")
                    container = ready(launch)
                    at_ready = time.perf_counter_ns()
                    result = single(container, "worker.js", True)
                    if standby:
                        iteration["standby_started_at_validation"] = elapsed(begun, result)
                        iteration["standby_ready_to_result"] = elapsed(at_ready, result)
                    else:
                        iteration["fresh_container"] = elapsed(begun, result)
                        iteration["verification"] = elapsed(before_verify, verified)
                        iteration["container_ready"] = elapsed(verified, at_ready)
                finally:
                    remove(name)
            if i >= 2:
                for key, value in iteration.items():
                    vals[key].append(value)
            if i % 5 == 1:
                print(f"{i + 1} iterations completed", file=os.sys.stderr, flush=True)
    finally:
        remove(shared_name)
    print(json.dumps({
        "kernel": os.uname().release,
        "podman": podman("version", "--format", "{{.Client.Version}}"),
        "image": podman("image", "inspect", "--format", "{{.Id}}", IMAGE),
        "node": podman("run", "--rm", "--pull=never", "--network=none", IMAGE, NODE, "--version"),
        "bundle_sha256": hashlib.sha256(SOURCE).hexdigest(),
        "samples_ms": vals,
        "summary": {key: summary(values) for key, values in vals.items()},
    }, indent=2))


if __name__ == "__main__":
    main()
