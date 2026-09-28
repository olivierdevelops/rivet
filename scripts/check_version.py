#!/usr/bin/env python3
"""Version sync check (PLAN-2026-0001 TASK-074 / T-33, DOCUMENTATION §32).

Checks that the one canonical version in Cargo.toml is what every surface reports:

    Cargo.toml [package].version
      ├── rivet --version                    "rivet X.Y.Z"
      ├── MCP initialize serverInfo.version  (serve --stdio)
      ├── rivet.capabilities result.version
      └── with --tag: HEAD is exactly tag vX.Y.Z and the tree is clean

Usage: python3 scripts/check_version.py [--bin target/release/rivet] [--tag]
Exit 0 when everything agrees, 1 otherwise.
"""
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def cargo_version():
    text = open(os.path.join(ROOT, 'Cargo.toml')).read()
    package = text.split('[package]', 1)[1].split('\n[', 1)[0]
    return re.search(r'^version\s*=\s*"([^"]+)"', package, re.M).group(1)


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
            check('rivet.capabilities version', json.loads(out)['result']['version'])
        except (ValueError, KeyError):
            check('rivet.capabilities version', f'<no answer: {out[:80]!r}>')

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
