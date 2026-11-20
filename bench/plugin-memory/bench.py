#!/usr/bin/env python3
"""Bench-only driver for the plugin split memory lane. Runs on a perf VM under
/usr/local/bin/perf-lock.sh. Boots the bench guest in Firecracker, sends
commands to guest/host.mjs over vsock and appends one JSON line per event to
results/<phase>.jsonl.

Usage: bench.py <phase>   (layouts | wake | lazy | timeline | faults)
Environment: FIRECRACKER, KERNEL, IMAGE (absolute paths).
"""
import contextlib
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess as sp
import sys
import time

D = Path(__file__).resolve().parent
WORK = D / 'work'
RESULTS = D / 'results'
FC, KERNEL, IMAGE = os.environ['FIRECRACKER'], os.environ['KERNEL'], os.environ['IMAGE']
GUEST_MIB = 512
SEVEN = ['threads', 'notes', 'guide', 'threads-claude-code', 'submission', 'settings', 'trivial']
DEADLINE = 30
PHASE = sys.argv[1]
WORK.mkdir(exist_ok=True)
RESULTS.mkdir(exist_ok=True)
EVENTS = open(RESULTS / f'{PHASE}.jsonl', 'a', buffering=1)


def event(kind, **fields):
    EVENTS.write(json.dumps({'kind': kind, 'at': time.time(), **fields}) + '\n')


def ms(t):
    return (time.perf_counter() - t) * 1e3


def evict(*files):
    """Drop host page cache for the given files (cold rows)."""
    for f in files:
        fd = os.open(f, os.O_RDONLY)
        try:
            os.fsync(fd)
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        finally:
            os.close(fd)


def pss_mib(pid):
    for line in Path(f'/proc/{pid}/smaps_rollup').read_text().splitlines():
        if line.startswith('Pss:'):
            return int(line.split()[1]) / 1024
    return None


class UnixHTTP(http.client.HTTPConnection):
    def __init__(self, path):
        super().__init__('localhost', timeout=60)
        self.path = path

    def connect(self):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(self.timeout)
        self.sock.connect(str(self.path))


class Channel:
    def __init__(self, sock):
        self.sock, self.buf = sock, b''

    def call(self, op, timeout=600, **kw):
        self.sock.settimeout(timeout)
        self.sock.sendall((json.dumps({'op': op, **kw}) + '\n').encode())
        while b'\n' not in self.buf:
            chunk = self.sock.recv(1 << 20)
            if not chunk:
                raise EOFError('guest closed')
            self.buf += chunk
        line, self.buf = self.buf.split(b'\n', 1)
        reply = json.loads(line)
        if 'error' in reply:
            raise RuntimeError(reply['error'])
        return reply['result']


class VM:
    count = 0

    def __init__(self):
        VM.count += 1
        self.dir = WORK / f'vm-{os.getpid()}-{VM.count}'
        self.dir.mkdir()
        self.api_path, self.vsock = self.dir / 'api', self.dir / 'vsock'
        self.log = open(self.dir / 'log', 'wb')
        self.proc = self.sock = None

    def __enter__(self):
        return self

    def __exit__(self, typ, value, tb):
        if self.sock:
            self.sock.close()
        if self.proc:
            if self.proc.poll() is None:
                self.proc.kill()
            self.proc.wait()
        self.log.close()
        if typ:
            event('vm_error', message=str(value)[:4000], log=(self.dir / 'log').read_text(errors='replace')[-4000:])
        shutil.rmtree(self.dir)

    def launch(self):
        self.proc = sp.Popen([FC, '--api-sock', str(self.api_path)], stdin=sp.DEVNULL, stdout=self.log, stderr=self.log)
        limit = time.monotonic() + DEADLINE
        while not self.api_path.exists():
            if self.proc.poll() is not None or time.monotonic() > limit:
                raise RuntimeError('Firecracker API unavailable')
            time.sleep(.001)

    def api(self, path, body=None, method='PUT'):
        with contextlib.closing(UnixHTTP(self.api_path)) as h:
            h.request(method, path, None if body is None else json.dumps(body), {'Content-Type': 'application/json'})
            r = h.getresponse()
            data = r.read()
            if r.status >= 300:
                raise RuntimeError((path, r.status, data.decode()))
            return json.loads(data) if data else None

    def attach(self):
        stop = time.monotonic() + DEADLINE
        while time.monotonic() < stop:
            s = socket.socket(socket.AF_UNIX)
            s.settimeout(2)
            try:
                s.connect(str(self.vsock))
                s.sendall(b'CONNECT 5000\n')
                line = b''
                while not line.endswith(b'\n'):
                    c = s.recv(1)
                    if not c:
                        raise EOFError
                    line += c
                if not line.startswith(b'OK '):
                    raise ConnectionError('vsock handshake')
                self.sock = s
                return Channel(s)
            except (OSError, EOFError):
                s.close()
                if self.proc.poll() is not None:
                    break
                time.sleep(.004)
        raise TimeoutError('vsock handshake')

    def boot(self):
        """Boot fresh and wait until the guest host answers; returns ms."""
        t = time.perf_counter()
        self.launch()
        self.api('/machine-config', dict(vcpu_count=2, mem_size_mib=GUEST_MIB, track_dirty_pages=False))
        self.api('/boot-source', dict(kernel_image_path=KERNEL,
                                      boot_args='console=ttyS0 reboot=k panic=1 root=/dev/vda ro init=/init quiet'))
        self.api('/drives/root', dict(drive_id='root', path_on_host=IMAGE, is_root_device=True, is_read_only=True))
        self.api('/vsock', dict(guest_cid=3, uds_path=str(self.vsock)))
        self.api('/actions', dict(action_type='InstanceStart'))
        stop = time.monotonic() + DEADLINE
        while time.monotonic() < stop:
            try:
                ch = self.attach()
                ch.call('events', timeout=5)
                return ch, ms(t)
            except (OSError, EOFError, ValueError, TimeoutError):
                if self.sock:
                    self.sock.close()
                self.sock = None
                time.sleep(.02)
        raise TimeoutError('guest host not ready')

    def snapshot(self, prefix):
        self.sock.close()
        self.sock = None
        self.api('/vm', dict(state='Paused'), 'PATCH')
        state, mem = prefix.with_suffix('.state'), prefix.with_suffix('.mem')
        self.api('/snapshot/create', dict(snapshot_type='Full', snapshot_path=str(state), mem_file_path=str(mem)))
        for f in (state, mem):
            fd = os.open(f, os.O_RDONLY)
            os.fsync(fd)
            os.close(fd)
        return state, mem

    def restore(self, prefix):
        self.launch()
        self.api('/snapshot/load', dict(snapshot_path=str(prefix.with_suffix('.state')),
                                        mem_backend=dict(backend_path=str(prefix.with_suffix('.mem')), backend_type='File'),
                                        resume_vm=True, vsock_override=dict(uds_path=str(self.vsock))))
        return self.attach()


def layouts(repeats=5):
    """Baseline (process) and approach 3 (shared): cold start, memory, latency."""
    for mode in ('process', 'shared'):
        for n in (1, 2, 4, 7):
            for r in range(repeats):
                with VM() as vm:
                    ch, boot_ms = vm.boot()
                    t = time.perf_counter()
                    ready = ch.call('layout', mode=mode, plugins=SEVEN[:n])
                    event('cold_start', mode=mode, n=n, repeat=r, boot_ms=boot_ms, layout_ms=ms(t), ready_ms=ready['ready_ms'])
                    time.sleep(2)
                    event('mem', mode=mode, n=n, repeat=r, stage='ready', vmm_pss_mib=pss_mib(vm.proc.pid), **ch.call('mem'))
                    if r == 0:
                        for plugin in SEVEN[:n]:
                            res = ch.call('bench', plugin=plugin, n=1000, warmup=50)
                            event('latency', mode=mode, n=n, plugin=plugin, samples_us=res['samples'])
                    else:
                        for plugin in SEVEN[:n]:
                            ch.call('bench', plugin=plugin, n=100, warmup=0)
                    time.sleep(2)
                    event('mem', mode=mode, n=n, repeat=r, stage='after_calls', vmm_pss_mib=pss_mib(vm.proc.pid), **ch.call('mem'))


def zero(repeats=5):
    """Baseline with the guest host but no plugin process, for marginal costs."""
    for r in range(repeats):
        with VM() as vm:
            ch, boot_ms = vm.boot()
            ch.call('layout', mode='process', plugins=[])
            time.sleep(2)
            event('mem', mode='none', n=0, repeat=r, stage='after_calls', vmm_pss_mib=pss_mib(vm.proc.pid), **ch.call('mem'))


def wake(trials=20):
    """Wake from a full snapshot against the number of plugin processes."""
    snap = WORK / 'snap'
    snap.mkdir(exist_ok=True)
    for mode in ('process', 'shared'):
        for n in (1, 2, 4, 7):
            prefix = snap / f'{mode}-{n}'
            with VM() as vm:
                ch, _ = vm.boot()
                ch.call('layout', mode=mode, plugins=SEVEN[:n])
                for plugin in SEVEN[:n]:
                    ch.call('bench', plugin=plugin, n=100, warmup=0)
                time.sleep(1)
                event('snapshot_mem', mode=mode, n=n, vmm_pss_mib=pss_mib(vm.proc.pid), **ch.call('mem'))
                state, mem = vm.snapshot(prefix)
            for cache in ('warm', 'cold'):
                for i in range(trials):
                    if cache == 'cold':
                        evict(state, mem, IMAGE)
                    else:
                        sp.run(['cat', str(state), str(mem), IMAGE], stdout=sp.DEVNULL, check=True)
                    with VM() as vm:
                        t = time.perf_counter()
                        ch = vm.restore(prefix)
                        res = ch.call('call', plugin='threads')
                        wake_ms = ms(t)
                        if not res['ok']:
                            raise RuntimeError('wake call failed')
                        event('wake', mode=mode, n=n, cache=cache, trial=i, wake_ms=wake_ms, vmm_pss_mib=pss_mib(vm.proc.pid))
            for f in (state, mem):
                f.unlink()


def lazy(samples=40):
    """Approach 2: first-call delay after a stop, warm and cold caches."""
    variants = [('process', False), ('shared', False), ('process', True)]
    with VM() as vm:
        ch, _ = vm.boot()
        for runtime, cc in variants:
            ch.call('layout', mode='lazy', runtime=runtime, plugins=SEVEN, idle_ms=0, compile_cache=cc)
            for plugin in SEVEN:
                ch.call('cold_call', plugin=plugin, following=0)  # populate caches (and compile cache)
                for cache in ('warm', 'cold'):
                    for i in range(samples):
                        if cache == 'cold':
                            evict(IMAGE)
                        res = ch.call('cold_call', plugin=plugin, drop_caches=cache == 'cold', following=20 if i == 0 else 0)
                        if not res['ok']:
                            raise RuntimeError(f'{plugin} first call failed')
                        event('first_call', runtime=runtime, compile_cache=cc, plugin=plugin, cache=cache, trial=i,
                              first_ms=res['first_ms'], following_us=res['following_us'])


def timeline():
    """Approach 2: memory over time with idle stop versus always-on."""
    for mode, runtime, idle in (('process', 'process', 0), ('lazy', 'process', 15000), ('lazy', 'shared', 15000), ('shared', 'shared', 0)):
        with VM() as vm:
            ch, _ = vm.boot()
            ch.call('layout', mode=mode, runtime=runtime, plugins=SEVEN, idle_ms=idle)
            res = ch.call('timeline', hot='threads', period_ms=2000, duration_ms=60000, sample_ms=500)
            event('timeline', mode=mode, runtime=runtime, idle_ms=idle, **res)


def faults():
    """Approach 3 isolation: what one misbehaving plugin does to the others."""
    plugins = ['threads', 'notes', 'trivial', 'faults']
    for mode in ('process', 'shared'):
        for kind in ('throw', 'exit', 'loop', 'heap', 'buffer', 'abort', 'kill', 'forge', 'peek'):
            with VM() as vm:
                ch, _ = vm.boot()
                ch.call('layout', mode=mode, plugins=plugins)
                res = ch.call('fault', kind=kind, target=0, watch_ms=8000 if kind == 'buffer' else 3000)
                event('fault', mode=mode, fault_kind=res.pop('kind'), **res)


{'zero': zero, 'layouts': layouts, 'wake': wake, 'lazy': lazy, 'timeline': timeline, 'faults': faults}[PHASE]()
