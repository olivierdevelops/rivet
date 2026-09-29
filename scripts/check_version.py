#!/usr/bin/env python3
"""Version sync check (PLAN-2026-0001 TASK-074 / T-33, DOCUMENTATION §32).

Checks that the one canonical version in Cargo.toml is what every surface reports:

    Cargo.toml [workspace.package].version   (PLAN-2026-0002 TASK-034)
      ├── rivet-runtime and ffi/ (rivet-ffi) inherit it: version.workspace = true
      ├── rivet --version                    "rivet X.Y.Z"  (binary built with --features cli)
      ├── MCP initialize serverInfo.version  (serve --stdio)
      ├── rivet.capabilities result.version
      ├── librivet rivet_version() and rivet_abi_version() == rivet.capabilities abi_version
      │     (--ffi PATH, else target/release/librivet.{dylib,so} / rivet.dll when present)
      └── with --tag: HEAD is exactly tag vX.Y.Z and the tree is clean

Usage: python3 scripts/check_version.py [--bin target/release/rivet] [--ffi LIB] [--tag]
Exit 0 when everything agrees, 1 otherwise.
"""
import ctypes
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def section(text, name):
    """The body of one TOML table ([name]) up to the next table header."""
    if f'[{name}]' not in text:
        return None
    return text.split(f'[{name}]', 1)[1].split('\n[', 1)[0]


def cargo_version():
    """[workspace.package].version (0.2.0 layout), else [package].version (0.1.0)."""
    text = open(os.path.join(ROOT, 'Cargo.toml')).read()
    table = section(text, 'workspace.package') or section(text, 'package')
    return re.search(r'^version\s*=\s*"([^"]+)"', table, re.M).group(1)


def inherited(manifest):
    """True when a member manifest takes the workspace version."""
    path = os.path.join(ROOT, manifest)
    if not os.path.exists(path):
        return None
    package = section(open(path).read(), 'package') or ''
    return re.search(r'^version\.workspace\s*=\s*true', package, re.M) is not None


def default_ffi():
    names = {'darwin': 'librivet.dylib', 'win32': 'rivet.dll'}
    path = os.path.join(ROOT, 'target', 'release', names.get(sys.platform, 'librivet.so'))
    return path if os.path.exists(path) else None


def ffi_versions(path):
    """(rivet_version(), rivet_abi_version()) of a built librivet."""
    lib = ctypes.CDLL(path)
    lib.rivet_version.restype = ctypes.c_char_p
    lib.rivet_abi_version.restype = ctypes.c_uint32
    return lib.rivet_version().decode(), lib.rivet_abi_version()


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, cwd=kw.pop('cwd', ROOT), **kw)


def main():
    args = sys.argv[1:]
    binary = os.path.join(ROOT, args[args.index('--bin') + 1] if '--bin' in args else 'target/release/rivet')
    want = cargo_version()
    problems = []

    def check(label, got):
        mark = 'ok ' if got == want else 'BAD'
        print(f'{mark} {label:<34} {got}')
        if got != want:
            problems.append(label)

    print(f'    Cargo.toml version                 {want}')
    for manifest in ('Cargo.toml', 'ffi/Cargo.toml'):
        got = inherited(manifest)
        if got is None:
            continue
        print(f'{"ok " if got else "BAD"} {manifest + " version.workspace":<34} {"yes" if got else "no"}')
        if not got:
            problems.append(manifest)
    # rivet-ffi pins its path dependency on rivet-runtime to the same version.
    ffi_manifest = os.path.join(ROOT, 'ffi', 'Cargo.toml')
    if os.path.exists(ffi_manifest):
        m = re.search(r'package\s*=\s*"rivet-runtime"[^}]*version\s*=\s*"([^"]+)"', open(ffi_manifest).read())
        if m:
            check('ffi/Cargo.toml rivet-runtime requirement', m.group(1))
    if not os.path.exists(binary):
        print(f'BAD {"rivet binary":<34} missing: {binary} (cargo build --release --features cli)')
        problems.append('rivet binary')
        print('version sync: FAILED (' + ', '.join(problems) + ')')
        return 1
    check('rivet --version', run([binary, '--version']).stdout.strip().removeprefix('rivet '))

    with tempfile.TemporaryDirectory() as d:
        open(os.path.join(d, 'app.rivet'), 'w').write('operation v.ok\n    output json\n    return 1\nend\n')
        init = {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
                'params': {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'check', 'version': '0'}}}
        p = subprocess.Popen([binary, '--file', 'app.rivet', 'serve', '--stdio'], cwd=d,
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        p.stdin.write(json.dumps(init) + '\n')
        p.stdin.flush()
        line = p.stdout.readline()
        p.kill()
        p.wait()
        try:
            check('MCP serverInfo.version', json.loads(line)['result']['serverInfo']['version'])
        except (ValueError, KeyError):
            check('MCP serverInfo.version', f'<no answer: {line[:80]!r}>')
        out = run([binary, '--file', 'app.rivet', 'request', 'rivet.capabilities'], cwd=d).stdout
        try:
            j = json.loads(out)
            # 0.2.0 envelopes carry the payload in `data` (0.1.0 used `result`).
            check('rivet.capabilities version', (j.get('data') or j.get('result'))['version'])
        except (ValueError, KeyError):
            check('rivet.capabilities version', f'<no answer: {out[:80]!r}>')
            j = {}

    ffi = os.path.join(ROOT, args[args.index('--ffi') + 1]) if '--ffi' in args else default_ffi()
    if ffi:
        try:
            got, abi = ffi_versions(ffi)
        except OSError as e:
            got, abi = f'<cannot load {ffi}: {e}>', None
        check('librivet rivet_version()', got)
        want_abi = ((j.get('data') or {}) if isinstance(j, dict) else {}).get('abi_version')
        ok = abi is not None and abi == want_abi
        print(f'{"ok " if ok else "BAD"} {"rivet_abi_version() == capabilities":<34} {abi} / {want_abi}')
        if not ok:
            problems.append('rivet_abi_version')
    else:
        print(f'    {"librivet":<34} not built (cargo build --release -p rivet-ffi); skipped')

    if '--tag' in args:
        tag = run(['git', 'describe', '--tags', '--exact-match', 'HEAD']).stdout.strip()
        check('git describe --exact-match HEAD', tag.removeprefix('v') if tag.startswith('v') else f'<{tag or "no tag"}>')
        dirty = run(['git', 'status', '--porcelain']).stdout.strip()
        print(f'{"ok " if not dirty else "BAD"} {"clean working tree":<34} {"yes" if not dirty else "no"}')
        if dirty:
            problems.append('clean working tree')

    print('version sync: ' + ('OK' if not problems else 'FAILED (' + ', '.join(problems) + ')'))
    return 0 if not problems else 1


if __name__ == '__main__':
    sys.exit(main())
