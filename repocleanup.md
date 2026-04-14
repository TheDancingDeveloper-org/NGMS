# Repo Public Release Cleanup

Audit date: 2026-04-14

---

## CRITICAL — Fix before anything else

### 1. Hardcoded usenet credentials (real passwords)

| File | Credentials |
|------|-------------|
| `NZBFailTest/src/main.rs` | Frugal: `sprooty` / `3MemP7tRt`, ViperNews: `vqx312783495` / `fkc7e4k9k2` |
| `docker/config-test.toml` | `sprooty` / `3MemP7tRt` |
| `tests/e2e/config-fresh.toml` | `sprooty` / `podoxydyg5r` |
| `tests/e2e/config-import.toml` | password `podoxydyg5r` |
| `tests/e2e/config-existing.toml` | `sprooty` / `podoxydyg5r` |
| `tests/e2e/config-ngms-test.toml` | password `podoxydyg5r` |
| `tests/e2e/config-quality-parity.toml` | password `podoxydyg5r` |

**Action**: Change those passwords now, then replace values in files with placeholders like `your_password_here`.

> **Note**: The `podoxydyg5r` and `3MemP7tRt` passwords are in git history too. Changing them in files isn't enough — you'll need to rotate those credentials and either rewrite git history (e.g. `git filter-repo`) or accept the old creds are burned.

---

## HIGH — Must clean up

### 2. Internal infrastructure IPs

`100.92.54.45` (Tailscale Forgejo/CI), `192.168.1.75` (Node B), `100.92.4.57` (Vultr) appear in:
- `Cargo.toml` — repository field
- `.woodpecker/main.yml` — deploy target, git auth URLs
- `docs/DEPLOYMENT.md`
- `IMPLEMENTATION_PLAN.md`
- `docs/phase1-user-system.md`
- `website/DOCUMENTATION-STATUS.md`

### 3. Private Forgejo registry

`repo.indexarr.net/api/packages/indexarr/cargo/` appears in:
- `Cargo.toml`
- `Cargo.lock`
- `NZBFailTest/Cargo.toml`
- `.woodpecker/main.yml`

The vendored usenet/torrent crates pull from this private registry — need a public story for these deps before going public.

### 4. Woodpecker CI pipeline (`.woodpecker/main.yml`)

Exposes full internal CI/CD:
- Tailscale IPs for Forgejo and deploy target
- SSH deploy key secret names
- Private Docker registry at `repo.indexarr.net`
- Discord webhook integration
- GitHub org push to `AusAgentSmith-org`

Either delete this file, replace with a GitHub Actions workflow, or scrub all internal references.

### 5. Personal identifiers

- Username `sprooty` in multiple test configs and source files
- GitHub org `AusAgentSmith-org` / `AusAgentSmith` in `docs/DEPLOYMENT.md`
- Comment references `Sprootyf` in `crates/stackarr-core/src/db.rs`

---

## MEDIUM — Tidy up

### 6. `NZBFailTest/` directory

Hardcoded test harness with real credentials and absolute local paths. Should be excluded entirely or heavily sanitized before public release.

### 7. Docs with internal infrastructure details

These files reference internal hostnames/IPs that need scrubbing or replacing with generic examples:
- `docs/DEPLOYMENT.md`
- `IMPLEMENTATION_PLAN.md`
- `docs/phase1-user-system.md`
- `website/DOCUMENTATION-STATUS.md`

### 8. `Cargo.lock`

Contains private registry URLs throughout. Will need to be regenerated after switching deps to public sources.

---

## Checklist

- [ ] Rotate Frugal usenet password (`3MemP7tRt` / `podoxydyg5r`)
- [ ] Rotate ViperNews/NGD usenet password (`fkc7e4k9k2`)
- [ ] Replace all hardcoded passwords in test configs with placeholders
- [ ] Rewrite git history or document that old creds are burned
- [ ] Remove/replace `NZBFailTest/` directory
- [ ] Scrub internal IPs from all docs and config files
- [ ] Replace `.woodpecker/main.yml` with a public CI workflow (or delete it)
- [ ] Update `Cargo.toml` repository field to public GitHub URL
- [ ] Decide on public strategy for private registry deps (vendor, publish publicly, or document)
- [ ] Remove `sprooty` username and `AusAgentSmith` org references
- [ ] Regenerate `Cargo.lock` after dep changes
