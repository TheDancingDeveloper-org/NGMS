# Arr API compatibility

**Current state:** no legacy arr façade is implemented on `main`.

**Target state:** settled in [the product plan](UNIFIED-ARR-PLAN.md).

StackArr's native `/api/v1` API is independent of compatibility work. Arr
compatibility is additive and is proved only by pinned contracts, golden
captures, and unmodified clients—not by similarly named native routes.

## Frozen v1 targets

| Façade | API | System-status version | Reference |
| --- | --- | --- | --- |
| Sonarr | v3 | `4.0.13.2931` | tag `v4.0.13.2931` |
| Radarr | v3 | `6.2.0.10390` | tag `v6.2.0.10390` |
| Prowlarr | v1 | `2.1.4.5212` | tag `v2.1.4.5212` (source commit `574721bfb5e5c929b1e585bd5d4d144665dd7a05`) |

Sonarr v5 is outside v1. P2 copies each OpenAPI document into `contracts/`
with its source revision, license, and SHA-256. A target changes only through a
reviewed plan and fixture update.

## Logical instances

One StackArr process exposes persisted logical Sonarr, Radarr, and Prowlarr
instances. An instance has a stable ID and slug, its own API-key hash, and
root-folder/tag/profile scope over the shared domain. A canonical path prefix is
always available; an optional dedicated listener maps to the same identity.
Multiple quality tiers therefore use multiple scoped façades, not duplicated
libraries or processes.

System-status reports the pinned upstream version above so client feature
detection is deterministic. Native `/api/v1/system/status` reports the actual
StackArr version.

## Required shared behavior

- `X-Api-Key`, `?apikey=`, and captured forms-auth cookie behavior;
- exact arr status codes, error bodies, pagination, and date/time formats;
- ordered `ProviderResource.fields[]`, including options, privacy, and hidden
  fields;
- SignalR negotiation and JSON hub queue/command events; and
- stable behavior through both the instance path prefix and dedicated listener.

Prowlarr `application` and `appprofile` resources are intentional deviations:
the shared core eliminates cross-app synchronization. Their captured
unsupported/not-found behavior is tested and published rather than silently
omitted.

## Client acceptance

| Client | Required flow | Current status |
| --- | --- | --- |
| Overseerr | Connect Sonarr/Radarr, add media, observe availability | Not implemented |
| Bazarr | Discover TV/film libraries and complete subtitle flow | Not implemented |
| Recyclarr | Read/write quality and custom-format configuration | Not implemented |
| nzb360 | Browse, search, mutate queue, trigger command, receive SignalR | Not implemented |
| Homepage/Homarr | Read health and correct media/queue counts | Not implemented |

P2 builds the conformance evidence; P3 implements reads; P4 implements writes.
The phase gates and every owning issue are in the
[canonical plan](UNIFIED-ARR-PLAN.md#6-delivery-phases-and-gates).
