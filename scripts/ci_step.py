#!/usr/bin/env python3
"""Run one CI step and surface its failure as GitHub annotations.

Job logs need admin rights to download, but check-run annotations are readable by anyone with
access to the repository. When the command fails, this script re-emits the relevant lines
(failing tests, panics, compiler errors) and the output tail as `::error` annotations.

    python scripts/ci_step.py <title> -- <command> [args...]

Read them without auth (TRBL-2026-0007):

    GET https://api.github.com/repos/<owner>/<repo>/actions/runs/<run>/jobs
    GET https://api.github.com/repos/<owner>/<repo>/check-runs/<job_id>/annotations?per_page=50

    output ──strip ANSI──▶ relevant lines ─┐
                          tail (no Compiling/Checking noise) ─┴─▶ ≤ 4000-char chunks ─▶ ::error

GitHub truncates an annotation at 4096 characters and keeps at most 10 error annotations per
step, so the colour codes are disabled and stripped, and each message is split into chunks.

Exit status: the command's own (1 if this script itself fails).
"""
import os
import re
import subprocess
import sys
import traceback

MAX_LINES = 80
CHUNK = 4000
MAX_CHUNKS = 4
RELEVANT = re.compile(r'(FAILED|panicked at|^error(\[|:)|^\s+--> |^failures:|assertion|thread .* panicked|BAD |^warning: unused|^Traceback|Error:)')
NOISE = re.compile(r'^\s*(Compiling|Checking|Downloaded|Downloading|Fresh|Running|Blocking) ')
ANSI = re.compile(r'\x1b\[[0-9;?]*[A-Za-z]')


def escape(text):
    # GitHub workflow-command escaping for the message part.
    return text.replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')


def annotate(title, lines):
    """Emit `lines` as one or more ::error annotations of at most CHUNK characters each."""
    chunks, current = [], ''
    for line in lines:
        line = line[:CHUNK]
        if current and len(current) + len(line) + 1 > CHUNK:
            chunks.append(current)
            current = ''
        current += line + '\n'
    if current:
        chunks.append(current)
    chunks = chunks[:MAX_CHUNKS]
    for n, chunk in enumerate(chunks, 1):
        suffix = f' ({n}/{len(chunks)})' if len(chunks) > 1 else ''
        print(f'::error title={title}{suffix}::{escape(chunk.rstrip())}', flush=True)


def main():
    # Windows consoles default to cp1252; cargo and test output are UTF-8.
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding='utf-8', errors='replace')
        except AttributeError:
            pass
    if '--' not in sys.argv:
        sys.exit('usage: ci_step.py <title> -- <command> [args...]')
    i = sys.argv.index('--')
    title = ' '.join(sys.argv[1:i]) or 'step'
    cmd = sys.argv[i + 1:]
    env = dict(os.environ, CARGO_TERM_COLOR='never')
    try:
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                text=True, encoding='utf-8', errors='replace', env=env)
        lines = []
        for line in proc.stdout:
            sys.stdout.write(line)
            sys.stdout.flush()
            lines.append(ANSI.sub('', line.rstrip('\n')))
        code = proc.wait()
    except Exception:  # the step must never fail silently
        annotate(f'{title}: ci_step.py error', traceback.format_exc().splitlines())
        return 1
    if code != 0:
        relevant = [l for l in lines if RELEVANT.search(l)][:MAX_LINES]
        if relevant:
            annotate(f'{title}: relevant lines', relevant)
        tail = [l for l in lines if not NOISE.search(l)][-MAX_LINES:]
        annotate(f'{title}: last {MAX_LINES} lines (exit {code})', tail)
    return code


if __name__ == '__main__':
    sys.exit(main())
