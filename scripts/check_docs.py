#!/usr/bin/env python3
"""Documentation checks from DOCUMENTATION.md §19 (PLAN-2026-0001 TASK-071, test T-31).

Run from the repository root:  python3 scripts/check_docs.py [--warn-only]

Checks every Markdown file under docs/ plus README.md:
  front matter present with the always-required fields; valid status; ISO dates;
  unique document_id; ID prefix matches document_type; file name starts with the
  lower-case ID (except index.md / README.md); document_revision equals the newest
  Change History row; relative links and anchors resolve; code fences balance;
  each document is linked from its directory's index.md or README.md.
Exit status: 0 clean, 1 errors found.
"""

import os
import re
import sys
from glob import glob

REQUIRED = ["document_id", "title", "document_type", "status", "created_date", "last_updated", "owner", "reason"]
STATUSES = {
    "draft", "under-review", "proposed", "approved", "active", "implemented", "mitigated", "resolved",
    "completed", "rejected", "cancelled", "deprecated", "superseded", "archived",
}
PREFIX = {
    "ARCH": "architecture", "PROP": "proposal", "DISC": "discussion", "ADR": "decision", "PLAN": "plan",
    "TECH": "technique", "SYS": "system", "API": "api", "DATA": "data", "SEC": "security", "OPS": "operations",
    "RUN": "runbook", "TRBL": "troubleshooting", "MAN": "manual", "ONB": "onboarding", "DEMO": "demo",
    "TEST": "test", "RPT": "report", "INC": "incident", "REL": "release", "MIG": "migration", "RES": "research",
    "CMP": "compliance", "REF": "reference", "TMPL": "template", "STD": "standard",
}
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def front_matter(text):
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---\n", 4)
    if end < 0:
        return None
    meta = {}
    for line in text[4:end].split("\n"):
        m = re.match(r"^([A-Za-z_]+):\s*(.*?)\s*(#.*)?$", line)
        if m:
            meta[m.group(1)] = m.group(2).strip().strip('"')
    return meta


def slug(heading):
    h = heading.strip().lower()
    h = re.sub(r"[`*_~]", "", h)
    h = re.sub(r"[^\w\- ]", "", h)
    return h.replace(" ", "-")


def anchors_of(text):
    out, seen = set(), {}
    stripped = re.sub(r"```.*?```", "", text, flags=re.S)
    for m in re.finditer(r"^#{1,6} (.+)$", stripped, re.M):
        s = slug(m.group(1))
        n = seen.get(s, 0)
        seen[s] = n + 1
        out.add(s if n == 0 else f"{s}-{n}")
    out |= set(re.findall(r'<a id="([^"]+)"', text))
    return out


def newest_revision(text):
    m = re.search(r"## Change History\s*\n\s*\| Revision[^\n]*\n\|[-| ]+\|\n((?:\|[^\n]*\n)+)", text)
    if not m:
        return None
    revs = [int(r) for r in re.findall(r"^\|\s*(\d+)\s*\|", m.group(1), re.M)]
    return max(revs) if revs else None


def main():
    warn_only = "--warn-only" in sys.argv
    files = ["README.md"] + sorted(glob("docs/**/*.md", recursive=True))
    texts = {f: open(f, encoding="utf-8").read() for f in files}
    extra_anchor_sources = ["DOCUMENTATION.md", "AGENTS.md"]
    anchors = {os.path.normpath(f): anchors_of(t) for f, t in texts.items()}
    for f in extra_anchor_sources:
        if os.path.exists(f):
            anchors[os.path.normpath(f)] = anchors_of(open(f, encoding="utf-8").read())
    errors, ids = [], {}

    def err(f, msg):
        errors.append(f"{f}: {msg}")

    for f, t in texts.items():
        base = os.path.basename(f)
        meta = front_matter(t)
        if meta is None:
            err(f, "missing YAML front matter")
            continue
        for k in REQUIRED:
            if not meta.get(k):
                err(f, f"front matter lacks `{k}`")
        if "affected_versions" not in t[: t.find("\n---\n", 4)]:
            err(f, "front matter lacks `affected_versions`")
        status = meta.get("status", "")
        if status and status not in STATUSES:
            err(f, f"invalid status `{status}`")
        for k in ("created_date", "last_updated"):
            if meta.get(k) and not DATE.match(meta[k]):
                err(f, f"`{k}` is not an ISO date: {meta[k]}")
        did = meta.get("document_id", "")
        if did:
            ids.setdefault(did, []).append(f)
            prefix = did.split("-")[0]
            expected = PREFIX.get(prefix)
            if expected is None:
                err(f, f"unknown ID prefix `{prefix}`")
            elif meta.get("document_type") and meta["document_type"] != expected:
                err(f, f"ID prefix {prefix} expects document_type `{expected}`, found `{meta['document_type']}`")
            if base not in ("index.md", "README.md") and not base.startswith(did.lower() + "-") and base != did.lower() + ".md":
                err(f, f"file name should start with `{did.lower()}-`")
        rev = meta.get("document_revision")
        newest = newest_revision(t)
        if rev and newest is not None and int(rev) != newest:
            err(f, f"document_revision {rev} but newest Change History row is {newest}")
        if len(re.findall(r"^\s*```", t, re.M)) % 2:
            err(f, "unbalanced code fences")
        for m in re.finditer(r"\]\(([^)\s]+)\)", re.sub(r"```.*?```", "", t, flags=re.S)):
            url = m.group(1)
            if re.match(r"(https?|mailto):", url):
                continue
            path, _, frag = url.partition("#")
            target = os.path.normpath(os.path.join(os.path.dirname(f), path)) if path else os.path.normpath(f)
            if not os.path.exists(target):
                err(f, f"broken link {url}")
            elif frag and target.endswith(".md") and target in anchors and frag not in anchors[target]:
                err(f, f"missing anchor {url}")
        # index membership
        if base not in ("index.md", "README.md") and f.startswith("docs/"):
            d = os.path.dirname(f)
            linked = False
            for idx in ("index.md", "README.md"):
                p = os.path.join(d, idx)
                if os.path.exists(p) and (base in open(p, encoding="utf-8").read()):
                    linked = True
            if not linked:
                err(f, f"not linked from {d}/index.md or README.md")
    for did, fs in ids.items():
        if len(fs) > 1:
            err(fs[0], f"document_id {did} is also used by {', '.join(fs[1:])}")
    for e in errors:
        print(e)
    print(f"check_docs: {len(files)} files, {len(errors)} problem(s)")
    return 0 if warn_only or not errors else 1


if __name__ == "__main__":
    sys.exit(main())
