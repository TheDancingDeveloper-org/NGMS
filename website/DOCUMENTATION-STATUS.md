# Website Documentation — Status & Continuation Guide

Last updated: 2026-04-13

---

## What was built

The `website/` directory was converted from a single `index.html` to an **Astro 6** static site. The existing landing page is preserved as-is in `public/index.html` (static passthrough). Three new sections were scaffolded and deployed.

### Live URLs (test.indexarr.net)

| Page | URL | Status |
|------|-----|--------|
| Landing page | `https://test.indexarr.net/` | Existing, nav updated |
| Introduction to NGMS | `https://test.indexarr.net/intro/` | ✅ Full content |
| How To — overview | `https://test.indexarr.net/how-to/` | ✅ Full content |
| How To — Installation | `https://test.indexarr.net/how-to/installation/` | ✅ Full content |
| How To — Indexers | `https://test.indexarr.net/how-to/indexers/` | ✅ Full content |
| How To — Download Clients | `https://test.indexarr.net/how-to/download-clients/` | ✅ Full content |
| How To — Quality Profiles | `https://test.indexarr.net/how-to/quality-profiles/` | ✅ Full content |
| How To — Adding Series | `https://test.indexarr.net/how-to/adding-series/` | ✅ Full content |
| How To — Adding Movies | `https://test.indexarr.net/how-to/adding-movies/` | ✅ Full content |
| How To — Import Pipeline | `https://test.indexarr.net/how-to/import-pipeline/` | ✅ Full content |
| How To — Plex Integration | `https://test.indexarr.net/how-to/plex-integration/` | ✅ Full content |
| How To — Notifications | `https://test.indexarr.net/how-to/notifications/` | ✅ Full content |
| How To — Migration | `https://test.indexarr.net/how-to/migration/` | ✅ Full content |
| How To — Common Tasks | `https://test.indexarr.net/how-to/common-tasks/` | ✅ Full content |
| Component Status | `https://test.indexarr.net/status/` | ✅ Data-driven |

---

## Directory structure

```
website/
├── astro.config.mjs          trailingSlash: 'always', outDir: './dist'
├── package.json              astro ^6.0.0
├── tsconfig.json
├── public/
│   └── index.html            landing page (static, not Astro-managed)
│                             images reference: images/NGMS_Logo.png (relative)
├── src/
│   ├── styles/
│   │   └── global.css        shared tokens, nav, buttons, callouts, code blocks
│   ├── layouts/
│   │   ├── BaseLayout.astro  HTML shell, fonts, NavBar, bg effects
│   │   └── DocsLayout.astro  sidebar + content area, docs typography, prev/next pagination
│   ├── components/
│   │   ├── NavBar.astro      top nav, activeNav prop highlights current section
│   │   └── Sidebar.astro     fixed left sidebar with grouped links, active link indicator
│   ├── data/
│   │   └── components.ts     source of truth for Component Status page — edit this to update status
│   └── pages/
│       ├── intro/index.astro
│       ├── how-to/
│       │   ├── index.astro
│       │   ├── installation.astro
│       │   ├── indexers.astro
│       │   ├── download-clients.astro
│       │   ├── quality-profiles.astro
│       │   ├── adding-series.astro
│       │   ├── adding-movies.astro
│       │   ├── import-pipeline.astro
│       │   ├── plex-integration.astro
│       │   ├── notifications.astro
│       │   ├── migration.astro
│       │   └── common-tasks.astro
│       └── status/index.astro
└── dist/                     build output — DO NOT edit directly
```

---

## Development workflow

```bash
cd website/
npm install          # first time only
npm run dev          # local dev server on :4321 (hot reload)
npm run build        # build to dist/
npm run preview      # preview built output
```

---

## Deployment

Manual deploy to Vultr (`test.indexarr.net`). Caddy serves `/root/websites/stackarr/` as the web root. Only sync the Astro-managed paths — don't touch demos/, downloads/, js/, css/ etc. which are unrelated.

```bash
cd website/
npm run build

rsync -az --delete -e "ssh -i ~/.ssh/id_ed25519" dist/_astro/ root@100.92.4.57:/root/websites/stackarr/_astro/
rsync -az --delete -e "ssh -i ~/.ssh/id_ed25519" dist/intro/  root@100.92.4.57:/root/websites/stackarr/intro/
rsync -az --delete -e "ssh -i ~/.ssh/id_ed25519" dist/how-to/ root@100.92.4.57:/root/websites/stackarr/how-to/
rsync -az --delete -e "ssh -i ~/.ssh/id_ed25519" dist/status/ root@100.92.4.57:/root/websites/stackarr/status/
rsync -az          -e "ssh -i ~/.ssh/id_ed25519" dist/index.html root@100.92.4.57:/root/websites/stackarr/index.html
```

Images are already on the server at `/root/websites/stackarr/images/` — no need to sync them.

---

## Updating the Component Status page

Edit `src/data/components.ts`. Each entry has:

```typescript
{
  name:     string,          // display name
  crate:    string,          // crate or path
  category: 'Backend' | 'Frontend' | 'Engine' | 'Integration',
  build:    'implemented' | 'partial' | 'planned',
  tests:    ('unit' | 'integration' | 'e2e' | 'untested')[],
  notes?:   string,          // optional
}
```

The page renders from this data at build time — no HTML to touch. Rebuild and redeploy after editing.

---

## Adding a new How To page

1. Create `src/pages/how-to/your-page.astro`
2. Use this template:

```astro
---
import DocsLayout from '../../layouts/DocsLayout.astro';
---
<DocsLayout
  title="Page Title"
  description="One-line description for meta tags."
  section="how-to"
  prev={{ title: 'Previous Page', href: '/how-to/previous-page/' }}
  next={{ title: 'Next Page',     href: '/how-to/next-page/' }}
>

<h1>Page Title</h1>
<p class="page-meta">Subtitle shown under the heading.</p>

<!-- content here -->

</DocsLayout>
```

3. Add the page to the sidebar in `src/components/Sidebar.astro` (the `nav` array)
4. Update prev/next links on adjacent pages

---

## Known gaps / next steps

### Content
- **Landing page** — nav was updated (added Introduction / How To / Status links) but the existing sections (features, architecture, comparison, quickstart) are unchanged. The landing page is not yet Astro-managed — it's a static `public/index.html`.
- **How To pages** — all 11 have real content covering the main workflows. Deeper detail (screenshots, edge cases, troubleshooting sections) can be added incrementally.

### Infrastructure
- **No CI/CD for the website** — deployment is a manual rsync. A Woodpecker pipeline could be added (build → rsync on push to main). Follow the pattern in `.forgejo/workflows/` from other apps.
- **Landing page conversion** — the `public/index.html` should eventually be converted to `src/pages/index.astro` so the nav is shared and maintained in one place (NavBar component). It was left as static to avoid risking the existing demos/downloads paths.
- **Mobile sidebar** — the sidebar is hidden on screens < 1024px via CSS. A hamburger menu / slide-in sidebar for mobile has not been implemented.
- **Search** — no site search. Could add Pagefind (static search, zero-dependency) with `npx pagefind --site dist` after build.
