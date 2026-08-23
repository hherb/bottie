/** Native Localmail connection and bearer-authentication contracts. */
import { invoke, isTauri } from "@tauri-apps/api/core";

/** Secret-free saved Localmail connection and vault availability. */
export type LocalmailConnectionStatus = {
  origin: string | null;
  certificateSha256: string | null;
  credentialConfigured: boolean;
  credentialUnlocked: boolean;
  biometricProtected: boolean;
};

/** Confirms that Email may be offered without retrieving connector trust or credential material. */
export function localmailToolsConfigured(status: LocalmailConnectionStatus): boolean {
  return Boolean(status.origin && status.certificateSha256 && status.credentialConfigured);
}

/** Server identity and leaf fingerprint returned before trust confirmation. */
export type LocalmailProbeResult = {
  origin: string;
  apiMajor: number;
  apiMinor: number;
  serverVersion: string;
  certificateSha256: string;
};

/** Result of the pinned identity and optional authentication probe. */
export type LocalmailConnectionTest = {
  origin: string;
  serverVersion: string;
  authenticatedAs: string | null;
  elapsedMs: number;
  message: string;
};

/** Reads Localmail settings without returning any credential value. */
export async function getLocalmailConnectionStatus(): Promise<LocalmailConnectionStatus> {
  if (!isTauri()) throw new Error("Localmail setup requires the native Bottie application.");
  return invoke<LocalmailConnectionStatus>("get_localmail_connection_status");
}

/** Inspects one HTTPS server identity and certificate without persisting trust. */
export async function probeLocalmailConnection(origin: string): Promise<LocalmailProbeResult> {
  if (!isTauri()) throw new Error("Localmail setup requires the native Bottie application.");
  return invoke<LocalmailProbeResult>("probe_localmail_connection", { draft: { origin } });
}

/** Persists confirmed trust and optionally replaces or removes the vault token. */
export async function updateLocalmailConnection(
  origin: string,
  certificateSha256: string,
  bearerToken: string | null,
  removeToken: boolean,
): Promise<LocalmailConnectionStatus> {
  if (!isTauri()) throw new Error("Localmail setup requires the native Bottie application.");
  return invoke<LocalmailConnectionStatus>("update_localmail_connection", {
    update: { origin, certificateSha256, bearerToken, removeToken },
  });
}

/** Tests only server identity and bearer authentication; no email endpoint is called. */
export async function testLocalmailConnection(
  origin: string,
  certificateSha256: string,
  bearerToken: string | null,
): Promise<LocalmailConnectionTest> {
  if (!isTauri()) throw new Error("Localmail setup requires the native Bottie application.");
  return invoke<LocalmailConnectionTest>("test_localmail_connection", {
    draft: { origin, certificateSha256, bearerToken },
  });
}
