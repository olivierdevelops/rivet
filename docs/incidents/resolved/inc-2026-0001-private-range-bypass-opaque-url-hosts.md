---
document_id: INC-2026-0001
title: "Private-range rule skipped for udp://, quic:// and tcp:// IP literals"
document_type: incident
status: resolved
created_date: 2026-09-28
last_updated: 2026-09-28
document_revision: 1
authors: [Claude]
owner: Project maintainer
severity: "S2"
start_time: 2026-09-28T10:40:00Z
end_time: 2026-09-28T10:55:00Z
root_cause_status: identified
systems: [Rivet]
components: [policy, authorize_effect]
affected_versions:
  from: "0.1.0-dev"
  to: "0.1.0-dev"
confidentiality: internal
scope: Defect found and fixed during PLAN-2026-0001 implementation; never released.
reason: DOCUMENTATION §25 — unexpected defects found during implementation are recorded as incidents.
related_documents: [PLAN-2026-0001, PROP-2026-0001]
supersedes: null
superseded_by: null
tags: [rivet, incident, implementation]
---

# Private-range rule skipped for udp://, quic:// and tcp:// IP literals

> **Status:** Resolved
> **Created:** 2026-09-28
> **Last Updated:** 2026-09-28
> **Affected Versions:** 0.1.0-dev (unreleased)
> **Owner:** Project maintainer
> **Affected Components:** policy, authorize_effect

## Incident Summary

`network.deny_private_ranges` did not deny private or loopback IP literals written in non-special URL schemes (`udp://10.0.0.5:53`, `quic://[::1]:4433`, `tcp://…`) when a broad `"*"` grant existed. The URL parser keeps hosts of non-special schemes as opaque text, so the private-range predicate never saw an IP address.

## Severity

S2 — security control bypass (SSRF protection) in unreleased code.

## Status

Resolved in commit `2c3d09b` before any release.

## Discovery Context

Reported by the WS-D implementer (UDP/QUIC, TASK-042/043) while writing policy fixtures for S81–S98: loopback literals were being treated as host names.

## Start Time

2026-09-28T10:40:00Z

## End Time

2026-09-28T10:55:00Z

## Affected Systems

Rivet.

## Affected Versions

0.1.0-dev development builds only; no release contained the defect.

## Affected Components

- policy
- authorize_effect

## Customer Impact

None: the defect never shipped.

## Detection

Code review during implementation; confirmed with a failing unit test.

## Reproduction Steps

1. policy.json: `{"version":1,"grants":[{"capability":"allow_network","targets":["*"]}]}`
2. Authorize `allow_network connect udp://10.0.0.5:53`.
3. Before the fix: `allowed`. Expected: `denied` (private range not named literally).

## Timeline

```text
2026-09-28T10:40:00Z  defect observed (code review during implementation)
2026-09-28T10:55:00Z  fix merged in 2c3d09b, regression test green
```

## Logs and Evidence

The failing and passing test runs are listed under Verifying Tests; the fix commit is `2c3d09b`.

## Source Files

- `src/features/policy/authorize_effect.rs (host_ip)`

## Root Cause

`url::Url::host()` returns `Host::Domain(text)` for non-special schemes even when the text is an IP literal. `host_ip` only matched `Host::Ipv4/Ipv6` and the name `localhost`.

## Contributing Factors

Parallel implementation streams and a grammar driven by priorities increase the chance of interactions that only a corpus-wide test reveals.

## Resolution

`host_ip` now parses `Host::Domain` text (with or without IPv6 brackets) as an IP address before deciding the host is a name.

## Verifying Tests

- `features::policy::authorize_effect::tests::opaque_scheme_ip_literals_get_the_private_range_rule`
- `tests/conformance_udp.rs, tests/conformance_quic.rs (denied targets send zero packets)`

## Corrective Actions

Fix merged before any release; T-21 (conformance_policy_file) covers every private range per scheme.

## Preventive Actions

Policy tests enumerate every URL scheme the adapters use (http, https, ws, wss, tcp, udp, quic, grpc).

## Owners

Implementer (fix); project maintainer (review).

## Remaining Risks

Adapters must still re-check resolved addresses for host names (DNS rebinding); covered by the transport adapters and T-08/T-15.

## Lessons Learned

Standards-compliant URL parsing differs between special and non-special schemes; security predicates must not assume parsed host types.

## Related Documents

- [PLAN-2026-0001](../../plans/plan-2026-0001-rivet-v0-1-0-implementation-and-release.md)
- [PROP-2026-0001](../../proposals/implemented/prop-2026-0001-rivet-runtime.md)
- [RES-2026-0001](../../research/res-2026-0001-capy-grammar-spike.md)

## Change History

| Revision | Date | Author | Change |
|---|---|---|---|
| 1 | 2026-09-28 | Claude | Recorded and resolved. |
