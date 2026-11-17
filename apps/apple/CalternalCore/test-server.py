#!/usr/bin/env python3
"""Run Swift tests with an isolated real server; never print credential contents."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import sys

root = Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix="calternal-swift-") as data:
    process = subprocess.Popen([str(root / "target/debug/calternal"), "serve", "--development", "--port", "0", "--data-dir", data], cwd=root, stdout=subprocess.DEVNULL)
    try:
        startup = Path(data) / "startup.json"
        for _ in range(300):
            if startup.exists():
                break
            if process.poll() is not None:
                raise RuntimeError("disposable server exited during startup")
            time.sleep(0.1)
        info = json.loads(startup.read_text())
        env = dict(os.environ, CALTERNAL_TEST_URL=info["base_url"], CALTERNAL_TEST_TOKEN_FILE=info["token_files"]["owner"])
        command = ["swift", "run", "-j", "4", "CalternalCoreProbe"] if "--probe" in sys.argv else ["swift", "test", "-j", "4"]
        subprocess.run(command, cwd=Path(__file__).parent, env=env, check=True)
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
