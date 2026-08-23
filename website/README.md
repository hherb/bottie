# Bottie website

The public landing site for [bottie.org](https://bottie.org). It presents Bottie's current developer-preview
capabilities, trust model, and product roadmap without claiming unreleased desktop packaging.

## Local development

Requires Node.js 22.13 or newer.

```bash
npm ci
npm run dev
```

Open `http://localhost:3000`.

## Validation

```bash
npm run lint
npm test
```

The production build emits a Cloudflare Worker-compatible ESM entry point and static assets under `dist/`.

## Cloudflare deployment

The site needs no database, object storage, server-side secrets, analytics, or paid Cloudflare service. It is suitable
for the Cloudflare Workers Free plan.

1. Authenticate Wrangler with the Cloudflare account that owns `bottie.org`.
2. From this directory, run `npm run build` and then `npx wrangler deploy`.
3. The deployment configuration attaches `bottie.org` as the Worker's custom domain.
4. Optionally attach `www.bottie.org` and redirect it to the apex domain with a Cloudflare redirect rule.

The site-specific configuration is in `wrangler.jsonc`. The generated social card is `public/og.png`.

## Project shape

- `app/page.tsx` contains the single-page product story.
- `app/globals.css` contains the visual system and responsive layout.
- `app/layout.tsx` owns canonical, Open Graph, and X metadata.
- `tests/rendered-html.test.mjs` verifies the rendered content and metadata.

The product claims are derived from Bottie's root `README.md` and `ROADMAP.md`. Keep the current/future labels honest
when those documents change.
