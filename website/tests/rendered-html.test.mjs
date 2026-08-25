import assert from "node:assert/strict";
import { access } from "node:fs/promises";
import test from "node:test";

const projectRoot = new URL("../", import.meta.url);

async function render(path = "/") {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);

  return worker.fetch(
    new Request(`https://bottie.org${path}`, {
      headers: { accept: "text/html", host: "bottie.org", "x-forwarded-proto": "https" },
    }),
    { ASSETS: { fetch: async () => new Response("Not found", { status: 404 }) } },
    { waitUntil() {}, passThroughOnException() {} },
  );
}

test("server-renders the complete Bottie landing page", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>Bottie — Your context\. Your models\. Your rules\.<\/title>/i);
  assert.match(html, /Local-first AI, thoughtfully connected/);
  assert.match(html, /Bottie in action/);
  assert.match(html, /screenshots\/bottie-web-research\.webp/);
  assert.match(html, /screenshots\/bottie-memory\.webp/);
  assert.match(html, /screenshots\/bottie-email\.webp/);
  assert.match(html, /Email search and reading require a separate installation of Localmail/);
  assert.match(html, /href="https:\/\/github\.com\/hherb\/localmail"/);
  assert.match(html, /Available in the developer preview/);
  assert.match(html, /A boundary you can see/);
  assert.match(html, /Where Bottie is going/);
  assert.match(html, /NOW · DEVELOPER PREVIEW/);
  assert.match(html, /NEXT · DESKTOP BETA/);
  assert.match(html, /LATER · LOCAL VOICE/);
  assert.doesNotMatch(html, /codex-preview|Building your site|Your site is taking shape/);
});

test("emits production social metadata and assets", async () => {
  const response = await render();
  const html = await response.text();

  assert.match(html, /property="og:image" content="https:\/\/bottie\.org\/og\.png"/);
  assert.match(html, /name="twitter:card" content="summary_large_image"/);
  assert.match(html, /<link(?=[^>]*\brel="canonical")(?=[^>]*\bhref="https:\/\/bottie\.org\/")[^>]*>/);

  await Promise.all([
    access(new URL("public/og.png", projectRoot)),
    access(new URL("public/favicon.png", projectRoot)),
  ]);
});
