#!/usr/bin/env python3
"""Real CLI/HTTP conformance smoke, using disposable data; never prints credentials.

Build cl and calternal first. Default spawns a private server, tests restart, and
removes only its own TemporaryDirectory. --running-data-dir uses an existing
preview startup.json and leaves that server/data intact (no restart).
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--running-data-dir', type=Path)
args = parser.parse_args()


def smoke(data_dir, spawn):
    process = None
    run_id = uuid.uuid4().hex
    log = tempfile.TemporaryFile()

    def start():
        nonlocal process
        process = subprocess.Popen([str(ROOT/'target/debug/calternal'), 'serve', '--development', '--create', '--appdata', str(data_dir/'appdata'), '--userdata-ssd', str(data_dir/'userdata-ssd'), '--userdata-hdd', str(data_dir/'userdata-hdd'), '--port', '0'], stdout=log, stderr=log, cwd=ROOT)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise AssertionError('private preview failed to start (credentials omitted)')
            ready = data_dir/'appdata'/'startup.json'
            if ready.exists():
                info = json.loads(ready.read_text())
                if info['pid'] == process.pid:
                    return info
            time.sleep(0.025)
        raise AssertionError('preview readiness timed out')

    def stop():
        if process is not None and process.poll() is None:
            process.terminate()
            process.wait(timeout=10)

    info = start() if spawn else json.loads((data_dir/'appdata'/'startup.json').read_text())

    def cl(*command, principal='owner', error=None):
        env = dict(os.environ)
        env.pop('CALTERNAL_TOKEN', None)
        env.pop('CALTERNAL_TOKEN_FILE', None)
        result = subprocess.run([str(ROOT/'target/debug/cl'), '--base-url', info['base_url'], '--token-file', str(data_dir/'appdata'/'dev-tokens'/f'{principal}.token'), *command], capture_output=True, text=True, env=env, timeout=20)
        payload = json.loads(result.stderr if error else result.stdout)
        if error:
            assert result.returncode != 0 and payload['error']['code'] == error, 'unexpected CLI error category'
        else:
            assert result.returncode == 0, 'CLI failed (credentials omitted)'
        return payload

    try:
        assert cl('describe')['principal']['id'] == 'owner'
        operation = ['--request-id', f'cli-smoke-create-{run_id}', 'notes', 'create', '--title', 'CLI conformance fixture', '--markdown', 'Initial Markdown\n']
        first = cl(*operation)['data']['note']
        assert cl(*operation)['data']['note'] == first
        note_id = first['id']
        assert cl('notes', 'get', note_id)['data']['note'] == first
        listed = cl('notes', 'list')['data']['notes']
        assert any(note['id'] == note_id for note in listed)
        update = ['--request-id', f'cli-smoke-update-{run_id}', 'notes', 'update', note_id, '--base-revision', first['revision'], '--markdown', 'Updated Markdown\n']
        updated = cl(*update)['data']['note']
        assert updated['revision'] != first['revision']
        assert cl(*update)['data']['note'] == updated
        cl('notes', 'update', note_id, '--base-revision', first['revision'], '--markdown', 'Stale text', error='conflict')
        history = cl('notes', 'history', note_id)['data']['versions']
        assert len(history) == 2 and {v['markdown'] for v in history} == {'Initial Markdown\n', 'Updated Markdown\n'}
        cl('notes', 'get', note_id, principal='guest', error='forbidden')
        cl('notes', 'create', '--space', 'shared', '--title', 'Forbidden', '--markdown', 'read only', principal='guest', error='forbidden')
        cl('plugins', 'enable', 'notes-insights', '--space', 'shared', principal='guest', error='forbidden')
        assert cl('notes', 'list', '--space', 'shared', principal='guest')['ok']
        assert cl('plugins', 'list')['ok']
        if spawn:
            stop()
            info = start()
            assert cl('notes', 'get', note_id)['data']['note'] == updated
            assert len(cl('notes', 'history', note_id)['data']['versions']) == 2
            assert cl(*update)['data']['note'] == updated
        print('CLI real-server smoke passed: describe, create/get/list/update/history, stale conflict, exact retry, principal/role checks' + (', restart persistence' if spawn else ''))
    finally:
        stop() if spawn else None
        log.close()


if args.running_data_dir:
    smoke(args.running_data_dir.resolve(), False)
else:
    with tempfile.TemporaryDirectory(prefix='calternal-cli-conformance-') as directory:
        smoke(Path(directory), True)
