"""Extract every rivet block from the reference, demos and proposal into one corpus file.

Usage (from the repository root): python3 docs/research/res-2026-0001-capy-grammar-spike/extract_corpus.py OUT
"""
import glob
import re
import sys

out = []
ref = open('docs/references/ref-2026-0002-language-and-usage.md').read()
for m in re.finditer(r'```rivet\n(.*?)```', ref, re.S):
    heads = list(re.finditer(r'^### (.+)$', ref[:m.start()], re.M))
    name = heads[-1].group(1).split(' — ')[0] if heads else 'top'
    out.append(f'=====SRC ref:{name}:{m.start()}\n{m.group(1)}')
for f in sorted(glob.glob('docs/demos/**/*.rivet', recursive=True)):
    out.append(f'=====SRC {f}\n' + open(f).read())
prop = open(glob.glob('docs/proposals/*/prop-2026-0001-rivet-runtime.md')[0]).read()
for m in re.finditer(r'```rivet\n(.*?)```', prop, re.S):
    out.append(f'=====SRC prop:{prop[:m.start()].count(chr(10)) + 1}\n{m.group(1)}')
open(sys.argv[1], 'w').write('\n'.join(out))
print(len(out), 'blocks')
