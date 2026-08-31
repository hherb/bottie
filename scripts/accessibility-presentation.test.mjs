import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = new URL("../src/lib/styles/", import.meta.url);
const settingsCss = readFileSync(new URL("settings.css", styles), "utf8");
const contextCss = readFileSync(new URL("context.css", styles), "utf8");
const shellCss = readFileSync(new URL("shell.css", styles), "utf8");
const appearanceCss = readFileSync(new URL("appearance.css", styles), "utf8");
const composerCss = readFileSync(new URL("composer.css", styles), "utf8");

/** Converts one six-digit sRGB colour to relative luminance. */
function luminance(hex) {
  const channels = hex.match(/[0-9a-f]{2}/gi)?.map((channel) => Number.parseInt(channel, 16) / 255) ?? [];
  const linear = channels.map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4),
  );
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

/** Returns the WCAG contrast ratio for two six-digit sRGB colours. */
function contrastRatio(foreground, background) {
  const values = [luminance(foreground), luminance(background)].sort((left, right) => right - left);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

/** Reads one six-digit custom-property colour from a CSS rule body. */
function customColour(css, property) {
  return css.match(new RegExp(`${property}:\\s*#([0-9a-f]{6})`, "i"))?.[1] ?? "";
}

describe("accessibility presentation policy", () => {
  it("keeps small secondary text above normal-text contrast in both themes", () => {
    const darkSubtle = customColour(shellCss, "--subtle-text");
    const lightRule = appearanceCss.match(/html\[data-theme="light"\]\s*\{([\s\S]*?)\}/)?.[1] ?? "";
    const lightSubtle = customColour(lightRule, "--subtle-text");

    expect(contrastRatio(darkSubtle, "0a0a0e")).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(lightSubtle, "f1efe9")).toBeGreaterThanOrEqual(4.5);
  });

  it("hides collapsed responsive navigation while preserving the reduced-motion override", () => {
    expect(settingsCss).toMatch(/@media \(max-width: 820px\)[\s\S]*?\.sidebar\s*\{[\s\S]*?visibility:\s*hidden/);
    expect(settingsCss).toMatch(/\.sidebar\.mobile-open\s*\{[\s\S]*?visibility:\s*visible/);
    expect(contextCss).toMatch(/\.context-panel\.closed\s*\{[\s\S]*?visibility:\s*hidden/);
    expect(settingsCss).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]*?animation-duration:\s*0\.01ms/);
    expect(composerCss).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.input-level span\s*\{[\s\S]*?transition:\s*none/,
    );
  });
});
