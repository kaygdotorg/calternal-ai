#!/usr/bin/env python3
"""Export the native amd64, capability-empty worker policy via libseccomp."""
import ctypes as c
import json
from pathlib import Path
import sys


class Compare(c.Structure):
    _fields_ = [('arg', c.c_uint), ('op', c.c_uint),
                ('datum_a', c.c_uint64), ('datum_b', c.c_uint64)]


def matches(constraints):
    assert set(constraints) <= {'caps', 'arches', 'minKernel'}
    assert constraints.get('minKernel', '4.8') == '4.8'
    return not constraints.get('caps') and 'amd64' in constraints.get('arches', ['amd64'])


policy = json.loads(Path(sys.argv[1]).read_text())
assert policy['defaultAction'] == 'SCMP_ACT_ERRNO' and policy['defaultErrnoRet'] == 1
lib = c.CDLL('libseccomp.so.2')
lib.seccomp_init.argtypes = [c.c_uint32]
lib.seccomp_init.restype = c.c_void_p
lib.seccomp_release.argtypes = [c.c_void_p]
lib.seccomp_syscall_resolve_name.argtypes = [c.c_char_p]
lib.seccomp_rule_add_array.argtypes = [c.c_void_p, c.c_uint32, c.c_int, c.c_uint, c.POINTER(Compare)]
lib.seccomp_export_bpf.argtypes = [c.c_void_p, c.c_int]
assert (ctx := lib.seccomp_init(0x50001))
try:
    for rule in policy['syscalls']:
        includes, excludes = rule.get('includes', {}), rule.get('excludes', {})
        if not matches(includes):
            continue
        if 'amd64' in excludes.get('arches', []):
            continue
        assert set(excludes) <= {'caps', 'arches'}, excludes
        action = rule['action']
        assert action in {'SCMP_ACT_ALLOW', 'SCMP_ACT_ERRNO'}
        value = 0x7fff0000 if action == 'SCMP_ACT_ALLOW' else 0x50000 | rule['errnoRet']
        fields = rule.get('args', [])
        comparisons = (Compare * len(fields))(*[
            Compare(a['index'], {'SCMP_CMP_EQ': 4, 'SCMP_CMP_MASKED_EQ': 7}[a['op']],
                    a['value'], a.get('valueTwo', 0)) for a in fields
        ])
        for name in rule['names']:
            number = lib.seccomp_syscall_resolve_name(name.encode())
            assert number != -1, name
            rc = lib.seccomp_rule_add_array(ctx, value, number, len(fields), comparisons)
            assert rc == 0, (name, rc)
    with Path(sys.argv[2]).open('wb') as out:
        assert lib.seccomp_export_bpf(ctx, out.fileno()) == 0
finally:
    lib.seccomp_release(ctx)
