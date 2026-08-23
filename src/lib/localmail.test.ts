import { describe, expect, it } from "vitest";

import {
  localmailConnectionTestMessage,
  localmailToolsConfigured,
  type LocalmailConnectionStatus,
  type LocalmailConnectionTest,
} from "./localmail";

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

  it("distinguishes a successful draft-token test from saved vault readiness", () => {
    const result: LocalmailConnectionTest = {
      origin: "https://127.0.0.1:3000",
      serverVersion: "1.0.0",
      authenticatedAs: "tester@example.com",
      elapsedMs: 12,
      message: "Connection and authentication succeeded.",
    };

    expect(localmailConnectionTestMessage(result, true, false)).toContain(
      "Save this connection before enabling Email; the tested token is not in the credential vault yet.",
    );
    expect(localmailConnectionTestMessage(result, true, true)).toContain(
      "Save this connection before Email uses the tested replacement token.",
    );
    expect(localmailConnectionTestMessage(result, false, true)).toContain("The saved vault token is ready for Email.");
  });
});
