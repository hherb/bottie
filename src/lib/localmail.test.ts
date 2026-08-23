import { describe, expect, it } from "vitest";

import { localmailToolsConfigured, type LocalmailConnectionStatus } from "./localmail";

/** Creates one secret-free Localmail status with focused overrides. */
function status(overrides: Partial<LocalmailConnectionStatus> = {}): LocalmailConnectionStatus {
  return {
    origin: "https://127.0.0.1:3000",
    certificateSha256: "a".repeat(64),
    credentialConfigured: true,
    credentialUnlocked: false,
    biometricProtected: true,
    ...overrides,
  };
}

describe("Localmail Email readiness", () => {
  it("requires saved pinned trust and a configured credential without requiring prior unlock", () => {
    expect(localmailToolsConfigured(status())).toBe(true);
    expect(localmailToolsConfigured(status({ credentialUnlocked: true }))).toBe(true);
    expect(localmailToolsConfigured(status({ origin: null }))).toBe(false);
    expect(localmailToolsConfigured(status({ certificateSha256: null }))).toBe(false);
    expect(localmailToolsConfigured(status({ credentialConfigured: false }))).toBe(false);
  });
});
