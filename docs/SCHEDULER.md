# Scheduler

The `stackarr-scheduler` crate runs all periodic background tasks in StackArr. It spawns one Tokio task per job, tracks their state in a shared registry, and exposes an API for manual triggers and status inspection.

Source: `crates/stackarr-scheduler/src/`

## Overview

The scheduler is created during server startup via `Scheduler::new(pool)`, configured with builder methods (`.with_managers()`, `.with_tmdb_client()`), then started with `.start()`. The `start()` method returns a `SchedulerHandle` whose lifetime controls all spawned tasks -- when the handle is dropped, the `JoinSet` goes out of scope and all tasks are cancelled.

Before spawning tasks, the scheduler queries `enabled_modules` to determine which modules are active. If no modules are enabled (first boot), only the always-on tasks (`cleanup`, `recycle_bin_cleanup`) are started. Plex-related tasks are only started when the `plex_integration` module is enabled. Tasks requiring both a download manager and indexer manager (`auto_search`, `health_check`) are only spawned when both managers are provided.

## Task Registry

`TaskRegistry` (`task_registry.rs`) is the shared state system that tracks all scheduled tasks. It uses `DashMap` for lock-free concurrent access from multiple Tokio tasks.

### TaskInfo

Each registered task is represented by a `TaskInfo` struct:

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Unique task identifier (e.g. `rss_sync`) |
| `interval_secs` | `u64` | Configured interval between runs |
| `last_run` | `Option<DateTime<Utc>>` | When the task last started |
| `last_status` | `Option<String>` | `"success"` or `"failed"` |
| `last_message` | `Option<String>` | Human-readable result or error message |
| `last_duration_ms` | `Option<u64>` | Execution time of the last run |
| `next_run` | `Option<DateTime<Utc>>` | Estimated next execution time |
| `running` | `bool` | Whether the task is currently executing |

### Registry Operations

| Method | Purpose |
|--------|---------|
| `register(name, interval_secs)` | Register a task and create its `Notify` trigger. Called once per task during setup. |
| `mark_running(name)` | Set `running = true` and record `last_run = now()`. Called at the start of each execution. |
| `mark_completed(name, success, message, duration_ms)` | Set `running = false`, record result, and calculate `next_run`. Called after each execution. |
| `list_tasks()` | Return a snapshot of all registered tasks (used by the API). |
| `trigger(name)` | Fire the task's `Notify` handle to wake it immediately. Returns `false` if the task does not exist. |
| `trigger_handle(name)` | Get the `Arc<Notify>` for a task (used during task setup). |

## Task List

### Core Tasks (require at least one enabled module)

| Task Name | Default Interval | Description |
|-----------|-----------------|-------------|
| `rss_sync` | 15 minutes | Polls all enabled RSS feeds, inserts new items, auto-downloads matches via rules and feed filters. Prunes items beyond 5000 per feed. |
| `download_sync` | 1 minute | Polls all download clients for item status, updates the `queue` table, handles stale/orphaned downloads, purges old failed items. |
| `importer` | 30 seconds | Picks up `completed` downloads from the queue, runs the import pipeline (file move/rename), records history, dispatches notifications. Retries up to 10 times on partial failure. |
| `metadata_refresh` | 12 hours | Finds stale series and movies, refreshes metadata from TMDB (overview, status, network, runtime, studio). |
| `import_list_sync` | 1 hour | Syncs all configured import lists (Plex watchlists, TMDB lists, etc.) to add new media to the library. |
| `disk_scan` | 12 hours | Scans all configured media library folders for new files on disk. Creates an activity record with progress updates. |

### Conditional Tasks (require download + indexer managers)

| Task Name | Default Interval | Startup Delay | Description |
|-----------|-----------------|---------------|-------------|
| `auto_search` | 6 hours | 2 minutes | Searches indexers for all missing monitored episodes (up to 100) and movies (up to 50). Uses the quality decision engine to evaluate and grab approved releases. Skips if another search is already running. 2-second delay between individual searches to avoid hammering indexers. |
| `health_check` | 5 minutes | 30 seconds | Tests all download clients and indexers. Tracks consecutive failures. Auto-disables services after 3 consecutive failures; automatically re-enables when they recover. Attempts to rebuild auto-disabled download clients from stored config. |

### Plex Tasks (require `plex_integration` module)

| Task Name | Default Interval | Description |
|-----------|-----------------|-------------|
| `plex_recent` | 5 minutes | Scans recently added items in Plex libraries. |
| `plex_full` | 24 hours | Full scan of all Plex libraries. |
| `plex_watchlist` | 1 hour | Syncs Plex user watchlists to StackArr. |
| `plex_token_refresh` | 12 hours | Refreshes Plex authentication tokens. |
| `availability_sync` | 24 hours | Syncs media availability status between Plex and StackArr. |

### Always-On Tasks

| Task Name | Default Interval | Description |
|-----------|-----------------|-------------|
| `cleanup` | 24 hours | Prunes activities older than 7 days and notifications older than 30 days. |
| `recycle_bin_cleanup` | 6 hours | Deletes expired recycle bin entries based on configured retention period. |

## Manual Triggers

Any registered task can be triggered immediately via the API. This wakes the task's `tokio::select!` loop without waiting for the next interval tick.

```
POST /api/v1/scheduler/tasks/{name}/trigger
```

The task name must match exactly (e.g. `rss_sync`, `health_check`, `auto_search`). See the API Endpoints section below for details.

## Task Lifecycle

### 1. Registration

During `Scheduler::start()`, each task is registered with the `TaskRegistry`:

```rust
registry.register("rss_sync", rss_dur.as_secs());
let trigger = registry.trigger_handle("rss_sync").unwrap();
```

This creates a `TaskInfo` entry and a `Notify` handle for manual triggers.

### 2. Spawn and Loop

Each task is spawned into a `JoinSet<()>` and runs an infinite loop:

```rust
join_set.spawn(async move {
    let mut tick = interval(duration);
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = trigger.notified() => {
                tracing::info!("task_name: manually triggered");
            }
        }
        // ... execute task ...
    }
});
```

The `tokio::select!` pattern means each task wakes on either:
- The interval timer firing
- A manual trigger via the `Notify` handle

### 3. Execution Tracking

Every task execution follows this pattern:

```rust
reg.mark_running("task_name");
let start = std::time::Instant::now();

match do_task_work().await {
    Ok(()) => reg.mark_completed("task_name", true, None, start.elapsed().as_millis() as u64),
    Err(e) => {
        tracing::error!(error = %e, "task failed");
        reg.mark_completed("task_name", false, Some(e.to_string()), start.elapsed().as_millis() as u64);
    }
}
```

### 4. Error Handling

Tasks never panic or exit their loop on failure. Errors are:
- Logged via `tracing::error!`
- Recorded in the registry (`last_status = "failed"`, `last_message = error text`)
- The task continues to its next scheduled execution

Some tasks have additional error handling:
- **`importer`**: Retries up to 10 times with a stale counter. After max retries, marks as failed, adds to blocklist, and dispatches a failure notification.
- **`health_check`**: Tracks consecutive failures per service. Auto-disables after 3 failures and auto-re-enables on recovery.
- **`auto_search`**: Skips execution if another search activity is already running. Logs per-release rejection reasons for diagnostics.

### 5. Shutdown

When the `SchedulerHandle` is dropped, the `JoinSet` is dropped, which cancels all spawned tasks. There is no explicit graceful shutdown -- Tokio cancels the futures at their next `.await` point.

## How to Add a New Task

### 1. Implement the task function

Create the task logic as an `async fn` returning `Result<()>`. For complex tasks, add a new module in `crates/stackarr-scheduler/src/`.

```rust
async fn my_new_task(pool: PgPool) -> Result<()> {
    // task logic here
    Ok(())
}
```

### 2. Register and spawn in `Scheduler::start()`

Add the task inside the `start()` method, following the established pattern:

```rust
// My new task (every 2 hours)
let my_dur = Duration::from_secs(2 * 3600);
let my_pool = self.pool.clone();
registry.register("my_task", my_dur.as_secs());
let reg = Arc::clone(&registry);
let trigger = registry.trigger_handle("my_task").unwrap();
join_set.spawn(async move {
    let mut tick = interval(my_dur);
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = trigger.notified() => {
                tracing::info!("my_task: manually triggered");
            }
        }
        reg.mark_running("my_task");
        let start = std::time::Instant::now();
        match my_new_task(my_pool.clone()).await {
            Ok(()) => reg.mark_completed("my_task", true, None, start.elapsed().as_millis() as u64),
            Err(e) => {
                tracing::error!(error = %e, "my task failed");
                reg.mark_completed("my_task", false, Some(e.to_string()), start.elapsed().as_millis() as u64);
            }
        }
    }
});
task_count += 1;
```

### 3. Add a configurable interval (optional)

If the interval should be user-configurable, add a field to the `Scheduler` struct and set it in `new()` and `with_intervals()`.

### 4. Conditional execution

Place the task inside the appropriate guard:
- Inside `if !enabled.is_empty()` for core tasks
- Inside `if enabled.contains(&"plex_integration".to_string())` for Plex tasks
- Inside `if let (Some(dl_mgr), Some(idx_mgr)) = ...` for tasks needing both managers
- Outside all guards for always-on tasks

### 5. Add a startup delay (optional)

For tasks that depend on other services being ready, add a delay before the first tick:

```rust
join_set.spawn(async move {
    tokio::time::sleep(Duration::from_secs(30)).await;
    let mut tick = interval(my_dur);
    // ...
});
```

## API Endpoints

Both endpoints require admin authentication (`RequireAdmin` extractor, via API key or bearer token).

### GET /api/v1/scheduler/tasks

Returns an array of all registered tasks sorted alphabetically by name.

**Response (200)**:
```json
[
  {
    "name": "rss_sync",
    "intervalSecs": 900,
    "lastRun": "2025-01-15T10:30:00Z",
    "lastStatus": "success",
    "lastMessage": null,
    "lastDurationMs": 1234,
    "nextRun": "2025-01-15T10:45:00Z",
    "running": false
  }
]
```

Returns an empty array if the scheduler is not running.

### POST /api/v1/scheduler/tasks/{name}/trigger

Manually triggers a task by name. The task wakes immediately regardless of its interval timer.

**Path parameter**: `name` -- the task name (e.g. `rss_sync`, `health_check`, `auto_search`)

**Responses**:

| Status | Body | Condition |
|--------|------|-----------|
| 200 | `{"ok": true, "message": "task 'rss_sync' triggered"}` | Task found and triggered |
| 404 | `{"error": "task 'foo' not found"}` | No task with that name |
| 503 | `{"error": "scheduler not running"}` | Scheduler has not started |

## Architecture

### Concurrency Model

```
Scheduler::start()
    |
    +-- TaskRegistry (Arc, DashMap-backed)
    |       |
    |       +-- tasks: DashMap<String, TaskInfo>
    |       +-- triggers: DashMap<String, Arc<Notify>>
    |
    +-- JoinSet<()>
            |
            +-- Task: rss_sync          (tokio::select! on interval + Notify)
            +-- Task: download_sync     (tokio::select! on interval + Notify)
            +-- Task: importer          (tokio::select! on interval + Notify)
            +-- Task: metadata_refresh  (tokio::select! on interval + Notify)
            +-- Task: import_list_sync  (tokio::select! on interval + Notify)
            +-- Task: disk_scan         (tokio::select! on interval + Notify)
            +-- Task: auto_search       (tokio::select! on interval + Notify)
            +-- Task: health_check      (tokio::select! on interval + Notify)
            +-- Task: plex_recent       (tokio::select! on interval + Notify)
            +-- Task: plex_full         (tokio::select! on interval + Notify)
            +-- Task: plex_watchlist    (tokio::select! on interval + Notify)
            +-- Task: plex_token_refresh(tokio::select! on interval + Notify)
            +-- Task: availability_sync (tokio::select! on interval + Notify)
            +-- Task: cleanup           (tokio::select! on interval + Notify)
            +-- Task: recycle_bin_cleanup(tokio::select! on interval + Notify)
```

### Key Design Decisions

- **`tokio::select!` pattern**: Each task uses `select!` over its interval timer and a `Notify` handle. This allows the API to wake any task immediately without disrupting its regular schedule.
- **`JoinSet` for lifecycle**: All tasks are spawned into a single `JoinSet`. Dropping the `SchedulerHandle` drops the `JoinSet`, which cancels all tasks.
- **`DashMap` for the registry**: Provides lock-free concurrent reads and writes. Multiple tasks can update their status simultaneously without contention.
- **`Arc<Notify>` per task**: Each task gets its own `Notify` instance so triggers are independent and targeted.
- **Lock discipline for network I/O**: Tasks that hold `RwLock<DownloadClientManager>` or `RwLock<IndexerManager>` clone/extract what they need from behind the lock, then drop the guard before performing any network I/O (which may take seconds or time out).

### SchedulerHandle

The `SchedulerHandle` struct owns the `JoinSet` and provides access to the `TaskRegistry`:

```rust
pub struct SchedulerHandle {
    _join_set: tokio::task::JoinSet<()>,
    registry: Arc<TaskRegistry>,
}
```

The web layer stores the registry in `AppState` via `ArcSwap` so it can be accessed by API routes even if the scheduler has not started yet.

### Source Files

| File | Purpose |
|------|---------|
| `lib.rs` | `Scheduler` struct, `start()` method, all task spawn logic, inline task implementations (`download_sync_task`, `importer_task`, `metadata_refresh_task`, `import_list_sync_task`, `scheduled_disk_scan`) |
| `task_registry.rs` | `TaskRegistry` and `TaskInfo` types |
| `rss.rs` | RSS feed sync logic (`rss_sync`), feed parsing, rule matching, auto-download |
| `auto_search.rs` | Missing media search (`auto_search_missing`), quality decision engine integration, `search_and_grab` |
| `health.rs` | Download client and indexer health checks, auto-disable/re-enable logic |
