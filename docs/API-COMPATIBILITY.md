# Arr API compatibility

StackArr's native `/api/v1` API is independent of the compatibility work and is
not a claim of arr compatibility. Compatibility is implemented as additive,
thin façades over the shared core.

## Pinned targets

| Façade | Target contract | Status |
| --- | --- | --- |
| Sonarr | v3 API from Sonarr v4.0.13.2931 | Not implemented |
| Radarr | v3 API from Radarr v6.2.0.10390 | Not implemented |
| Prowlarr | v1 API from the 2025-10-04 reference snapshot | Not implemented |

Sonarr v5 is explicitly out of scope for v1. Target versions change only with
an intentional contract update and reviewed golden-file diffs.

The governing wire specifications are the checked-in reference OpenAPI files.
The conformance harness will generate tests for every operation and compare
recorded responses structurally. Matching a resource name in `/api/v1` does not
count as implementing its arr counterpart.

## Required compatibility details

- `X-Api-Key` header and `?apikey=` query authentication, plus forms-auth cookie
  behavior used by legacy UIs;
- arr error response shapes and status codes;
- `ProviderResource.fields[]`, preserving option shape, privacy, visibility,
  and ordering;
- SignalR negotiation and JSON hub messages;
- deliberately selected version values from each system-status endpoint; and
- per-façade API keys with both dedicated-port and path-prefix deployment modes.

## Client support matrix

No client is supported yet. A client moves out of “Not implemented” only after
an unmodified client passes its recorded end-to-end flow.

| Client | Required flow | Status |
| --- | --- | --- |
| Overseerr | Connect both façades; add series/movie; track availability | Not implemented |
| Bazarr | Discover series/movie libraries and fetch subtitles | Not implemented |
| Recyclarr | Read and write quality/custom-format configuration | Not implemented |
| nzb360 | Browse, mutate, manage queues, receive SignalR updates | Not implemented |
| Homepage/Homarr | Read status and correct media/queue counts | Not implemented |

## Not implemented

All legacy arr façade endpoints are currently unimplemented. P2 builds the
capturing proxy, golden store, replay/diff runner, generated OpenAPI tests, and
traffic-ranked backlog. P3 and P4 then implement read and write behavior in
that measured order. See [UNIFIED-ARR-PLAN.md](UNIFIED-ARR-PLAN.md).
