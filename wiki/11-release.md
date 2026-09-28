# Release Process

Every release must be fully documented, tested, validated, and traceable from the original implementation plan through the final release documentation.

## 1. Documentation Standard

**ALL documents MUST follow the requirements defined in `DOCUMENTATION.md`.**

Every document must preserve enough information to reconstruct why a change was made, how it was implemented, and how it was verified.

Where applicable, documents must include:

* Document date and last-updated date
* Current software/version information
* Related plan, release, incident, troubleshooting, test, demo, manual, and system documents
* Relevant source files and code locations
* Relevant code snippets
* Commands, APIs, configuration, or environment details
* External references and URLs
* Decisions made and their reasoning
* Known limitations and unresolved issues
* Verification evidence

Documentation must be **cross-referenced** so that a reader can trace a release backward from:

`Release → Tests → Implementation → Plan`

and sideways into:

`Incidents / Troubleshooting / Demo / Manual / System Documentation`

---

# 2. Release Workflow

A release MUST follow this workflow:

```text
Plan
  │
  ▼
Implementation
  │
  ├──► Incident Documentation
  │       if unexpected bugs are discovered
  │
  ├──► Troubleshooting Documentation
  │       if difficult, unusual, or unresolved
  │       technical problems are encountered
  │
  ▼
Testing
  │
  ▼
Validation
  │
  ▼
Demo
  │
  ▼
Manual Documentation
  │
  ▼
System Documentation
  │
  ▼
Release Documentation
```

A release is not considered complete until every applicable stage has been completed.

---

# 3. Implement an Approved Plan

Every release MUST originate from a plan in the `plans/` directory.

Before implementation:

1. Identify the plan being implemented.
2. Confirm its expected changes, requirements, affected components, and expected results.
3. Use the plan as the primary implementation reference.
4. Keep the plan linked to all resulting documentation.

Implementation work should remain traceable to specific plan requirements whenever possible.

If implementation differs materially from the original plan, document:

* What changed
* Why it changed
* What impact the deviation has
* Whether the plan itself needs to be updated

---

# 4. Record Incidents During Implementation

If an unexpected bug, regression, failure, or abnormal system behavior is discovered during implementation, a document MUST be created or updated under:

```text
incidents/
```

An incident should capture at minimum:

* What happened
* When it was discovered
* How it was discovered
* Affected components
* Reproduction steps
* Relevant logs/errors
* Relevant source files
* Root cause, if known
* Fix or mitigation
* Tests used to verify the fix
* Remaining risks or unresolved questions
* Related plan/release documents

Do not hide significant bugs inside implementation notes or commit messages.

---

# 5. Record Troubleshooting Knowledge

If implementation involves a problem that:

* Was difficult to diagnose
* Required significant investigation
* Had a non-obvious solution
* Required experimentation
* Was caused by an unusual environment/configuration issue
* Could reasonably happen again
* Could not be fully solved
* Revealed useful technical knowledge

then document it under:

```text
troubleshooting/
```

Troubleshooting documents should preserve reusable knowledge rather than only describing that a problem occurred.

Include:

```text
Problem
↓
Symptoms
↓
Investigation
↓
Possible Causes
↓
Experiments / Attempts
↓
Root Cause
↓
Solution / Workaround
↓
Verification
↓
Remaining Limitations
```

If the problem remains unresolved, clearly mark its current status and reference it from the release document.

---

# 6. Test the Implementation

After implementation is complete, all affected functionality MUST be tested.

Testing should cover, where applicable:

* New functionality
* Modified functionality
* Existing functionality affected by the change
* Regression scenarios
* Error handling
* Edge cases
* API behavior
* CLI behavior
* Configuration changes
* Integration behavior
* Backward compatibility
* Upgrade/migration behavior

Test evidence must be preserved in the appropriate test documentation.

Tests should link back to the requirements they validate.

Example:

```text
PLAN-001 Requirement R3
        │
        ▼
Implementation
        │
        ▼
TEST-014
        │
        ▼
PASS
```

A feature should not be considered complete merely because the code compiles or the primary path works.

---

# 7. Validate the Result Against the Plan

After testing, explicitly validate the implementation against the original plan.

For every important requirement, determine:

```text
PASS
PARTIAL
FAIL
NOT APPLICABLE
```

Validation must answer:

* Was the planned functionality actually implemented?
* Does it behave as expected?
* Were all expected files/components modified?
* Did implementation introduce unintended behavior?
* Are there remaining limitations?
* Are there unresolved incidents?
* Are follow-up changes required?

Any `PARTIAL` or `FAIL` result must be documented and referenced from the final release.

---

# 8. Create Demo Documentation

Once the implementation has passed testing and validation, create or update the relevant files under the demo documentation.

The demo must show how the new or modified functionality is actually used.

Include all newly introduced or changed interfaces such as:

* CLI commands
* API endpoints
* Request/response formats
* Configuration
* Environment variables
* UI workflows
* SDK functions
* Scripts
* Operational commands
* Migration commands

The demo must contain reproducible examples.

For example:

```bash
vhco example create --name demo
```

Expected result:

```text
Created example: demo
```

For an API:

```http
POST /api/examples
Content-Type: application/json

{
  "name": "demo"
}
```

Expected response:

```json
{
  "id": "...",
  "name": "demo"
}
```

The demo should not merely describe the feature.

**The documented demo must be executed or otherwise verified to ensure that the instructions actually work.**

---

# 9. Update Manual Documentation

After the demo is complete, create or update the relevant documentation under:

```text
manual/
```

The manual should provide deeper user-facing or developer-facing explanations of the functionality.

Unlike the demo, which focuses on quickly proving and demonstrating usage, the manual should explain:

* Purpose
* Concepts
* Installation/setup
* Configuration
* Usage
* Commands/APIs
* Parameters
* Expected behavior
* Examples
* Common workflows
* Edge cases
* Limitations
* Troubleshooting references
* Related features

Update existing manual documents rather than creating duplicate documentation when the feature logically belongs to an existing section.

---

# 10. Update System Documentation

After user-facing documentation is updated, synchronize the relevant documentation under:

```text
system/
```

System documentation must accurately describe the **current implemented system**, not merely the intended architecture.

Update or create documentation covering affected areas such as:

* Architecture
* Components
* Services
* Packages
* Modules
* Data flow
* APIs
* Storage
* Configuration
* Runtime behavior
* Dependencies
* Deployment
* Security boundaries
* External integrations
* Internal interfaces

You may use:

```bash
vhco doc
```

to perform a broader scan of the codebase and help compare the actual implementation against the existing system documentation.

The purpose is to detect documentation drift.

After the scan, verify that:

```text
Actual Code
    ≈
System Documentation
```

Any meaningful mismatch should either be corrected or explicitly documented.

---

# 11. Create the Release Document

Only after implementation, testing, validation, demos, manuals, and system documentation are complete should the final release document be created.

The release document must include the release version and act as the central traceability document for the release.

It should reference all relevant artifacts, including:

* Original plan
* Implementation-related documents
* Tests
* Validation results
* Incidents
* Troubleshooting documents
* Demo documents
* Manual updates
* System documentation updates
* Relevant source files
* Relevant commits, tags, or branches where available

The release document should clearly summarize:

## Added

New functionality introduced.

## Changed

Existing functionality that was modified.

## Fixed

Bugs or defects resolved.

## Removed

Functionality that was removed or deprecated.

## Testing

What was tested and the resulting status.

## Documentation

Which demos, manuals, and system documents were created or updated.

## Incidents

Unexpected issues discovered during development and their final status.

## Troubleshooting

Important technical problems encountered and the resulting knowledge or solutions.

## Known Issues

Problems that remain unresolved.

## Limitations

Known functional or technical limitations.

## Follow-up Work

Work intentionally deferred to a future plan or release.

---

# 12. Release Traceability

The final release should make it possible to navigate the entire development history.

Example:

```text
RELEASE-1.4.0
│
├── Plan
│   └── PLAN-024
│
├── Tests
│   ├── TEST-081
│   ├── TEST-082
│   └── TEST-083
│
├── Incidents
│   └── INC-017
│
├── Troubleshooting
│   └── TROUBLE-009
│
├── Demo
│   └── DEMO-024
│
├── Manual
│   ├── CLI Manual
│   └── API Manual
│
├── System
│   ├── API Architecture
│   ├── Runtime Architecture
│   └── Configuration System
│
└── Source
    ├── src/...
    ├── internal/...
    └── config/...
```

The release document therefore serves as the **root evidence record for everything included in that version**.

---

# 13. Release Completion Gate

A release MUST NOT be marked complete until the following conditions are satisfied:

```text
[ ] An approved plan exists in plans/
[ ] Implementation corresponds to the plan
[ ] Unexpected bugs were documented in incidents/
[ ] Significant technical problems were documented in troubleshooting/
[ ] Relevant tests were completed
[ ] Implementation was validated against plan requirements
[ ] Demo documentation was created or updated
[ ] Demo commands/APIs/examples were verified
[ ] Relevant manual documentation was updated
[ ] Relevant system documentation was updated
[ ] Code and system documentation were checked for consistency
[ ] Known issues and limitations were documented
[ ] Final release document was created
[ ] Release document links all relevant artifacts
[ ] Version information is correct
```

Only after this gate passes should the release be considered complete.

---

# Core Principle

The objective is not simply to produce documentation after coding.

The objective is to maintain a traceable chain of evidence:

```text
Why was this needed?
        │
        ▼
What was planned?
        │
        ▼
What was implemented?
        │
        ▼
What problems occurred?
        │
        ▼
How were they solved?
        │
        ▼
How was the implementation tested?
        │
        ▼
Did it satisfy the original requirements?
        │
        ▼
How can someone use it?
        │
        ▼
How does it affect the system?
        │
        ▼
What exactly was released?
```

A person or agent reviewing the repository months or years later should be able to reconstruct this entire chain without relying on undocumented knowledge.

## Release Versioning and Git Traceability

Every release MUST be tied to an exact version of the source code.

Before the release is finalized, the following MUST be completed:

```text
Update Version
      │
      ▼
Commit Release State
      │
      ▼
Record Commit Hash
      │
      ▼
Create Git Tag
      │
      ▼
Verify Tag → Commit
      │
      ▼
Finalize Release Document
```

### 1. Update the Software Version

Update the canonical version of the project before creating the release.

Examples may include:

```text
VERSION
package.json
pyproject.toml
Cargo.toml
pubspec.yaml
go.mod / build metadata
application configuration
```

There MUST be one clearly defined authoritative version source.

Example:

```text
1.7.0
```

All generated documentation and release information must use the same version.

If multiple files contain version information, verify that they are synchronized.

---

### 2. Commit the Final Release State

After:

* implementation
* testing
* validation
* demo verification
* manual updates
* system documentation updates
* version update

commit the complete release state to Git.

The release commit SHOULD contain all files required for the release so that checking out that commit reproduces the documented release state.

Example:

```bash
git add .
git commit -m "release: v1.7.0"
```

The exact commit hash MUST then be recorded.

Example:

```bash
git rev-parse HEAD
```

Result:

```text
8f1c42d73c5277c194e301ef7f03ea9de2639aad
```

Prefer storing the full commit SHA rather than only the shortened hash.

---

### 3. Create the Git Tag

Every official release MUST have a Git tag corresponding to the release version.

Recommended format:

```text
v<MAJOR>.<MINOR>.<PATCH>
```

Example:

```text
v1.7.0
```

Recommended command:

```bash
git tag -a v1.7.0 -m "Release v1.7.0"
```

Then verify that the tag points to the intended release commit:

```bash
git rev-list -n 1 v1.7.0
```

The returned commit hash MUST match the commit recorded in the release document.

Push the commit and tag when the repository uses a remote:

```bash
git push origin <branch>
git push origin v1.7.0
```

---

# Release Identity

Every release document MUST prominently include a release identity section.

Example:

```text
Release Version: 1.7.0
Git Tag: v1.7.0
Git Commit:
8f1c42d73c5277c194e301ef7f03ea9de2639aad

Release Date: 2026-08-11
Source Branch: main
Previous Version: 1.6.2
Previous Tag: v1.6.2
```

Where applicable, also include:

```text
Repository
Release branch
Previous release commit
Build identifier
CI/CD run
Artifact checksums
Deployment version
Container image digest
```

This information establishes exactly which source state corresponds to the release.

---

# Updated Release Workflow

```text
Plan
  │
  ▼
Implementation
  │
  ├──► Incidents
  │
  └──► Troubleshooting
  │
  ▼
Testing
  │
  ▼
Validation
  │
  ▼
Demo
  │
  ▼
Manual Documentation
  │
  ▼
System Documentation
  │
  ▼
Update Version
  │
  ▼
Final Release Commit
  │
  ▼
Record Commit Hash
  │
  ▼
Create Git Tag
  │
  ▼
Verify Tag → Commit
  │
  ▼
Release Documentation
```

---

# Release Document

The final release document MUST include:

## Release Identity

* Version
* Git tag
* Full Git commit hash
* Release date
* Source branch
* Previous version
* Previous Git tag
* Previous release commit, where available

## Plan

Reference the plan that initiated the work.

## Added

New functionality.

## Changed

Modified functionality.

## Fixed

Resolved defects.

## Removed

Removed or deprecated functionality.

## Tests

Tests executed and their results.

## Validation

Plan requirements and their final status:

```text
PASS
PARTIAL
FAIL
NOT APPLICABLE
```

## Incidents

Incidents discovered during implementation and their resolution status.

## Troubleshooting

Important technical investigations and reusable findings.

## Demo

Demo documents and verified commands/APIs/workflows.

## Manual

Manual documents created or updated.

## System

System/architecture documents created or updated.

## Source Changes

Important source files changed.

Where useful:

```text
| Source | Change | Reason |
|---|---|---|
| internal/api/server.go | Modified | Add new endpoint |
| config/settings.go | Modified | Add configuration option |
| cmd/vhco/main.go | Modified | Add CLI command |
```

## Known Issues

Anything remaining unresolved.

## Limitations

Known limitations of the release.

## Follow-up Work

Work deferred to later plans/releases.

---

# Git Verification

Before considering a release complete, verify:

```bash
git status
```

The release working tree SHOULD be clean.

Verify the current commit:

```bash
git rev-parse HEAD
```

Verify the tag:

```bash
git describe --tags --exact-match HEAD
```

Expected result:

```text
v1.7.0
```

Verify that the tag resolves to the same commit:

```bash
git rev-list -n 1 v1.7.0
```

Conceptually:

```text
Release Document
      │
      │ version = 1.7.0
      │ tag     = v1.7.0
      │ commit  = 8f1c42...
      ▼
   Git Tag
   v1.7.0
      │
      ▼
Git Commit
8f1c42...
      │
      ▼
Exact Source State
```

The release MUST NOT reference a tag and commit that point to different source states.

---

# Updated Release Completion Gate

```text
[ ] Approved plan exists in plans/

[ ] Implementation corresponds to the plan

[ ] Unexpected bugs are documented in incidents/

[ ] Significant or reusable technical problems are documented
    in troubleshooting/

[ ] Relevant tests were completed

[ ] Implementation was validated against plan requirements

[ ] Demo documentation was created or updated

[ ] Demo commands/APIs/examples were actually verified

[ ] Relevant manual documentation was updated

[ ] Relevant system documentation was updated

[ ] Code and system documentation were checked for consistency

[ ] Canonical project version was updated

[ ] All version references are synchronized

[ ] Final release state was committed

[ ] Working tree is clean

[ ] Full release commit SHA was recorded

[ ] Git release tag was created

[ ] Git tag matches the release version

[ ] Git tag resolves to the recorded release commit

[ ] Commit/tag were pushed to the remote, when applicable

[ ] Known issues and limitations are documented

[ ] Final release document was created

[ ] Release document contains version, tag and commit hash

[ ] Release document links all relevant artifacts

[ ] Release document accurately represents the tagged source state
```

# Core Traceability Rule

The complete release chain should be:

```text
Plan
 ↓
Implementation
 ↓
Tests
 ↓
Validation
 ↓
Demo
 ↓
Manual
 ↓
System Documentation
 ↓
Version
 ↓
Commit
 ↓
Git Tag
 ↓
Release
```

And the inverse lookup must also be possible:

```text
Release v1.7.0
      │
      ├── Git Tag: v1.7.0
      │
      ├── Commit: 8f1c42...
      │
      ├── Plan
      │
      ├── Tests
      │
      ├── Incidents
      │
      ├── Troubleshooting
      │
      ├── Demo
      │
      ├── Manual
      │
      ├── System Docs
      │
      └── Source Changes
```

Someone reviewing a release months or years later must be able to identify the **exact source commit that produced that release**, understand why it exists, see what changed, reproduce its demos/tests, and trace every important decision back to its supporting documentation.
