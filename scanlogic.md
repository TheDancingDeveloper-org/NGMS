# Shadow Scan Logic

Retry/backoff state machine for the nzb-mirror shadow worker. Priority-based scheduling with graduated backoff for live pre-db, one-shot scans for user-requested and historical entries.

## Priority model

| Priority | Source | Schedule |
|----------|--------|----------|
| **P1** | Live IRC (`now - pre_epoch < 7d`) | 8 full scans graduated over ~43h |
| **P2** | User-requested (Newznab search) | Single full scan |
| **P3 (≤36h)** | Historical, recent | Full immediate + full +24h |
| **P3 (>36h)** | Historical, older | Single full scan |

All scans are Full. The Quick variant (capped group set) is removed — P1's first attempt is a Full at `pre_epoch + 10m`.

## Reaper semantics

When a row stuck in `scanning` is reaped, `scan_attempts` is **decremented** (not just status reset). A crash mid-scan should not consume a schedule slot — the same step gets re-attempted on the next claim.

## Schema additions required

```sql
ALTER TABLE shadow_releases ADD COLUMN priority smallint NOT NULL DEFAULT 3;
ALTER TABLE shadow_releases ADD COLUMN size_mismatch_count smallint NOT NULL DEFAULT 0;
CREATE INDEX idx_shadow_releases_queue
  ON shadow_releases (priority, next_retry_at)
  WHERE status IN ('pending', 'retrying');
```

## State machine

```mermaid
flowchart TD
    %% ============ Entry points ============
    subgraph Entry["Entry points"]
        IRC["IRC daemon<br/>source: irc/*"]
        UserReq["Newznab search<br/>sets user_requested_at"]
        HistImport["Historical bulk import<br/>is_historical=true"]
        NukeEvent["Nuke event<br/>from predb-irc"]
    end

    %% Priority assignment on insert
    PrioAssign{{"Assign priority<br/>P1: now - pre_epoch < 7d<br/>P2: user_requested_at set<br/>P3: is_historical"}}

    IRC --> PrioAssign
    UserReq --> PrioAssign
    HistImport --> PrioAssign

    ShadowDB[("shadow_releases<br/>+ priority<br/>+ size_mismatch_count<br/>+ status, next_retry_at, expires_at")]
    PrioAssign --> ShadowDB

    %% ============ Nuke short-circuit ============
    NukeEvent --> NukeMatch{"shadow exists<br/>for release_name?"}
    NukeMatch -->|yes| NukedTerm(["🛑 nuked"])
    NukeMatch -->|no| NukeStore["record in<br/>nuke_events only"]

    %% ============ Background sweeps ============
    subgraph Background["Background sweeps (parallel)"]
        Reaper["Reaper<br/>status=scanning &<br/>updated_at < now-300s<br/>→ retrying"]
        Expiry["Expiry sweep (300s)<br/>pre_epoch + 30d < now<br/>→ expired<br/>(skips is_historical)"]
    end

    Reaper -.recovers stuck.-> ShadowDB
    Expiry -.->|live only| ExpiredTerm(["🛑 expired"])

    %% ============ Worker ============
    Worker["Shadow Worker pool (220)<br/>ORDER BY priority ASC,<br/>next_retry_at ASC<br/>FOR UPDATE SKIP LOCKED"]
    ShadowDB --> Worker

    Worker --> Route{priority?}
    Route -->|P1| P1Sched
    Route -->|P2| P2Sched
    Route -->|P3| P3Age{"now - pre_epoch<br/>< 36h?"}
    P3Age -->|yes| P3RecentSched
    P3Age -->|no| P3OldSched

    %% ============ P1 schedule ============
    subgraph P1Sched["P1 — Live IRC (≤7d old) — all Full"]
        direction TB
        P1_1["1 · Full @ pre+10m"]
        P1_2["2 · Full +5m"]
        P1_3["3 · Full +15m"]
        P1_4["4 · Full +30m"]
        P1_5["5 · Full +1h"]
        P1_6["6 · Full +3h"]
        P1_7["7 · Full +12h"]
        P1_8["8 · Full +24h"]
        P1_1 --> P1_2 --> P1_3 --> P1_4 --> P1_5 --> P1_6 --> P1_7 --> P1_8
    end

    %% ============ P2 schedule ============
    subgraph P2Sched["P2 — User-requested"]
        P2_1["1 × Full scan"]
    end

    %% ============ P3 schedules ============
    subgraph P3RecentSched["P3 — Historical ≤36h"]
        P3R_1["Full immediate"] --> P3R_2["Full +24h"]
    end

    subgraph P3OldSched["P3 — Historical >36h"]
        P3O_1["1 × Full scan"]
    end

    %% ============ Outcome handling ============
    Outcome{{Scan outcome}}
    P1_1 & P1_2 & P1_3 & P1_4 & P1_5 & P1_6 & P1_7 & P1_8 --> Outcome
    P2_1 --> Outcome
    P3R_1 --> Outcome
    P3R_2 --> Outcome
    P3O_1 --> Outcome

    Outcome -->|articles found<br/>+ size ≥95%| Complete(["✅ complete"])
    Outcome -->|articles found<br/>size mismatch<br/>predb ≥1 MiB| SizeCap{"size_mismatch_count<br/>&lt; max (3–5)?"}
    Outcome -->|no articles| Next{"more attempts<br/>in schedule?"}

    %% Size mismatch path
    SizeCap -->|yes| SizeRetry["+30 min retry<br/>count++"]
    SizeCap -->|no| SizeMMTerm(["🛑 size_mismatch_cap"])
    SizeRetry --> ShadowDB

    %% Schedule exhaustion
    Next -->|yes| NextRetry["set next_retry_at<br/>per schedule"]
    Next -->|no| NotFoundTerm(["🛑 not_found_full"])
    NextRetry --> ShadowDB

    %% ============ Styling ============
    classDef terminal fill:#ffcdd2,stroke:#c62828,stroke-width:3px,color:#000
    classDef success fill:#c8e6c9,stroke:#2e7d32,stroke-width:3px,color:#000
    classDef bg fill:#e1f5fe,stroke:#0277bd,color:#000
    classDef retry fill:#ffe0b2,stroke:#ef6c00,color:#000
    classDef decision fill:#fff9c4,stroke:#f57f17,color:#000

    class Complete success
    class NotFoundTerm,SizeMMTerm,ExpiredTerm,NukedTerm terminal
    class Reaper,Expiry bg
    class SizeRetry,NextRetry retry
    class PrioAssign,Route,P3Age,SizeCap,Next,NukeMatch,Outcome decision
```

## Terminal states

| State | Cause |
|-------|-------|
| `complete` | Articles found, size match ≥95% — release row created |
| `nuked` | Nuke event received from predb-irc |
| `expired` | `pre_epoch + 30d < now` (live shadows only) |
| `not_found_full` | Schedule exhausted without finding articles |
| `size_mismatch_cap` | Found articles but size wrong N times (3–5) — likely wrong variant |

## Config replacement

These knobs become obsolete under the new model:

| Current | Status |
|---------|--------|
| `max_scan_attempts_cap = 5` | Removed — replaced by priority-specific schedules |
| `full_after_quick_failures = 3` | Removed — P1 does 1 quick then all fulls |
| `no_match_retry_secs = 21600` | Removed — replaced by graduated backoff |

These stay:

| Knob | Purpose |
|------|---------|
| `pre_age_min_secs = 600` | Propagation wait before first scan |
| `size_match_floor = 0.95` | Size agreement threshold |
| `size_check_min_predb_bytes = 1048576` | Skip size check for <1 MiB items |
| `size_short_retry_secs = 1800` | Size-mismatch retry interval |
| `stuck_scan_after_secs = 300` | Reaper threshold |
| `expiry_sweep_secs = 300` | Expiry sweep cadence |
| `worker_concurrency = 220` | Worker pool size |
