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
import selectors
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
        process = subprocess.Popen([str(ROOT/'target/debug/calternal'), 'serve', '--development', '--create', '--installation-id', '00000000-0000-0000-0000-000000000001', '--appdata', str(data_dir/'appdata'), '--userdata-ssd', str(data_dir/'userdata-ssd'), '--userdata-hdd', str(data_dir/'userdata-hdd'), '--port', '0'], stdout=log, stderr=log, cwd=ROOT)
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
        result = subprocess.run([str(ROOT/'target/debug/cl'), '--json', '--base-url', info['base_url'], '--token-file', str(data_dir/'appdata'/'dev-tokens'/f'{principal}.token'), *command], capture_output=True, text=True, env=env, timeout=20)
        payload = json.loads(result.stdout)
        if error:
            assert result.returncode != 0 and payload['error']['code'] == error, 'unexpected CLI error category'
        else:
            assert result.returncode == 0, 'CLI failed (credentials omitted)'
        return payload

    def account_cli(*command):
        env = dict(os.environ)
        env.pop('CALTERNAL_TOKEN', None)
        env.pop('CALTERNAL_TOKEN_FILE', None)
        result = subprocess.run([str(ROOT/'target/debug/calternal'), '--json', *command,
            '--base-url', info['base_url'], '--token-file', str(data_dir/'appdata'/'dev-tokens'/'owner.token')],
            capture_output=True, text=True, env=env, timeout=20)
        payload = json.loads(result.stdout)
        assert result.returncode == 0, f'account CLI failed: {payload.get("error", {})}'
        return payload['data']

    def watched_create(space, operation):
        watcher = subprocess.Popen([str(ROOT/'target/debug/calternal'), '--json', 'watch', space,
            '--base-url', info['base_url'], '--token-file', str(data_dir/'appdata'/'dev-tokens'/'owner.token')],
            stdout=subprocess.PIPE, stderr=log, text=True)
        selector = selectors.DefaultSelector()
        selector.register(watcher.stdout, selectors.EVENT_READ)
        def event():
            assert selector.select(timeout=10), 'watch event timed out'
            line = watcher.stdout.readline()
            assert line, 'watch ended before an event'
            payload = json.loads(line)
            assert payload['ok']
            return payload['data']
        try:
            assert event()['type'] == 'ready'
            first = cl(*operation)['data']['note']
            frame = event()
            assert frame['type'] == 'entry' and frame['entry']['item'] == first['id']
            return first
        finally:
            selector.close()
            watcher.terminate()
            watcher.wait(timeout=10)

    try:
        owner = cl('describe')['data']['principal']
        guest = cl('describe', principal='guest')['data']['principal']
        assert str(uuid.UUID(owner['id'])) == owner['id']
        assert owner['id'] != guest['id']
        shared = next(space['id'] for space in guest['spaces'] if space['role'] == 'reader')
        private = next(space['id'] for space in owner['spaces'] if space['id'] != shared)
        operation = ['--request-id', f'cli-smoke-create-{run_id}', 'notes', 'create', '--space', private, '--title', 'CLI conformance fixture', '--markdown', 'Initial Markdown\n']
        first = watched_create(private, operation)
        assert cl(*operation)['data']['note'] == first
        note_id = first['id']
        assert cl('notes', 'get', note_id)['data']['note'] == first
        listed = cl('notes', 'list', '--space', private)['data']['notes']
        assert any(note['id'] == note_id for note in listed)
        update = ['--request-id', f'cli-smoke-update-{run_id}', 'notes', 'update', note_id, '--base-revision', first['revision'], '--markdown', 'Updated Markdown\n']
        updated = cl(*update)['data']['note']
        assert updated['revision'] != first['revision']
        assert cl(*update)['data']['note'] == updated
        cl('notes', 'update', note_id, '--base-revision', first['revision'], '--markdown', 'Stale text', error='conflict')
        history = cl('notes', 'history', note_id)['data']['versions']
        assert len(history) == 2 and {v['markdown'] for v in history} == {'Initial Markdown\n', 'Updated Markdown\n'}
        cl('notes', 'get', note_id, principal='guest', error='not_found')
        cl('notes', 'create', '--space', shared, '--title', 'Forbidden', '--markdown', 'read only', principal='guest', error='forbidden')
        cl('plugins', 'enable', 'calternal-notes-insights', '--space', shared, principal='guest', error='forbidden')
        assert cl('notes', 'list', '--space', shared, principal='guest')['ok']
        assert cl('plugins', 'list', '--space', private)['ok']
        saved = cl('drafts', 'save', note_id, '--space', private, '--text', 'Private unsent text')['data']['record']
        assert account_cli('drafts', 'get', note_id, '--space', private)['record'] == saved
        assert any(r['id'] == note_id for r in account_cli('drafts', 'list')['records'])
        cl('drafts', 'get', note_id, '--space', private, principal='guest', error='not_found')
        pending_id = f'cli-pending-{run_id}'
        queue_file = data_dir/'pending-smoke.json'
        queue_file.write_text(json.dumps({'space_id': private, 'item_id': note_id,
            'created_at_ms': int(time.time()*1000), 'request': {'request_id': pending_id,
                'operation': 'notes.create', 'input': {'space_id': private, 'title': 'Pending CLI', 'markdown': 'Unsent'}}}))
        account_cli('pending', 'queue', str(queue_file))
        edited = json.loads(queue_file.read_text())
        edited['request']['input']['markdown'] = 'Edited pending'
        queue_file.write_text(json.dumps(edited))
        account_cli('pending', 'edit', str(queue_file))
        assert any(r['id'] == pending_id for r in cl('pending', 'list')['data']['records'])
        sent = account_cli('pending', 'submit', '--space', private, pending_id)['result']
        assert sent['note']['markdown'] == 'Edited pending'
        assert account_cli('pending', 'submit', '--space', private, pending_id)['result'] == sent
        assert account_cli('drafts', 'list')['records'] == []
        edited['request']['request_id'] = f'cli-old-{run_id}'
        edited['created_at_ms'] -= 8*24*60*60*1000
        queue_file.write_text(json.dumps(edited))
        account_cli('pending', 'queue', str(queue_file))
        held = account_cli('pending', 'submit', '--space', private, edited['request']['request_id'])
        assert held['record']['state'] == 'confirmation_required'
        account_cli('pending', 'confirm', '--space', private, edited['request']['request_id'])
        edited['request']['request_id'] = f'cli-cancel-{run_id}'
        queue_file.write_text(json.dumps(edited))
        account_cli('pending', 'queue', str(queue_file))
        account_cli('pending', 'cancel', '--space', private, edited['request']['request_id'])
        assert account_cli('pending', 'list')['records'] == []
        account_cli('drafts', 'save', note_id, '--space', private, '--text', 'Retained across restart')
        if spawn:
            stop()
            info = start()
            assert cl('notes', 'get', note_id)['data']['note'] == updated
            assert len(cl('notes', 'history', note_id)['data']['versions']) == 2
            assert cl(*update)['data']['note'] == updated
            assert account_cli('drafts', 'get', note_id, '--space', private)['record']['text'] == 'Retained across restart'
        account_cli('drafts', 'delete', note_id, '--space', private)
        print('CLI real-server smoke passed: describe, create/get/list/update/history, stale conflict, exact retry, principal/role checks' + (', restart persistence' if spawn else ''))
    finally:
        stop() if spawn else None
        log.close()


if args.running_data_dir:
    smoke(args.running_data_dir.resolve(), False)
else:
    with tempfile.TemporaryDirectory(prefix='calternal-cli-conformance-') as directory:
        smoke(Path(directory), True)
