export type BuildStatus = 'implemented' | 'partial' | 'planned';
export type TestLevel  = 'unit' | 'integration' | 'e2e' | 'untested';
export type Category   = 'Backend' | 'Frontend' | 'Engine' | 'Integration';

export interface Component {
  name:        string;
  crate:       string;
  category:    Category;
  build:       BuildStatus;
  tests:       TestLevel[];
  notes?:      string;
}

export const components: Component[] = [
  // ── Backend — Media ──────────────────────────────────────────
  {
    name: 'Series Management',
    crate: 'stackarr-media',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit', 'e2e'],
  },
  {
    name: 'Movie Management',
    crate: 'stackarr-media',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit', 'e2e'],
  },
  {
    name: 'Quality Profile Scoring',
    crate: 'stackarr-quality',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit'],
  },
  {
    name: 'Release Parser',
    crate: 'stackarr-parser',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit'],
  },
  {
    name: 'Import Pipeline',
    crate: 'stackarr-import',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit'],
  },
  {
    name: 'Background Scheduler',
    crate: 'stackarr-scheduler',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit'],
  },
  {
    name: 'Auth / Users / RBAC',
    crate: 'stackarr-web + DB',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit', 'e2e'],
  },
  {
    name: 'Notifications',
    crate: 'stackarr-notify',
    category: 'Backend',
    build: 'implemented',
    tests: ['untested'],
    notes: 'Discord, Telegram, Slack, Email, Webhook — dispatch logic complete, no test coverage yet',
  },
  {
    name: 'Migration Tool (*arr)',
    crate: 'stackarr-migrate',
    category: 'Backend',
    build: 'partial',
    tests: ['untested'],
    notes: 'Sonarr + Radarr functional; Prowlarr in progress',
  },
  {
    name: 'Remote Discovery (Bootstrap)',
    crate: 'stackarr-bootstrap',
    category: 'Backend',
    build: 'partial',
    tests: ['untested'],
    notes: 'Phase 1 relay via Vultr — blocked on CI Docker build',
  },

  // ── Backend — Indexers ───────────────────────────────────────
  {
    name: 'Newznab / Torznab Search',
    crate: 'stackarr-indexer',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit'],
  },
  {
    name: 'Cardigann YAML Engine',
    crate: 'stackarr-cardigann',
    category: 'Backend',
    build: 'implemented',
    tests: ['unit'],
    notes: 'CSRF token extraction + hidden form fields added; Prowlarr parity testing via stackarr-cardigann-parity',
  },

  // ── Embedded Engines ─────────────────────────────────────────
  {
    name: 'Torrent Engine (librtbit)',
    crate: 'torrent/ (vendored)',
    category: 'Engine',
    build: 'implemented',
    tests: ['integration'],
  },
  {
    name: 'Usenet Engine (nzb-web)',
    crate: 'usenet/ (vendored)',
    category: 'Engine',
    build: 'implemented',
    tests: ['integration', 'e2e'],
  },

  // ── External Integrations ────────────────────────────────────
  {
    name: 'TMDB Metadata',
    crate: 'stackarr-metadata',
    category: 'Integration',
    build: 'implemented',
    tests: ['unit'],
    notes: 'Cached + rate-limited',
  },
  {
    name: 'Plex Integration',
    crate: 'stackarr-plex',
    category: 'Integration',
    build: 'implemented',
    tests: ['unit'],
    notes: 'Watchlist sync, scan-on-import, playback activity',
  },
  {
    name: 'Video Streaming (HLS)',
    crate: 'stackarr-stream',
    category: 'Integration',
    build: 'implemented',
    tests: ['untested'],
    notes: 'Direct play + HLS transcode via ffmpeg; no automated tests',
  },

  // ── Frontend ─────────────────────────────────────────────────
  {
    name: 'Series UI',
    crate: 'ui/pages/Series*',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Movie UI',
    crate: 'ui/pages/Movie*',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Queue UI',
    crate: 'ui/pages/Queue',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Calendar UI',
    crate: 'ui/pages/Calendar',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Search & Discover UI',
    crate: 'ui/pages/Search, Discover',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Streaming / Player UI',
    crate: 'ui/pages/Player, Stream',
    category: 'Frontend',
    build: 'implemented',
    tests: ['untested'],
    notes: 'HLS.js player + settings; no automated E2E coverage',
  },
  {
    name: 'Settings UI',
    crate: 'ui/pages/Settings',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Usenet UI',
    crate: 'ui/pages/Usenet',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Auth / Login UI',
    crate: 'ui/pages/Login, Users',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'First Boot Wizard',
    crate: 'ui/pages/FirstBoot',
    category: 'Frontend',
    build: 'implemented',
    tests: ['e2e'],
  },
  {
    name: 'Import UI',
    crate: 'ui/pages/Import',
    category: 'Frontend',
    build: 'implemented',
    tests: ['untested'],
  },
  {
    name: 'Migration UI',
    crate: 'ui/pages/Migrate',
    category: 'Frontend',
    build: 'partial',
    tests: ['untested'],
    notes: 'Mirrors backend partial status',
  },
];
