#!/usr/bin/env python3
"""Run one CI step and surface its failure as GitHub annotations.

Job logs need admin rights to download, but check-run annotations are readable by anyone with
access to the repository. When the command fails, this script re-emits the relevant lines
(failing tests, panics, compiler errors) and the output tail as `::error` annotations.

    python scripts/ci_step.py <title> -- <command> [args...]

Exit status: the command's own.
"""
import re
import subprocess
import sys

MAX_LINES = 80
RELEVANT = re.compile(r'(FAILED|panicked at|^error(\[|:)|^\s+--> |^failures:|assertion|thread .* panicked|BAD )')


def escape(text):
    # GitHub workflow-command escaping for the message part.
    return text.replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')


def main():
    if '--' not in sys.argv:
        sys.exit('usage: ci_step.py <title> -- <command> [args...]')
    i = sys.argv.index('--')
    title = ' '.join(sys.argv[1:i]) or 'step'
    cmd = sys.argv[i + 1:]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                            text=True, encoding='utf-8', errors='replace')
    lines = []
    for line in proc.stdout:
        sys.stdout.write(line)
        sys.stdout.flush()
        lines.append(line.rstrip('\n'))
    code = proc.wait()
    if code != 0:
        relevant = [l for l in lines if RELEVANT.search(l)][:MAX_LINES]
        if relevant:
            print(f'::error title={title}: relevant lines::{escape(chr(10).join(relevant))}')
        print(f'::error title={title}: last {MAX_LINES} lines::{escape(chr(10).join(lines[-MAX_LINES:]))}')
    return code


if __name__ == '__main__':
    sys.exit(main())
