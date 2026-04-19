# nzb-web

Download orchestration engine for the `nzb-*` usenet crate stack.

Provides the queue manager (job lifecycle state machine with SQLite persistence), download engine (per-job orchestrator), background services (RSS monitor, directory watcher, speed tracker), and an Axum-based REST API with SABnzbd-compatible endpoints.

## Features

- Job lifecycle management: Queued → Downloading → Verifying → Repairing → Extracting → Completed/Failed
- Multi-server download with priority-based failover
- SABnzbd-compatible API (drop-in for Sonarr/Radarr/etc.)
- RSS feed monitoring with regex filtering
- Watch directory for auto-enqueue
- Newsgroup browsing with full-text search

## Part of the nzb-* stack

| Crate | Role |
|-------|------|
| [nzb-nntp](https://crates.io/crates/nzb-nntp) | Async NNTP client, connection pool |
| [nzb-core](https://crates.io/crates/nzb-core) | Shared models, config, SQLite DB |
| [nzb-decode](https://crates.io/crates/nzb-decode) | yEnc decode + file assembly |
| [nzb-news](https://crates.io/crates/nzb-news) | NNTP fetch engine |
| [nzb-dispatch](https://crates.io/crates/nzb-dispatch) | Article dispatcher, retry, hopeless tracking |
| [nzb-postproc](https://crates.io/crates/nzb-postproc) | PAR2 repair, archive extraction |
| **nzb-web** | Queue manager, download orchestration (this crate) |

## License

MIT
