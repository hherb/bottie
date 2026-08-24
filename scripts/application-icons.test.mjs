import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, it } from "vitest";

import {
  APPLICATION_ICON_SOURCE,
  DESKTOP_ICON_PNG_SIZES,
  FAVICON_SOURCE,
  canonicalizeIcns,
  inspectPng,
  tauriIconArguments,
  verifyCheckedInApplicationIcons,
} from "./application-icons.mjs";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

describe("application icon assets", () => {
  it("uses the approved source and a locked desktop-only generation boundary", () => {
    assert.equal(APPLICATION_ICON_SOURCE, "assets/bottie-logo-kit/bottie-icon-512.png");
    assert.equal(FAVICON_SOURCE, "assets/bottie-logo-kit/favicon-64.png");
    assert.deepEqual(tauriIconArguments("/tmp/bottie-icons"), [
      "icon",
      APPLICATION_ICON_SOURCE,
      "--output",
      "/tmp/bottie-icons",
    ]);
  });

  it("declares every required transparent desktop PNG size", () => {
    assert.deepEqual(DESKTOP_ICON_PNG_SIZES, {
      "32x32.png": 32,
      "64x64.png": 64,
      "128x128.png": 128,
      "128x128@2x.png": 256,
      "icon.png": 512,
      "Square30x30Logo.png": 30,
      "Square44x44Logo.png": 44,
      "StoreLogo.png": 50,
      "Square71x71Logo.png": 71,
      "Square89x89Logo.png": 89,
      "Square107x107Logo.png": 107,
      "Square142x142Logo.png": 142,
      "Square150x150Logo.png": 150,
      "Square284x284Logo.png": 284,
      "Square310x310Logo.png": 310,
    });
  });

  it("reads square 8-bit RGBA PNG metadata without an image decoder", async () => {
    const source = await readFile(join(REPOSITORY_ROOT, APPLICATION_ICON_SOURCE));

    assert.deepEqual(inspectPng(source), {
      bitDepth: 8,
      colorType: 6,
      hasAlpha: true,
      height: 512,
      width: 512,
    });
  });

  it("canonicalizes Tauri ICNS entries whose generator order is unstable", () => {
    const entry = (type, payload) => {
      const bytes = Buffer.alloc(8 + payload.length);
      bytes.write(type, 0, 4, "ascii");
      bytes.writeUInt32BE(bytes.length, 4);
      bytes.set(payload, 8);
      return bytes;
    };
    const container = (...entries) => {
      const length = 8 + entries.reduce((total, bytes) => total + bytes.length, 0);
      const header = Buffer.alloc(8);
      header.write("icns", 0, 4, "ascii");
      header.writeUInt32BE(length, 4);
      return Buffer.concat([header, ...entries]);
    };
    const smaller = entry("ic07", Buffer.from("small"));
    const larger = entry("ic10", Buffer.from("large"));

    assert.deepEqual(canonicalizeIcns(container(larger, smaller)), canonicalizeIcns(container(smaller, larger)));
  });

  it("verifies the checked-in favicon plus macOS, Windows, and Linux icon assets", async () => {
    const verification = await verifyCheckedInApplicationIcons(REPOSITORY_ROOT);

    assert.deepEqual(verification, {
      favicon: { height: 64, width: 64 },
      icns: true,
      ico: true,
      pngCount: 15,
    });
  });
});
