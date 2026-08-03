# NGMS handoff and AI harness issue register

**Date:** 2026-08-03  
**Status:** AI execution paused after worker-lifecycle smoke test.
**Scope:** `docs/2026-08-02-ngms-unified-arr-handover.md`, the `feat/mariadb-baseline`
checkout, and the connected AIDevEnv/agent-harness runtime.

This is an evidence register, not a claim that the items below are resolved.
No credentials or token values are included.

## NGMS handoff issues

### T20 schema approval is still a gate — blocking

Issue T20 (#58) remains open. The MariaDB baseline and the dependent T21–T26
work are present in the rescue branch, but the handoff explicitly requires
human acceptance of the target schema. The queue is therefore paused rather
than treating the generated baseline as approved.

### The rescue branch bundles unrelated handoff items — high

The local branch contains three rescue commits spanning multiple task IDs
(including T21–T25 and the runtime/CI/docs layers). This makes review, rollback,
and per-issue acceptance ambiguous. The changes should be split into reviewable
commits or PRs after T20 is accepted.

### T22 removed the embedded database provisioner without a replacement decision — high

The former `stackarr-postgres` crate was an embedded PostgreSQL provisioner,
not merely a query layer. It was reduced to a `stackarr-mariadb` stub while
`docker/Dockerfile.standalone` still references the removed `managed-postgres`
feature. This silently changes the self-provisioning product behavior. D8
(#29) must decide how MariaDB is delivered before standalone packaging is
considered complete.

### T19 CI requirements are incomplete — high

The handoff requested conformance, a coverage ratchet, MariaDB-backed tests,
and multi-architecture output. The branch originally supplied only the MariaDB
service and ignored-test invocation. The current working-tree edit adds a
conformance gate and `linux/amd64,linux/arm64` Docker build, but coverage
ratcheting is still absent and musl is not represented by a Docker platform.
The referenced `coverage-watchdog` is a monitoring web application, not an
existing Rust coverage-ratchet command. A separate coverage-tool decision and
baseline are required.

The organization runner policy requires explicit self-hosted labels; all NGMS
jobs in the current working tree now use those labels. The handoff text that
asks for `ubuntu-latest` is superseded by that workspace policy.

### Database tests are not locally verified against MariaDB — medium

The Rust workspace gates passed locally, but the 31 ignored database tests were
not run against a live MariaDB because this environment has no Docker daemon or
MariaDB service. CI must provide and successfully exercise the service before
the database migration can be called verified.

## Harness and AIDevEnv issues

### Project start can report running with zero workers — high

In `agent-harness/src/agent_harness/api.py`, `POST /api/projects/{id}/start`
sets queue control to `RUNNING` when `app.state.fleet` is `None`. The returned
summary then reports zero workers. This can look like successful execution
while no worker can claim work. Start should fail clearly, or require a live
fleet, instead of creating a false-running state. A regression test is needed.

**Tracked:** [agent-harness #85](https://github.com/TheDancingDeveloper-org/agent-harness/issues/85).

### Harness preflight does not prove an executable configuration — high

The NGMS queue contained 49 pending items but no usable model/reviewer route or
GitHub write credential in this runtime. Project registration/start does not
preflight the executor, reviewer, helper, repository write access, or required
checks. A queue can therefore be resumed into a nonproductive or misleading
state. Add an explicit preflight/readiness gate before admission or worker
claiming.

### Feature AIDevEnv has no credential broker mounted — high

The running `aidevenv-feat` stack is configured with
`AIDEVENV_AGENT_AUTH_REQUIRED=0` and `AIDEVENV_AUTO_AGENT_AUTH=0`; no
`aidevenv-agent-auth` executable or Infisical machine identity is present in
the container. The shared MyDevEnv2 helper source exists, but its required
runtime identity and `infisical` CLI are not available here. GitHub issue/PR
writes consequently cannot be authenticated from this session.

**Status update:** resolved in the running environment on 2026-08-03. The
broker is now installed with a machine identity and its `check` command proves
Infisical authentication plus destination GitHub-admin access. This item remains
here as the reason the earlier execution attempt was paused; it is not an open
defect to file.

### Broker status is not a readiness check — medium

The AIDevEnv status surface reports the configured helper name and whether
auto-auth is enabled, but not whether the helper executable exists, whether
Infisical identity variables are present, or whether `check` succeeds. It can
therefore advertise a configured broker while the session is credential-free.

**Status update:** already fixed in the current `aidevenv` `feat` branch. Its
status model now distinguishes helper presence, identity presence, configured,
verified, ready, and a safe failure reason. No duplicate issue was filed.

### Harness test dependency mismatch — medium

The agent-harness project declares `httpx>=0.27` for development tests. In this
runtime, the installed Starlette TestClient requires the separately named
`httpx2` package, so `tests/test_projects.py` fails during fixture setup before
endpoint behavior is tested. The supported Python/dependency matrix should be
pinned and tested in CI.

### Worker exit leaves a live session and claim — critical

The first real Fleet smoke test claimed T19, then the worker process exited
while the AIDevEnv session remained live and the queue row stayed claimed. The
row was re-queued through the queue API and the project stopped to prevent
repeated unattended attempts.

**Tracked:** [agent-harness #87](https://github.com/TheDancingDeveloper-org/agent-harness/issues/87).

### Work-list item identifiers serialize as null — high

`GET /api/work?project_id=default` returned `id: null` for every row even
though the same rows can be addressed by their canonical IDs through
`GET /api/work/{item_id}`.

**Tracked:** [agent-harness #88](https://github.com/TheDancingDeveloper-org/agent-harness/issues/88).

### No supported operator transition for human-decision rows — high

The queue has a `blocked` state, but the API exposes no authenticated operator
action to mark a decision item blocked with a reason. D8 and D9 were marked
blocked through the queue abstraction as data cleanup so an implementation
worker cannot answer them.

**Tracked:** [agent-harness #89](https://github.com/TheDancingDeveloper-org/agent-harness/issues/89).

## Current execution state

- The AIDevEnv queue is **stopped** with 40 pending, 9 blocked, 0 running, 0
  done, and 0 failed items. D8/D9 and the seven epic rows are blocked with
  explicit cleanup reasons; T19 was re-queued after the worker exit.
- The NGMS checkout is clean on `feat/mariadb-baseline`; the partial T19
  changes are committed and pushed.
- `just` is not installed in this AIDevEnv container, so the new CI
  `just conformance` command could not be run locally. The current placeholder
  recipe is inspectable, but CI or a provisioned development image must execute
  it before it is claimed as validated.
- No secret values were printed, committed, or written to this document.
- Resume only after the worker-lifecycle defect is fixed upstream (or a
  verified replacement executor is deployed), and after T20 approval before
  schema-dependent work runs.
