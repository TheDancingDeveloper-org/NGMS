# SABnzbd vs StackArr Usenet Engine Comparison

## Overview

Comparison of how SABnzbd (Python, mature) and StackArr's embedded nzb engine (Rust) handle the usenet download process. Based on source code review of both codebases.

## Circuit Breaker / Server Failure Handling

### SABnzbd

Implements a **penalty-based circuit breaker** with automatic recovery timers. When errors occur, the server is disabled temporarily via `plan_server()`.

**Penalty Schedule** (`downloader.py`):
| Error | Penalty | Notes |
|-------|---------|-------|
| Unknown 502 | 5 min | `_PENALTY_502` |
| Connection timeout | 10 min | `_PENALTY_TIMEOUT` |
| Account sharing (multi-IP) | 10 min | `_PENALTY_SHARE`, permanent block |
| Too many connections | 10 min | `_PENALTY_TOOMANY` |
| Bad credentials (452, 481, 482) | 10 min | `_PENALTY_PERM`, permanent block |
| Unspecified 400 errors | 6 sec | `_PENALTY_VERYSHORT` |

**Error Classification**: Inspects error message keywords ("connections", "exceed", "limit") to distinguish "too many connections" from "bad credentials" — both may return 502 but require different responses.

**Required vs Optional**: Required servers are never disabled; articles queue up waiting. Optional servers are deactivated and scheduled for re-enabling after penalty.

**Automatic Recovery**: Timer scheduled via Scheduler to re-enable servers after penalty period. `trigger_server()` reinitializes the server when the timer fires.

### StackArr (after circuit breaker fix)

Implements a **threshold-based circuit breaker** per server, shared across all workers in a job.

**Circuit Break Rules**:
| Error Type | Threshold | Cooldown |
|------------|-----------|----------|
| Auth failures (481, 482) | Immediate (1st failure) | 120s |
| Service unavailable (502) | Immediate (1st failure) | 120s |
| Transient failures | 3 consecutive | 30s |

**Key Behavior**: When one worker circuit-breaks a server, all other workers for that server check the shared health map and exit immediately — preventing the N-worker retry storm.

**No required/optional distinction yet** — the `optional` field exists on `ServerConfig` but isn't used by the circuit breaker.

## Connection Management

### SABnzbd

- **Fixed thread pool per server**: `busy_threads` / `idle_threads` sets, one `NewsWrapper` per configured connection
- **No dynamic resizing**: When "too many connections" detected, disables server entirely rather than reducing thread count
- **Article prefetching**: Only 20 articles prefetched into server queues at a time
- **Hard reset on failure**: Socket closed, SSL info cleared, article discarded or returned to queue

### StackArr

- **Workers spawned at job start**: One tokio task per connection slot per server
- **Connect gate**: Global per-host rate limiter — max 5 concurrent handshakes, 100ms spacing between SYNs
- **Worker stagger**: 50ms delay between worker startups to avoid thundering herd
- **Shared work queue**: All workers pull from a single `Mutex<VecDeque<WorkItem>>`

## Server Priority and Article Failover

### SABnzbd

- **Static priority ordering**: Servers sorted by priority (0 = highest), then by name
- **Per-article `TryList`**: Tracks which servers attempted, with priority-aware failover
- **Priority-aware retry**: When a higher-priority server comes back online, try lists are reset so articles can be retried there
- **Prefetch gating**: Articles only assigned to a server if no higher-priority server is available and hasn't been tried

### StackArr

- **Static priority ordering**: Servers sorted by priority at job start
- **Per-article `tried_servers: Vec<String>`**: Tracks which servers have been attempted
- **Simple failover**: Article re-queued for other servers when not found (430) or decode error
- **Circuit-breaker-aware**: Articles count circuit-broken servers as "tried" to avoid waiting for unreachable servers

## Config Changes on Running Downloads

### SABnzbd

- **Graceful restart**: Marks server for restart (`server.restart = True`)
- **Drains in-flight**: Stops assigning new articles, waits for busy threads to complete
- **Then reinitializes**: Removes old server, creates new one with updated config
- **No article loss**: In-flight articles complete on old config; queued articles available to new server

### StackArr

- **Fresh config on reconnect**: Workers read current config from shared server list when reconnecting
- **Credential changes take effect on next reconnect** (not immediately on existing connections)
- **No restart mechanism**: Running connections keep old config until they naturally disconnect
- **New servers not used by running jobs**: Only available for the next job

## Queue Visibility

### SABnzbd

- **Articles stay in NzbObject always**: Never removed from the job, only prefetched into server queues
- **Transitions visible**: Job stays in queue through all phases
- **History only after cleanup**: Job moves to history only after all post-processing and file moves complete

### StackArr

- **In-memory jobs map**: Jobs live in `HashMap<String, JobState>` during active lifecycle
- **Fast completion race**: Small downloads could go Queued → History faster than UI poll interval
- **Fix**: Completed jobs now kept visible for 8 seconds before removal from jobs map

## Key Takeaways for Future Work

### Things SABnzbd does better (potential improvements):

1. **Keyword-based error classification** — Distinguishing "too many connections" from "bad credentials" when both return 502. Different responses needed: reduce connections vs disable server.

2. **Required server concept** — Required servers never disabled; articles wait. Prevents permanent failure when the only server has a transient issue.

3. **Graceful config restart** — Wait for in-flight to drain before applying changes. More correct than our "fresh config on reconnect" approach.

4. **Per-error penalty tuning** — Mature penalty periods tuned per error class over years of production use.

5. **Priority-aware try list reset** — When a higher-priority server recovers, article try lists are reset so articles get retried on the better server.

### Things StackArr does well:

1. **Shared circuit breaker across workers** — One worker's failure immediately stops all workers for that server. SABnzbd has independent threads.

2. **Connect gate (global rate limiter)** — Per-host semaphore + pacing prevents SYN storms. SABnzbd has no equivalent.

3. **Worker stagger** — Spreads initial connections over time. SABnzbd opens all threads at once.

4. **Async pipeline support** — Pipelined NNTP commands for higher throughput. SABnzbd uses serial request/response per thread.

5. **yEnc deobfuscation** — Intelligent filename resolution from yEnc headers vs NZB subjects.
