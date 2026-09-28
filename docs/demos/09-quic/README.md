---
document_id: DEMO-2026-0009
title: "QUIC streams and HTTP3"
document_type: demo
status: active
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 5
authors: [Codex, Claude]
owner: Project maintainer
reviewers: [Project maintainer]
systems: [Rivet]
components: [quic, transports, policy, cli]
affected_versions:
  from: "0.1.0"
  to: null
applicable_environments: [development]
audience: [developers, reviewers]
scope: Runnable QUIC and HTTP/3 demo — a scoped native QUIC connection with a length-prefixed bidirectional stream and a strict `version 3` HTTP request, trusted through `tls ca_file`, with timeout, wrong-ALPN, untrusted-certificate and policy failures, against a shipped aioquic fixture.
reason: User requested sample files in folders with READMEs showing usage; UQ-14 asks for QUIC and HTTP/3; UQ-17 adds declared outputs and policy.json-only policy; TASK-067 executed every step against the 0.1.0 release candidate.
dependencies: [PROP-2026-0001, REF-2026-0002]
related_documents: [PLAN-2026-0001, DEMO-2026-0015, DEMO-2026-0013, MAN-2026-0008, TEST-2026-0013, TEST-2026-0014, TEST-2026-0015, TEST-2026-0025]
supersedes: null
superseded_by: null
tags: [rivet, demo, quic, http3, tls]
confidentiality: internal
review_cycle: on-release
next_review_date: 2026-10-28
verified_against: "0.1.0"
---

# QUIC streams and HTTP3

> **Status:** Active
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0 and later
> **Owner:** Project maintainer
> **Affected Components:** quic, transports, policy, cli

## Purpose

QUIC streams and HTTP3. Delivery stage: **B**. Read [app.rivet](app.rivet) alongside this walkthrough.

| Operation ID | Output | Behavior |
|---|---|---|
| `engine.status` | `object {state}` | Scoped QUIC connection (ALPN `rivet-rpc/1`), one bidirectional stream with 32-bit big-endian length framing. |
| `items.http3` | `object {items, version}` | `http get … version 3`: HTTP/3 or a typed failure, never a silent fallback. |

```text
 engine.status                                   items.http3
 with quic "quic://engine.example.com:4433"      http get "https://api.example.com/items"
   alpn / max_streams / migration  (options)       version 3   (no silent fallback)
   with connection.open bidi as stream              |
     framing length32 endian big (option)           v
     send {action:"status"} --> <-- {"state":"ready"}   {"items":[], "version":3}
   scope exit: stream finished, connection closed  -- even on the early return
```

## Verified Against Version

0.1.0. Verified on 0.1.0-dev at commit `829ca43`, the release candidate (the version bump to 0.1.0 happens at release, P5), with `target/release/rivet` on macOS 26.4.1 (Darwin 25.4.0, arm64), 2026-09-28. Every output block below was pasted from that run. Request and trace IDs, hashes and ports vary.

## Prerequisites

```sh
cargo build --release                       # from the repository root
export PATH="$PWD/target/release:$PATH"     # `rivet --version` prints rivet 0.1.0
python3 -m venv "$TMPDIR/rivet-quic-venv" && "$TMPDIR/rivet-quic-venv/bin/pip" install aioquic
```

- [fixtures/quic_fixture.py](fixtures/quic_fixture.py) (aioquic): UDP 18890 speaks `rivet-rpc/1` (one length32 JSON frame per bidi stream → `{"state":"ready"}`), UDP 18891 speaks `h3` (`GET /items` → `{"items":[]}`). `--rpc-silent` never answers.
- `openssl` to create a throwaway CA and a `localhost` certificate in the scratch directory; no key material is committed.

## Setup

```sh
cd docs/demos/09-quic
```

```text
  policy.json            (auto-discovered)  allow_network quic://engine.example.com:4433   -> engine.status
  policies/http3.json    (--policy)         allow_network https://api.example.com:443      -> items.http3
```

The hosts are placeholders. Steps 1–2 inspect this folder; steps 3–6 run against the fixture in a scratch copy that points both operations at loopback and adds `tls server_name "localhost"` and `tls ca_file "./ca.pem"` (the fixture's CA is not in the system trust store).

## Steps

### 1. Check the bundle and view outputs

#### Command / Request

```sh
rivet --file app.rivet check --strict-docs
rivet --file app.rivet outputs items.http3
rivet --file app.rivet outputs --all --json
```

#### Expected Output / Response

```text
ok: 2 operations, 0 connectors, 0 auth profiles
```

```text
items.http3 — Fetch items over HTTP3
output  object   Items plus the negotiated HTTP version.
  items    list json required  Items returned by GET /items.
  version  integer  required  Negotiated HTTP version; always 3.
emits    —
receives —
errors   —
```

`outputs --all --json` prints both entries on one line, the first being:

```json
{"id":"engine.status","output":{"type":"object","properties":{"state":{"type":"string","description":"Engine state; the fixture answers \"ready\"."}},"required":["state"],"additionalProperties":false,"description":"Engine status frame returned on the QUIC stream."},"emits":null,"receives":null,"errors":[]}
```

All exit 0. Resource options (`alpn`, `max_streams`, `migration`, `framing`) precede the first body statement; an option after a statement is `syntax.option_after_body` (exit 2).

### 2. I/O manifest: protocol-tagged sites

#### Command / Request

```sh
rivet --file app.rivet io --by target
rivet --file app.rivet io --check-policy
rivet --file app.rivet io engine.status --check-policy
rivet --file app.rivet --policy ./policies/http3.json io items.http3 --check-policy
rivet --file app.rivet io --check-files
```

#### Expected Output / Response

HTTP/3 over QUIC is not confused with raw QUIC:

```text
TARGET                          ACCESS               CAPABILITY     ORIGIN     PHASE    NEEDS FILE  USED BY
quic://engine.example.com:4433  connect (quic)       allow_network  with quic  connect  —           engine.status
https://api.example.com:443     connect GET (http3)  allow_network  http get   connect  —           items.http3
```

Each policy file covers exactly one operation, so the whole-bundle check exits **3**:

```text
OPERATION      KIND     ACCESS       TARGET                          KNOWLEDGE  SOURCE        DECISION
engine.status  network  connect      quic://engine.example.com:4433  exact      app.rivet:7   allowed
items.http3    network  connect GET  https://api.example.com/items   exact      app.rivet:26  denied
1 allowed · 1 denied
```

The per-operation checks print one `allowed` row and `1 allowed` each, and exit 0. `io --check-files` prints `… needs no existing files.` for both and `0 files` (exit 0). In `--format json`, `engine.status#1` has `"protocol":"quic"` and `items.http3#1` has `"protocol":"http3","method":"GET"`.

### 3. Local fixture run: certificates, scratch copy, fixture

#### Command / Request

```sh
WORK="$(mktemp -d)"; cp -R app.rivet policy.json policies fixtures "$WORK/"; cd "$WORK"
openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 2 -subj '/CN=rivet demo CA' \
  -addext 'basicConstraints=critical,CA:TRUE' -addext 'keyUsage=critical,keyCertSign'
openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -subj '/CN=localhost'
printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\n' > ext.cnf
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out server.pem -days 2 -extfile ext.cnf
python3 - <<'PY'
s = open("app.rivet").read()
s = s.replace('quic://engine.example.com:4433', 'quic://127.0.0.1:18890')
s = s.replace('https://api.example.com/items', 'https://127.0.0.1:18891/items')
tls = '        tls server_name "localhost"\n        tls ca_file "./ca.pem"\n'
s = s.replace('        migration false\n', '        migration false\n' + tls)
s = s.replace('        version 3\n', '        version 3\n' + tls)
open("app.rivet", "w").write(s)
for f, old, new, ca in [("policy.json", "quic://engine.example.com:4433", "quic://127.0.0.1:18890", "./ca.pem"),
                        ("policies/http3.json", "https://api.example.com:443", "https://127.0.0.1:18891", "../ca.pem")]:
    t = open(f).read().replace(old, new)
    t = t.replace('"grants": [', '"grants": [\n    {"capability": "allow_read", "targets": ["%s"]},' % ca, 1)
    open(f, "w").write(t)
PY
"$TMPDIR/rivet-quic-venv/bin/python" fixtures/quic_fixture.py --cert server.pem --key server.key > fixture.log 2>&1 & FX=$!
rivet --file app.rivet io --check-policy
```

#### Expected Output / Response

`tls ca_file` is itself a file read that policy must grant (the new rows at lines 12 and 31); `items.http3`'s network site is still denied under policy.json (exit 3):

```text
OPERATION      KIND     ACCESS       TARGET                         KNOWLEDGE  SOURCE        DECISION
engine.status  network  connect      quic://127.0.0.1:18890         exact      app.rivet:7   allowed
engine.status  file     read         ./ca.pem                       exact      app.rivet:12  allowed
items.http3    network  connect GET  https://127.0.0.1:18891/items  exact      app.rivet:28  denied
items.http3    file     read         ./ca.pem                       exact      app.rivet:31  allowed
3 allowed · 1 denied
```

`fixture.log` starts with `quic rivet-rpc/1 on udp 127.0.0.1:18890; h3 on udp 127.0.0.1:18891`. The fixture needs about a second to start; wait for that line after every fixture start below before running `rivet`.

### 4. QUIC stream and strict HTTP/3

#### Command / Request

```sh
rivet --file app.rivet request engine.status --params '{}'
rivet --file app.rivet --policy ./policies/http3.json request items.http3 --params '{}'
rivet --file app.rivet request items.http3 --params '{}'
rivet --file app.rivet --policy ./policies/http3.json request engine.status --params '{}'
```

#### Expected Output / Response

Both succeed under their own policy file (exit 0):

```json
{"request_id":"req_01db5ccd9d","trace_id":"tr_01db5ccd9d","result":{"state":"ready"},"data_count":0,"effects":"committed"}
{"request_id":"req_01c84dc925","trace_id":"tr_01c84dc925","result":{"items":[],"version":3},"data_count":0,"effects":"none"}
```

Under the other file each is denied (exit 3), for example:

```json
{"request_id":"req_01c72f72dd","trace_id":"tr_01c72f72dd","error":{"kind":"permission","code":"permission.denied","message":"allow_network connect https://127.0.0.1:18891/items denied: 127.0.0.1 is a private/loopback/link-local address; grant it literally (e.g. \"https://127.0.0.1:18891/items\") to allow it","retryable":false,"effects":"none","source":{"file":"app.rivet","line":28,"column":5,"end_line":33,"end_column":8},"operation_id":"items.http3","details":{"capability":"allow_network","access":"connect","target":"https://127.0.0.1:18891/items"}}}
```

`engine.status` under http3.json is denied the same way for `quic://127.0.0.1:18890`. `fixture.log` gains `rpc stream 0: {'action': 'status'}` and `h3 GET /items`.

### 5. Stream timeout

#### Command / Request

```sh
kill $FX
"$TMPDIR/rivet-quic-venv/bin/python" fixtures/quic_fixture.py --cert server.pem --key server.key --rpc-silent >> fixture.log 2>&1 & FX=$!
rivet --file app.rivet request engine.status --params '{}'
kill $FX
```

#### Expected Output / Response

After `timeout "5s"` (exit 6); the request frame was sent, so `effects` is `committed`:

```json
{"request_id":"req_014d51c805","trace_id":"tr_014d51c805","error":{"kind":"timeout","code":"timeout","message":"receiving from the stream did not complete within 5000 ms","retryable":false,"effects":"committed","source":{"file":"app.rivet","line":16,"column":13,"end_line":16,"end_column":52},"operation_id":"engine.status"}}
```

### 6. Handshake failures: wrong ALPN, no HTTP/3, untrusted certificate

#### Command / Request

Point each operation at the other fixture port, so the ALPN never matches:

```sh
sed -i.bak2 's#quic://127.0.0.1:18890#quic://127.0.0.1:18891#' app.rivet policy.json
sed -i.bak3 's#https://127.0.0.1:18891#https://127.0.0.1:18890#' app.rivet policies/http3.json
"$TMPDIR/rivet-quic-venv/bin/python" fixtures/quic_fixture.py --cert server.pem --key server.key >> fixture.log 2>&1 & FX=$!
rivet --file app.rivet request engine.status --params '{}'
rivet --file app.rivet --policy ./policies/http3.json request items.http3 --params '{}'
kill $FX
```

#### Expected Output / Response

Native QUIC with the wrong ALPN (exit 5):

```json
{"request_id":"req_019e1d3ddd","trace_id":"tr_019e1d3ddd","error":{"kind":"tls","code":"quic.tls","message":"QUIC handshake failed: aborted by peer: the cryptographic handshake failed: error 40: No common ALPN protocols","retryable":false,"effects":"none","source":{"file":"app.rivet","line":7,"column":5,"end_line":18,"end_column":8},"operation_id":"engine.status","details":{"tls_alert":40}}}
```

`version 3` against a peer that does not speak `h3` fails before any request byte and does not fall back (exit 5):

```json
{"request_id":"req_019c068725","trace_id":"tr_019c068725","error":{"kind":"tls","code":"tls.handshake","message":"HTTP/3 TLS handshake failed: aborted by peer: the cryptographic handshake failed: error 40: No common ALPN protocols","retryable":false,"effects":"none","source":{"file":"app.rivet","line":28,"column":5,"end_line":33,"end_column":8},"operation_id":"items.http3","details":{"tls_alert":40,"request_sent":false,"version":3}}}
```

Without the two `tls` lines (the platform verifier does not know the demo CA), the QUIC handshake fails with `quic.tls` and `invalid peer certificate: UnknownIssuer` (`tls_alert` 48, exit 5).

## Effects and policy

```text
  outcome table (verified in steps 4–6)
  ┌──────────────────────────────────────────┬───────────────────┬──────┬───────────┐
  │ situation                                │ code              │ exit │ effects   │
  ├──────────────────────────────────────────┼───────────────────┼──────┼───────────┤
  │ QUIC status frame                        │ —                 │  0   │ committed │
  │ HTTP/3 GET /items                        │ —                 │  0   │ none      │
  │ run under the other policy file          │ permission.denied │  3   │ none      │
  │ no reply within timeout "5s"             │ timeout           │  6   │ committed │
  │ QUIC ALPN mismatch / unknown issuer      │ quic.tls          │  5   │ none      │
  │ version 3 against a non-h3 peer          │ tls.handshake     │  5   │ none      │
  └──────────────────────────────────────────┴───────────────────┴──────┴───────────┘
```

The HTTPS-origin grant allows the HTTP adapter's own QUIC/UDP transport, not raw QUIC or UDP. Native QUIC has its own `quic://` grant. 0-RTT is off and migration is refused (`migration false` here).

## Release Updates

| Update | Inciting User Requirement | What Changed | Do This | Expected Result | Evidence Source |
|---|---|---|---|---|---|
| U-17 | UQ-14 / R17 | Native QUIC v1: ALPN, bidi streams, framing, TLS trust | Steps 3–6 | `{"state":"ready"}`; `timeout`; `quic.tls` | This README steps 3–6 (2026-09-28, 829ca43); TEST-2026-0013 |
| U-18 | UQ-14 / R18 | Strict HTTP/3 (`version 3`) with no silent fallback | Steps 4, 6 | `{"items":[],"version":3}`; `tls.handshake` with `request_sent:false` | This README steps 4, 6; TEST-2026-0014 |
| U-26 | UQ-18 / R26 | Protocol-tagged manifest; `tls ca_file` is a file site | Steps 2–3 | `quic` vs `http3`; `./ca.pem` rows | This README steps 2–3; TEST-2026-0025 |

## Cleanup

```sh
kill $FX 2>/dev/null
cd - && rm -rf "$WORK"                     # removes the scratch CA and keys too
rm -rf "$TMPDIR/rivet-quic-venv"           # optional
```

## Verification Record

| Step | Verified By | Verified At | Result |
|---|---|---|---|
| 1. `check --strict-docs`, `outputs` | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 2. `io --by target`; `io --check-policy` exit 3 / 0 / 0; `io --check-files` exit 0 | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 3. Certificates, scratch copy, `ca_file` sites | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 4. QUIC status, HTTP/3 items, cross-policy denials | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 5. Stream timeout | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |
| 6. Wrong ALPN, non-h3 peer, untrusted certificate | Claude (TASK-067) | 2026-09-28, commit 829ca43, macOS 26.4.1 arm64 | PASS |

Build: `cargo build` and `cargo build --release` at 829ca43; every command above was executed from this folder or the scratch copy and the output pasted from that run.

## Known Caveats

- The fixture certificate is signed by a throwaway CA, so the scratch copy adds `tls server_name` and `tls ca_file`; a real server with a publicly trusted certificate needs neither.
- `version prefer [3, 2]` fallback, QUIC datagrams, uni streams and migration refusal are covered by TEST-2026-0013/0014, not by this folder.
- aioquic logs `Error: 296, reason: No common ALPN protocols` lines in `fixture.log` during step 6; they are expected.

## Related Documents

- [All sample folders](../README.md) · [demos index](../index.md) · [release verification guide](../demo-2026-0015-v0-1-0-release-verification.md)
- [Protocols and connectors manual](../../manuals/man-2026-0008-protocols-and-connectors.md)
- [QUIC tests TEST-2026-0013](../../testing/test-2026-0013-quic.md) · [HTTP/3 tests TEST-2026-0014](../../testing/test-2026-0014-http3.md)
- [Usage reference](../../references/ref-2026-0002-language-and-usage.md) · [Proposal](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 5 | 2026-09-28 | Claude | TASK-067: added fixtures/quic_fixture.py (ports 18890/18891) and a scratch-copy run with a throwaway CA and `tls ca_file`; executed every step against 0.1.0-dev (829ca43) and pasted real output: check, outputs, manifest (summary line, `ca_file` file sites), QUIC status, HTTP/3 items, cross-policy denials, stream `timeout`, `quic.tls` (ALPN, unknown issuer), `tls.handshake` for a non-h3 peer; removed draft disclaimers; status active; verified_against 0.1.0. |
| 4 | 2026-09-28 | Claude | TASK-005/R26 (ADR-0001, proposal revision 8): `io --by target` gains ORIGIN (`with quic`, `http get`), PHASE and NEEDS FILE. |
| 3 | 2026-09-28 | Claude | UQ-18/R26: I/O manifest: protocol-tagged sites by target and `io --check-policy` against policy.json and policies/http3.json. |
| 2 | 2026-09-28 | Claude | UQ-17: declared outputs; quoted `timeout "5s"`; `--sandbox` variants became policy.json and policies/http3.json; leading-options note; View outputs, strict-docs, bootstrap and exit codes. |
| 1 | 2026-09-28 | Codex | Added draft source files, prerequisites, invocation examples and expected behavior. |
