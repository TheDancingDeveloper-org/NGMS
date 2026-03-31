---
name: logs
description: Query StackArr logs from Loki on Node B — NNTP debugging, download engine, general app logs
disable-model-invocation: true
allowed-tools: Bash(ssh *), Bash(curl *)
user-invocable: true
argument-hint: "[filter] [--since duration] [--limit N] [--level LEVEL]"
---

# StackArr Log Query

Query StackArr and related service logs from Loki running on Node B (192.168.0.30:3100).

## Usage

- `/logs` — Show recent StackArr logs (last 10m, 100 lines)
- `/logs nntp` — NNTP connection/auth/pool/worker logs
- `/logs usenet` — All usenet subsystem logs (NNTP + download engine + decode)
- `/logs errors` — Errors and warnings only
- `/logs auth` — Authentication-related logs (AUTHINFO, rejected, denied)
- `/logs reconnect` — Reconnection attempts and connection losses
- `/logs penalty` — Server penalty events
- `/logs pool` — Connection pool activity (acquire/release/discard/health)
- `/logs worker` — Download engine worker lifecycle
- `/logs download` — Download engine activity (articles, progress, failures)
- `/logs import` — Import/rename/post-processing logs
- `/logs scheduler` — Background task scheduler logs
- `/logs plex` — Plex integration logs
- `/logs indexer` — Indexer/search logs
- `/logs api` — HTTP API request logs
- `/logs "custom text"` — Free-text filter
- `/logs nntp --since 1h` — NNTP logs from last hour
- `/logs errors --limit 200` — Last 200 error/warning lines
- `/logs --level error` — Only ERROR level logs
- `/logs postgres` — PostgreSQL container logs

## Loki endpoint

Loki runs on Node B at `http://192.168.0.30:3100`. Promtail scrapes all Docker containers via the Docker socket.

## Container labels

| Container | Description |
|-----------|-------------|
| `stackarr` | Main StackArr application (all Rust logs) |
| `stackarr-postgres` | PostgreSQL database |
| `stackarr-indexarr` | Indexarr sidecar |

## Preset filter mappings

| Preset | LogQL filter | Description |
|--------|-------------|-------------|
| `nntp` | `\|~ "NNTP\|nzb_nntp\|Pool:\|Server:\|connect\|auth\|AUTHINFO"` | NNTP protocol layer |
| `usenet` | `\|~ "nzb_nntp\|nzb_web::download_engine\|nzb_decode\|usenet\|NNTP\|yEnc\|article\|segment"` | Full usenet subsystem |
| `errors` | `\|~ "WARN\|ERROR\|error\|panic\|failed\|FAILED\|denied\|rejected\|unavailable"` | Errors and warnings |
| `auth` | `\|~ "auth\|Auth\|AUTHINFO\|rejected\|denied\|credential\|login\|password\|481\|482\|480\|502"` | Authentication issues |
| `reconnect` | `\|~ "reconnect\|Reconnect\|connection lost\|Connection lost\|connect_with_retry\|Connect attempt\|worker exiting\|consecutive_errors"` | Reconnection activity |
| `penalty` | `\|~ "penalty\|penalize\|Penalized\|PENALTY\|blocked\|rate.limit"` | Server penalties |
| `pool` | `\|~ "Pool:\|acquire\|release\|discard\|health check\|idle\|semaphore\|permits"` | Connection pool |
| `worker` | `\|~ "Worker\|worker\|download_worker\|worker_id\|worker exiting\|Worker connected\|Worker starting"` | Worker lifecycle |
| `download` | `\|~ "download_engine\|ArticleComplete\|ArticleFailed\|JobFinished\|Download phase\|throughput\|decode\|assemble"` | Download progress |
| `import` | `\|~ "import\|Import\|rename\|deobfuscate\|post.process\|stackarr_import"` | Import/post-processing |
| `scheduler` | `\|~ "scheduler\|Scheduler\|stackarr_scheduler\|task\|cron\|RSS"` | Background scheduler |
| `plex` | `\|~ "plex\|Plex\|stackarr_plex\|library\|scan"` | Plex integration |
| `indexer` | `\|~ "indexer\|Indexer\|stackarr_indexer\|newznab\|torznab\|search\|cardigann"` | Indexer/search |
| `api` | `\|~ "stackarr_web::routes\|HTTP\|request\|response\|middleware\|api/v1"` | HTTP API layer |
| `postgres` | _(switches to container=stackarr-postgres)_ | PostgreSQL logs |

## Steps

1. Parse `$ARGUMENTS`:
   - First word: check if it matches a preset name (nntp, usenet, errors, auth, reconnect, penalty, pool, worker, download, import, scheduler, plex, indexer, api, postgres). If so, use the preset filter.
   - If it doesn't match a preset, treat the entire argument string (minus flags) as a free-text filter.
   - `--since <duration>`: time range (default `10m`). Accepts Go duration format: `5m`, `1h`, `30m`, `2h`, `24h`.
   - `--limit <N>`: max lines (default `100`).
   - `--level <LEVEL>`: filter by log level (error, warn, info, debug). Adds `|~ "<LEVEL>"` to the query.

2. If no arguments, show recent stackarr logs (100 lines, 10m).

3. Build LogQL query:
   - Default stream selector: `{container="stackarr"}`
   - For `postgres` preset: use `{container="stackarr-postgres"}`
   - For `indexarr` preset: use `{container="stackarr-indexarr"}`
   - Apply the preset's `|~` regex filter or the free-text `|= "text"` filter
   - If `--level` specified, add `|~ "<LEVEL>"` (case-insensitive match pattern like `(?i)level`)

4. Execute query:
   ```bash
   curl -sG 'http://192.168.0.30:3100/loki/api/v1/query_range' \
     --data-urlencode 'query=<logql>' \
     --data-urlencode 'limit=<N>' \
     --data-urlencode 'since=<duration>' \
     --data-urlencode 'direction=backward'
   ```

5. Format output with Python — strip ANSI codes, sort chronologically, dedup, show container prefix:
   ```bash
   | python3 -c "
   import json, sys, re
   data = json.load(sys.stdin)
   results = data.get('data', {}).get('result', [])
   lines = []
   ansi = re.compile(r'\x1b\[[0-9;]*m')
   for stream in results:
       labels = stream.get('stream', {})
       ctr = labels.get('container', '?')
       for ts, line in stream.get('values', []):
           clean = ansi.sub('', line).strip()
           lines.append((int(ts), ctr, clean))
   lines.sort()
   seen = set()
   for ts, ctr, line in lines:
       key = (ctr, line)
       if key in seen: continue
       seen.add(key)
       # Truncate very long lines but keep enough context
       display = line[:500]
       print(f'[{ctr}] {display}')
   if not lines:
       print('No results found. Try:')
       print('  - Broader time range: /logs <filter> --since 1h')
       print('  - Check containers: /logs --since 1m')
       print('  - Free text search: /logs \"your search term\"')
   "
   ```

6. If no results, suggest broadening the time range or changing filters.

## LogQL examples for common debugging scenarios

```logql
# All NNTP connection events in the last hour
{container="stackarr"} |~ "NNTP|Pool:|connect" | since=1h

# Authentication failures across all usenet servers
{container="stackarr"} |~ "auth|Auth|rejected|denied|481|482|480|502"

# Watch reconnection storms (indicates provider issues)
{container="stackarr"} |~ "reconnect|Connect attempt|consecutive_errors|worker exiting"

# Server penalties being applied
{container="stackarr"} |~ "penalty|penalize|Penalized"

# Connection pool health (are connections being reused or constantly recreated?)
{container="stackarr"} |~ "Pool:|acquire|release|discard|health check"

# Download worker lifecycle (are workers dying and restarting?)
{container="stackarr"} |~ "Worker starting|Worker connected|worker exiting|Worker FAILED"

# Article download failures
{container="stackarr"} |~ "ArticleFailed|Article not found|All retries|service unavailable"

# Full download job timeline
{container="stackarr"} |~ "Starting download engine|Download phase complete|JobFinished"

# PostgreSQL slow queries or errors
{container="stackarr-postgres"} |~ "ERROR|FATAL|slow|duration"
```

## Grafana

For interactive exploration: http://192.168.0.30:3001 (admin/admin) -> "StackArr Logs" dashboard.
